use std::{
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};

use async_trait::async_trait;
use dco3::{
    auth::Connected,
    nodes::{
        models::{NodeList, NodeType, NodesFilter},
        Node,
    },
    ListAllParams,
};
use secrecy::SecretString;

use crate::{
    app::{
        auth::AuthService,
        config::{api::SystemApi, service::ConfigService},
        groups::{api::GroupsApi, GroupsService},
        nodes::{
            api::NodesApi,
            command::{CmdDeleteOptions, CmdDownloadOptions, CmdUploadOptions},
            download::{
                api::DownloadApi, DownloadOutcome, NodesDownloadService, NoopTransferStateStore,
                PreparedDownloadSource,
            },
            filesystem::OSFileSystem,
            progress::{start_progress_bar, ProgressReporter},
            upload::{NodesUploadService, UploadApi, UploadOutcome},
            DeleteNodesPreparation, NodesService,
        },
        users::{
            api::{UsersApi, UsersCommandApi},
            UsersService,
        },
    },
    command::CreateContainerType,
    core::models::DcCmdError,
    mcp::{
        groups::{
            GroupsAddUsersRequest, GroupsAddUsersResponse, GroupsCreateRequest,
            GroupsCreateResponse, GroupsGetUsersRequest, GroupsGetUsersResponse, GroupsListRequest,
            GroupsListResponse, McpGroup, McpGroupUser,
        },
        nodes::{
            ListComparison, McpContainerType, NodeFilterTextOp, NodeListFilter, NodeSummary,
            NodesCopyRequest, NodesCopyResponse, NodesDownloadRequest, NodesListRequest,
            NodesListResponse, NodesMkdirRequest, NodesMkdirResponse, NodesReadRequest,
            NodesReadResponse, NodesRmRequest, NodesRmResponse, NodesUploadRequest,
            ReadableTextNode, ResolvedNode,
        },
        paths::WorkspacePathGuard,
        users::{
            McpUser, UsersCreateRequest, UsersCreateResponse, UsersGetOidcIdpsResponse,
            UsersInfoRequest, UsersInfoResponse, UsersListRequest, UsersListResponse,
            UsersWhoamiResponse,
        },
    },
};

const DEFAULT_PAGE_LIMIT: u64 = 500;
const MAX_READ_BYTES: usize = 1_048_576;

pub(crate) trait McpBackend:
    NodesApi + DownloadApi + UploadApi + UsersApi + UsersCommandApi + GroupsApi + SystemApi
{
}

impl<T> McpBackend for T where
    T: NodesApi + DownloadApi + UploadApi + UsersApi + UsersCommandApi + GroupsApi + SystemApi
{
}

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Clone)]
pub(crate) enum McpSession {
    Dracoon(dco3::Dracoon<Connected>),
    Mock(Arc<dyn McpBackend>),
}

#[async_trait]
pub(crate) trait McpSessionProvider: Send + Sync {
    async fn connect(&self, target: &str) -> Result<McpSession, DcCmdError>;

    async fn ensure_encryption(
        &self,
        target: &str,
        session: McpSession,
        encryption_password: Option<SecretString>,
    ) -> Result<McpSession, DcCmdError>;
}

#[derive(Debug, Default)]
struct DefaultMcpSessionProvider;

#[derive(Clone)]
pub struct McpPlatform {
    target: String,
    encryption_password: Option<SecretString>,
    path_guard: WorkspacePathGuard,
    session_provider: Arc<dyn McpSessionProvider>,
}

#[async_trait]
impl McpSessionProvider for DefaultMcpSessionProvider {
    async fn connect(&self, target: &str) -> Result<McpSession, DcCmdError> {
        let config = ConfigService::new();
        let refresh_token = config.get_refresh_token(target)?;
        let session = AuthService::new()
            .connect_with_refresh_token(target, refresh_token)
            .await?;
        Ok(McpSession::Dracoon(session.into_client()))
    }

    async fn ensure_encryption(
        &self,
        target: &str,
        session: McpSession,
        encryption_password: Option<SecretString>,
    ) -> Result<McpSession, DcCmdError> {
        if encryption_password.is_none() && !ConfigService::new().has_encryption_secret(target)? {
            return Err(DcCmdError::InvalidArgument(
                "Encrypted node access requires a stored encryption secret or startup --encryption-password.".to_string(),
            ));
        }

        let McpSession::Dracoon(client) = session else {
            return Err(DcCmdError::InvalidArgument(
                "Encrypted node access is not available for the current MCP backend.".to_string(),
            ));
        };

        let encrypted = AuthService::new()
            .ensure_encryption_client(target.to_string(), client, encryption_password)
            .await?;
        Ok(McpSession::Dracoon(encrypted))
    }
}

impl McpPlatform {
    pub fn new(
        target: String,
        encryption_password: Option<SecretString>,
        path_guard: WorkspacePathGuard,
    ) -> Result<Self, DcCmdError> {
        let config = ConfigService::new();
        let target = config.normalize_base_url(&target)?;

        Ok(Self {
            target,
            encryption_password,
            path_guard,
            session_provider: Arc::new(DefaultMcpSessionProvider),
        })
    }

    #[cfg(test)]
    pub(crate) fn with_session_provider(
        target: String,
        encryption_password: Option<SecretString>,
        path_guard: WorkspacePathGuard,
        session_provider: Arc<dyn McpSessionProvider>,
    ) -> Result<Self, DcCmdError> {
        let config = ConfigService::new();
        let target = config.normalize_base_url(&target)?;

        Ok(Self {
            target,
            encryption_password,
            path_guard,
            session_provider,
        })
    }

    pub fn target(&self) -> &str {
        &self.target
    }

    pub async fn preflight(&self) -> Result<(), DcCmdError> {
        let _ = self.connect_client().await?;
        Ok(())
    }

    pub async fn list_nodes(
        &self,
        request: NodesListRequest,
        progress: Arc<dyn ProgressReporter>,
    ) -> Result<NodesListResponse, DcCmdError> {
        let client = self.connect_client().await?;
        let (parent_id, resolved_parent) = self
            .resolve_list_parent(&client, request.node_id, request.path.as_deref())
            .await?;

        let offset = request.offset.unwrap_or(0);
        let limit = u64::from(request.limit.unwrap_or(DEFAULT_PAGE_LIMIT as u32));
        let filter = request.filter.clone().map(TryInto::try_into).transpose()?;

        let mut page = self
            .get_nodes_page(
                &client,
                parent_id,
                request.managed,
                filter.clone(),
                offset,
                limit,
            )
            .await?;

        if request.all && page.range.total > page.items.len() as u64 {
            let progress_bar =
                start_progress_bar(progress.as_ref(), page.range.total, Some("Listing nodes"));
            progress_bar.inc(page.items.len() as u64);

            let mut next_offset = offset + limit;
            while next_offset < page.range.total {
                let next = self
                    .get_nodes_page(
                        &client,
                        parent_id,
                        request.managed,
                        filter.clone(),
                        next_offset,
                        limit,
                    )
                    .await?;
                progress_bar.inc(next.items.len() as u64);
                page.items.extend(next.items);
                next_offset += limit;
            }
            progress_bar.finish_with_message("Listed nodes.");
        }

        Ok(NodesListResponse {
            target: self.target.clone(),
            resolved_parent,
            items: page.items.iter().map(NodeSummary::from).collect(),
            total: page.range.total,
            offset,
            limit: limit as u32,
            all_fetched: request.all,
            managed: request.managed,
        })
    }

    pub async fn list_users(
        &self,
        request: UsersListRequest,
    ) -> Result<UsersListResponse, DcCmdError> {
        let client = self.connect_client().await?;
        self.list_users_with_api(&client, request).await
    }

    pub async fn list_groups(
        &self,
        request: GroupsListRequest,
    ) -> Result<GroupsListResponse, DcCmdError> {
        let client = self.connect_client().await?;
        self.list_groups_with_api(&client, request).await
    }

    pub async fn get_user_info(
        &self,
        request: UsersInfoRequest,
    ) -> Result<UsersInfoResponse, DcCmdError> {
        let client = self.connect_client().await?;
        self.get_user_info_with_api(&client, request).await
    }

    pub async fn get_current_user_info(&self) -> Result<UsersWhoamiResponse, DcCmdError> {
        let client = self.connect_client().await?;
        self.get_current_user_info_with_api(&client).await
    }

    pub async fn create_user(
        &self,
        request: UsersCreateRequest,
    ) -> Result<UsersCreateResponse, DcCmdError> {
        let client = self.connect_client().await?;
        self.create_user_with_api(&client, request).await
    }

    pub async fn create_group(
        &self,
        request: GroupsCreateRequest,
    ) -> Result<GroupsCreateResponse, DcCmdError> {
        let client = self.connect_client().await?;
        self.create_group_with_api(&client, request).await
    }

    pub async fn get_oidc_idps(&self) -> Result<UsersGetOidcIdpsResponse, DcCmdError> {
        let client = self.connect_client().await?;
        self.get_oidc_idps_with_api(&client).await
    }

    pub async fn get_group_users(
        &self,
        request: GroupsGetUsersRequest,
    ) -> Result<GroupsGetUsersResponse, DcCmdError> {
        let client = self.connect_client().await?;
        self.get_group_users_with_api(&client, request).await
    }

    pub async fn add_group_users(
        &self,
        request: GroupsAddUsersRequest,
    ) -> Result<GroupsAddUsersResponse, DcCmdError> {
        let client = self.connect_client().await?;
        self.add_group_users_with_api(&client, request).await
    }

    pub async fn download_nodes(
        &self,
        request: NodesDownloadRequest,
        progress: Arc<dyn ProgressReporter>,
    ) -> Result<DownloadOutcome, DcCmdError> {
        let service =
            NodesDownloadService::with_progress(OSFileSystem, NoopTransferStateStore, progress);
        let mut client = self.connect_client().await?;
        let selection = self
            .resolve_node_selector(&client, request.node_id, request.path.as_deref())
            .await?;

        if selection.node.is_encrypted == Some(true) {
            client = self.ensure_encryption_client(client).await?;
        }

        let target = self
            .path_guard
            .resolve_target_path(&request.target_path)?
            .to_string_lossy()
            .to_string();

        let prepared = PreparedDownloadSource::Node {
            source: selection.path,
            node: Box::new(selection.node),
        };

        service
            .download_with_client(
                client,
                prepared,
                target,
                CmdDownloadOptions::new(
                    request.recursive,
                    request.velocity,
                    None,
                    request.include_rooms,
                ),
            )
            .await
    }

    pub async fn read_node_content(
        &self,
        request: NodesReadRequest,
    ) -> Result<NodesReadResponse, DcCmdError> {
        let mut client = self.connect_client().await?;
        let selection = self
            .resolve_node_selector(&client, request.node_id, request.path.as_deref())
            .await?;

        if selection.node.is_encrypted == Some(true) {
            client = self.ensure_encryption_client(client).await?;
        }

        self.read_resolved_node_content(&client, selection).await
    }

    pub async fn upload_nodes(
        &self,
        request: NodesUploadRequest,
        progress: Arc<dyn ProgressReporter>,
    ) -> Result<UploadOutcome, DcCmdError> {
        let service = NodesUploadService::with_progress(progress);
        let mut client = self.connect_client().await?;
        let parent = self
            .resolve_parent_selector(
                &client,
                request.parent_node_id,
                request.parent_path.as_deref(),
            )
            .await?;

        if parent.is_encrypted == Some(true) {
            client = self.ensure_encryption_client(client).await?;
        }

        let source = self
            .path_guard
            .resolve_existing_source(&request.source_path)?;

        service
            .upload_with_client(
                &client,
                source,
                &parent,
                CmdUploadOptions::new(
                    request.overwrite,
                    request.keep_share_links,
                    request.recursive,
                    request.skip_root,
                    request.share,
                    request.classification,
                    request.velocity,
                    request.share_password,
                ),
            )
            .await
    }

    pub async fn copy_nodes(
        &self,
        request: NodesCopyRequest,
    ) -> Result<NodesCopyResponse, DcCmdError> {
        let client = self.connect_client().await?;
        let service = NodesService::new(client.clone());
        let source = self
            .resolve_node_selector(
                &client,
                request.source_node_id,
                request.source_path.as_deref(),
            )
            .await?;
        let target_parent = self
            .resolve_parent_selector(
                &client,
                request.target_parent_node_id,
                request.target_parent_path.as_deref(),
            )
            .await?;

        let result = service
            .copy_nodes(
                &source.path,
                &build_node_path(&target_parent)?,
                self.target.as_ref(),
            )
            .await?;

        Ok(NodesCopyResponse {
            source_node_id: source.node.id,
            target_parent_node_id: target_parent.id,
            copied_count: result.count_nodes,
            source_path: source.path,
            target_parent_path: build_node_path(&target_parent)?,
        })
    }

    pub async fn create_container(
        &self,
        request: NodesMkdirRequest,
    ) -> Result<NodesMkdirResponse, DcCmdError> {
        let client = self.connect_client().await?;
        let service = NodesService::new(client.clone());
        let parent = self
            .resolve_parent_selector(
                &client,
                request.parent_node_id,
                request.parent_path.as_deref(),
            )
            .await?;

        let container_type = match request.r#type {
            McpContainerType::Folder => CreateContainerType::Folder,
            McpContainerType::Room => CreateContainerType::Room,
        };

        let created_path = format!("{}{name}", build_node_path(&parent)?, name = request.name);
        let message = service
            .create_container(
                &created_path,
                self.target.as_ref(),
                container_type,
                request.classification,
                request.notes,
                request.admin_users,
                request.inherit_permissions,
            )
            .await?;

        Ok(NodesMkdirResponse {
            message,
            parent_node_id: parent.id,
            created_path,
            r#type: request.r#type,
        })
    }

    pub async fn remove_nodes(
        &self,
        request: NodesRmRequest,
    ) -> Result<NodesRmResponse, DcCmdError> {
        let client = self.connect_client().await?;
        let service = NodesService::new(client.clone());
        let selector = self
            .resolve_node_selector(&client, request.node_id, request.path.as_deref())
            .await?;
        let source = selector.path.clone();
        let opts = CmdDeleteOptions::new(request.recursive);

        let preparation = service
            .prepare_delete(&source, self.target.as_ref(), request.recursive)
            .await?;

        let deleted_node_ids = match preparation {
            DeleteNodesPreparation::InvalidSearchRequiresRecursive => {
                return Err(DcCmdError::InvalidArgument(
                    "Recursive delete is required for wildcard selection.".to_string(),
                ));
            }
            DeleteNodesPreparation::ContainerRequiresRecursive => {
                return Err(DcCmdError::InvalidArgument(
                    "Recursive delete is required for folders and rooms.".to_string(),
                ));
            }
            DeleteNodesPreparation::Search { node_ids } => {
                service.delete_nodes(node_ids.clone()).await?;
                node_ids
            }
            DeleteNodesPreparation::SingleNode { node_id, .. } => {
                service.delete_node(node_id).await?;
                vec![node_id]
            }
        };

        Ok(NodesRmResponse {
            deleted_count: deleted_node_ids.len() as u64,
            deleted_node_ids,
            source_path: source,
            recursive: opts.recursive(),
        })
    }

    async fn connect_client(&self) -> Result<McpSession, DcCmdError> {
        self.session_provider.connect(&self.target).await
    }

    async fn ensure_encryption_client(&self, client: McpSession) -> Result<McpSession, DcCmdError> {
        self.session_provider
            .ensure_encryption(&self.target, client, self.encryption_password.clone())
            .await
    }

    async fn list_users_with_api<A: UsersCommandApi + Clone>(
        &self,
        api: &A,
        request: UsersListRequest,
    ) -> Result<UsersListResponse, DcCmdError> {
        let service = UsersService::new(api.clone());
        let offset = request.offset.unwrap_or(0);
        let limit = request.limit.unwrap_or(DEFAULT_PAGE_LIMIT as u32);
        let users = service
            .list_users(&crate::core::models::ListOptions::new(
                request.filter,
                Some(offset),
                Some(limit),
                request.all,
                false,
            ))
            .await?;

        Ok(UsersListResponse {
            target: self.target.clone(),
            items: users.items.into_iter().map(McpUser::from).collect(),
            total: users.range.total,
            offset,
            limit,
            all_fetched: request.all,
        })
    }

    async fn list_groups_with_api<A: GroupsApi + Clone>(
        &self,
        api: &A,
        request: GroupsListRequest,
    ) -> Result<GroupsListResponse, DcCmdError> {
        let service = GroupsService::new(api.clone());
        let offset = request.offset.unwrap_or(0);
        let limit = request.limit.unwrap_or(DEFAULT_PAGE_LIMIT as u32);
        let groups = service
            .list_groups(&crate::core::models::ListOptions::new(
                request.filter,
                Some(offset),
                Some(limit),
                request.all,
                false,
            ))
            .await?;

        Ok(GroupsListResponse {
            target: self.target.clone(),
            items: groups.items.into_iter().map(McpGroup::from).collect(),
            total: groups.range.total,
            offset,
            limit,
            all_fetched: request.all,
        })
    }

    async fn get_user_info_with_api<A: UsersCommandApi + Clone>(
        &self,
        api: &A,
        request: UsersInfoRequest,
    ) -> Result<UsersInfoResponse, DcCmdError> {
        validate_selector(
            "user_id",
            request.user_id,
            "username",
            request.username.as_deref(),
        )?;

        let service = UsersService::new(api.clone());
        let user = service
            .get_user_info(request.username, request.user_id)
            .await?;

        Ok(UsersInfoResponse {
            target: self.target.clone(),
            user: McpUser::from(user),
        })
    }

    async fn get_current_user_info_with_api<A: UsersCommandApi + SystemApi + Clone>(
        &self,
        api: &A,
    ) -> Result<UsersWhoamiResponse, DcCmdError> {
        let refresh_user = api.get_refresh_token_info().await?;
        let service = UsersService::new(api.clone());
        let user = service
            .get_user_info(Some(refresh_user.user_name), None)
            .await?;

        Ok(UsersWhoamiResponse {
            target: self.target.clone(),
            user: McpUser::from(user),
        })
    }

    async fn create_user_with_api<A: UsersCommandApi + Clone>(
        &self,
        api: &A,
        request: UsersCreateRequest,
    ) -> Result<UsersCreateResponse, DcCmdError> {
        let service = UsersService::new(api.clone());
        let user = service
            .create_user(
                &request.first_name,
                &request.last_name,
                &request.email,
                request.login.as_deref(),
                request.oidc_id,
                request.mfa_enforced,
                request.group_id,
            )
            .await?;
        let auth_method = user.auth_data.method.clone();

        Ok(UsersCreateResponse {
            target: self.target.clone(),
            user: McpUser::from(user),
            auth_method,
        })
    }

    async fn create_group_with_api<A: GroupsApi + Clone>(
        &self,
        api: &A,
        request: GroupsCreateRequest,
    ) -> Result<GroupsCreateResponse, DcCmdError> {
        let service = GroupsService::new(api.clone());
        let group = service.create_group(&request.name).await?;

        Ok(GroupsCreateResponse {
            target: self.target.clone(),
            group: McpGroup::from(group),
        })
    }

    async fn get_oidc_idps_with_api<A: SystemApi>(
        &self,
        api: &A,
    ) -> Result<UsersGetOidcIdpsResponse, DcCmdError> {
        let oidc_configs = api.get_oidc_idp_configs().await?;

        Ok(UsersGetOidcIdpsResponse {
            target: self.target.clone(),
            items: oidc_configs.into_iter().map(Into::into).collect(),
        })
    }

    async fn get_group_users_with_api<A: GroupsApi + Clone>(
        &self,
        api: &A,
        request: GroupsGetUsersRequest,
    ) -> Result<GroupsGetUsersResponse, DcCmdError> {
        validate_selector(
            "group_id",
            request.group_id,
            "group_name",
            request.group_name.as_deref(),
        )?;

        let service = GroupsService::new(api.clone());
        let offset = request.offset.unwrap_or(0);
        let limit = request.limit.unwrap_or(DEFAULT_PAGE_LIMIT as u32);
        let offset_u32 = u32::try_from(offset).map_err(|_| {
            DcCmdError::InvalidArgument("Offset exceeds supported range.".to_string())
        })?;
        let pages = service
            .list_group_users_by_selector(
                request.group_name.as_deref(),
                request.group_id,
                &request.filter,
                Some(offset_u32),
                Some(limit),
                request.all,
            )
            .await?;
        let page = pages.into_iter().next().ok_or_else(|| {
            DcCmdError::InvalidArgument("No group matches the provided selector.".to_string())
        })?;

        Ok(GroupsGetUsersResponse {
            target: self.target.clone(),
            group: McpGroup::from(page.group),
            items: page
                .users
                .items
                .into_iter()
                .map(McpGroupUser::from)
                .collect(),
            total: page.users.range.total,
            offset,
            limit,
            all_fetched: request.all,
        })
    }

    async fn add_group_users_with_api<A: GroupsApi + UsersApi + Clone>(
        &self,
        api: &A,
        request: GroupsAddUsersRequest,
    ) -> Result<GroupsAddUsersResponse, DcCmdError> {
        validate_selector(
            "group_id",
            request.group_id,
            "group_name",
            request.group_name.as_deref(),
        )?;

        if request.user_ids.is_empty()
            && request
                .usernames
                .iter()
                .all(|value| value.trim().is_empty())
        {
            return Err(DcCmdError::InvalidArgument(
                "Provide at least one user selector source: user_ids or usernames.".to_string(),
            ));
        }

        let service = GroupsService::new(api.clone());
        let (group, added_user_ids) = service
            .add_group_users(
                request.group_name,
                request.group_id,
                request.usernames,
                request.user_ids,
            )
            .await?;

        Ok(GroupsAddUsersResponse {
            target: self.target.clone(),
            group: McpGroup::from(group),
            added_count: added_user_ids.len() as u64,
            added_user_ids,
        })
    }

    async fn read_resolved_node_content<A: DownloadApi>(
        &self,
        api: &A,
        selection: ResolvedNodeSelection,
    ) -> Result<NodesReadResponse, DcCmdError> {
        if selection.node.node_type != NodeType::File {
            return Err(DcCmdError::InvalidArgument(
                "nodes_read requires a file selector.".to_string(),
            ));
        }

        selection.node.ensure_readable_text()?;

        if selection
            .node
            .size
            .is_some_and(|size| size > MAX_READ_BYTES as u64)
        {
            return Err(Self::read_size_limit_error());
        }

        let mut writer = LimitedBufferWriter::new(MAX_READ_BYTES);
        let download_result = api.download_node(&selection.node, &mut writer, None).await;
        if let Err(error) = download_result {
            if writer.limit_exceeded() {
                return Err(Self::read_size_limit_error());
            }
            return Err(error);
        }

        let bytes = writer.into_inner();
        let size = bytes.len() as u64;
        let content = String::from_utf8(bytes).map_err(|_| {
            DcCmdError::InvalidArgument("nodes_read requires valid UTF-8 content.".to_string())
        })?;

        Ok(NodesReadResponse {
            target: self.target.clone(),
            resolved_file: ResolvedNode::from_node(&selection.node)?,
            file_type: selection.node.file_type.clone(),
            media_type: selection.node.media_type.clone(),
            encoding: "utf-8".to_string(),
            size,
            content,
        })
    }

    fn read_size_limit_error() -> DcCmdError {
        DcCmdError::InvalidArgument(format!(
            "nodes_read only supports files up to {MAX_READ_BYTES} bytes."
        ))
    }

    async fn resolve_list_parent<A: NodesApi>(
        &self,
        client: &A,
        node_id: Option<u64>,
        path: Option<&str>,
    ) -> Result<(Option<u64>, Option<ResolvedNode>), DcCmdError> {
        validate_selector("node_id", node_id, "path", path)?;

        if let Some(node_id) = node_id {
            let node = client.get_node(node_id).await?;
            if node.node_type == NodeType::File {
                return Err(DcCmdError::InvalidArgument(
                    "nodes_list requires a room, folder, or root path selector.".to_string(),
                ));
            }
            return Ok((Some(node.id), Some(ResolvedNode::from_node(&node)?)));
        }

        let path = path.ok_or_else(|| {
            DcCmdError::InvalidArgument(
                "Provide exactly one selector: node_id or path.".to_string(),
            )
        })?;
        let normalized = normalize_remote_path(path);
        if normalized == "/" {
            return Ok((None, None));
        }

        let node = client
            .get_node_from_path(&normalized)
            .await?
            .ok_or_else(|| DcCmdError::InvalidPath(path.to_string()))?;
        if node.node_type == NodeType::File {
            return Err(DcCmdError::InvalidArgument(
                "nodes_list requires a room, folder, or root path selector.".to_string(),
            ));
        }

        Ok((Some(node.id), Some(ResolvedNode::from_node(&node)?)))
    }

    async fn resolve_node_selector<A: NodesApi>(
        &self,
        client: &A,
        node_id: Option<u64>,
        path: Option<&str>,
    ) -> Result<ResolvedNodeSelection, DcCmdError> {
        validate_selector("node_id", node_id, "path", path)?;

        let node = if let Some(node_id) = node_id {
            client.get_node(node_id).await?
        } else {
            let input = path.ok_or_else(|| {
                DcCmdError::InvalidArgument(
                    "Provide exactly one selector: node_id or path.".to_string(),
                )
            })?;
            client
                .get_node_from_path(&normalize_remote_path(input))
                .await?
                .ok_or_else(|| DcCmdError::InvalidPath(input.to_string()))?
        };

        Ok(ResolvedNodeSelection {
            path: build_node_path(&node)?,
            node,
        })
    }

    async fn resolve_parent_selector<A: NodesApi>(
        &self,
        client: &A,
        parent_node_id: Option<u64>,
        parent_path: Option<&str>,
    ) -> Result<Node, DcCmdError> {
        let selection = self
            .resolve_node_selector(client, parent_node_id, parent_path)
            .await?;

        if selection.node.node_type == NodeType::File {
            return Err(DcCmdError::InvalidArgument(
                "Parent selector must resolve to a room or folder.".to_string(),
            ));
        }

        Ok(selection.node)
    }

    async fn get_nodes_page<A: NodesApi>(
        &self,
        client: &A,
        parent_id: Option<u64>,
        managed: bool,
        filter: Option<NodesFilter>,
        offset: u64,
        limit: u64,
    ) -> Result<NodeList, DcCmdError> {
        let mut params = ListAllParams::builder()
            .with_offset(offset)
            .with_limit(limit);
        if let Some(filter) = filter {
            params = params.with_filter(filter);
        }

        client
            .get_nodes(parent_id, Some(managed), Some(params.build()))
            .await
    }
}

#[async_trait]
impl NodesApi for McpSession {
    async fn get_node(&self, node_id: u64) -> Result<Node, DcCmdError> {
        match self {
            Self::Dracoon(client) => client.get_node(node_id).await,
            Self::Mock(api) => api.get_node(node_id).await,
        }
    }

    async fn get_node_from_path(&self, node_path: &str) -> Result<Option<Node>, DcCmdError> {
        match self {
            Self::Dracoon(client) => NodesApi::get_node_from_path(client, node_path).await,
            Self::Mock(api) => NodesApi::get_node_from_path(api.as_ref(), node_path).await,
        }
    }

    async fn get_nodes(
        &self,
        parent_id: Option<u64>,
        managed: Option<bool>,
        params: Option<ListAllParams>,
    ) -> Result<NodeList, DcCmdError> {
        match self {
            Self::Dracoon(client) => client.get_nodes(parent_id, managed, params).await,
            Self::Mock(api) => api.get_nodes(parent_id, managed, params).await,
        }
    }

    async fn search_nodes(
        &self,
        search_string: &str,
        parent_id: Option<u64>,
        depth_level: Option<i8>,
        params: Option<ListAllParams>,
    ) -> Result<NodeList, DcCmdError> {
        match self {
            Self::Dracoon(client) => {
                client
                    .search_nodes(search_string, parent_id, depth_level, params)
                    .await
            }
            Self::Mock(api) => {
                api.search_nodes(search_string, parent_id, depth_level, params)
                    .await
            }
        }
    }

    async fn delete_node(&self, node_id: u64) -> Result<(), DcCmdError> {
        match self {
            Self::Dracoon(client) => client.delete_node(node_id).await,
            Self::Mock(api) => api.delete_node(node_id).await,
        }
    }

    async fn delete_nodes(&self, node_ids: Vec<u64>) -> Result<(), DcCmdError> {
        match self {
            Self::Dracoon(client) => client.delete_nodes(node_ids).await,
            Self::Mock(api) => api.delete_nodes(node_ids).await,
        }
    }

    async fn copy_nodes(
        &self,
        node_ids: Vec<u64>,
        target_parent_id: u64,
    ) -> Result<(), DcCmdError> {
        match self {
            Self::Dracoon(client) => client.copy_nodes(node_ids, target_parent_id).await,
            Self::Mock(api) => api.copy_nodes(node_ids, target_parent_id).await,
        }
    }

    async fn create_folder(
        &self,
        node_name: &str,
        parent_id: u64,
        classification: Option<u8>,
        notes: Option<String>,
    ) -> Result<Node, DcCmdError> {
        match self {
            Self::Dracoon(client) => {
                client
                    .create_folder(node_name, parent_id, classification, notes)
                    .await
            }
            Self::Mock(api) => {
                api.create_folder(node_name, parent_id, classification, notes)
                    .await
            }
        }
    }

    async fn create_room(
        &self,
        node_name: &str,
        parent_id: u64,
        classification: u8,
        inherit_permissions: bool,
        admin_ids: Option<Vec<u64>>,
    ) -> Result<(), DcCmdError> {
        match self {
            Self::Dracoon(client) => {
                client
                    .create_room(
                        node_name,
                        parent_id,
                        classification,
                        inherit_permissions,
                        admin_ids,
                    )
                    .await
            }
            Self::Mock(api) => {
                api.create_room(
                    node_name,
                    parent_id,
                    classification,
                    inherit_permissions,
                    admin_ids,
                )
                .await
            }
        }
    }
}

#[async_trait]
impl DownloadApi for McpSession {
    async fn download_node<'w>(
        &'w self,
        node: &Node,
        writer: &'w mut (dyn tokio::io::AsyncWrite + Send + Unpin),
        callback: Option<dco3::nodes::models::DownloadProgressCallback>,
    ) -> Result<(), DcCmdError> {
        match self {
            Self::Dracoon(client) => client.download_node(node, writer, callback).await,
            Self::Mock(api) => api.download_node(node, writer, callback).await,
        }
    }
}

#[async_trait]
impl UploadApi for McpSession {
    async fn upload_local_file(
        &self,
        source: std::path::PathBuf,
        target_node: &Node,
        classification: u8,
        overwrite: bool,
        keep_share_links: bool,
        on_progress: Option<crate::app::nodes::upload::UploadProgressFn>,
    ) -> Result<Node, DcCmdError> {
        match self {
            Self::Dracoon(client) => {
                client
                    .upload_local_file(
                        source,
                        target_node,
                        classification,
                        overwrite,
                        keep_share_links,
                        on_progress,
                    )
                    .await
            }
            Self::Mock(api) => {
                api.upload_local_file(
                    source,
                    target_node,
                    classification,
                    overwrite,
                    keep_share_links,
                    on_progress,
                )
                .await
            }
        }
    }

    async fn create_download_share_link(
        &self,
        node: &Node,
        share_password: Option<String>,
    ) -> Result<String, DcCmdError> {
        match self {
            Self::Dracoon(client) => {
                client
                    .create_download_share_link(node, share_password)
                    .await
            }
            Self::Mock(api) => api.create_download_share_link(node, share_password).await,
        }
    }
}

#[async_trait]
impl UsersApi for McpSession {
    async fn find_user_id_by_username(&self, user_name: &str) -> Result<u64, DcCmdError> {
        match self {
            Self::Dracoon(client) => client.find_user_id_by_username(user_name).await,
            Self::Mock(api) => api.find_user_id_by_username(user_name).await,
        }
    }
}

#[async_trait]
impl UsersCommandApi for McpSession {
    async fn create_user(
        &self,
        req: dco3::users::CreateUserRequest,
    ) -> Result<dco3::users::UserData, DcCmdError> {
        match self {
            Self::Dracoon(client) => client.create_user(req).await,
            Self::Mock(api) => api.create_user(req).await,
        }
    }

    async fn get_users(
        &self,
        params: Option<ListAllParams>,
    ) -> Result<dco3::RangedItems<dco3::users::UserItem>, DcCmdError> {
        match self {
            Self::Dracoon(client) => client.get_users(params).await,
            Self::Mock(api) => api.get_users(params).await,
        }
    }

    async fn delete_user(&self, user_id: u64) -> Result<(), DcCmdError> {
        match self {
            Self::Dracoon(client) => client.delete_user(user_id).await,
            Self::Mock(api) => api.delete_user(user_id).await,
        }
    }

    async fn get_user(&self, user_id: u64) -> Result<dco3::users::UserData, DcCmdError> {
        match self {
            Self::Dracoon(client) => client.get_user(user_id).await,
            Self::Mock(api) => api.get_user(user_id).await,
        }
    }

    async fn update_user(
        &self,
        user_id: u64,
        req: dco3::users::UpdateUserRequest,
    ) -> Result<dco3::users::UserData, DcCmdError> {
        match self {
            Self::Dracoon(client) => client.update_user(user_id, req).await,
            Self::Mock(api) => api.update_user(user_id, req).await,
        }
    }

    async fn add_group_users(&self, group_id: u64, user_ids: Vec<u64>) -> Result<(), DcCmdError> {
        match self {
            Self::Dracoon(client) => {
                UsersCommandApi::add_group_users(client, group_id, user_ids).await
            }
            Self::Mock(api) => {
                UsersCommandApi::add_group_users(api.as_ref(), group_id, user_ids).await
            }
        }
    }

    async fn get_group_users(
        &self,
        group_id: u64,
        params: Option<ListAllParams>,
    ) -> Result<dco3::RangedItems<dco3::groups::GroupUser>, DcCmdError> {
        match self {
            Self::Dracoon(client) => {
                UsersCommandApi::get_group_users(client, group_id, params).await
            }
            Self::Mock(api) => {
                UsersCommandApi::get_group_users(api.as_ref(), group_id, params).await
            }
        }
    }

    async fn get_node_from_path(&self, path: &str) -> Result<Option<Node>, DcCmdError> {
        match self {
            Self::Dracoon(client) => UsersCommandApi::get_node_from_path(client, path).await,
            Self::Mock(api) => UsersCommandApi::get_node_from_path(api.as_ref(), path).await,
        }
    }

    async fn invite_guest_users(
        &self,
        room_id: u64,
        users: Vec<dco3::nodes::RoomGuestUserInvitation>,
    ) -> Result<(), DcCmdError> {
        match self {
            Self::Dracoon(client) => client.invite_guest_users(room_id, users).await,
            Self::Mock(api) => api.invite_guest_users(room_id, users).await,
        }
    }
}

#[async_trait]
impl GroupsApi for McpSession {
    async fn get_groups(
        &self,
        params: Option<ListAllParams>,
    ) -> Result<dco3::RangedItems<dco3::groups::Group>, DcCmdError> {
        match self {
            Self::Dracoon(client) => client.get_groups(params).await,
            Self::Mock(api) => api.get_groups(params).await,
        }
    }

    async fn get_group(&self, group_id: u64) -> Result<dco3::groups::Group, DcCmdError> {
        match self {
            Self::Dracoon(client) => client.get_group(group_id).await,
            Self::Mock(api) => api.get_group(group_id).await,
        }
    }

    async fn get_group_users(
        &self,
        group_id: u64,
        params: Option<ListAllParams>,
    ) -> Result<dco3::RangedItems<dco3::groups::GroupUser>, DcCmdError> {
        match self {
            Self::Dracoon(client) => GroupsApi::get_group_users(client, group_id, params).await,
            Self::Mock(api) => GroupsApi::get_group_users(api.as_ref(), group_id, params).await,
        }
    }

    async fn create_group(&self, name: &str) -> Result<dco3::groups::Group, DcCmdError> {
        match self {
            Self::Dracoon(client) => client.create_group(name).await,
            Self::Mock(api) => api.create_group(name).await,
        }
    }

    async fn delete_group(&self, group_id: u64) -> Result<(), DcCmdError> {
        match self {
            Self::Dracoon(client) => client.delete_group(group_id).await,
            Self::Mock(api) => api.delete_group(group_id).await,
        }
    }

    async fn add_group_users(
        &self,
        group_id: u64,
        user_ids: Vec<u64>,
    ) -> Result<dco3::groups::Group, DcCmdError> {
        match self {
            Self::Dracoon(client) => GroupsApi::add_group_users(client, group_id, user_ids).await,
            Self::Mock(api) => GroupsApi::add_group_users(api.as_ref(), group_id, user_ids).await,
        }
    }
}

#[async_trait]
impl SystemApi for McpSession {
    async fn get_refresh_token_info(
        &self,
    ) -> Result<crate::app::config::api::RefreshTokenInfo, DcCmdError> {
        match self {
            Self::Dracoon(client) => SystemApi::get_refresh_token_info(client).await,
            Self::Mock(api) => SystemApi::get_refresh_token_info(api.as_ref()).await,
        }
    }

    async fn get_oidc_idp_configs(
        &self,
    ) -> Result<Vec<crate::app::config::api::OpenIdConfigInfo>, DcCmdError> {
        match self {
            Self::Dracoon(client) => SystemApi::get_oidc_idp_configs(client).await,
            Self::Mock(api) => SystemApi::get_oidc_idp_configs(api.as_ref()).await,
        }
    }

    async fn get_system_info(&self) -> Result<crate::app::config::api::SystemInfo, DcCmdError> {
        match self {
            Self::Dracoon(client) => SystemApi::get_system_info(client).await,
            Self::Mock(api) => SystemApi::get_system_info(api.as_ref()).await,
        }
    }
}

#[derive(Debug)]
struct ResolvedNodeSelection {
    path: String,
    node: Node,
}

#[derive(Debug)]
struct LimitedBufferWriter {
    buf: Vec<u8>,
    limit: usize,
    limit_exceeded: bool,
}

impl LimitedBufferWriter {
    fn new(limit: usize) -> Self {
        Self {
            buf: Vec::new(),
            limit,
            limit_exceeded: false,
        }
    }

    fn into_inner(self) -> Vec<u8> {
        self.buf
    }

    fn limit_exceeded(&self) -> bool {
        self.limit_exceeded
    }
}

impl tokio::io::AsyncWrite for LimitedBufferWriter {
    fn poll_write(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, std::io::Error>> {
        let available = self.limit.saturating_sub(self.buf.len());
        if buf.len() > available {
            self.limit_exceeded = true;
            return Poll::Ready(Err(std::io::Error::other(
                "nodes_read only supports files up to 1048576 bytes.",
            )));
        }

        self.buf.extend_from_slice(buf);
        Poll::Ready(Ok(buf.len()))
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), std::io::Error>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        Poll::Ready(Ok(()))
    }
}

fn build_node_path(node: &Node) -> Result<String, DcCmdError> {
    let parent = node.parent_path.clone().ok_or_else(|| {
        DcCmdError::InvalidPath(format!("Node '{}' has no parent path.", node.name))
    })?;

    if parent == "/" {
        Ok(format!("/{}/", node.name))
    } else {
        Ok(format!("{parent}{}/", node.name))
    }
}

fn normalize_remote_path(path: &str) -> String {
    let trimmed = path.trim().trim_matches('/');
    if trimmed.is_empty() {
        "/".to_string()
    } else {
        format!("/{trimmed}/")
    }
}

fn validate_selector(
    id_name: &str,
    id: Option<u64>,
    path_name: &str,
    path: Option<&str>,
) -> Result<(), DcCmdError> {
    match (id, path.filter(|value| !value.trim().is_empty())) {
        (Some(_), Some(_)) => Err(DcCmdError::InvalidArgument(format!(
            "Provide exactly one selector: {id_name} or {path_name}."
        ))),
        (None, None) => Err(DcCmdError::InvalidArgument(format!(
            "Provide exactly one selector: {id_name} or {path_name}."
        ))),
        _ => Ok(()),
    }
}

impl TryFrom<NodeListFilter> for NodesFilter {
    type Error = DcCmdError;

    fn try_from(value: NodeListFilter) -> Result<Self, Self::Error> {
        let filter = match value {
            NodeListFilter::Name { op, value } => match op {
                NodeFilterTextOp::Equals => NodesFilter::name_equals(value),
                NodeFilterTextOp::Contains => NodesFilter::name_contains(value),
            },
            NodeListFilter::Type { value } => NodesFilter::is_types(value),
            NodeListFilter::Encrypted { value } => NodesFilter::is_encrypted(value),
            NodeListFilter::BranchVersion { op, value } => match op {
                ListComparison::Gte => NodesFilter::branch_version_after(value),
                ListComparison::Lte => NodesFilter::branch_version_before(value),
            },
            NodeListFilter::CreatedAt { op, value } => match op {
                ListComparison::Gte => NodesFilter::created_after(value),
                ListComparison::Lte => NodesFilter::created_before(value),
            },
            NodeListFilter::UpdatedAt { op, value } => match op {
                ListComparison::Gte => NodesFilter::modified_after(value),
                ListComparison::Lte => NodesFilter::modified_before(value),
            },
            NodeListFilter::ReferenceId { value } => NodesFilter::reference_id_equals(value),
        };

        Ok(filter)
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::{HashMap, VecDeque},
        env, fs,
        path::PathBuf,
        sync::{Arc, Mutex},
        time::{SystemTime, UNIX_EPOCH},
    };

    use async_trait::async_trait;
    use dco3::{
        groups::{Group, GroupUser},
        nodes::{
            models::{UserInfo, UserType},
            RoomGuestUserInvitation,
        },
        users::{CreateUserRequest, UpdateUserRequest, UserData, UserItem},
        RangedItems,
    };
    use tokio::io::{AsyncWrite, AsyncWriteExt};

    use super::*;

    #[derive(Clone, Default)]
    struct MockReadApi {
        nodes_by_id: HashMap<u64, Node>,
        nodes_by_path: HashMap<String, Node>,
        payloads: HashMap<u64, Vec<u8>>,
    }

    #[derive(Clone, Default)]
    struct MockUsersApi {
        user_pages: Arc<Mutex<VecDeque<RangedItems<UserItem>>>>,
        users_by_id: Arc<HashMap<u64, UserData>>,
        current_user: Arc<Mutex<Option<crate::app::config::api::RefreshTokenInfo>>>,
        created_user: Arc<Mutex<Option<UserData>>>,
        oidc_configs: Arc<Mutex<Option<Vec<crate::app::config::api::OpenIdConfigInfo>>>>,
        system_info: Arc<Mutex<Option<crate::app::config::api::SystemInfo>>>,
    }

    #[derive(Clone, Default)]
    struct MockGroupsApi {
        group_pages: Arc<Mutex<VecDeque<RangedItems<Group>>>>,
        groups_by_id: Arc<HashMap<u64, Group>>,
        group_user_pages: Arc<Mutex<HashMap<u64, VecDeque<RangedItems<GroupUser>>>>>,
        users_by_name: Arc<HashMap<String, u64>>,
        created_group: Arc<Mutex<Option<Group>>>,
        add_group_users_calls: Arc<Mutex<Vec<(u64, Vec<u64>)>>>,
    }

    #[async_trait]
    impl NodesApi for MockReadApi {
        async fn get_node(&self, node_id: u64) -> Result<Node, DcCmdError> {
            self.nodes_by_id
                .get(&node_id)
                .cloned()
                .ok_or_else(|| DcCmdError::InvalidArgument(format!("Node not found: {node_id}")))
        }

        async fn get_node_from_path(&self, node_path: &str) -> Result<Option<Node>, DcCmdError> {
            Ok(self.nodes_by_path.get(node_path).cloned())
        }

        async fn get_nodes(
            &self,
            _parent_id: Option<u64>,
            _managed: Option<bool>,
            _params: Option<ListAllParams>,
        ) -> Result<NodeList, DcCmdError> {
            Ok(empty_node_list())
        }

        async fn search_nodes(
            &self,
            _search_string: &str,
            _parent_id: Option<u64>,
            _depth_level: Option<i8>,
            _params: Option<ListAllParams>,
        ) -> Result<NodeList, DcCmdError> {
            Ok(empty_node_list())
        }

        async fn delete_node(&self, _node_id: u64) -> Result<(), DcCmdError> {
            Ok(())
        }

        async fn delete_nodes(&self, _node_ids: Vec<u64>) -> Result<(), DcCmdError> {
            Ok(())
        }

        async fn copy_nodes(
            &self,
            _node_ids: Vec<u64>,
            _target_parent_id: u64,
        ) -> Result<(), DcCmdError> {
            Ok(())
        }

        async fn create_folder(
            &self,
            _node_name: &str,
            _parent_id: u64,
            _classification: Option<u8>,
            _notes: Option<String>,
        ) -> Result<Node, DcCmdError> {
            Ok(node(999, "mock-folder", NodeType::Folder, None, None, None))
        }

        async fn create_room(
            &self,
            _node_name: &str,
            _parent_id: u64,
            _classification: u8,
            _inherit_permissions: bool,
            _admin_ids: Option<Vec<u64>>,
        ) -> Result<(), DcCmdError> {
            Ok(())
        }
    }

    #[async_trait]
    impl DownloadApi for MockReadApi {
        async fn download_node<'w>(
            &'w self,
            node: &Node,
            writer: &'w mut (dyn AsyncWrite + Send + Unpin),
            _callback: Option<dco3::nodes::models::DownloadProgressCallback>,
        ) -> Result<(), DcCmdError> {
            if let Some(payload) = self.payloads.get(&node.id) {
                writer
                    .write_all(payload)
                    .await
                    .map_err(|_| DcCmdError::IoError)?;
            }
            Ok(())
        }
    }

    #[async_trait]
    impl UsersCommandApi for MockUsersApi {
        async fn create_user(&self, _req: CreateUserRequest) -> Result<UserData, DcCmdError> {
            self.created_user
                .lock()
                .expect("lock poisoned")
                .clone()
                .ok_or_else(|| {
                    DcCmdError::InvalidArgument("create_user not configured".to_string())
                })
        }

        async fn get_users(
            &self,
            _params: Option<ListAllParams>,
        ) -> Result<RangedItems<UserItem>, DcCmdError> {
            let mut pages = self.user_pages.lock().expect("lock poisoned");
            if pages.len() > 1 {
                Ok(pages.pop_front().expect("page present"))
            } else {
                Ok(pages.front().cloned().unwrap_or_else(empty_users_page))
            }
        }

        async fn delete_user(&self, _user_id: u64) -> Result<(), DcCmdError> {
            Err(DcCmdError::InvalidArgument(
                "delete_user not configured".to_string(),
            ))
        }

        async fn get_user(&self, user_id: u64) -> Result<UserData, DcCmdError> {
            self.users_by_id
                .get(&user_id)
                .cloned()
                .ok_or_else(|| DcCmdError::InvalidArgument(format!("User not found: {user_id}")))
        }

        async fn update_user(
            &self,
            _user_id: u64,
            _req: UpdateUserRequest,
        ) -> Result<UserData, DcCmdError> {
            Err(DcCmdError::InvalidArgument(
                "update_user not configured".to_string(),
            ))
        }

        async fn add_group_users(
            &self,
            _group_id: u64,
            _user_ids: Vec<u64>,
        ) -> Result<(), DcCmdError> {
            Err(DcCmdError::InvalidArgument(
                "add_group_users not configured".to_string(),
            ))
        }

        async fn get_group_users(
            &self,
            _group_id: u64,
            _params: Option<ListAllParams>,
        ) -> Result<RangedItems<GroupUser>, DcCmdError> {
            Err(DcCmdError::InvalidArgument(
                "get_group_users not configured".to_string(),
            ))
        }

        async fn get_node_from_path(&self, _path: &str) -> Result<Option<Node>, DcCmdError> {
            Err(DcCmdError::InvalidArgument(
                "get_node_from_path not configured".to_string(),
            ))
        }

        async fn invite_guest_users(
            &self,
            _room_id: u64,
            _users: Vec<RoomGuestUserInvitation>,
        ) -> Result<(), DcCmdError> {
            Err(DcCmdError::InvalidArgument(
                "invite_guest_users not configured".to_string(),
            ))
        }
    }

    #[async_trait]
    impl GroupsApi for MockGroupsApi {
        async fn get_groups(
            &self,
            _params: Option<ListAllParams>,
        ) -> Result<RangedItems<Group>, DcCmdError> {
            let mut pages = self.group_pages.lock().expect("lock poisoned");
            if pages.len() > 1 {
                Ok(pages.pop_front().expect("page present"))
            } else {
                Ok(pages.front().cloned().unwrap_or_else(empty_groups_page))
            }
        }

        async fn get_group(&self, group_id: u64) -> Result<Group, DcCmdError> {
            self.groups_by_id.get(&group_id).cloned().ok_or_else(|| {
                DcCmdError::InvalidArgument(format!("No group found with id: {group_id}"))
            })
        }

        async fn get_group_users(
            &self,
            group_id: u64,
            _params: Option<ListAllParams>,
        ) -> Result<RangedItems<GroupUser>, DcCmdError> {
            let mut pages = self.group_user_pages.lock().expect("lock poisoned");
            let pages = pages.get_mut(&group_id);
            if let Some(pages) = pages {
                if pages.len() > 1 {
                    Ok(pages.pop_front().expect("page present"))
                } else {
                    Ok(pages
                        .front()
                        .cloned()
                        .unwrap_or_else(empty_group_users_page))
                }
            } else {
                Ok(empty_group_users_page())
            }
        }

        async fn create_group(&self, _name: &str) -> Result<Group, DcCmdError> {
            self.created_group
                .lock()
                .expect("lock poisoned")
                .clone()
                .ok_or_else(|| {
                    DcCmdError::InvalidArgument("create_group not configured".to_string())
                })
        }

        async fn delete_group(&self, _group_id: u64) -> Result<(), DcCmdError> {
            Err(DcCmdError::InvalidArgument(
                "delete_group not configured".to_string(),
            ))
        }

        async fn add_group_users(
            &self,
            group_id: u64,
            user_ids: Vec<u64>,
        ) -> Result<Group, DcCmdError> {
            self.add_group_users_calls
                .lock()
                .expect("lock poisoned")
                .push((group_id, user_ids));
            self.get_group(group_id).await
        }
    }

    #[async_trait]
    impl UsersApi for MockGroupsApi {
        async fn find_user_id_by_username(&self, user_name: &str) -> Result<u64, DcCmdError> {
            self.users_by_name.get(user_name).copied().ok_or_else(|| {
                DcCmdError::InvalidArgument(format!("No user found with username: {user_name}"))
            })
        }
    }

    #[async_trait]
    impl SystemApi for MockUsersApi {
        async fn get_refresh_token_info(
            &self,
        ) -> Result<crate::app::config::api::RefreshTokenInfo, DcCmdError> {
            self.current_user
                .lock()
                .expect("lock poisoned")
                .clone()
                .ok_or_else(|| {
                    DcCmdError::InvalidArgument("refresh token info not configured".to_string())
                })
        }

        async fn get_oidc_idp_configs(
            &self,
        ) -> Result<Vec<crate::app::config::api::OpenIdConfigInfo>, DcCmdError> {
            self.oidc_configs
                .lock()
                .expect("lock poisoned")
                .clone()
                .ok_or_else(|| {
                    DcCmdError::InvalidArgument("oidc configs not configured".to_string())
                })
        }

        async fn get_system_info(&self) -> Result<crate::app::config::api::SystemInfo, DcCmdError> {
            self.system_info
                .lock()
                .expect("lock poisoned")
                .clone()
                .ok_or_else(|| {
                    DcCmdError::InvalidArgument("system info not configured".to_string())
                })
        }
    }

    fn unique_temp_dir(prefix: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock drift")
            .as_nanos();
        env::temp_dir().join(format!("{prefix}-{nanos}"))
    }

    fn platform() -> McpPlatform {
        let root = unique_temp_dir("dccmd-mcp-platform");
        fs::create_dir_all(&root).unwrap();
        McpPlatform::new(
            "https://example.com".to_string(),
            None,
            WorkspacePathGuard::new(root).unwrap(),
        )
        .unwrap()
    }

    fn empty_node_list() -> NodeList {
        dco3::models::RangedItems {
            range: dco3::models::Range {
                offset: 0,
                limit: 500,
                total: 0,
            },
            items: Vec::new(),
        }
    }

    fn empty_users_page() -> RangedItems<UserItem> {
        RangedItems {
            range: dco3::models::Range {
                offset: 0,
                limit: 500,
                total: 0,
            },
            items: Vec::new(),
        }
    }

    fn groups_page(items: Vec<Group>, total: u64) -> RangedItems<Group> {
        RangedItems {
            range: dco3::models::Range {
                offset: 0,
                limit: 500,
                total,
            },
            items,
        }
    }

    fn empty_groups_page() -> RangedItems<Group> {
        groups_page(Vec::new(), 0)
    }

    fn group_users_page(items: Vec<GroupUser>, total: u64) -> RangedItems<GroupUser> {
        RangedItems {
            range: dco3::models::Range {
                offset: 0,
                limit: 500,
                total,
            },
            items,
        }
    }

    fn empty_group_users_page() -> RangedItems<GroupUser> {
        group_users_page(Vec::new(), 0)
    }

    fn node(
        id: u64,
        name: &str,
        node_type: NodeType,
        file_type: Option<&str>,
        media_type: Option<&str>,
        size: Option<u64>,
    ) -> Node {
        Node {
            id,
            reference_id: None,
            node_type,
            name: name.to_string(),
            timestamp_creation: None,
            timestamp_modification: None,
            parent_id: None,
            parent_path: Some("/docs/".to_string()),
            created_at: None,
            created_by: None,
            updated_at: None,
            updated_by: None,
            expire_at: None,
            hash: None,
            file_type: file_type.map(str::to_string),
            media_type: media_type.map(str::to_string),
            size,
            classification: None,
            notes: None,
            permissions: None,
            inherit_permissions: None,
            is_encrypted: None,
            encryption_info: None,
            cnt_deleted_versions: None,
            cnt_comments: None,
            cnt_upload_shares: None,
            cnt_download_shares: None,
            recycle_bin_retention_period: None,
            has_activities_log: None,
            quota: None,
            is_favorite: None,
            branch_version: None,
            media_token: None,
            is_browsable: None,
            cnt_rooms: None,
            cnt_folders: None,
            cnt_files: None,
            auth_parent_id: None,
        }
    }

    fn user_item(id: u64, username: &str) -> UserItem {
        UserItem {
            id,
            user_name: username.to_string(),
            first_name: "First".to_string(),
            last_name: "Last".to_string(),
            is_locked: false,
            avatar_uuid: "avatar".to_string(),
            email: Some(format!("{username}@example.com")),
            phone: None,
            expire_at: None,
            has_manageable_rooms: None,
            is_encryption_enabled: None,
            last_login_success_at: Some("2026-04-03T12:00:00+00:00".to_string()),
            home_room_id: None,
            public_key_container: None,
            user_roles: None,
        }
    }

    fn user_data(id: u64, username: &str) -> UserData {
        UserData {
            id,
            user_name: username.to_string(),
            first_name: "First".to_string(),
            last_name: "Last".to_string(),
            is_locked: false,
            avatar_uuid: "avatar".to_string(),
            auth_data: dco3::user::UserAuthData::new_basic(None, None),
            email: Some(format!("{username}@example.com")),
            phone: None,
            expire_at: None,
            has_manageable_rooms: None,
            is_encryption_enabled: None,
            last_login_success_at: Some("2026-04-03T12:00:00+00:00".to_string()),
            home_room_id: None,
            public_key_container: None,
            user_roles: None,
            is_mfa_enabled: None,
            is_mfa_enforced: None,
        }
    }

    fn group(id: u64, name: &str, cnt_users: Option<u64>) -> Group {
        Group {
            id,
            name: name.to_string(),
            created_at: chrono::Utc::now(),
            created_by: UserInfo {
                id: 1,
                user_type: UserType::Internal,
                user_name: Some("creator".to_string()),
                first_name: Some("Group".to_string()),
                last_name: Some("Creator".to_string()),
                email: Some("creator@example.com".to_string()),
                avatar_uuid: "avatar".to_string(),
            },
            updated_at: None,
            updated_by: None,
            cnt_users,
            expire_at: None,
            group_roles: None,
        }
    }

    fn group_user(id: i64, username: &str) -> GroupUser {
        GroupUser {
            user_info: UserInfo {
                id,
                user_type: UserType::Internal,
                user_name: Some(username.to_string()),
                first_name: Some("First".to_string()),
                last_name: Some("Last".to_string()),
                email: Some(format!("{username}@example.com")),
                avatar_uuid: "avatar".to_string(),
            },
            is_member: true,
        }
    }

    #[tokio::test]
    async fn resolve_node_selector_uses_node_id_lookup_from_nodes_api() {
        let platform = platform();
        let expected = node(
            11,
            "report.json",
            NodeType::File,
            Some("json"),
            Some("application/json"),
            Some(12),
        );
        let api = MockReadApi {
            nodes_by_id: HashMap::from([(11, expected.clone())]),
            ..Default::default()
        };

        let selection = platform
            .resolve_node_selector(&api, Some(11), None)
            .await
            .unwrap();

        assert_eq!(selection.node.id, expected.id);
        assert_eq!(selection.path, "/docs/report.json/");
    }

    #[tokio::test]
    async fn resolve_node_selector_uses_normalized_path_lookup_from_nodes_api() {
        let platform = platform();
        let expected = node(
            22,
            "report.json",
            NodeType::File,
            Some("json"),
            Some("application/json"),
            Some(12),
        );
        let api = MockReadApi {
            nodes_by_path: HashMap::from([("/docs/report.json/".to_string(), expected.clone())]),
            ..Default::default()
        };

        let selection = platform
            .resolve_node_selector(&api, None, Some("docs/report.json"))
            .await
            .unwrap();

        assert_eq!(selection.node.id, expected.id);
        assert_eq!(selection.path, "/docs/report.json/");
    }

    #[tokio::test]
    async fn resolve_node_selector_rejects_missing_selector() {
        let platform = platform();
        let api = MockReadApi::default();

        let err = platform
            .resolve_node_selector(&api, None, None)
            .await
            .unwrap_err();

        assert_eq!(
            err,
            DcCmdError::InvalidArgument(
                "Provide exactly one selector: node_id or path.".to_string()
            )
        );
    }

    #[tokio::test]
    async fn resolve_node_selector_rejects_ambiguous_selector() {
        let platform = platform();
        let api = MockReadApi::default();

        let err = platform
            .resolve_node_selector(&api, Some(11), Some("/docs/report.json"))
            .await
            .unwrap_err();

        assert_eq!(
            err,
            DcCmdError::InvalidArgument(
                "Provide exactly one selector: node_id or path.".to_string()
            )
        );
    }

    #[tokio::test]
    async fn read_resolved_node_content_returns_structured_text_response() {
        let platform = platform();
        let file = node(
            31,
            "report.json",
            NodeType::File,
            Some("json"),
            Some("application/json"),
            Some(18),
        );
        let api = MockReadApi {
            payloads: HashMap::from([(31, br#"{"ok":true}"#.to_vec())]),
            ..Default::default()
        };

        let response = platform
            .read_resolved_node_content(
                &api,
                ResolvedNodeSelection {
                    path: "/docs/report.json/".to_string(),
                    node: file,
                },
            )
            .await
            .unwrap();

        assert_eq!(response.target, "https://example.com");
        assert_eq!(response.resolved_file.path, "/docs/report.json/");
        assert_eq!(response.file_type.as_deref(), Some("json"));
        assert_eq!(response.media_type.as_deref(), Some("application/json"));
        assert_eq!(response.encoding, "utf-8");
        assert_eq!(response.size, br#"{"ok":true}"#.len() as u64);
        assert_eq!(response.content, r#"{"ok":true}"#);
    }

    #[tokio::test]
    async fn read_resolved_node_content_rejects_non_file_nodes() {
        let platform = platform();
        let api = MockReadApi::default();

        let err = platform
            .read_resolved_node_content(
                &api,
                ResolvedNodeSelection {
                    path: "/docs/team/".to_string(),
                    node: node(41, "team", NodeType::Folder, None, None, None),
                },
            )
            .await
            .unwrap_err();

        assert_eq!(
            err,
            DcCmdError::InvalidArgument("nodes_read requires a file selector.".to_string())
        );
    }

    #[tokio::test]
    async fn read_resolved_node_content_rejects_oversized_metadata() {
        let platform = platform();
        let api = MockReadApi::default();

        let err = platform
            .read_resolved_node_content(
                &api,
                ResolvedNodeSelection {
                    path: "/docs/big.log/".to_string(),
                    node: node(
                        51,
                        "big.log",
                        NodeType::File,
                        Some("log"),
                        None,
                        Some((MAX_READ_BYTES + 1) as u64),
                    ),
                },
            )
            .await
            .unwrap_err();

        assert_eq!(err, McpPlatform::read_size_limit_error());
    }

    #[tokio::test]
    async fn read_resolved_node_content_rejects_streamed_oversize_payload() {
        let platform = platform();
        let file = node(61, "big.log", NodeType::File, Some("log"), None, None);
        let api = MockReadApi {
            payloads: HashMap::from([(61, vec![b'a'; MAX_READ_BYTES + 1])]),
            ..Default::default()
        };

        let err = platform
            .read_resolved_node_content(
                &api,
                ResolvedNodeSelection {
                    path: "/docs/big.log/".to_string(),
                    node: file,
                },
            )
            .await
            .unwrap_err();

        assert_eq!(err, McpPlatform::read_size_limit_error());
    }

    #[tokio::test]
    async fn read_resolved_node_content_rejects_invalid_utf8() {
        let platform = platform();
        let file = node(71, "data.txt", NodeType::File, Some("txt"), None, Some(2));
        let api = MockReadApi {
            payloads: HashMap::from([(71, vec![0xff, 0xfe])]),
            ..Default::default()
        };

        let err = platform
            .read_resolved_node_content(
                &api,
                ResolvedNodeSelection {
                    path: "/docs/data.txt/".to_string(),
                    node: file,
                },
            )
            .await
            .unwrap_err();

        assert_eq!(
            err,
            DcCmdError::InvalidArgument("nodes_read requires valid UTF-8 content.".to_string())
        );
    }

    #[tokio::test]
    async fn limited_buffer_writer_tracks_limit_exceeded() {
        let mut writer = LimitedBufferWriter::new(4);

        writer.write_all(b"abcd").await.unwrap();
        let err = writer.write_all(b"e").await.unwrap_err();

        assert_eq!(err.kind(), std::io::ErrorKind::Other);
        assert!(writer.limit_exceeded());
        assert_eq!(writer.into_inner(), b"abcd");
    }

    #[tokio::test]
    async fn list_groups_with_api_returns_structured_response() {
        let platform = platform();
        let engineering = group(11, "Engineering", Some(3));
        let api = MockGroupsApi {
            group_pages: Arc::new(Mutex::new(VecDeque::from([groups_page(
                vec![engineering.clone()],
                1,
            )]))),
            groups_by_id: Arc::new(HashMap::from([(engineering.id, engineering.clone())])),
            group_user_pages: Arc::default(),
            users_by_name: Arc::default(),
            created_group: Arc::new(Mutex::new(None)),
            add_group_users_calls: Arc::default(),
        };

        let response = platform
            .list_groups_with_api(
                &api,
                GroupsListRequest {
                    filter: None,
                    offset: None,
                    limit: None,
                    all: false,
                },
            )
            .await
            .unwrap();

        assert_eq!(response.target, "https://example.com");
        assert_eq!(response.total, 1);
        assert_eq!(response.items.len(), 1);
        assert_eq!(response.items[0].id, 11);
        assert_eq!(response.items[0].name, "Engineering");
    }

    #[tokio::test]
    async fn create_group_with_api_returns_created_group() {
        let platform = platform();
        let engineering = group(11, "Engineering", Some(0));
        let api = MockGroupsApi {
            group_pages: Arc::default(),
            groups_by_id: Arc::new(HashMap::from([(engineering.id, engineering.clone())])),
            group_user_pages: Arc::default(),
            users_by_name: Arc::default(),
            created_group: Arc::new(Mutex::new(Some(engineering.clone()))),
            add_group_users_calls: Arc::default(),
        };

        let response = platform
            .create_group_with_api(
                &api,
                GroupsCreateRequest {
                    name: "Engineering".to_string(),
                },
            )
            .await
            .unwrap();

        assert_eq!(response.target, "https://example.com");
        assert_eq!(response.group.id, 11);
        assert_eq!(response.group.name, "Engineering");
    }

    #[tokio::test]
    async fn get_group_users_with_api_supports_group_id_lookup() {
        let platform = platform();
        let engineering = group(11, "Engineering", Some(2));
        let api = MockGroupsApi {
            group_pages: Arc::new(Mutex::new(VecDeque::from([groups_page(
                vec![engineering.clone()],
                1,
            )]))),
            groups_by_id: Arc::new(HashMap::from([(engineering.id, engineering.clone())])),
            group_user_pages: Arc::new(Mutex::new(HashMap::from([(
                engineering.id,
                VecDeque::from([group_users_page(
                    vec![group_user(7, "alice"), group_user(8, "bob")],
                    2,
                )]),
            )]))),
            users_by_name: Arc::default(),
            created_group: Arc::new(Mutex::new(None)),
            add_group_users_calls: Arc::default(),
        };

        let response = platform
            .get_group_users_with_api(
                &api,
                GroupsGetUsersRequest {
                    group_id: Some(11),
                    group_name: None,
                    filter: None,
                    offset: None,
                    limit: None,
                    all: false,
                },
            )
            .await
            .unwrap();

        assert_eq!(response.target, "https://example.com");
        assert_eq!(response.group.id, 11);
        assert_eq!(response.items.len(), 2);
        assert_eq!(response.items[0].username.as_deref(), Some("alice"));
        assert_eq!(response.items[1].username.as_deref(), Some("bob"));
    }

    #[tokio::test]
    async fn add_group_users_with_api_resolves_usernames_and_deduplicates_ids() {
        let platform = platform();
        let engineering = group(11, "Engineering", Some(2));
        let api = MockGroupsApi {
            group_pages: Arc::new(Mutex::new(VecDeque::from([groups_page(
                vec![engineering.clone()],
                1,
            )]))),
            groups_by_id: Arc::new(HashMap::from([(engineering.id, engineering.clone())])),
            group_user_pages: Arc::default(),
            users_by_name: Arc::new(HashMap::from([
                ("alice".to_string(), 7_u64),
                ("bob".to_string(), 8_u64),
            ])),
            created_group: Arc::new(Mutex::new(None)),
            add_group_users_calls: Arc::default(),
        };

        let response = platform
            .add_group_users_with_api(
                &api,
                GroupsAddUsersRequest {
                    group_id: Some(11),
                    group_name: None,
                    user_ids: vec![7],
                    usernames: vec!["alice".to_string(), "bob".to_string()],
                },
            )
            .await
            .unwrap();

        assert_eq!(response.target, "https://example.com");
        assert_eq!(response.group.id, 11);
        assert_eq!(response.added_user_ids, vec![7, 8]);
        assert_eq!(response.added_count, 2);
        assert_eq!(
            api.add_group_users_calls
                .lock()
                .expect("lock poisoned")
                .as_slice(),
            &[(11, vec![7, 8])]
        );
    }

    #[tokio::test]
    async fn add_group_users_with_api_rejects_missing_user_selectors() {
        let platform = platform();
        let api = MockGroupsApi::default();

        let err = platform
            .add_group_users_with_api(
                &api,
                GroupsAddUsersRequest {
                    group_id: Some(11),
                    group_name: None,
                    user_ids: Vec::new(),
                    usernames: Vec::new(),
                },
            )
            .await
            .unwrap_err();

        assert_eq!(
            err,
            DcCmdError::InvalidArgument(
                "Provide at least one user selector source: user_ids or usernames.".to_string()
            )
        );
    }

    #[tokio::test]
    async fn list_users_with_api_returns_structured_response() {
        let platform = platform();
        let api = MockUsersApi {
            user_pages: Arc::new(Mutex::new(VecDeque::from([RangedItems {
                range: dco3::models::Range {
                    offset: 0,
                    limit: 500,
                    total: 1,
                },
                items: vec![user_item(1, "alice")],
            }]))),
            users_by_id: Arc::default(),
            current_user: Arc::new(Mutex::new(None)),
            created_user: Arc::new(Mutex::new(None)),
            oidc_configs: Arc::new(Mutex::new(None)),
            system_info: Arc::new(Mutex::new(None)),
        };

        let response = platform
            .list_users_with_api(
                &api,
                UsersListRequest {
                    filter: None,
                    offset: None,
                    limit: None,
                    all: false,
                },
            )
            .await
            .unwrap();

        assert_eq!(response.target, "https://example.com");
        assert_eq!(response.total, 1);
        assert_eq!(response.offset, 0);
        assert_eq!(response.limit, 500);
        assert!(!response.all_fetched);
        assert_eq!(response.items.len(), 1);
        assert_eq!(response.items[0].username, "alice");
    }

    #[tokio::test]
    async fn list_users_with_api_fetches_all_pages() {
        let platform = platform();
        let api = MockUsersApi {
            user_pages: Arc::new(Mutex::new(VecDeque::from([
                RangedItems {
                    range: dco3::models::Range {
                        offset: 0,
                        limit: 500,
                        total: 700,
                    },
                    items: vec![user_item(1, "alice")],
                },
                RangedItems {
                    range: dco3::models::Range {
                        offset: 500,
                        limit: 500,
                        total: 700,
                    },
                    items: vec![user_item(2, "bob")],
                },
            ]))),
            users_by_id: Arc::default(),
            current_user: Arc::new(Mutex::new(None)),
            created_user: Arc::new(Mutex::new(None)),
            oidc_configs: Arc::new(Mutex::new(None)),
            system_info: Arc::new(Mutex::new(None)),
        };

        let response = platform
            .list_users_with_api(
                &api,
                UsersListRequest {
                    filter: None,
                    offset: None,
                    limit: None,
                    all: true,
                },
            )
            .await
            .unwrap();

        assert_eq!(response.total, 700);
        assert!(response.all_fetched);
        assert_eq!(response.items.len(), 2);
        assert_eq!(response.items[1].username, "bob");
    }

    #[tokio::test]
    async fn get_user_info_with_api_supports_username_lookup() {
        let platform = platform();
        let api = MockUsersApi {
            user_pages: Arc::new(Mutex::new(VecDeque::from([RangedItems {
                range: dco3::models::Range {
                    offset: 0,
                    limit: 500,
                    total: 1,
                },
                items: vec![user_item(1, "alice")],
            }]))),
            users_by_id: Arc::default(),
            current_user: Arc::new(Mutex::new(None)),
            created_user: Arc::new(Mutex::new(None)),
            oidc_configs: Arc::new(Mutex::new(None)),
            system_info: Arc::new(Mutex::new(None)),
        };

        let response = platform
            .get_user_info_with_api(
                &api,
                UsersInfoRequest {
                    user_id: None,
                    username: Some("alice".to_string()),
                },
            )
            .await
            .unwrap();

        assert_eq!(response.target, "https://example.com");
        assert_eq!(response.user.id, 1);
        assert_eq!(response.user.username, "alice");
    }

    #[tokio::test]
    async fn get_user_info_with_api_supports_id_lookup() {
        let platform = platform();
        let api = MockUsersApi {
            user_pages: Arc::new(Mutex::new(VecDeque::new())),
            users_by_id: Arc::new(HashMap::from([(7, user_data(7, "alice"))])),
            current_user: Arc::new(Mutex::new(None)),
            created_user: Arc::new(Mutex::new(None)),
            oidc_configs: Arc::new(Mutex::new(None)),
            system_info: Arc::new(Mutex::new(None)),
        };

        let response = platform
            .get_user_info_with_api(
                &api,
                UsersInfoRequest {
                    user_id: Some(7),
                    username: None,
                },
            )
            .await
            .unwrap();

        assert_eq!(response.user.id, 7);
        assert_eq!(response.user.username, "alice");
    }

    #[tokio::test]
    async fn get_user_info_with_api_rejects_ambiguous_selector() {
        let platform = platform();
        let api = MockUsersApi::default();

        let err = platform
            .get_user_info_with_api(
                &api,
                UsersInfoRequest {
                    user_id: Some(7),
                    username: Some("alice".to_string()),
                },
            )
            .await
            .unwrap_err();

        assert_eq!(
            err,
            DcCmdError::InvalidArgument(
                "Provide exactly one selector: user_id or username.".to_string()
            )
        );
    }

    #[tokio::test]
    async fn get_current_user_info_with_api_returns_current_user() {
        let platform = platform();
        let api = MockUsersApi {
            user_pages: Arc::new(Mutex::new(VecDeque::from([RangedItems {
                range: dco3::models::Range {
                    offset: 0,
                    limit: 500,
                    total: 1,
                },
                items: vec![user_item(9, "alice")],
            }]))),
            users_by_id: Arc::default(),
            current_user: Arc::new(Mutex::new(Some(
                crate::app::config::api::RefreshTokenInfo {
                    first_name: "First".to_string(),
                    last_name: "Last".to_string(),
                    email: Some("alice@example.com".to_string()),
                    user_name: "alice".to_string(),
                },
            ))),
            created_user: Arc::new(Mutex::new(None)),
            oidc_configs: Arc::new(Mutex::new(None)),
            system_info: Arc::new(Mutex::new(None)),
        };

        let response = platform.get_current_user_info_with_api(&api).await.unwrap();

        assert_eq!(response.target, "https://example.com");
        assert_eq!(response.user.id, 9);
        assert_eq!(response.user.username, "alice");
    }

    #[tokio::test]
    async fn create_user_with_api_returns_created_user() {
        let platform = platform();
        let api = MockUsersApi {
            user_pages: Arc::default(),
            users_by_id: Arc::default(),
            current_user: Arc::new(Mutex::new(None)),
            created_user: Arc::new(Mutex::new(Some(user_data(17, "alice")))),
            oidc_configs: Arc::new(Mutex::new(None)),
            system_info: Arc::new(Mutex::new(None)),
        };

        let response = platform
            .create_user_with_api(
                &api,
                UsersCreateRequest {
                    first_name: "Alice".to_string(),
                    last_name: "Admin".to_string(),
                    email: "alice@example.com".to_string(),
                    login: None,
                    oidc_id: None,
                    mfa_enforced: false,
                    group_id: None,
                },
            )
            .await
            .unwrap();

        assert_eq!(response.target, "https://example.com");
        assert_eq!(response.user.id, 17);
        assert_eq!(response.user.username, "alice");
        assert_eq!(response.auth_method, "basic");
    }

    #[tokio::test]
    async fn get_oidc_idps_with_api_returns_structured_response() {
        let platform = platform();
        let api = MockUsersApi {
            user_pages: Arc::default(),
            users_by_id: Arc::default(),
            current_user: Arc::new(Mutex::new(None)),
            created_user: Arc::new(Mutex::new(None)),
            oidc_configs: Arc::new(Mutex::new(Some(vec![
                crate::app::config::api::OpenIdConfigInfo {
                    id: 7,
                    name: "Okta".to_string(),
                },
                crate::app::config::api::OpenIdConfigInfo {
                    id: 8,
                    name: "Azure AD".to_string(),
                },
            ]))),
            system_info: Arc::new(Mutex::new(Some(crate::app::config::api::SystemInfo {
                customer: crate::app::config::api::CustomerInfo {
                    name: "Example".to_string(),
                    space_used: 0,
                    space_limit: 0,
                    accounts_used: 0,
                    accounts_limit: 0,
                },
                oidc_configs: vec![],
                ad_configs: Vec::new(),
            }))),
        };

        let response = platform.get_oidc_idps_with_api(&api).await.unwrap();

        assert_eq!(response.target, "https://example.com");
        assert_eq!(response.items.len(), 2);
        assert_eq!(response.items[0].id, 7);
        assert_eq!(response.items[0].name, "Okta");
        assert_eq!(response.items[1].id, 8);
        assert_eq!(response.items[1].name, "Azure AD");
    }
}

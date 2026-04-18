use std::{
    collections::{HashMap, HashSet},
    fs,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use async_trait::async_trait;
use dco3::{
    auth::models::DracoonErrorResponse,
    groups::{Group, GroupUser},
    nodes::{
        models::{NodeList, UserInfo, UserType},
        Node, RoomGuestUserInvitation,
    },
    user::UserAuthData,
    users::{CreateUserRequest, UpdateUserRequest, UserData, UserItem},
    ListAllParams, RangedItems,
};
use rmcp::{
    model::{CallToolRequestParams, ErrorCode},
    serde_json::Value,
    serve_client,
    service::RunningService,
    RoleClient, RoleServer, ServiceError, ServiceExt,
};
use serde::de::DeserializeOwned;
use tokio::io::AsyncWrite;

use crate::{
    app::{
        config::api::{CustomerInfo, OpenIdConfigInfo, RefreshTokenInfo, SystemApi, SystemInfo},
        groups::api::GroupsApi,
        nodes::{
            api::NodesApi,
            download::api::DownloadApi,
            upload::{UploadApi, UploadProgressFn},
        },
        users::api::{UsersApi, UsersCommandApi},
    },
    core::models::DcCmdError,
    mcp::{
        paths::WorkspacePathGuard,
        platform::{McpBackend, McpPlatform, McpSession, McpSessionProvider},
        server::DccmdMcpServer,
    },
};

const TEST_TARGET: &str = "https://staging.dracoon.com";

#[derive(Clone, Default)]
pub(super) struct MockMcpBackend {
    state: Arc<Mutex<MockMcpState>>,
}

#[derive(Default)]
struct MockMcpState {
    next_node_id: u64,
    next_group_id: u64,
    nodes_by_id: HashMap<u64, Node>,
    download_payloads: HashMap<u64, Vec<u8>>,
    share_links: HashMap<u64, String>,
    upload_failures: HashMap<String, String>,
    conflict_folders: HashSet<(u64, String)>,
    users_by_id: HashMap<u64, UserData>,
    created_user: Option<UserData>,
    current_user: Option<RefreshTokenInfo>,
    oidc_configs: Vec<OpenIdConfigInfo>,
    oidc_error: bool,
    groups_by_id: HashMap<u64, Group>,
    group_users: HashMap<u64, Vec<GroupUser>>,
    created_group: Option<Group>,
    add_group_users_calls: Vec<(u64, Vec<u64>)>,
}

impl MockMcpBackend {
    pub(super) fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(MockMcpState {
                next_node_id: 10_000,
                next_group_id: 20_000,
                ..MockMcpState::default()
            })),
        }
    }

    pub(super) fn insert_node(&self, node: Node) {
        self.state
            .lock()
            .expect("lock poisoned")
            .nodes_by_id
            .insert(node.id, node);
    }

    pub(super) fn set_download_payload(&self, node_id: u64, payload: impl Into<Vec<u8>>) {
        self.state
            .lock()
            .expect("lock poisoned")
            .download_payloads
            .insert(node_id, payload.into());
    }

    pub(super) fn insert_user(&self, user: UserData) {
        self.state
            .lock()
            .expect("lock poisoned")
            .users_by_id
            .insert(user.id, user);
    }

    pub(super) fn set_created_user(&self, user: UserData) {
        self.state.lock().expect("lock poisoned").created_user = Some(user);
    }

    pub(super) fn set_current_user(&self, user: &UserData) {
        self.state.lock().expect("lock poisoned").current_user = Some(RefreshTokenInfo {
            first_name: user.first_name.clone(),
            last_name: user.last_name.clone(),
            email: user.email.clone(),
            user_name: user.user_name.clone(),
        });
    }

    pub(super) fn set_oidc_configs(&self, configs: Vec<OpenIdConfigInfo>) {
        self.state.lock().expect("lock poisoned").oidc_configs = configs;
    }

    pub(super) fn set_oidc_error(&self, enabled: bool) {
        self.state.lock().expect("lock poisoned").oidc_error = enabled;
    }

    pub(super) fn insert_group(&self, group: Group) {
        self.state
            .lock()
            .expect("lock poisoned")
            .groups_by_id
            .insert(group.id, group);
    }

    pub(super) fn set_created_group(&self, group: Group) {
        self.state.lock().expect("lock poisoned").created_group = Some(group);
    }

    pub(super) fn set_group_users(&self, group_id: u64, users: Vec<GroupUser>) {
        self.state
            .lock()
            .expect("lock poisoned")
            .group_users
            .insert(group_id, users);
    }

    pub(super) fn add_group_users_calls(&self) -> Vec<(u64, Vec<u64>)> {
        self.state
            .lock()
            .expect("lock poisoned")
            .add_group_users_calls
            .clone()
    }

    fn state(&self) -> std::sync::MutexGuard<'_, MockMcpState> {
        self.state.lock().expect("lock poisoned")
    }
}

#[async_trait]
impl NodesApi for MockMcpBackend {
    async fn get_node(&self, node_id: u64) -> Result<Node, DcCmdError> {
        self.state()
            .nodes_by_id
            .get(&node_id)
            .cloned()
            .ok_or_else(|| DcCmdError::InvalidPath(format!("node:{node_id}")))
    }

    async fn get_node_from_path(&self, node_path: &str) -> Result<Option<Node>, DcCmdError> {
        let normalized = normalize_remote_path(node_path);
        Ok(self
            .state()
            .nodes_by_id
            .values()
            .find(|node| remote_node_path(node).ok().as_deref() == Some(normalized.as_str()))
            .cloned())
    }

    async fn get_nodes(
        &self,
        parent_id: Option<u64>,
        _managed: Option<bool>,
        params: Option<ListAllParams>,
    ) -> Result<NodeList, DcCmdError> {
        let mut items = self
            .state()
            .nodes_by_id
            .values()
            .filter(|node| node.parent_id == parent_id)
            .cloned()
            .collect::<Vec<_>>();
        items.sort_by_key(|node| node.id);

        Ok(node_page(items, params))
    }

    async fn search_nodes(
        &self,
        search_string: &str,
        parent_id: Option<u64>,
        _depth_level: Option<i8>,
        params: Option<ListAllParams>,
    ) -> Result<NodeList, DcCmdError> {
        let query = search_string.replace('*', "").to_lowercase();
        let mut items = self
            .state()
            .nodes_by_id
            .values()
            .filter(|node| parent_id.is_none() || node.parent_id == parent_id)
            .filter(|node| node.name.to_lowercase().contains(&query))
            .cloned()
            .collect::<Vec<_>>();
        items.sort_by_key(|node| node.id);

        Ok(node_page(items, params))
    }

    async fn delete_node(&self, node_id: u64) -> Result<(), DcCmdError> {
        self.state().nodes_by_id.remove(&node_id);
        Ok(())
    }

    async fn delete_nodes(&self, node_ids: Vec<u64>) -> Result<(), DcCmdError> {
        let mut state = self.state();
        for node_id in node_ids {
            state.nodes_by_id.remove(&node_id);
        }
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
        node_name: &str,
        parent_id: u64,
        _classification: Option<u8>,
        _notes: Option<String>,
    ) -> Result<Node, DcCmdError> {
        let mut state = self.state();
        if state
            .conflict_folders
            .contains(&(parent_id, node_name.to_string()))
        {
            return Err(DcCmdError::DracoonError(DracoonErrorResponse::new(
                409, "Conflict",
            )));
        }

        let parent = state
            .nodes_by_id
            .get(&parent_id)
            .cloned()
            .ok_or_else(|| DcCmdError::InvalidPath(format!("parent:{parent_id}")))?;
        let folder = node(
            state.next_node_id,
            node_name,
            dco3::nodes::models::NodeType::Folder,
            Some(parent_id),
            Some(remote_node_path(&parent)?),
            None,
            None,
            None,
            parent.is_encrypted,
        );
        state.next_node_id += 1;
        state.nodes_by_id.insert(folder.id, folder.clone());
        Ok(folder)
    }

    async fn create_room(
        &self,
        node_name: &str,
        parent_id: u64,
        _classification: u8,
        _inherit_permissions: bool,
        _admin_ids: Option<Vec<u64>>,
    ) -> Result<(), DcCmdError> {
        let mut state = self.state();
        let parent = state
            .nodes_by_id
            .get(&parent_id)
            .cloned()
            .ok_or_else(|| DcCmdError::InvalidPath(format!("parent:{parent_id}")))?;
        let room = node(
            state.next_node_id,
            node_name,
            dco3::nodes::models::NodeType::Room,
            Some(parent_id),
            Some(remote_node_path(&parent)?),
            None,
            None,
            None,
            parent.is_encrypted,
        );
        state.next_node_id += 1;
        state.nodes_by_id.insert(room.id, room);
        Ok(())
    }
}

#[async_trait]
impl DownloadApi for MockMcpBackend {
    async fn download_node<'w>(
        &'w self,
        node: &Node,
        writer: &'w mut (dyn AsyncWrite + Send + Unpin),
        _callback: Option<dco3::nodes::models::DownloadProgressCallback>,
    ) -> Result<(), DcCmdError> {
        let payload = self
            .state()
            .download_payloads
            .get(&node.id)
            .cloned()
            .unwrap_or_default();
        tokio::io::AsyncWriteExt::write_all(writer, &payload)
            .await
            .map_err(|_| DcCmdError::IoError)
    }
}

#[async_trait]
impl UploadApi for MockMcpBackend {
    async fn upload_local_file(
        &self,
        source: PathBuf,
        target_node: &Node,
        _classification: u8,
        _overwrite: bool,
        _keep_share_links: bool,
        on_progress: Option<UploadProgressFn>,
    ) -> Result<Node, DcCmdError> {
        let source_display = source.to_string_lossy().to_string();
        let maybe_error = self.state().upload_failures.get(&source_display).cloned();
        if let Some(reason) = maybe_error {
            return Err(DcCmdError::InvalidArgument(reason));
        }

        let metadata = fs::metadata(&source).map_err(|_| DcCmdError::IoError)?;
        if let Some(on_progress) = on_progress {
            on_progress(metadata.len());
        }

        let file_name = source
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .ok_or_else(|| DcCmdError::InvalidPath(source_display.clone()))?;

        let mut state = self.state();
        let uploaded = node(
            state.next_node_id,
            &file_name,
            dco3::nodes::models::NodeType::File,
            Some(target_node.id),
            Some(remote_node_path(target_node)?),
            Some("txt"),
            Some("text/plain"),
            Some(metadata.len()),
            target_node.is_encrypted,
        );
        state.next_node_id += 1;
        state.nodes_by_id.insert(uploaded.id, uploaded.clone());
        Ok(uploaded)
    }

    async fn create_download_share_link(
        &self,
        node: &Node,
        _share_password: Option<String>,
    ) -> Result<String, DcCmdError> {
        Ok(self
            .state()
            .share_links
            .get(&node.id)
            .cloned()
            .unwrap_or_else(|| format!("https://example.com/public/download-shares/{}", node.id)))
    }
}

#[async_trait]
impl UsersApi for MockMcpBackend {
    async fn find_user_id_by_username(&self, user_name: &str) -> Result<u64, DcCmdError> {
        self.state()
            .users_by_id
            .values()
            .find(|user| user.user_name == user_name)
            .map(|user| user.id)
            .ok_or_else(|| {
                DcCmdError::InvalidArgument(format!("No user found with username: {user_name}"))
            })
    }
}

#[async_trait]
impl UsersCommandApi for MockMcpBackend {
    async fn create_user(&self, _req: CreateUserRequest) -> Result<UserData, DcCmdError> {
        let maybe_user = self.state().created_user.clone();
        let user = maybe_user
            .ok_or_else(|| DcCmdError::InvalidArgument("create_user not configured".to_string()))?;
        self.insert_user(user.clone());
        Ok(user)
    }

    async fn get_users(
        &self,
        params: Option<ListAllParams>,
    ) -> Result<RangedItems<UserItem>, DcCmdError> {
        let mut items = self
            .state()
            .users_by_id
            .values()
            .cloned()
            .map(user_item_from_data)
            .collect::<Vec<_>>();
        items.sort_by_key(|user| user.id);
        Ok(user_page(items, params))
    }

    async fn delete_user(&self, user_id: u64) -> Result<(), DcCmdError> {
        self.state().users_by_id.remove(&user_id);
        Ok(())
    }

    async fn get_user(&self, user_id: u64) -> Result<UserData, DcCmdError> {
        self.state()
            .users_by_id
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

    async fn add_group_users(&self, group_id: u64, user_ids: Vec<u64>) -> Result<(), DcCmdError> {
        let group_users = user_ids
            .iter()
            .map(|user_id| {
                let user = self
                    .state()
                    .users_by_id
                    .get(user_id)
                    .cloned()
                    .ok_or_else(|| {
                        DcCmdError::InvalidArgument(format!("User not found: {user_id}"))
                    })?;
                Ok(group_user(
                    i64::try_from(user.id).expect("user id fits"),
                    &user.user_name,
                ))
            })
            .collect::<Result<Vec<_>, DcCmdError>>()?;

        let mut state = self.state();
        state
            .group_users
            .entry(group_id)
            .or_default()
            .extend(group_users);
        Ok(())
    }

    async fn get_group_users(
        &self,
        group_id: u64,
        params: Option<ListAllParams>,
    ) -> Result<RangedItems<GroupUser>, DcCmdError> {
        let items = self
            .state()
            .group_users
            .get(&group_id)
            .cloned()
            .unwrap_or_default();
        Ok(group_users_page(items, params))
    }

    async fn get_node_from_path(&self, path: &str) -> Result<Option<Node>, DcCmdError> {
        NodesApi::get_node_from_path(self, path).await
    }

    async fn invite_guest_users(
        &self,
        _room_id: u64,
        _users: Vec<RoomGuestUserInvitation>,
    ) -> Result<(), DcCmdError> {
        Ok(())
    }
}

#[async_trait]
impl GroupsApi for MockMcpBackend {
    async fn get_groups(
        &self,
        params: Option<ListAllParams>,
    ) -> Result<RangedItems<Group>, DcCmdError> {
        let mut items = self
            .state()
            .groups_by_id
            .values()
            .cloned()
            .collect::<Vec<_>>();
        items.sort_by_key(|group| group.id);
        Ok(groups_page(items, params))
    }

    async fn get_group(&self, group_id: u64) -> Result<Group, DcCmdError> {
        self.state()
            .groups_by_id
            .get(&group_id)
            .cloned()
            .ok_or_else(|| DcCmdError::InvalidArgument(format!("Group not found: {group_id}")))
    }

    async fn get_group_users(
        &self,
        group_id: u64,
        params: Option<ListAllParams>,
    ) -> Result<RangedItems<GroupUser>, DcCmdError> {
        let items = self
            .state()
            .group_users
            .get(&group_id)
            .cloned()
            .unwrap_or_default();
        Ok(group_users_page(items, params))
    }

    async fn create_group(&self, name: &str) -> Result<Group, DcCmdError> {
        let mut state = self.state();
        let group = state
            .created_group
            .clone()
            .unwrap_or_else(|| group(state.next_group_id, name, Some(0)));
        state.next_group_id += 1;
        state.groups_by_id.insert(group.id, group.clone());
        Ok(group)
    }

    async fn delete_group(&self, group_id: u64) -> Result<(), DcCmdError> {
        self.state().groups_by_id.remove(&group_id);
        Ok(())
    }

    async fn add_group_users(
        &self,
        group_id: u64,
        user_ids: Vec<u64>,
    ) -> Result<Group, DcCmdError> {
        let mut state = self.state();
        state
            .add_group_users_calls
            .push((group_id, user_ids.clone()));
        let cnt_users = Some(user_ids.len() as u64);
        let updated_group = state
            .groups_by_id
            .get(&group_id)
            .cloned()
            .map(|group| Group { cnt_users, ..group })
            .ok_or_else(|| DcCmdError::InvalidArgument(format!("Group not found: {group_id}")))?;
        state.groups_by_id.insert(group_id, updated_group.clone());
        Ok(updated_group)
    }
}

#[async_trait]
impl SystemApi for MockMcpBackend {
    async fn get_refresh_token_info(&self) -> Result<RefreshTokenInfo, DcCmdError> {
        self.state()
            .current_user
            .clone()
            .ok_or_else(|| DcCmdError::InvalidArgument("current_user not configured".to_string()))
    }

    async fn get_oidc_idp_configs(&self) -> Result<Vec<OpenIdConfigInfo>, DcCmdError> {
        let state = self.state();
        if state.oidc_error {
            return Err(DcCmdError::ConnectionFailed);
        }
        Ok(state.oidc_configs.clone())
    }

    async fn get_system_info(&self) -> Result<SystemInfo, DcCmdError> {
        let oidc_configs = self.get_oidc_idp_configs().await?;
        Ok(SystemInfo {
            customer: CustomerInfo {
                name: "Mock Customer".to_string(),
                space_used: 0,
                space_limit: 0,
                accounts_used: 0,
                accounts_limit: 0,
            },
            oidc_configs,
            ad_configs: Vec::new(),
        })
    }
}

#[derive(Clone)]
struct MockSessionProviderImpl {
    backend: MockMcpBackend,
    encryption_enabled: bool,
}

#[async_trait]
impl McpSessionProvider for MockSessionProviderImpl {
    async fn connect(&self, _target: &str) -> Result<McpSession, DcCmdError> {
        Ok(McpSession::Mock(
            Arc::new(self.backend.clone()) as Arc<dyn McpBackend>
        ))
    }

    async fn ensure_encryption(
        &self,
        _target: &str,
        session: McpSession,
        _encryption_password: Option<secrecy::SecretString>,
    ) -> Result<McpSession, DcCmdError> {
        if self.encryption_enabled {
            Ok(session)
        } else {
            Err(DcCmdError::InvalidArgument(
                "Encrypted node access requires a stored encryption secret or startup --encryption-password.".to_string(),
            ))
        }
    }
}

pub(super) struct McpE2eHarness {
    client: RunningService<RoleClient, ()>,
    server: RunningService<RoleServer, DccmdMcpServer>,
    workspace: PathBuf,
}

impl McpE2eHarness {
    pub(super) async fn start(
        backend: MockMcpBackend,
        allow_destructive: bool,
        encryption_enabled: bool,
    ) -> Self {
        let workspace = unique_temp_dir("dccmd-mcp-e2e");
        fs::create_dir_all(&workspace).expect("create workspace");
        let path_guard = WorkspacePathGuard::new(workspace.clone()).expect("workspace guard");
        let provider = Arc::new(MockSessionProviderImpl {
            backend,
            encryption_enabled,
        });
        let platform =
            McpPlatform::with_session_provider(TEST_TARGET.to_string(), None, path_guard, provider)
                .expect("mcp platform");
        let server = DccmdMcpServer::new(platform, allow_destructive);
        let (server_transport, client_transport) = tokio::io::duplex(16 * 1024);
        let (server, client) = tokio::time::timeout(Duration::from_secs(5), async {
            tokio::join!(
                server.serve(server_transport),
                serve_client((), client_transport)
            )
        })
        .await
        .expect("rmcp startup timed out");
        let server = server.expect("start rmcp server");
        let client: RunningService<RoleClient, ()> = client.expect("start rmcp client");

        Self {
            client,
            server,
            workspace,
        }
    }

    pub(super) fn workspace_path(&self, relative: &str) -> PathBuf {
        self.workspace.join(relative)
    }

    pub(super) async fn list_tool_names(&self) -> Vec<String> {
        let mut names = tokio::time::timeout(Duration::from_secs(5), self.client.list_all_tools())
            .await
            .expect("list_all_tools timed out")
            .expect("list tools")
            .into_iter()
            .map(|tool| tool.name.to_string())
            .collect::<Vec<_>>();
        names.sort();
        names
    }

    pub(super) async fn call_tool<T>(&self, name: &str, arguments: Value) -> T
    where
        T: DeserializeOwned,
    {
        let mut request = CallToolRequestParams::new(name.to_string());
        if let Some(arguments) = arguments.as_object() {
            request = request.with_arguments(arguments.clone());
        }

        let result = tokio::time::timeout(Duration::from_secs(5), self.client.call_tool(request))
            .await
            .expect("tool call timed out")
            .expect("tool call succeeded");
        result.into_typed().expect("typed tool result")
    }

    pub(super) async fn call_tool_error(&self, name: &str, arguments: Value) -> ServiceError {
        let mut request = CallToolRequestParams::new(name.to_string());
        if let Some(arguments) = arguments.as_object() {
            request = request.with_arguments(arguments.clone());
        }

        tokio::time::timeout(Duration::from_secs(5), self.client.call_tool(request))
            .await
            .expect("tool call timed out")
            .expect_err("tool call fails")
    }

    pub(super) async fn shutdown(self) {
        let _ = tokio::time::timeout(Duration::from_secs(5), self.client.cancel()).await;
        let _ = tokio::time::timeout(Duration::from_secs(5), self.server.waiting()).await;
        let _ = fs::remove_dir_all(self.workspace);
    }
}

pub(super) fn assert_mcp_error(error: ServiceError, code: ErrorCode, message: &str) {
    match error {
        ServiceError::McpError(data) => {
            assert_eq!(data.code, code);
            assert_eq!(data.message.as_ref(), message);
        }
        other => panic!("expected MCP error, got {other:?}"),
    }
}

pub(super) fn node(
    id: u64,
    name: &str,
    node_type: dco3::nodes::models::NodeType,
    parent_id: Option<u64>,
    parent_path: Option<String>,
    file_type: Option<&str>,
    media_type: Option<&str>,
    size: Option<u64>,
    is_encrypted: Option<bool>,
) -> Node {
    Node {
        id,
        reference_id: None,
        node_type,
        name: name.to_string(),
        timestamp_creation: None,
        timestamp_modification: None,
        parent_id,
        parent_path,
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
        is_encrypted,
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

pub(super) fn user_data(id: u64, username: &str) -> UserData {
    UserData {
        id,
        user_name: username.to_string(),
        first_name: "First".to_string(),
        last_name: "Last".to_string(),
        is_locked: false,
        avatar_uuid: "avatar".to_string(),
        auth_data: UserAuthData::new_basic(None, None),
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

pub(super) fn group(id: u64, name: &str, cnt_users: Option<u64>) -> Group {
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

pub(super) fn group_user(id: i64, username: &str) -> GroupUser {
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

fn user_item_from_data(user: UserData) -> UserItem {
    UserItem {
        id: user.id,
        user_name: user.user_name,
        first_name: user.first_name,
        last_name: user.last_name,
        is_locked: user.is_locked,
        avatar_uuid: user.avatar_uuid,
        email: user.email,
        phone: user.phone,
        expire_at: None,
        has_manageable_rooms: user.has_manageable_rooms,
        is_encryption_enabled: user.is_encryption_enabled,
        last_login_success_at: user.last_login_success_at,
        home_room_id: user.home_room_id,
        public_key_container: user.public_key_container,
        user_roles: user.user_roles,
    }
}

fn node_page(items: Vec<Node>, params: Option<ListAllParams>) -> NodeList {
    let total = items.len() as u64;
    let offset = params
        .as_ref()
        .and_then(|params| params.offset)
        .unwrap_or(0);
    let limit = params
        .as_ref()
        .and_then(|params| params.limit)
        .unwrap_or(500);
    let paged = paginate(items, offset, limit);

    RangedItems {
        range: dco3::models::Range {
            offset,
            limit,
            total,
        },
        items: paged,
    }
}

fn user_page(items: Vec<UserItem>, params: Option<ListAllParams>) -> RangedItems<UserItem> {
    let total = items.len() as u64;
    let offset = params
        .as_ref()
        .and_then(|params| params.offset)
        .unwrap_or(0);
    let limit = params
        .as_ref()
        .and_then(|params| params.limit)
        .unwrap_or(500);
    let paged = paginate(items, offset, limit);

    RangedItems {
        range: dco3::models::Range {
            offset,
            limit,
            total,
        },
        items: paged,
    }
}

fn groups_page(items: Vec<Group>, params: Option<ListAllParams>) -> RangedItems<Group> {
    let total = items.len() as u64;
    let offset = params
        .as_ref()
        .and_then(|params| params.offset)
        .unwrap_or(0);
    let limit = params
        .as_ref()
        .and_then(|params| params.limit)
        .unwrap_or(500);
    let paged = paginate(items, offset, limit);

    RangedItems {
        range: dco3::models::Range {
            offset,
            limit,
            total,
        },
        items: paged,
    }
}

fn group_users_page(
    items: Vec<GroupUser>,
    params: Option<ListAllParams>,
) -> RangedItems<GroupUser> {
    let total = items.len() as u64;
    let offset = params
        .as_ref()
        .and_then(|params| params.offset)
        .unwrap_or(0);
    let limit = params
        .as_ref()
        .and_then(|params| params.limit)
        .unwrap_or(500);
    let paged = paginate(items, offset, limit);

    RangedItems {
        range: dco3::models::Range {
            offset,
            limit,
            total,
        },
        items: paged,
    }
}

fn paginate<T: Clone>(items: Vec<T>, offset: u64, limit: u64) -> Vec<T> {
    let start = usize::try_from(offset).expect("offset fits usize");
    let limit = usize::try_from(limit).expect("limit fits usize");
    items.into_iter().skip(start).take(limit).collect()
}

fn remote_node_path(node: &Node) -> Result<String, DcCmdError> {
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

fn unique_temp_dir(prefix: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock drift")
        .as_nanos();
    std::env::temp_dir().join(format!("{prefix}-{}-{nanos}", std::process::id()))
}

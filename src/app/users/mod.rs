pub mod api;
pub mod models;

use dco3::{
    groups::GroupUser,
    nodes::{models::NodeType, RoomGuestUserInvitation},
    user::UserAuthData,
    users::{
        CreateUserRequest, UpdateUserRequest, UserAuthDataUpdateRequest, UserData, UserItem,
        UsersFilter,
    },
    ListAllParams, RangedItems,
};
use futures_util::{stream, StreamExt};
use tracing::{error, info};

use crate::{
    app::users::api::UsersCommandApi,
    core::{
        constants::MAX_CONCURRENT_REQUESTS,
        models::{build_params, DcCmdError, ListOptions},
        utils::strings::{build_node_path, parse_path},
    },
};

use self::models::{AuthMethod, UserImport, UserInfo, UsersSwitchAuthOptions};

pub struct UsersService<C: UsersCommandApi> {
    api: C,
}

#[derive(Debug)]
pub struct SwitchAuthResult {
    pub current_method: String,
    pub new_method: String,
    pub updated_users: usize,
}

#[derive(Debug)]
pub struct EnforceMfaResult {
    pub success_count: usize,
    pub failed_count: usize,
}

#[derive(Debug)]
pub struct ImportUsersResult {
    pub imported: usize,
    pub failed: usize,
    pub total: usize,
}

impl<C: UsersCommandApi> UsersService<C> {
    pub fn new(api: C) -> Self {
        Self { api }
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn create_user(
        &self,
        first_name: &str,
        last_name: &str,
        email: &str,
        login: Option<&str>,
        oidc_id: Option<u32>,
        mfa_enforced: bool,
        first_group_id: Option<u64>,
    ) -> Result<UserData, DcCmdError> {
        let payload = if let (Some(login), Some(oidc_id)) = (login, oidc_id) {
            let user_auth_data = UserAuthData::new_oidc(login, oidc_id.into());
            CreateUserRequest::builder(first_name, last_name)
                .with_auth_data(user_auth_data)
                .with_email(email)
        } else if let (None, Some(oidc_id)) = (login, oidc_id) {
            let user_auth_data = UserAuthData::new_oidc(email, oidc_id.into());
            CreateUserRequest::builder(first_name, last_name)
                .with_auth_data(user_auth_data)
                .with_email(email)
        } else {
            let user_auth_data = UserAuthData::builder(dco3::users::AuthMethod::Basic)
                .with_must_change_password(true)
                .build();

            CreateUserRequest::builder(first_name, last_name)
                .with_auth_data(user_auth_data)
                .with_user_name(login.unwrap_or(email))
                .with_email(email)
                .with_notify_user(true)
        };

        let payload = if mfa_enforced {
            payload.with_mfa_enforced(true).build()
        } else {
            payload.build()
        };

        let user = self.api.create_user(payload).await?;

        if let Some(group_id) = first_group_id {
            let result = self.api.add_group_users(group_id, vec![user.id]).await;
            if let Err(err) = result {
                error!("Failed to add user to group: {}", err);
            }
        }

        Ok(user)
    }

    pub fn read_user_imports_from_csv(&self, source: &str) -> Result<Vec<UserImport>, DcCmdError> {
        let file = std::fs::File::open(source).map_err(|e| {
            error!("Error reading file: {}", e);
            DcCmdError::InvalidArgument(format!("File not found: {source}"))
        })?;

        Self::parse_user_imports_from_reader(file)
    }

    pub async fn import_users<F>(
        &self,
        imports: Vec<UserImport>,
        oidc_id: Option<u32>,
        mut on_processed: F,
    ) -> Result<ImportUsersResult, DcCmdError>
    where
        F: FnMut(bool),
    {
        let total = imports.len();

        let results = stream::iter(imports)
            .map(|import| async move {
                self.create_user(
                    &import.first_name,
                    &import.last_name,
                    &import.email,
                    import.login.as_deref(),
                    oidc_id,
                    import.mfa_enabled.unwrap_or(false),
                    None,
                )
                .await
                .map(|_| ())
            })
            .buffer_unordered(MAX_CONCURRENT_REQUESTS)
            .collect::<Vec<_>>()
            .await;

        let mut imported = 0usize;
        let mut failed = 0usize;

        for result in results {
            match result {
                Ok(_) => {
                    imported += 1;
                    on_processed(true);
                }
                Err(e) => {
                    error!("Failed to import user: {e}");
                    failed += 1;
                    on_processed(false);
                }
            }
        }

        Ok(ImportUsersResult {
            imported,
            failed,
            total,
        })
    }

    pub async fn invite_user(
        &self,
        room_id: u64,
        first_name: &str,
        last_name: &str,
        email: &str,
    ) -> Result<(), DcCmdError> {
        let payload = RoomGuestUserInvitation::new(email, first_name, last_name);
        self.api.invite_guest_users(room_id, vec![payload]).await
    }

    pub async fn resolve_invite_room_id(
        &self,
        target: &str,
        base_url: &str,
    ) -> Result<u64, DcCmdError> {
        let (parent_path, node_name, depth) = parse_path(target, base_url)?;
        let node_path = build_node_path((parent_path.clone(), node_name.clone(), depth));

        let node = self
            .api
            .get_node_from_path(&node_path)
            .await?
            .ok_or_else(|| DcCmdError::InvalidPath(target.to_string()))?;

        let room_id = match node.node_type {
            NodeType::Room => node.id,
            NodeType::Folder => node.auth_parent_id.expect("Folder must have parent room"),
            _ => {
                return Err(DcCmdError::InvalidPath(target.to_string()));
            }
        };

        Ok(room_id)
    }

    pub async fn list_users(
        &self,
        opts: &ListOptions,
    ) -> Result<RangedItems<UserItem>, DcCmdError> {
        let params = build_params(
            opts.filter(),
            opts.offset().unwrap_or(0),
            opts.limit().unwrap_or(500).into(),
        )?;

        let mut users = self.api.get_users(Some(params)).await?;

        if opts.all() && users.range.total > 500 {
            for offset in (500..=users.range.total).step_by(500) {
                let params = build_params(opts.filter(), offset, 500.into())?;
                let next_users = self.api.get_users(Some(params)).await?;
                users.items.extend(next_users.items);
            }
        }

        Ok(users)
    }

    pub async fn delete_user(
        &self,
        user_name: Option<String>,
        user_id: Option<u64>,
    ) -> Result<String, DcCmdError> {
        if let Some(user_name) = user_name {
            let user = self.find_user_by_username(&user_name).await?;
            self.api.delete_user(user.id).await?;
            return Ok(format!("User {user_name} deleted"));
        }

        if let Some(user_id) = user_id {
            self.api.delete_user(user_id).await?;
            return Ok(format!("User {user_id} (id) deleted"));
        }

        Err(DcCmdError::InvalidArgument(
            "User name or user id must be provided".to_string(),
        ))
    }

    pub async fn get_user_info(
        &self,
        user_name: Option<String>,
        user_id: Option<u64>,
    ) -> Result<UserInfo, DcCmdError> {
        if let Some(user_name) = user_name {
            return self.find_user_by_username(&user_name).await?.try_into();
        }

        if let Some(user_id) = user_id {
            return self.api.get_user(user_id).await?.try_into();
        }

        Err(DcCmdError::InvalidArgument(
            "User name or user id must be provided".to_string(),
        ))
    }

    pub async fn find_user_by_username(&self, user_name: &str) -> Result<UserItem, DcCmdError> {
        let user_filter = UsersFilter::username_equals(user_name);
        let params = ListAllParams::builder().with_filter(user_filter).build();

        let users = self.api.get_users(Some(params)).await?;

        let Some(user) = users.items.into_iter().find(|u| u.user_name == user_name) else {
            error!("No user found with username: {user_name}");
            return Err(DcCmdError::InvalidArgument(format!(
                "No user found with username: {user_name}"
            )));
        };

        Ok(user)
    }

    pub async fn switch_auth(
        &self,
        opts: UsersSwitchAuthOptions,
    ) -> Result<SwitchAuthResult, DcCmdError> {
        let curr_method = AuthMethod::try_from(opts.curr_method().to_string())?;
        let new_method = AuthMethod::try_from(opts.new_method().to_string())?;

        let user_ids: Vec<u64> = self
            .list_users(&ListOptions::new(opts.filter(), None, None, true, false))
            .await?
            .items
            .iter()
            .map(|u| u.id)
            .collect();

        let current_user_infos = stream::iter(user_ids)
            .map(|id| self.api.get_user(id))
            .buffer_unordered(5)
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .filter_map(|result| result.ok())
            .filter(|u| {
                let matches_auth = u.auth_data.method == String::from(&curr_method);
                if curr_method == AuthMethod::Oidc {
                    matches_auth && u.auth_data.oid_config_id == opts.curr_oidc_id()
                } else if curr_method == AuthMethod::Ad {
                    matches_auth && u.auth_data.ad_config_id == opts.curr_ad_id()
                } else {
                    matches_auth
                }
            })
            .map(|u| {
                let login = opts.transform_login()(&u);
                (u.id, login)
            })
            .collect::<Vec<_>>();

        info!(
            "Switching auth method from {} to {}",
            curr_method, new_method
        );
        info!("Affected users: {}", current_user_infos.len());

        let update_results = stream::iter(current_user_infos)
            .map(|(id, login)| {
                let auth_method = match &new_method {
                    AuthMethod::Local => dco3::users::AuthMethod::new_basic(),
                    AuthMethod::Oidc => dco3::users::AuthMethod::new_open_id_connect(
                        opts.new_oidc_id().expect("validated by options"),
                        login.clone(),
                    ),
                    AuthMethod::Ad => dco3::users::AuthMethod::new_active_directory(
                        opts.new_ad_id().expect("validated by options"),
                        login.clone(),
                    ),
                };

                let auth_update_req = UserAuthDataUpdateRequest::auth_method(auth_method);
                let user_update_req = UpdateUserRequest::builder().with_auth_data(auth_update_req);
                let user_update_req = if new_method == AuthMethod::Local {
                    user_update_req.with_user_name(login).build()
                } else {
                    user_update_req.build()
                };

                self.api.update_user(id, user_update_req)
            })
            .buffer_unordered(5)
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .inspect(|r| {
                if let Err(err) = r {
                    error!("Failed to update user: {}", err);
                }
            })
            .filter_map(|r| r.ok())
            .collect::<Vec<_>>();

        let updated_users = update_results.len();
        info!("Updated users: {}", updated_users);

        Ok(SwitchAuthResult {
            current_method: curr_method.to_string(),
            new_method: new_method.to_string(),
            updated_users,
        })
    }

    fn parse_user_imports_from_reader<R: std::io::Read>(
        reader: R,
    ) -> Result<Vec<UserImport>, DcCmdError> {
        let mut reader = csv::ReaderBuilder::new()
            .flexible(true)
            .trim(csv::Trim::All)
            .from_reader(reader);

        reader
            .deserialize::<UserImport>()
            .collect::<Result<Vec<_>, csv::Error>>()
            .map_err(|e| {
                error!("Error reading record: {e}");
                DcCmdError::InvalidArgument(format!(
                    "Invalid CSV format. Expected fields: first_name, last_name, email, login (optional), mfa_enabled (optional).\n{e})"
                ))
            })
    }

    pub async fn enforce_mfa(
        &self,
        auth_method: Option<String>,
        filter: Option<String>,
        auth_method_id: Option<u64>,
        group_id: Option<u64>,
    ) -> Result<EnforceMfaResult, DcCmdError> {
        let auth_method = auth_method.map(AuthMethod::try_from).transpose()?;

        if let Some(ref auth_method) = auth_method {
            if *auth_method != AuthMethod::Local && auth_method_id.is_none() {
                return Err(DcCmdError::InvalidArgument(
                    "Auth method id must be provided for non-local auth methods.".to_string(),
                ));
            }
        }

        if auth_method.is_none() && filter.is_none() && group_id.is_none() {
            return Err(DcCmdError::InvalidArgument(
                "Either auth method, group or filter must be provided. This would enforce MFA for all users and can be achieved via system settings.".to_string(),
            ));
        }

        let user_ids = if let Some(group_id) = group_id {
            self.list_group_user_ids(group_id, filter.clone()).await?
        } else {
            self.list_users(&ListOptions::new(filter, None, None, true, false))
                .await?
                .items
                .iter()
                .map(|u| u.id)
                .collect::<Vec<_>>()
        };

        let user_ids = if auth_method.is_some() {
            stream::iter(user_ids)
                .map(|id| self.api.get_user(id))
                .buffer_unordered(5)
                .collect::<Vec<_>>()
                .await
                .into_iter()
                .filter_map(|r| {
                    if let Err(e) = r {
                        error!("Failed to get user. Error: {}", e);
                        return None;
                    }
                    r.ok()
                })
                .filter(|u| {
                    if let Some(auth_method) = &auth_method {
                        let same_auth = u.auth_data.method == String::from(auth_method);
                        let same_auth_id = match auth_method {
                            AuthMethod::Oidc => u.auth_data.oid_config_id == auth_method_id,
                            AuthMethod::Ad => u.auth_data.ad_config_id == auth_method_id,
                            _ => true,
                        };
                        same_auth && same_auth_id
                    } else {
                        true
                    }
                })
                .map(|u| u.id)
                .collect::<Vec<_>>()
        } else {
            user_ids
        };

        info!("Enforcing MFA for {} users", user_ids.len());

        let update_results = stream::iter(user_ids)
            .map(|id| {
                let update_user_req = UpdateUserRequest::builder().with_mfa_enforced(true).build();
                self.api.update_user(id, update_user_req)
            })
            .buffer_unordered(5)
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .map(|r| match r {
                Ok(_) => Ok(()),
                Err(e) => {
                    error!("Failed to enforce MFA for user. Error: {}", e);
                    Err(e)
                }
            })
            .collect::<Vec<_>>();

        let success_count = update_results.iter().filter(|r| r.is_ok()).count();
        let failed_count = update_results.iter().filter(|r| r.is_err()).count();

        info!(
            "Enforced MFA for {} users successfully. Failed for {} users",
            success_count, failed_count
        );

        Ok(EnforceMfaResult {
            success_count,
            failed_count,
        })
    }

    async fn list_group_user_ids(
        &self,
        group_id: u64,
        filter: Option<String>,
    ) -> Result<Vec<u64>, DcCmdError> {
        let params = build_params(&filter, 0, None)?;
        let mut users = self.api.get_group_users(group_id, Some(params)).await?;

        if users.range.total > 500 {
            for offset in (500..=users.range.total).step_by(500) {
                let params = build_params(&filter, offset, None)?;
                let next_users = self.api.get_group_users(group_id, Some(params)).await?;
                users.items.extend(next_users.items);
            }
        }

        Ok(users
            .items
            .iter()
            .filter_map(group_user_id_as_u64)
            .collect::<Vec<_>>())
    }
}

fn group_user_id_as_u64(user: &GroupUser) -> Option<u64> {
    user.user_info.id.try_into().ok()
}

#[cfg(test)]
mod tests {
    use std::{
        collections::{HashMap, HashSet, VecDeque},
        io::Cursor,
        sync::{Arc, Mutex},
    };

    use async_trait::async_trait;
    use dco3::{
        groups::GroupUser,
        nodes::{
            models::{NodeType, UserInfo as NodeUserInfo, UserType},
            Node, RoomGuestUserInvitation,
        },
        users::{CreateUserRequest, UpdateUserRequest, UserData, UserItem},
        ListAllParams, RangedItems,
    };

    use crate::{
        app::users::api::UsersCommandApi,
        core::models::{DcCmdError, ListOptions},
    };

    use super::{
        models::{UserImport, UsersSwitchAuthOptions},
        UsersService,
    };

    struct MockUsersApi {
        create_user_results: Mutex<VecDeque<Result<UserData, DcCmdError>>>,
        users_pages: Mutex<VecDeque<RangedItems<UserItem>>>,
        group_user_pages: Mutex<VecDeque<RangedItems<GroupUser>>>,
        users_by_id: HashMap<u64, UserData>,
        deleted_users: Mutex<Vec<u64>>,
        updated_users: Mutex<Vec<u64>>,
        failed_updates: HashSet<u64>,
        invite_calls: Mutex<Vec<(u64, usize)>>,
        nodes_by_path: HashMap<String, Node>,
    }

    impl MockUsersApi {
        fn new(
            pages: Vec<RangedItems<UserItem>>,
            nodes_by_path: HashMap<String, Node>,
            users_by_id: HashMap<u64, UserData>,
        ) -> Self {
            Self {
                create_user_results: Mutex::new(VecDeque::new()),
                users_pages: Mutex::new(VecDeque::from(pages)),
                group_user_pages: Mutex::new(VecDeque::new()),
                users_by_id,
                deleted_users: Mutex::new(Vec::new()),
                updated_users: Mutex::new(Vec::new()),
                failed_updates: HashSet::new(),
                invite_calls: Mutex::new(Vec::new()),
                nodes_by_path,
            }
        }

        fn with_group_user_pages(self, pages: Vec<RangedItems<GroupUser>>) -> Self {
            *self.group_user_pages.lock().expect("lock poisoned") = VecDeque::from(pages);
            self
        }

        fn with_failed_updates(mut self, failed_updates: HashSet<u64>) -> Self {
            self.failed_updates = failed_updates;
            self
        }

        fn with_create_user_results(self, results: Vec<Result<UserData, DcCmdError>>) -> Self {
            *self.create_user_results.lock().expect("lock poisoned") = VecDeque::from(results);
            self
        }
    }

    #[async_trait]
    impl UsersCommandApi for Arc<MockUsersApi> {
        async fn create_user(
            &self,
            _req: CreateUserRequest,
        ) -> Result<UserData, crate::core::models::DcCmdError> {
            let mut results = self.create_user_results.lock().expect("lock poisoned");
            if let Some(result) = results.pop_front() {
                return result;
            }
            Ok(user_data_basic(99, "created.user"))
        }

        async fn get_users(
            &self,
            _params: Option<ListAllParams>,
        ) -> Result<RangedItems<UserItem>, crate::core::models::DcCmdError> {
            let mut users_pages = self.users_pages.lock().expect("lock poisoned");
            if users_pages.len() > 1 {
                Ok(users_pages.pop_front().expect("at least one page"))
            } else {
                Ok(users_pages.front().cloned().unwrap_or_else(empty_users))
            }
        }

        async fn delete_user(&self, user_id: u64) -> Result<(), crate::core::models::DcCmdError> {
            self.deleted_users
                .lock()
                .expect("lock poisoned")
                .push(user_id);
            Ok(())
        }

        async fn get_user(
            &self,
            user_id: u64,
        ) -> Result<UserData, crate::core::models::DcCmdError> {
            Ok(self
                .users_by_id
                .get(&user_id)
                .cloned()
                .unwrap_or_else(|| user_data_basic(user_id, &format!("user-{user_id}"))))
        }

        async fn update_user(
            &self,
            user_id: u64,
            _req: UpdateUserRequest,
        ) -> Result<UserData, crate::core::models::DcCmdError> {
            self.updated_users
                .lock()
                .expect("lock poisoned")
                .push(user_id);

            if self.failed_updates.contains(&user_id) {
                return Err(DcCmdError::InvalidArgument("update failed".to_string()));
            }

            Ok(self
                .users_by_id
                .get(&user_id)
                .cloned()
                .unwrap_or_else(|| user_data_basic(user_id, &format!("user-{user_id}"))))
        }

        async fn add_group_users(
            &self,
            _group_id: u64,
            _user_ids: Vec<u64>,
        ) -> Result<(), crate::core::models::DcCmdError> {
            Ok(())
        }

        async fn get_group_users(
            &self,
            _group_id: u64,
            _params: Option<ListAllParams>,
        ) -> Result<RangedItems<GroupUser>, crate::core::models::DcCmdError> {
            let mut group_user_pages = self.group_user_pages.lock().expect("lock poisoned");
            if group_user_pages.len() > 1 {
                Ok(group_user_pages.pop_front().expect("at least one page"))
            } else {
                Ok(group_user_pages
                    .front()
                    .cloned()
                    .unwrap_or_else(empty_group_users))
            }
        }

        async fn get_node_from_path(
            &self,
            path: &str,
        ) -> Result<Option<Node>, crate::core::models::DcCmdError> {
            Ok(self.nodes_by_path.get(path).cloned())
        }

        async fn invite_guest_users(
            &self,
            room_id: u64,
            users: Vec<RoomGuestUserInvitation>,
        ) -> Result<(), crate::core::models::DcCmdError> {
            self.invite_calls
                .lock()
                .expect("lock poisoned")
                .push((room_id, users.len()));
            Ok(())
        }
    }

    fn users_page(items: Vec<UserItem>, total: u64) -> RangedItems<UserItem> {
        RangedItems {
            range: dco3::models::Range {
                offset: 0,
                limit: 500,
                total,
            },
            items,
        }
    }

    fn empty_users() -> RangedItems<UserItem> {
        users_page(Vec::new(), 0)
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

    fn empty_group_users() -> RangedItems<GroupUser> {
        group_users_page(Vec::new(), 0)
    }

    fn group_user(id: i64, user_name: &str) -> GroupUser {
        GroupUser {
            user_info: NodeUserInfo {
                id,
                user_type: UserType::Internal,
                user_name: Some(user_name.to_string()),
                first_name: None,
                last_name: None,
                email: None,
                avatar_uuid: "avatar".to_string(),
            },
            is_member: true,
        }
    }

    fn user_item(id: u64, user_name: &str) -> UserItem {
        UserItem {
            id,
            user_name: user_name.to_string(),
            first_name: "First".to_string(),
            last_name: "Last".to_string(),
            is_locked: false,
            avatar_uuid: "avatar".to_string(),
            email: Some(format!("{user_name}@example.com")),
            phone: None,
            expire_at: None,
            has_manageable_rooms: None,
            is_encryption_enabled: None,
            last_login_success_at: None,
            home_room_id: None,
            public_key_container: None,
            user_roles: None,
        }
    }

    fn user_data_basic(id: u64, user_name: &str) -> UserData {
        UserData {
            id,
            user_name: user_name.to_string(),
            first_name: "First".to_string(),
            last_name: "Last".to_string(),
            is_locked: false,
            avatar_uuid: "avatar".to_string(),
            auth_data: dco3::user::UserAuthData::new_basic(None, None),
            email: Some(format!("{user_name}@example.com")),
            phone: None,
            expire_at: None,
            has_manageable_rooms: None,
            is_encryption_enabled: None,
            last_login_success_at: None,
            home_room_id: None,
            public_key_container: None,
            user_roles: None,
            is_mfa_enabled: None,
            is_mfa_enforced: None,
        }
    }

    fn user_data_oidc(id: u64, user_name: &str, oidc_id: u64) -> UserData {
        let mut user = user_data_basic(id, user_name);
        user.auth_data = dco3::user::UserAuthData::new_oidc(user_name, oidc_id);
        user
    }

    fn user_data_ad(id: u64, user_name: &str, ad_id: u64) -> UserData {
        let mut user = user_data_basic(id, user_name);
        user.auth_data = dco3::user::UserAuthData::new_ad(user_name, ad_id);
        user
    }

    fn node(id: u64, node_type: NodeType, auth_parent_id: Option<u64>) -> Node {
        Node {
            id,
            reference_id: None,
            node_type,
            name: format!("node-{id}"),
            timestamp_creation: None,
            timestamp_modification: None,
            parent_id: None,
            parent_path: None,
            created_at: None,
            created_by: None,
            updated_at: None,
            updated_by: None,
            expire_at: None,
            hash: None,
            file_type: None,
            media_type: None,
            size: None,
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
            auth_parent_id,
        }
    }

    #[tokio::test]
    async fn test_list_users_collects_all_pages() {
        let users_by_id = HashMap::new();
        let mock = Arc::new(MockUsersApi::new(
            vec![
                users_page(vec![user_item(1, "u1"), user_item(2, "u2")], 501),
                users_page(vec![user_item(3, "u3")], 501),
            ],
            HashMap::new(),
            users_by_id,
        ));
        let service = UsersService::new(mock);

        let users = service
            .list_users(&ListOptions::new(None, None, None, true, false))
            .await
            .expect("users");

        assert_eq!(users.items.len(), 3);
    }

    #[tokio::test]
    async fn test_delete_user_by_name_resolves_id_and_deletes() {
        let users_by_id = HashMap::new();
        let mock = Arc::new(MockUsersApi::new(
            vec![users_page(vec![user_item(7, "alice")], 1)],
            HashMap::new(),
            users_by_id,
        ));
        let service = UsersService::new(mock.clone());

        let message = service
            .delete_user(Some("alice".to_string()), None)
            .await
            .expect("delete result");

        assert_eq!(message, "User alice deleted");
        assert_eq!(
            mock.deleted_users.lock().expect("lock poisoned").as_slice(),
            [7]
        );
    }

    #[tokio::test]
    async fn test_resolve_invite_room_id_for_room() {
        let mut nodes = HashMap::new();
        nodes.insert("/teams/".to_string(), node(12, NodeType::Room, None));
        let mock = Arc::new(MockUsersApi::new(
            vec![empty_users()],
            nodes,
            HashMap::new(),
        ));
        let service = UsersService::new(mock);

        let room_id = service
            .resolve_invite_room_id("https://example.com/teams/", "https://example.com")
            .await
            .expect("room id");

        assert_eq!(room_id, 12);
    }

    #[tokio::test]
    async fn test_resolve_invite_room_id_for_folder() {
        let mut nodes = HashMap::new();
        nodes.insert(
            "/teams/sub/".to_string(),
            node(99, NodeType::Folder, Some(42)),
        );
        let mock = Arc::new(MockUsersApi::new(
            vec![empty_users()],
            nodes,
            HashMap::new(),
        ));
        let service = UsersService::new(mock);

        let room_id = service
            .resolve_invite_room_id("https://example.com/teams/sub/", "https://example.com")
            .await
            .expect("room id");

        assert_eq!(room_id, 42);
    }

    #[tokio::test]
    async fn test_invite_user_delegates_to_api() {
        let mock = Arc::new(MockUsersApi::new(
            vec![empty_users()],
            HashMap::new(),
            HashMap::new(),
        ));
        let service = UsersService::new(mock.clone());

        service
            .invite_user(88, "A", "B", "a.b@example.com")
            .await
            .expect("invite");

        assert_eq!(
            mock.invite_calls.lock().expect("lock poisoned").as_slice(),
            [(88, 1)]
        );
    }

    #[tokio::test]
    async fn test_delete_user_requires_identifier() {
        let mock = Arc::new(MockUsersApi::new(
            vec![empty_users()],
            HashMap::new(),
            HashMap::new(),
        ));
        let service = UsersService::new(mock);

        let err = service
            .delete_user(None, None)
            .await
            .expect_err("expected error");

        assert_eq!(
            err,
            DcCmdError::InvalidArgument("User name or user id must be provided".to_string())
        );
    }

    #[tokio::test]
    async fn test_read_user_imports_from_csv_parses_rows() {
        let imports =
            UsersService::<Arc<MockUsersApi>>::parse_user_imports_from_reader(Cursor::new(
            "first_name,last_name,email,login,mfa_enabled\nAlice,Doe,alice@example.com,alice,true\nBob,Smith,bob@example.com,,false\n",
            ))
            .expect("imports");

        assert_eq!(imports.len(), 2);
        assert_eq!(imports[0].first_name, "Alice");
        assert_eq!(imports[0].login.as_deref(), Some("alice"));
        assert_eq!(imports[1].first_name, "Bob");
        assert_eq!(imports[1].login, None);
    }

    #[tokio::test]
    async fn test_import_users_counts_success_and_failures() {
        let mock = Arc::new(
            MockUsersApi::new(vec![empty_users()], HashMap::new(), HashMap::new())
                .with_create_user_results(vec![
                    Ok(user_data_basic(1, "u1")),
                    Err(DcCmdError::InvalidArgument("create failed".to_string())),
                    Ok(user_data_basic(2, "u2")),
                ]),
        );
        let service = UsersService::new(mock);

        let imports = vec![
            UserImport {
                first_name: "A".to_string(),
                last_name: "One".to_string(),
                email: "a.one@example.com".to_string(),
                login: Some("a.one".to_string()),
                mfa_enabled: Some(true),
            },
            UserImport {
                first_name: "B".to_string(),
                last_name: "Two".to_string(),
                email: "b.two@example.com".to_string(),
                login: None,
                mfa_enabled: Some(false),
            },
            UserImport {
                first_name: "C".to_string(),
                last_name: "Three".to_string(),
                email: "c.three@example.com".to_string(),
                login: None,
                mfa_enabled: None,
            },
        ];

        let mut processed = 0usize;
        let result = service
            .import_users(imports, None, |_| processed += 1)
            .await
            .expect("import result");

        assert_eq!(result.imported, 2);
        assert_eq!(result.failed, 1);
        assert_eq!(result.total, 3);
        assert_eq!(processed, 3);
    }

    #[tokio::test]
    async fn test_switch_auth_updates_only_matching_users() {
        let mut users_by_id = HashMap::new();
        users_by_id.insert(1, user_data_oidc(1, "alice", 7));
        users_by_id.insert(2, user_data_oidc(2, "bob", 7));
        users_by_id.insert(3, user_data_basic(3, "carol"));

        let mock = Arc::new(MockUsersApi::new(
            vec![users_page(
                vec![
                    user_item(1, "alice"),
                    user_item(2, "bob"),
                    user_item(3, "carol"),
                ],
                3,
            )],
            HashMap::new(),
            users_by_id,
        ));
        let service = UsersService::new(mock.clone());

        let opts = UsersSwitchAuthOptions::try_new(
            "oidc".to_string(),
            "local".to_string(),
            Some(7),
            None,
            None,
            None,
            None,
            Some("username".to_string()),
        )
        .expect("valid switch opts");

        let result = service.switch_auth(opts).await.expect("switch auth");

        assert_eq!(result.updated_users, 2);
        assert_eq!(result.current_method, "openid");
        assert_eq!(result.new_method, "basic");
        assert_eq!(
            mock.updated_users.lock().expect("lock poisoned").as_slice(),
            [1, 2]
        );
    }

    #[tokio::test]
    async fn test_enforce_mfa_with_group_id_and_auth_filter() {
        let mut users_by_id = HashMap::new();
        users_by_id.insert(11, user_data_ad(11, "u11", 9));
        users_by_id.insert(12, user_data_ad(12, "u12", 9));
        users_by_id.insert(13, user_data_basic(13, "u13"));

        let mock = Arc::new(
            MockUsersApi::new(vec![empty_users()], HashMap::new(), users_by_id)
                .with_group_user_pages(vec![group_users_page(
                    vec![
                        group_user(11, "u11"),
                        group_user(12, "u12"),
                        group_user(13, "u13"),
                    ],
                    3,
                )]),
        );
        let service = UsersService::new(mock.clone());

        let result = service
            .enforce_mfa(Some("ad".to_string()), None, Some(9), Some(777))
            .await
            .expect("enforce mfa");

        assert_eq!(result.success_count, 2);
        assert_eq!(result.failed_count, 0);
        assert_eq!(
            mock.updated_users.lock().expect("lock poisoned").as_slice(),
            [11, 12]
        );
    }

    #[tokio::test]
    async fn test_enforce_mfa_counts_failures() {
        let mut users_by_id = HashMap::new();
        users_by_id.insert(1, user_data_basic(1, "u1"));
        users_by_id.insert(2, user_data_basic(2, "u2"));

        let mut failed = HashSet::new();
        failed.insert(2);

        let mock = Arc::new(
            MockUsersApi::new(
                vec![users_page(vec![user_item(1, "u1"), user_item(2, "u2")], 2)],
                HashMap::new(),
                users_by_id,
            )
            .with_failed_updates(failed),
        );
        let service = UsersService::new(mock);

        let result = service
            .enforce_mfa(None, Some("userName:cn:u".to_string()), None, None)
            .await
            .expect("enforce mfa");

        assert_eq!(result.success_count, 1);
        assert_eq!(result.failed_count, 1);
    }

    #[tokio::test]
    async fn test_enforce_mfa_requires_selector() {
        let mock = Arc::new(MockUsersApi::new(
            vec![empty_users()],
            HashMap::new(),
            HashMap::new(),
        ));
        let service = UsersService::new(mock);

        let err = service
            .enforce_mfa(None, None, None, None)
            .await
            .expect_err("expected error");

        assert_eq!(
            err,
            DcCmdError::InvalidArgument(
                "Either auth method, group or filter must be provided. This would enforce MFA for all users and can be achieved via system settings.".to_string()
            )
        );
    }
}

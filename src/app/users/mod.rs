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
mod tests;

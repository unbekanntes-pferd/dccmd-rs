use dco3::{
    users::{UpdateUserRequest, UserAuthDataUpdateRequest},
    Users,
};
use futures_util::{stream, StreamExt};
use tracing::{error, info};

use crate::cmd::{
    models::{DcCmdError, ListOptions},
    utils::strings::format_success_message,
};

use super::{models::UsersSwitchAuthOptions, UserCommandHandler};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthMethod {
    Local,
    Oidc,
    Ad,
}

impl TryFrom<String> for AuthMethod {
    type Error = DcCmdError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        match value.to_lowercase().as_str() {
            "local" | "basic" => Ok(AuthMethod::Local),
            "oidc" | "openid" => Ok(AuthMethod::Oidc),
            "ad" | "active_directory" | "activedirectory" => Ok(AuthMethod::Ad),
            _ => Err(DcCmdError::InvalidArgument(format!(
                "Invalid auth method: {}",
                value
            ))),
        }
    }
}

impl From<&AuthMethod> for String {
    fn from(value: &AuthMethod) -> Self {
        match value {
            AuthMethod::Local => "basic".to_string(),
            AuthMethod::Oidc => "openid".to_string(),
            AuthMethod::Ad => "active_directory".to_string(),
        }
    }
}

impl std::fmt::Display for AuthMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", String::from(self))
    }
}

impl UserCommandHandler {
    pub async fn switch_auth(&self, opts: UsersSwitchAuthOptions) -> Result<(), DcCmdError> {
        let curr_method = AuthMethod::try_from(opts.curr_method().to_string())?;
        let new_method = AuthMethod::try_from(opts.new_method().to_string())?;

        // get all user ids
        let user_ids: Vec<u64> = self
            .list_users(
                ListOptions::new(opts.filter(), None, None, true, false),
                false,
            )
            .await?
            .items
            .iter()
            .map(|u| u.id)
            .collect();

        let current_user_infos = stream::iter(user_ids)
            .map(|id| self.client.users().get_user(id, None))
            .buffer_unordered(5)
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .filter_map(|r| r.ok())
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
                        opts.new_oidc_id().unwrap(),
                        login.clone(),
                    ),
                    AuthMethod::Ad => dco3::users::AuthMethod::new_active_directory(
                        opts.new_ad_id().unwrap(),
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

                self.client.users().update_user(id, user_update_req)
            })
            .buffer_unordered(5)
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .map(|r| {
                if let Err(err) = &r {
                    error!("Failed to update user: {}", err);
                }
                r
            })
            .filter_map(|r| r.ok())
            .collect::<Vec<_>>();

        let updated_users = update_results.len();

        info!("Updated users: {}", updated_users);

        let msg = format_success_message(&format!(
            "Switched auth method from {curr_method} to {new_method} for {updated_users} users."
        ));

        self.term.write_line(&msg).map_err(|e| {
            error!("Error writing message to terminal: {}", e);
            DcCmdError::IoError
        })?;

        Ok(())
    }
}

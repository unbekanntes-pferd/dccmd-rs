use dco3::{users::UpdateUserRequest, Groups, Users};
use futures_util::{stream, StreamExt};
use tracing::{error, info};

use crate::cmd::{
    models::{build_params, DcCmdError, ListOptions},
    users::auth::AuthMethod,
    utils::strings::format_success_message,
};

use super::UserCommandHandler;

impl UserCommandHandler {
    pub async fn enforce_mfa(
        &self,
        auth_method: Option<String>,
        filter: Option<String>,
        auth_method_id: Option<u64>,
        group_id: Option<u64>,
    ) -> Result<(), DcCmdError> {
        let auth_method = auth_method.map(AuthMethod::try_from).transpose()?;

        // bail if oidc / ad and no auth method id
        if let Some(ref auth_method) = auth_method {
            if *auth_method != AuthMethod::Local && auth_method_id.is_none() {
                return Err(DcCmdError::InvalidArgument(
                    "Auth method id must be provided for non-local auth methods.".to_string(),
                ));
            }
        }

        // if no auth method or filter is provided, return error
        if auth_method.is_none() && filter.is_none() && group_id.is_none() {
            return Err(DcCmdError::InvalidArgument(
                "Either auth method, group or filter must be provided. This would enforce MFA for all users and can be achieved via system settings.".to_string(),
            ));
        }

        // get all user ids
        let user_ids: Vec<u64> = if let Some(group_id) = group_id {
            let params = build_params(&filter, 0, None)?;
            let mut users = self
                .client
                .groups()
                .get_group_users(group_id, Some(params))
                .await?;
            if users.range.total > 500 {
                for offset in (500..=users.range.total).step_by(500) {
                    let params = build_params(&filter, offset, None)?;
                    let new_users = self
                        .client
                        .groups()
                        .get_group_users(group_id, Some(params))
                        .await?;
                    users.items.extend(new_users.items);
                }
            }
            users
                .items
                .iter()
                .filter_map(|u| u.user_info.id.try_into().ok())
                .collect()
        } else {
            self.list_users(ListOptions::new(filter, None, None, true, false), false)
                .await?
                .items
                .iter()
                .map(|u| u.id)
                .collect()
        };

        // filter out users if auth method is provided
        let user_ids = if auth_method.is_some() {
            stream::iter(user_ids)
                .map(|id| self.client.users().get_user(id, None))
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
                self.client.users().update_user(id, update_user_req)
            })
            .buffer_unordered(5)
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .map(|r| match r {
                Ok(_) => Ok(()),
                Err(e) => {
                    error!("Failed to enforce MFA for user. Error: {}", e);
                    Err(DcCmdError::from(e))
                }
            })
            .collect::<Vec<_>>();

        let success_count = update_results.iter().filter(|r| r.is_ok()).count();
        let failed_count = update_results.iter().filter(|r| r.is_err()).count();

        info!(
            "Enforced MFA for {} users successfully. Failed for {} users",
            success_count, failed_count
        );

        self.term
            .write_line(&format_success_message(&format!(
                "Enforced MFA for {} users successfully.",
                success_count
            )))
            .map_err(|_| DcCmdError::IoError)?;

        Ok(())
    }
}

use crate::{
    app::{
        auth::AuthService,
        results::UsersPlatformResult,
        users::{models::UsersSwitchAuthOptions, UsersService},
    },
    command::UsersCommand,
    core::models::{DcCmdError, ListOptions, PasswordAuth},
};

use super::CliPlatform;

impl CliPlatform {
    pub(super) async fn execute_users_cmd(
        &self,
        cmd: UsersCommand,
        auth: Option<PasswordAuth>,
    ) -> Result<UsersPlatformResult, DcCmdError> {
        let target = Self::users_target(&cmd);
        let is_import = matches!(cmd, UsersCommand::Import { .. });
        let client = AuthService::new()
            .connect_client(target, auth, is_import)
            .await?;
        let base_url = client.get_base_url().to_string();
        let service = UsersService::new(client);

        match cmd {
            UsersCommand::Create {
                target: _,
                first_name,
                last_name,
                email,
                login,
                oidc_id,
                mfa_enforced,
                group_id,
            } => {
                let user = service
                    .create_user(
                        &first_name,
                        &last_name,
                        &email,
                        login.as_deref(),
                        oidc_id,
                        mfa_enforced,
                        group_id,
                    )
                    .await?;

                Ok(UsersPlatformResult::Created {
                    user_name: user.user_name,
                    user_id: user.id,
                    auth_method: user.auth_data.method,
                })
            }
            UsersCommand::Invite {
                target,
                first_name,
                last_name,
                email,
            } => {
                let room_id = service.resolve_invite_room_id(&target, &base_url).await?;
                service
                    .invite_user(room_id, &first_name, &last_name, &email)
                    .await?;

                Ok(UsersPlatformResult::Invited {
                    first_name,
                    last_name,
                })
            }
            UsersCommand::Ls {
                target: _,
                filter,
                offset,
                limit,
                all,
                csv,
            } => {
                let users = service
                    .list_users(&ListOptions::new(filter, offset, limit, all, csv))
                    .await?;
                Ok(UsersPlatformResult::Listed { users, csv })
            }
            UsersCommand::Rm {
                target: _,
                user_name,
                user_id,
            } => {
                let message = service.delete_user(user_name, user_id).await?;
                Ok(UsersPlatformResult::Removed { message })
            }
            UsersCommand::Import {
                target: _,
                source,
                oidc_id,
            } => {
                let imports = service.read_user_imports_from_csv(&source)?;
                let result = service.import_users(imports, oidc_id, |_| {}).await?;
                Ok(UsersPlatformResult::Imported(result))
            }
            UsersCommand::Info {
                target: _,
                user_name,
                user_id,
            } => {
                let user = service.get_user_info(user_name, user_id).await?;
                Ok(UsersPlatformResult::Info { user })
            }
            UsersCommand::SwitchAuth {
                target: _,
                current_method,
                new_method,
                current_oidc_id,
                new_oidc_id,
                current_ad_id,
                new_ad_id,
                filter,
                login,
            } => {
                let opts = UsersSwitchAuthOptions::try_new(
                    current_method,
                    new_method,
                    current_oidc_id,
                    new_oidc_id,
                    current_ad_id,
                    new_ad_id,
                    filter,
                    login,
                )?;
                let result = service.switch_auth(opts).await?;
                Ok(UsersPlatformResult::SwitchedAuth(result))
            }
            UsersCommand::EnforceMfa {
                target: _,
                auth_method,
                filter,
                auth_method_id,
                group_id,
            } => {
                let result = service
                    .enforce_mfa(auth_method, filter, auth_method_id, group_id)
                    .await?;
                Ok(UsersPlatformResult::EnforcedMfa(result))
            }
        }
    }

    fn users_target(cmd: &UsersCommand) -> &str {
        match cmd {
            UsersCommand::Create { target, .. }
            | UsersCommand::Ls { target, .. }
            | UsersCommand::Rm { target, .. }
            | UsersCommand::Import { target, .. }
            | UsersCommand::Info { target, .. }
            | UsersCommand::SwitchAuth { target, .. }
            | UsersCommand::EnforceMfa { target, .. }
            | UsersCommand::Invite { target, .. } => target.as_str(),
        }
    }
}

use crate::{
    app::{
        outcome::CommandOutcome,
        requests::UsersRequest,
        results::{AppResult, UsersPlatformResult},
        App, Platform, Ui,
    },
    core::models::DcCmdError,
};

impl<P: Platform, U: Ui> App<P, U> {
    pub(in crate::app) async fn handle_users(
        &self,
        cmd: UsersRequest,
    ) -> Result<AppResult, DcCmdError> {
        let mut outcome = CommandOutcome::default();
        match self.platform.users(cmd).await? {
            UsersPlatformResult::Created {
                user_name,
                user_id,
                auth_method,
            } => {
                self.write_success(&mut outcome, &format!("User {user_name} created"))?;
                self.write_info(&mut outcome, &format!("► user id: {user_id}"))?;
                self.write_info(&mut outcome, &format!("► user login: {user_name}"))?;
                self.write_info(&mut outcome, &format!("► auth method: {auth_method}"))?;
            }
            UsersPlatformResult::Invited {
                first_name,
                last_name,
            } => {
                self.write_success(
                    &mut outcome,
                    &format!("User {first_name} {last_name} invited"),
                )?;
            }
            UsersPlatformResult::Listed { users, csv } => {
                self.ui.print_users(&users, csv)?;
            }
            UsersPlatformResult::Removed { message } => {
                self.write_success(&mut outcome, &message)?;
            }
            UsersPlatformResult::Imported(result) => {
                self.write_success(&mut outcome, &format!("{} users imported", result.imported))?;
                self.write_info(
                    &mut outcome,
                    &format!(
                        "Import summary: {} total, {} failed.",
                        result.total, result.failed
                    ),
                )?;
            }
            UsersPlatformResult::Info { user } => {
                self.ui.print_user_info(user)?;
            }
            UsersPlatformResult::SwitchedAuth(result) => {
                self.write_success(
                    &mut outcome,
                    &format!(
                        "Switched auth method from {} to {} for {} users.",
                        result.current_method, result.new_method, result.updated_users
                    ),
                )?;
            }
            UsersPlatformResult::EnforcedMfa(result) => {
                self.write_success(
                    &mut outcome,
                    &format!(
                        "Enforced MFA for {} users successfully.",
                        result.success_count
                    ),
                )?;
                self.write_info(
                    &mut outcome,
                    &format!("Failed MFA updates: {}", result.failed_count),
                )?;
            }
        }

        Ok(AppResult::success(outcome))
    }
}

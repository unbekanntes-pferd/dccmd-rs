use crate::{
    app::{
        outcome::CommandOutcome,
        requests::GroupsRequest,
        results::{AppResult, GroupsPlatformResult},
        App, Platform, Ui,
    },
    core::models::{DcCmdError, PasswordAuth},
};

impl<P: Platform, U: Ui> App<P, U> {
    pub(in crate::app) async fn handle_groups(
        &self,
        cmd: GroupsRequest,
        auth: Option<PasswordAuth>,
    ) -> Result<AppResult, DcCmdError> {
        let mut outcome = CommandOutcome::default();
        match self.platform.groups(cmd, auth).await? {
            GroupsPlatformResult::Created {
                group_name,
                group_id,
            } => {
                self.write_success(
                    &mut outcome,
                    &format!("Group {group_name} ({group_id}) created"),
                )?;
            }
            GroupsPlatformResult::Listed { groups, csv } => {
                self.ui.print_groups(groups, csv)?;
            }
            GroupsPlatformResult::Removed { group_id } => {
                self.write_success(&mut outcome, &format!("Group {group_id} deleted"))?;
            }
            GroupsPlatformResult::UsersListed { pages, csv } => {
                self.ui.print_group_users(&pages, csv)?;
            }
            GroupsPlatformResult::UserAdded { group_id, user_id } => {
                self.write_success(
                    &mut outcome,
                    &format!("User {user_id} added to group {group_id}"),
                )?;
            }
        }
        Ok(AppResult::success(outcome))
    }
}

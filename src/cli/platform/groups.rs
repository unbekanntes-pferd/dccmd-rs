use crate::{
    app::requests::{GroupsRequest, GroupsUsersRequest},
    app::{auth::AuthService, groups::GroupsService, results::GroupsPlatformResult},
    core::models::{DcCmdError, ListOptions, PasswordAuth},
};

use super::CliPlatform;

impl CliPlatform {
    pub(super) async fn execute_groups_cmd(
        &self,
        cmd: GroupsRequest,
        auth: Option<PasswordAuth>,
    ) -> Result<GroupsPlatformResult, DcCmdError> {
        let client = AuthService::new()
            .connect_client(Self::groups_target(&cmd), auth, false)
            .await?;
        let service = GroupsService::new(client);

        match cmd {
            GroupsRequest::Create { target: _, name } => {
                let group = service.create_group(&name).await?;
                Ok(GroupsPlatformResult::Created {
                    group_name: group.name,
                    group_id: group.id,
                })
            }
            GroupsRequest::Ls {
                target: _,
                filter,
                offset,
                limit,
                all,
                csv,
            } => {
                let groups = service
                    .list_groups(&ListOptions::new(filter, offset, limit, all, csv))
                    .await?;
                Ok(GroupsPlatformResult::Listed { groups, csv })
            }
            GroupsRequest::Rm {
                group_name,
                target: _,
                group_id,
            } => {
                let group_id = service.delete_group(group_name, group_id).await?;
                Ok(GroupsPlatformResult::Removed { group_id })
            }
            GroupsRequest::Users { cmd } => match cmd {
                GroupsUsersRequest::Ls {
                    target,
                    filter,
                    offset,
                    limit,
                    all,
                    csv,
                } => {
                    let group_name = target.split('/').next_back();
                    let pages = service
                        .list_group_users(group_name, &filter, offset, limit, all)
                        .await?;
                    Ok(GroupsPlatformResult::UsersListed { pages, csv })
                }
                GroupsUsersRequest::Add {
                    target: _,
                    group_name,
                    group_id,
                    user_name,
                    user_id,
                } => {
                    let (group_id, user_id) = service
                        .add_group_user(group_name, group_id, user_name, user_id)
                        .await?;
                    Ok(GroupsPlatformResult::UserAdded { group_id, user_id })
                }
            },
        }
    }

    fn groups_target(cmd: &GroupsRequest) -> &str {
        match cmd {
            GroupsRequest::Create { target, .. }
            | GroupsRequest::Ls { target, .. }
            | GroupsRequest::Rm { target, .. } => target,
            GroupsRequest::Users { cmd } => match cmd {
                GroupsUsersRequest::Ls { target, .. } => target,
                GroupsUsersRequest::Add { target, .. } => target,
            },
        }
    }
}

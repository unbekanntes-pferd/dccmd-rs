use std::sync::Arc;

use console::Term;
use dco3::{
    auth::Connected,
    groups::{CreateGroupRequest, Group, GroupsFilter},
    Dracoon, Groups, ListAllParams,
};

use futures_util::{stream, StreamExt};
use models::GroupUsersOptions;
use tokio::sync::Mutex;
use tracing::error;

mod models;
mod print;
mod users;

use super::{
    init_dracoon,
    models::{DcCmdError, GroupsCommand},
    users::UserCommandHandler,
    utils::strings::format_success_message,
};

pub use models::GroupsUsersCommand;

pub struct GroupCommandHandler {
    client: Dracoon<Connected>,
    term: Term,
}

impl GroupCommandHandler {
    pub async fn try_new(target_domain: String, term: Term) -> Result<Self, DcCmdError> {
        let client = init_dracoon(&target_domain, None, false).await?;

        Ok(Self { client, term })
    }

    async fn create_group(&self, name: String) -> Result<(), DcCmdError> {
        let req = CreateGroupRequest::new(name, None);
        let group = self.client.groups.create_group(req).await?;

        let msg = format!("Group {} ({}) created", group.name, group.id);

        self.term
            .write_line(format_success_message(&msg).as_str())
            .map_err(|_| DcCmdError::IoError)?;

        Ok(())
    }

    async fn delete_group(&self, name: Option<String>, id: Option<u64>) -> Result<(), DcCmdError> {
        let group_id = match (name, id) {
            (_, Some(id)) => id,
            (Some(name), _) => self.find_group_by_name(name).await?.id,
            _ => {
                return Err(DcCmdError::InvalidArgument(
                    "Either group name or id must be provided".to_string(),
                ))
            }
        };

        self.client.groups.delete_group(group_id).await?;

        let msg = format!("Group {} deleted", group_id);

        self.term
            .write_line(format_success_message(&msg).as_str())
            .map_err(|_| DcCmdError::IoError)?;

        Ok(())
    }

    async fn find_group_by_name(&self, name: String) -> Result<Group, DcCmdError> {
        let params = ListAllParams::builder()
            .with_filter(GroupsFilter::name_contains(&name))
            .build();
        let groups = self.client.groups.get_groups(Some(params)).await?;

        let Some(group) = groups.items.iter().find(|g| g.name == name) else {
            error!("No group found with name: {name}");
            let msg = format!("No group found with name: {name}");
            return Err(DcCmdError::InvalidArgument(msg));
        };

        Ok(group.clone())
    }

    async fn list_groups(
        &self,
        search: Option<String>,
        offset: Option<u32>,
        limit: Option<u32>,
        all: bool,
        csv: bool,
    ) -> Result<(), DcCmdError> {
        let params = UserCommandHandler::build_params(
            &search,
            offset.unwrap_or(0).into(),
            limit.unwrap_or(500).into(),
        );

        let groups = self.client.groups.get_groups(Some(params)).await?;

        if all {
            let total = groups.range.total;
            let shared_results = Arc::new(Mutex::new(groups.clone()));

            let reqs = (500..=total)
                .step_by(500)
                .map(|offset| {
                    let params = UserCommandHandler::build_params(&search, offset, 500);

                    self.client.groups.get_groups(Some(params))
                })
                .collect::<Vec<_>>();

            stream::iter(reqs)
                .for_each_concurrent(5, |f| {
                    let shared_results_clone = Arc::clone(&shared_results);
                    async move {
                        match f.await {
                            Ok(mut users) => {
                                let mut shared_results = shared_results_clone.lock().await;
                                shared_results.items.append(&mut users.items);
                            }
                            Err(e) => {
                                error!("Failed to fetch users: {}", e);
                            }
                        }
                    }
                })
                .await;

            let results = shared_results.lock().await.clone();

            self.print_groups(results, csv)?;
        } else {
            self.print_groups(groups, csv)?;
        }

        Ok(())
    }
}

pub async fn handle_groups_cmd(cmd: GroupsCommand, term: Term) -> Result<(), DcCmdError> {
    let target = match &cmd {
        GroupsCommand::Create { target, .. }
        | GroupsCommand::Ls { target, .. }
        | GroupsCommand::Rm { target, .. } => target,
        GroupsCommand::Users { cmd } => match cmd {
            GroupsUsersCommand::Ls { target, .. } => target,
        },
    };

    let handler = GroupCommandHandler::try_new(target.to_string(), term).await?;
    match cmd {
        GroupsCommand::Create { target: _, name } => handler.create_group(name).await,
        GroupsCommand::Ls {
            target: _,
            search,
            offset,
            limit,
            all,
            csv,
        } => handler.list_groups(search, offset, limit, all, csv).await,
        GroupsCommand::Rm {
            group_name,
            target: _,
            group_id,
        } => handler.delete_group(group_name, group_id).await,
        GroupsCommand::Users { cmd } => users::handle_group_users_cmd(cmd, handler).await,
    }
}

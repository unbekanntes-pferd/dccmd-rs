use std::sync::Arc;

use console::Term;
use dco3::{
    auth::Connected,
    groups::{CreateGroupRequest, Group, GroupsFilter},
    Dracoon, Groups, ListAllParams,
};

use tokio::sync::Semaphore;
use tracing::error;

mod models;
mod print;
mod users;

use super::{
    config::MAX_CONCURRENT_REQUESTS,
    init_dracoon,
    models::{build_params, DcCmdError, GroupsCommand, ListOptions},
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
        let group = self.client.groups().create_group(req).await?;

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

        self.client.groups().delete_group(group_id).await?;

        let msg = format!("Group {group_id} deleted");

        self.term
            .write_line(format_success_message(&msg).as_str())
            .map_err(|_| DcCmdError::IoError)?;

        Ok(())
    }

    async fn find_group_by_name(&self, name: String) -> Result<Group, DcCmdError> {
        let params = ListAllParams::builder()
            .with_filter(GroupsFilter::name_contains(&name))
            .build();
        let groups = self.client.groups().get_groups(Some(params)).await?;

        let Some(group) = groups.items.iter().find(|g| g.name == name) else {
            error!("No group found with name: {name}");
            let msg = format!("No group found with name: {name}");
            return Err(DcCmdError::InvalidArgument(msg));
        };

        Ok(group.clone())
    }

    async fn list_groups(&self, opts: ListOptions) -> Result<(), DcCmdError> {
        let params = build_params(
            opts.filter(),
            opts.offset().unwrap_or(0),
            opts.limit().unwrap_or(500).into(),
        )?;

        let mut groups = self.client.groups().get_groups(Some(params)).await?;

        if opts.all() && groups.range.total > 500 {
            let (tx, mut rx) = tokio::sync::mpsc::channel(MAX_CONCURRENT_REQUESTS);
            let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT_REQUESTS));

            let mut handles = Vec::new();

            (500..=groups.range.total).step_by(500).for_each(|offset| {
                let tx = tx.clone();
                let dracoon_client = self.client.clone();
                let filter = opts.filter().clone();
                let semaphore = semaphore.clone();
                let handle = tokio::spawn(async move {
                    let _permit = semaphore.acquire().await.map_err(|_| {
                        error!("Error acquiring semaphore permit");
                        DcCmdError::IoError
                    })?;

                    let params = build_params(&filter, offset, 500.into())?;
                    let users = dracoon_client.groups().get_groups(Some(params)).await?;

                    tx.send(users).await.map_err(|e| {
                        error!("Error sending users: {}", e);
                        DcCmdError::IoError
                    })?;

                    Ok::<(), DcCmdError>(())
                });

                handles.push(handle);
            });

            drop(tx);

            while let Some(results) = rx.recv().await {
                groups.items.extend(results.items);
            }

            for handle in handles {
                if let Err(e) = handle.await {
                    error!("Error fetching users: {}", e);
                    return Err(DcCmdError::IoError);
                }
            }
        }

        self.print_groups(groups, opts.csv())?;

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
            GroupsUsersCommand::Add { target, .. } => target,
        },
    };

    let handler = GroupCommandHandler::try_new(target.to_string(), term).await?;
    match cmd {
        GroupsCommand::Create { target: _, name } => handler.create_group(name).await,
        GroupsCommand::Ls {
            target: _,
            filter,
            offset,
            limit,
            all,
            csv,
        } => {
            handler
                .list_groups(ListOptions::new(filter, offset, limit, all, csv))
                .await
        }
        GroupsCommand::Rm {
            group_name,
            target: _,
            group_id,
        } => handler.delete_group(group_name, group_id).await,
        GroupsCommand::Users { cmd } => users::handle_group_users_cmd(cmd, handler).await,
    }
}

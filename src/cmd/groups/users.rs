use dco3::{
    groups::{Group, GroupsFilter},
    Groups, ListAllParams,
};
use tracing::error;

use crate::cmd::models::{build_params, DcCmdError};

use super::{models::GroupUsersOptions, GroupCommandHandler, GroupsUsersCommand};

pub async fn handle_group_users_cmd(
    cmd: GroupsUsersCommand,
    handler: GroupCommandHandler,
) -> Result<(), DcCmdError> {
    match cmd {
        GroupsUsersCommand::Ls {
            target,
            filter,
            offset,
            limit,
            all,
            csv,
        } => {
            let group_name = target.split('/').last();
            let options = GroupUsersOptions::new(filter, offset, limit, all, csv);
            handler.list_group_users(group_name, options).await
        }
    }
}

impl GroupCommandHandler {
    async fn list_group_users(
        &self,
        group_name: Option<&str>,
        opts: GroupUsersOptions,
    ) -> Result<(), DcCmdError> {
        let groups = if let Some(group_name) = group_name.filter(|name| !name.is_empty()) {
            vec![self.get_group_by_name(group_name).await?]
        } else {
            let mut groups = self.client.groups().get_groups(None).await?;

            for offset in (500..=groups.range.total).step_by(500) {
                let params = ListAllParams::builder()
                    .with_offset(offset)
                    .with_limit(500)
                    .build();
                let mut new_groups = self.client.groups().get_groups(Some(params)).await?;

                groups.items.append(&mut new_groups.items);
            }

            groups.items
        };

        for (idx, group) in groups.iter().enumerate() {
            let params = build_params(
                &opts.filter,
                opts.offset.unwrap_or(0).into(),
                opts.limit.unwrap_or(500).into(),
            )?;
            let mut users = self
                .client
                .groups()
                .get_group_users(group.id, Some(params))
                .await?;

            if opts.all {
                for offset in (500..=users.range.total).step_by(500) {
                    let params =
                        build_params(&opts.filter, offset, opts.limit.unwrap_or(500).into())?;
                    let mut new_users = self
                        .client
                        .groups()
                        .get_group_users(group.id, Some(params))
                        .await?;

                    users.items.append(&mut new_users.items);
                }
            }

            let is_first = idx == 0;

            self.print_group_users(users, group, opts.csv, is_first)?;
        }

        Ok(())
    }

    async fn get_group_by_name(&self, group_name: &str) -> Result<Group, DcCmdError> {
        let filter = GroupsFilter::name_contains(group_name);
        let params = ListAllParams::builder().with_filter(filter).build();
        let group_results = self.client.groups().get_groups(Some(params)).await?;

        let Some(group) = group_results
            .items
            .into_iter()
            .find(|g| g.name == group_name)
        else {
            error!("No group found with username: {group_name}");
            let msg = format!("No group found with name: {group_name}");
            return Err(DcCmdError::InvalidArgument(msg));
        };

        Ok(group)
    }
}

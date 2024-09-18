use dco3::{
    eventlog::{AuditNodeList, AuditNodesFilter},
    Eventlog, FilterQuery, ListAllParams, Users,
};

use crate::cmd::models::{build_params, DcCmdError, ListOptions};

use super::ReportsCommandHandler;

impl ReportsCommandHandler {
    #[allow(deprecated)]
    pub async fn get_permissions(&self, opts: ListOptions) -> Result<AuditNodeList, DcCmdError> {
        let offset = opts.offset().unwrap_or(0);

        if let Some(filter) = opts.filter() {
            let params = build_params(&Some(filter.to_string()), offset, None)?;

            return Ok(self.client.eventlog().get_node_permissions(params).await?);
        }

        let user_ids = self.get_all_user_ids().await?;

        let mut perms = Vec::new();

        for user in user_ids {
            let user_filter = AuditNodesFilter::user_id_equals(user).to_filter_string();

            let params = build_params(&Some(user_filter), offset, None)?;

            let next_perms = self.client.eventlog().get_node_permissions(params).await?;

            perms.extend(next_perms);
        }

        Ok(perms)
    }

    async fn get_all_user_ids(&self) -> Result<Vec<u64>, DcCmdError> {
        let mut users = self.client.users().get_users(None, None, None).await?;

        let user_reqs = (500..users.range.total)
            .step_by(500)
            .map(|offset| {
                let params = ListAllParams::builder().with_offset(offset).build();
                self.client.users().get_users(Some(params), None, None)
            })
            .collect::<Vec<_>>();

        for user_req in user_reqs {
            let user = user_req.await?;
            users.items.extend(user.items);
        }

        let user_ids = users.items.iter().map(|u| u.id).collect::<Vec<_>>();

        Ok(user_ids)
    }
}

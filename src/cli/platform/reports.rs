use crate::{
    app::{
        auth::AuthService,
        reports::{EventOptions, ReportsService},
        results::ReportsPlatformResult,
    },
    command::ReportsCommand,
    core::models::{DcCmdError, ListOptions, PasswordAuth},
};

use super::CliPlatform;

impl CliPlatform {
    pub(super) async fn execute_reports_cmd(
        &self,
        cmd: ReportsCommand,
        auth: Option<PasswordAuth>,
    ) -> Result<ReportsPlatformResult, DcCmdError> {
        let client = AuthService::new()
            .connect_client(Self::reports_target(&cmd), auth, false)
            .await?;
        let service = ReportsService::new(client);

        match cmd {
            ReportsCommand::Events {
                target: _,
                filter,
                offset,
                limit,
                all,
                csv,
                operation_type,
                user_id,
                status,
                start_date,
                end_date,
            } => {
                service.check_dracoon_api_version().await?;

                let list_opts = ListOptions::new(filter, offset, limit, all, csv);
                let opts = EventOptions::new(
                    list_opts,
                    start_date,
                    end_date,
                    user_id,
                    operation_type,
                    status,
                )?;
                let events = service.get_events(opts).await?;

                Ok(ReportsPlatformResult::Events { events, csv })
            }
            ReportsCommand::OperationTypes { target: _ } => {
                let operations = service.get_event_operations().await?;
                Ok(ReportsPlatformResult::OperationTypes { operations })
            }
            ReportsCommand::Permissions {
                target: _,
                filter,
                offset,
                limit,
                all,
                csv,
            } => {
                let list_opts = ListOptions::new(filter, offset, limit, all, csv);
                service.check_dracoon_api_version().await?;
                let permissions = service.get_permissions(list_opts).await?;

                Ok(ReportsPlatformResult::Permissions { permissions, csv })
            }
        }
    }

    fn reports_target(cmd: &ReportsCommand) -> &str {
        match cmd {
            ReportsCommand::Events { target, .. }
            | ReportsCommand::Permissions { target, .. }
            | ReportsCommand::OperationTypes { target } => target,
        }
    }
}

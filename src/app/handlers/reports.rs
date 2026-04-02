use crate::{
    app::{
        requests::ReportsRequest,
        results::{AppResult, ReportsPlatformResult},
        App, Platform, Ui,
    },
    core::models::{DcCmdError, PasswordAuth},
};

impl<P: Platform, U: Ui> App<P, U> {
    pub(in crate::app) async fn handle_reports(
        &self,
        cmd: ReportsRequest,
        auth: Option<PasswordAuth>,
    ) -> Result<AppResult, DcCmdError> {
        match self.platform.reports(cmd, auth).await? {
            ReportsPlatformResult::Events { events, csv } => self.ui.print_events(events, csv)?,
            ReportsPlatformResult::Permissions { permissions, csv } => {
                self.ui.print_permissions(permissions, csv)?
            }
            ReportsPlatformResult::OperationTypes { operations } => {
                self.ui.print_event_types(operations)?
            }
        }
        Ok(AppResult::success(Default::default()))
    }
}

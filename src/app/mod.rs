pub mod auth;
pub mod config;
pub mod groups;
mod handlers;
pub mod nodes;
pub mod outcome;
pub mod reports;
pub mod requests;
pub mod results;
pub mod shares;
pub mod users;

use async_trait::async_trait;
use dco3::{
    eventlog::{AuditNodeList, LogEventList, LogOperationList},
    groups::Group,
    nodes::Node,
    users::UserItem,
    RangedItems,
};

use crate::{
    app::{
        groups::GroupUsersPage,
        nodes::{
            command::{
                CmdCopyOptions, CmdCreateContainerOptions, CmdDeleteOptions, CmdDownloadOptions,
                CmdListNodesOptions, CmdTransferOptions, CmdUploadOptions,
            },
            download::DownloadOutcome,
            transfer::TransferOutcome,
            upload::UploadOutcome,
            CopyNodesResult, DeleteNodesPreparation, ListNodesResult,
        },
        requests::{ConfigRequest, GroupsRequest, ReportsRequest, UsersRequest},
        users::models::UserInfo,
    },
    command::AppCommand,
    core::models::DcCmdError,
};

use self::{
    outcome::{CommandOutcome, OutcomeMessageKind},
    results::{
        AppResult, AppStatus, ConfigPlatformResult, GroupsPlatformResult, ReportsPlatformResult,
        UsersPlatformResult,
    },
};

// App-level architecture:
// - `App` orchestrates command flow and user-facing outcomes.
// - `Platform` encapsulates external side effects (auth/session/service calls).
// - `Ui` encapsulates all interactive and rendering concerns.
#[async_trait]
pub trait Platform: Send + Sync {
    async fn config(&self, cmd: ConfigRequest) -> Result<ConfigPlatformResult, DcCmdError>;

    async fn upload(
        &self,
        source: String,
        target: String,
        opts: CmdUploadOptions,
    ) -> Result<UploadOutcome, DcCmdError>;

    async fn download(
        &self,
        source: String,
        target: String,
        opts: CmdDownloadOptions,
    ) -> Result<DownloadOutcome, DcCmdError>;

    async fn transfer(
        &self,
        source: String,
        target: String,
        opts: CmdTransferOptions,
    ) -> Result<TransferOutcome, DcCmdError>;

    async fn copy_nodes(
        &self,
        source: String,
        target: String,
        opts: CmdCopyOptions,
    ) -> Result<CopyNodesResult, DcCmdError>;

    async fn list_nodes(
        &self,
        source: String,
        opts: CmdListNodesOptions,
    ) -> Result<ListNodesResult, DcCmdError>;

    async fn prepare_delete(
        &self,
        source: String,
        opts: CmdDeleteOptions,
    ) -> Result<DeleteNodesPreparation, DcCmdError>;

    async fn delete_node(
        &self,
        source: String,
        opts: CmdDeleteOptions,
        node_id: u64,
    ) -> Result<(), DcCmdError>;

    async fn delete_nodes(
        &self,
        source: String,
        opts: CmdDeleteOptions,
        node_ids: Vec<u64>,
    ) -> Result<(), DcCmdError>;

    async fn create_container(
        &self,
        source: String,
        opts: CmdCreateContainerOptions,
    ) -> Result<String, DcCmdError>;

    async fn users(&self, cmd: UsersRequest) -> Result<UsersPlatformResult, DcCmdError>;

    async fn groups(&self, cmd: GroupsRequest) -> Result<GroupsPlatformResult, DcCmdError>;

    async fn reports(&self, cmd: ReportsRequest) -> Result<ReportsPlatformResult, DcCmdError>;
}

pub trait Ui: Send + Sync {
    fn print_node(&self, node: &Node, long: bool, human_readable: bool) -> Result<(), DcCmdError>;

    fn write_success(&self, message: &str) -> Result<(), DcCmdError>;

    fn write_warning(&self, message: &str) -> Result<(), DcCmdError>;

    fn write_error(&self, message: &str) -> Result<(), DcCmdError>;

    fn write_info(&self, message: &str) -> Result<(), DcCmdError>;

    fn confirm(&self, prompt: &str) -> Result<bool, DcCmdError>;

    fn print_users(&self, users: &RangedItems<UserItem>, csv: bool) -> Result<(), DcCmdError>;

    fn print_user_info(&self, user: UserInfo) -> Result<(), DcCmdError>;

    fn print_groups(&self, groups: RangedItems<Group>, csv: bool) -> Result<(), DcCmdError>;

    fn print_group_users(&self, pages: &[GroupUsersPage], csv: bool) -> Result<(), DcCmdError>;

    fn print_events(&self, events: LogEventList, csv: bool) -> Result<(), DcCmdError>;

    fn print_permissions(&self, permissions: AuditNodeList, csv: bool) -> Result<(), DcCmdError>;

    fn print_event_types(&self, operations: LogOperationList) -> Result<(), DcCmdError>;
}

const MKROOM_DEPRECATION_WARNING: &str =
    "Warning: `mkroom` is deprecated and will be removed in a future release. Use `mkdir --type room` instead.";
const DOWNLOAD_FAILURE_PREVIEW_LIMIT: usize = 3;

pub struct App<P: Platform, U: Ui> {
    platform: P,
    ui: U,
}

impl<P: Platform, U: Ui> App<P, U> {
    pub fn new(platform: P, ui: U) -> Self {
        Self { platform, ui }
    }

    pub async fn execute(&self, command: AppCommand) -> Result<AppResult, DcCmdError> {
        // Each command is handled through a dedicated app-level handler method to keep
        // orchestration shape consistent and testable at command-flow level.
        match command {
            AppCommand::Config { cmd } => self.handle_config(cmd).await,
            AppCommand::Upload {
                source,
                target,
                opts,
            } => self.handle_upload(source, target, opts).await,
            AppCommand::Download {
                source,
                target,
                opts,
            } => self.handle_download(source, target, opts).await,
            AppCommand::Transfer {
                source,
                target,
                opts,
            } => self.handle_transfer(source, target, opts).await,
            AppCommand::Cp {
                source,
                target,
                opts,
            } => self.handle_copy(source, target, opts).await,
            AppCommand::Ls { source, opts } => self.handle_list(source, opts).await,
            AppCommand::Rm { source, opts } => self.handle_remove(source, opts).await,
            AppCommand::Mkdir {
                source,
                opts,
                deprecated_alias,
            } => self.handle_mkdir(source, opts, deprecated_alias).await,
            AppCommand::Users { cmd } => self.handle_users(cmd).await,
            AppCommand::Groups { cmd } => self.handle_groups(cmd).await,
            AppCommand::Reports { cmd } => self.handle_reports(cmd).await,
        }
    }

    fn write_success(&self, outcome: &mut CommandOutcome, message: &str) -> Result<(), DcCmdError> {
        self.ui.write_success(message)?;
        outcome.push(OutcomeMessageKind::Success, message);
        Ok(())
    }

    fn write_warning(&self, outcome: &mut CommandOutcome, message: &str) -> Result<(), DcCmdError> {
        self.ui.write_warning(message)?;
        outcome.push(OutcomeMessageKind::Warning, message);
        Ok(())
    }

    fn write_error(&self, outcome: &mut CommandOutcome, message: &str) -> Result<(), DcCmdError> {
        self.ui.write_error(message)?;
        outcome.push(OutcomeMessageKind::Error, message);
        Ok(())
    }

    fn write_info(&self, outcome: &mut CommandOutcome, message: &str) -> Result<(), DcCmdError> {
        self.ui.write_info(message)?;
        outcome.push(OutcomeMessageKind::Info, message);
        Ok(())
    }
}

fn app_result_from_status(status: AppStatus, outcome: CommandOutcome) -> AppResult {
    match status {
        AppStatus::Success => AppResult::success(outcome),
        AppStatus::PartialFailure => AppResult::partial_failure(outcome),
        AppStatus::Failure => AppResult::failure(outcome),
    }
}

#[cfg(test)]
mod tests;

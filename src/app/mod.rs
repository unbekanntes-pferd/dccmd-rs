pub mod auth;
pub mod config;
pub mod groups;
pub mod nodes;
pub mod outcome;
pub mod reports;
pub mod results;
pub mod shares;
pub mod users;

use async_trait::async_trait;
use dco3::{
    eventlog::{AuditNodeList, LogEventList, LogOperationList},
    groups::Group,
    nodes::models::NodeType,
    nodes::Node,
    users::UserItem,
    RangedItems,
};
use tracing::warn;

use crate::{
    app::{
        groups::GroupUsersPage,
        nodes::{
            command::{
                CmdCopyOptions, CmdCreateContainerOptions, CmdDeleteOptions, CmdDownloadOptions,
                CmdListNodesOptions, CmdTransferOptions, CmdUploadOptions,
            },
            download::DownloadOutcome,
            CopyNodesResult, DeleteNodesPreparation, ListNodesResult,
        },
        users::models::UserInfo,
    },
    command::{
        config::{ConfigAuthCommand, ConfigCommand},
        AppCommand, GroupsCommand, ReportsCommand, UsersCommand,
    },
    core::models::{DcCmdError, PasswordAuth},
};

use self::{
    outcome::{CommandOutcome, OutcomeMessageKind},
    results::{
        AppResult, ConfigPlatformResult, GroupsPlatformResult, ReportsPlatformResult,
        UsersPlatformResult,
    },
};

// App-level architecture:
// - `App` orchestrates command flow and user-facing outcomes.
// - `Platform` encapsulates external side effects (auth/session/service calls).
// - `Ui` encapsulates all interactive and rendering concerns.
#[async_trait]
pub trait Platform: Send + Sync {
    async fn config(&self, cmd: ConfigCommand) -> Result<ConfigPlatformResult, DcCmdError>;

    async fn upload(
        &self,
        source: String,
        target: String,
        opts: CmdUploadOptions,
    ) -> Result<Option<String>, DcCmdError>;

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
    ) -> Result<Option<String>, DcCmdError>;

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

    async fn users(
        &self,
        cmd: UsersCommand,
        auth: Option<PasswordAuth>,
    ) -> Result<UsersPlatformResult, DcCmdError>;

    async fn groups(
        &self,
        cmd: GroupsCommand,
        auth: Option<PasswordAuth>,
    ) -> Result<GroupsPlatformResult, DcCmdError>;

    async fn reports(
        &self,
        cmd: ReportsCommand,
        auth: Option<PasswordAuth>,
    ) -> Result<ReportsPlatformResult, DcCmdError>;
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
            AppCommand::Users { cmd, auth } => self.handle_users(cmd, auth).await,
            AppCommand::Groups { cmd, auth } => self.handle_groups(cmd, auth).await,
            AppCommand::Reports { cmd, auth } => self.handle_reports(cmd, auth).await,
        }
    }

    async fn handle_config(&self, cmd: ConfigCommand) -> Result<AppResult, DcCmdError> {
        let mut outcome = CommandOutcome::default();
        if matches!(
            &cmd,
            ConfigCommand::Auth {
                cmd: ConfigAuthCommand::Rm { .. }
            }
        ) && !self
            .ui
            .confirm("Are you sure you want to remove the token?")?
        {
            return Ok(AppResult::messages(outcome));
        }

        match self.platform.config(cmd).await? {
            ConfigPlatformResult::AuthTokenInfo {
                base_url,
                user_info,
            } => {
                self.write_info(&mut outcome, &format!("► Token stored for: {base_url}"))?;
                self.write_info(
                    &mut outcome,
                    &format!("► User: {} {}", user_info.first_name, user_info.last_name),
                )?;
                self.write_info(
                    &mut outcome,
                    &format!(
                        "► Email: {}",
                        user_info.email.unwrap_or_else(|| "N/A".to_string())
                    ),
                )?;
                self.write_info(
                    &mut outcome,
                    &format!("► Username: {}", user_info.user_name),
                )?;
            }
            ConfigPlatformResult::AuthTokenRemoved { base_url } => {
                self.write_success(&mut outcome, &format!("Token removed for {base_url}"))?;
            }
            ConfigPlatformResult::CryptoSecretStored { base_url } => {
                self.write_info(
                    &mut outcome,
                    &format!("► Encryption secret securely stored for {base_url}."),
                )?;
            }
            ConfigPlatformResult::CryptoSecretRemoved { base_url } => {
                self.write_success(
                    &mut outcome,
                    &format!("Encryption secret removed for {base_url}."),
                )?;
            }
            ConfigPlatformResult::SystemInfo {
                base_url,
                system_info,
            } => {
                let customer_info = system_info.customer;
                self.write_info(&mut outcome, &format!("► System info for: {base_url}"))?;
                self.write_info(&mut outcome, &format!("► Customer: {}", customer_info.name))?;

                let percent_space_used =
                    (customer_info.space_used as f64 / customer_info.space_limit as f64) * 100.0;
                let percent_users_used = (customer_info.accounts_used as f64
                    / customer_info.accounts_limit as f64)
                    * 100.0;

                self.write_info(
                    &mut outcome,
                    &format!(
                        "► Space used: {} / {} ({percent_space_used:.2}%)",
                        crate::core::utils::strings::to_readable_size(customer_info.space_used),
                        crate::core::utils::strings::to_readable_size(customer_info.space_limit)
                    ),
                )?;
                self.write_info(
                    &mut outcome,
                    &format!(
                        "► Users used: {} / {} ({percent_users_used:.2}%)",
                        customer_info.accounts_used, customer_info.accounts_limit
                    ),
                )?;

                if !system_info.oidc_configs.is_empty() {
                    self.write_info(&mut outcome, "\n► OpenID Connect IDP configurations:")?;
                    for info in system_info.oidc_configs {
                        self.write_info(&mut outcome, &format!("► {} ({})", info.name, info.id))?;
                    }
                } else {
                    self.write_info(
                        &mut outcome,
                        "\n► No OpenID Connect IDP configurations found.",
                    )?;
                }

                if !system_info.ad_configs.is_empty() {
                    self.write_info(&mut outcome, "\n► Active Directory configurations:")?;
                    for info in system_info.ad_configs {
                        self.write_info(&mut outcome, &format!("► {} ({})", info.alias, info.id))?;
                    }
                } else {
                    self.write_info(
                        &mut outcome,
                        "\n► No Active Directory configurations found.",
                    )?;
                }
            }
            ConfigPlatformResult::MissingToken { base_url } => {
                self.write_error(
                    &mut outcome,
                    &format!("No token found for this DRACOON url: {base_url}."),
                )?;
            }
            ConfigPlatformResult::MissingCryptoSecret => {
                self.write_error(&mut outcome, "No encryption secret found.")?;
            }
        }

        Ok(AppResult::messages(outcome))
    }

    async fn handle_download(
        &self,
        source: String,
        target: String,
        opts: CmdDownloadOptions,
    ) -> Result<AppResult, DcCmdError> {
        let mut outcome = CommandOutcome::default();
        let download_outcome = self.platform.download(source, target, opts).await?;

        if download_outcome.failed == 0 {
            self.write_success(
                &mut outcome,
                &format!(
                    "Downloaded {} item(s) to {}.",
                    download_outcome.succeeded, download_outcome.target_root
                ),
            )?;
            return Ok(AppResult::messages(outcome));
        }

        let summary = format!(
            "Downloaded {}/{} item(s) to {}; {} failed.",
            download_outcome.succeeded,
            download_outcome.requested_total,
            download_outcome.target_root,
            download_outcome.failed
        );

        if download_outcome.succeeded > 0 {
            self.write_warning(&mut outcome, &summary)?;
        } else {
            self.write_error(&mut outcome, &summary)?;
        }

        for failure in download_outcome
            .failures
            .iter()
            .take(DOWNLOAD_FAILURE_PREVIEW_LIMIT)
        {
            self.write_info(
                &mut outcome,
                &format!(
                    "Failed {} -> {} ({})",
                    failure.node_name, failure.target, failure.reason
                ),
            )?;
        }

        if download_outcome.failures.len() > DOWNLOAD_FAILURE_PREVIEW_LIMIT {
            self.write_info(
                &mut outcome,
                &format!(
                    "... and {} more failed item(s).",
                    download_outcome.failures.len() - DOWNLOAD_FAILURE_PREVIEW_LIMIT
                ),
            )?;
        }

        Ok(AppResult::messages(outcome))
    }

    async fn handle_upload(
        &self,
        source: String,
        target: String,
        opts: CmdUploadOptions,
    ) -> Result<AppResult, DcCmdError> {
        let mut outcome = CommandOutcome::default();
        if let Some(message) = self.platform.upload(source, target, opts).await? {
            self.write_success(&mut outcome, &message)?;
        }
        Ok(AppResult::messages(outcome))
    }

    async fn handle_transfer(
        &self,
        source: String,
        target: String,
        opts: CmdTransferOptions,
    ) -> Result<AppResult, DcCmdError> {
        let mut outcome = CommandOutcome::default();
        if let Some(message) = self.platform.transfer(source, target, opts).await? {
            self.write_success(&mut outcome, &message)?;
        }
        Ok(AppResult::messages(outcome))
    }

    async fn handle_copy(
        &self,
        source: String,
        target: String,
        opts: CmdCopyOptions,
    ) -> Result<AppResult, DcCmdError> {
        let mut outcome = CommandOutcome::default();
        let result = self.platform.copy_nodes(source, target, opts).await?;
        let success_message = format!(
            "Copied {} node(s) from {} to {}.",
            result.count_nodes, result.source_parent_path, result.target_path
        );
        self.write_success(&mut outcome, &success_message)?;
        Ok(AppResult::messages(outcome))
    }

    async fn handle_list(
        &self,
        source: String,
        opts: CmdListNodesOptions,
    ) -> Result<AppResult, DcCmdError> {
        let mut outcome = CommandOutcome::default();
        let long = opts.long();
        let human_readable = opts.human_readable();
        let result = self.platform.list_nodes(source, opts).await?;
        let node_path = result.node_path.clone().unwrap_or_else(|| "/".to_string());
        outcome.push(
            OutcomeMessageKind::Info,
            format!("Listed {} node(s) in {node_path}.", result.list.items.len()),
        );

        result
            .list
            .items
            .iter()
            .try_for_each(|node| self.ui.print_node(node, long, human_readable))?;

        Ok(AppResult::messages(outcome))
    }

    async fn handle_remove(
        &self,
        source: String,
        opts: CmdDeleteOptions,
    ) -> Result<AppResult, DcCmdError> {
        let mut outcome = CommandOutcome::default();
        let preparation = self
            .platform
            .prepare_delete(source.clone(), opts.clone())
            .await?;

        match preparation {
            DeleteNodesPreparation::InvalidSearchRequiresRecursive => {
                self.write_error(
                    &mut outcome,
                    "Deleting search results not allowed. Use --recursive flag to delete recursively.",
                )?;
            }
            DeleteNodesPreparation::ContainerRequiresRecursive => {
                self.write_error(
                    &mut outcome,
                    "Deleting non-empty folder or room not allowed. Use --recursive flag to delete recursively.",
                )?;
            }
            DeleteNodesPreparation::Search { node_ids } => {
                let confirmed = self.ui.confirm(&format!(
                    "Do you really want to delete {} items?",
                    node_ids.len()
                ))?;
                if confirmed {
                    self.platform.delete_nodes(source, opts, node_ids).await?;
                }
            }
            DeleteNodesPreparation::SingleNode {
                node_id,
                node_name,
                node_type,
            } => {
                if node_type == NodeType::Room {
                    let confirmed = self
                        .ui
                        .confirm(&format!("Do you really want to delete room {node_name}?"))?;
                    if !confirmed {
                        self.write_error(&mut outcome, "Deleting room not confirmed.")?;
                        return Ok(AppResult::messages(outcome));
                    }
                }

                self.platform.delete_node(source, opts, node_id).await?;
                self.write_success(&mut outcome, &format!("Node {node_name} deleted."))?;
            }
        }

        Ok(AppResult::messages(outcome))
    }

    async fn handle_mkdir(
        &self,
        source: String,
        opts: CmdCreateContainerOptions,
        deprecated_alias: bool,
    ) -> Result<AppResult, DcCmdError> {
        let mut outcome = CommandOutcome::default();
        if deprecated_alias {
            warn!("{}", MKROOM_DEPRECATION_WARNING);
            self.write_warning(&mut outcome, MKROOM_DEPRECATION_WARNING)?;
        }

        let success_message = self.platform.create_container(source, opts).await?;
        self.write_success(&mut outcome, &success_message)?;
        Ok(AppResult::messages(outcome))
    }

    async fn handle_users(
        &self,
        cmd: UsersCommand,
        auth: Option<PasswordAuth>,
    ) -> Result<AppResult, DcCmdError> {
        let mut outcome = CommandOutcome::default();
        match self.platform.users(cmd, auth).await? {
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

        Ok(AppResult::messages(outcome))
    }

    async fn handle_groups(
        &self,
        cmd: GroupsCommand,
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
        Ok(AppResult::messages(outcome))
    }

    async fn handle_reports(
        &self,
        cmd: ReportsCommand,
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
        Ok(AppResult::messages(CommandOutcome::default()))
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

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use async_trait::async_trait;
    use dco3::{
        models::{Range, RangedItems},
        nodes::{models::NodeType, Node, NodeList},
    };
    use secrecy::SecretString;

    use crate::{
        app::nodes::{
            command::{
                CmdCopyOptions, CmdCreateContainerOptions, CmdDeleteOptions, CmdDownloadOptions,
                CmdListNodesOptions, CmdTransferOptions, CmdUploadOptions,
            },
            download::{DownloadFailure, DownloadOutcome},
            CopyNodesResult, DeleteNodesPreparation, ListNodesResult,
        },
        command::{
            config::{ConfigAuthCommand, ConfigCommand, ConfigCryptoCommand},
            AppCommand, CreateContainerType, GroupsCommand, ReportsCommand, UsersCommand,
        },
        core::models::{DcCmdError, ListOptions, PasswordAuth},
    };

    use super::{
        outcome::OutcomeMessageKind, results::ConfigPlatformResult, results::GroupsPlatformResult,
        results::ReportsPlatformResult, results::UsersPlatformResult, App, Platform, Ui,
        MKROOM_DEPRECATION_WARNING,
    };

    #[derive(Default)]
    struct MockPlatform {
        preparation: Mutex<Option<DeleteNodesPreparation>>,
        list_result: Mutex<Option<ListNodesResult>>,
        list_error: Mutex<bool>,
        create_message: Mutex<Option<String>>,
        create_error: Mutex<bool>,
        copy_result: Mutex<Option<CopyNodesResult>>,
        copy_error: Mutex<bool>,
        download_error: Mutex<bool>,
        download_outcome: Mutex<Option<DownloadOutcome>>,
        config_result: Mutex<Option<ConfigPlatformResult>>,
        users_result: Mutex<Option<UsersPlatformResult>>,
        users_error: Mutex<bool>,
        groups_result: Mutex<Option<GroupsPlatformResult>>,
        groups_error: Mutex<bool>,
        reports_result: Mutex<Option<ReportsPlatformResult>>,
        reports_error: Mutex<bool>,
        transfer_error: Mutex<bool>,
        upload_message: Mutex<Option<String>>,
        transfer_message: Mutex<Option<String>>,
        download_calls: Mutex<Vec<(String, String)>>,
        transfer_calls: Mutex<Vec<(String, String, CmdTransferOptions)>>,
        deleted_nodes: Mutex<Vec<u64>>,
        deleted_batch_nodes: Mutex<Vec<Vec<u64>>>,
        prepare_recursive: Mutex<Vec<bool>>,
        prepare_has_auth: Mutex<Vec<bool>>,
    }

    impl MockPlatform {
        fn with_preparation(preparation: DeleteNodesPreparation) -> Self {
            Self {
                preparation: Mutex::new(Some(preparation)),
                ..Self::default()
            }
        }

        fn with_create_message(message: &str) -> Self {
            Self {
                create_message: Mutex::new(Some(message.to_string())),
                ..Self::default()
            }
        }

        fn with_create_error() -> Self {
            Self {
                create_error: Mutex::new(true),
                ..Self::default()
            }
        }

        fn with_list_result(list_result: ListNodesResult) -> Self {
            Self {
                list_result: Mutex::new(Some(list_result)),
                ..Self::default()
            }
        }

        fn with_list_error() -> Self {
            Self {
                list_error: Mutex::new(true),
                ..Self::default()
            }
        }

        fn with_copy_result(
            count_nodes: usize,
            source_parent_path: &str,
            target_path: &str,
        ) -> Self {
            Self {
                copy_result: Mutex::new(Some(CopyNodesResult {
                    count_nodes,
                    source_parent_path: source_parent_path.to_string(),
                    target_path: target_path.to_string(),
                })),
                ..Self::default()
            }
        }

        fn with_copy_error() -> Self {
            Self {
                copy_error: Mutex::new(true),
                ..Self::default()
            }
        }

        fn with_download_error() -> Self {
            Self {
                download_error: Mutex::new(true),
                ..Self::default()
            }
        }

        fn with_transfer_error() -> Self {
            Self {
                transfer_error: Mutex::new(true),
                ..Self::default()
            }
        }

        fn with_download_outcome(download_outcome: DownloadOutcome) -> Self {
            Self {
                download_outcome: Mutex::new(Some(download_outcome)),
                ..Self::default()
            }
        }

        fn with_upload_message(message: &str) -> Self {
            Self {
                upload_message: Mutex::new(Some(message.to_string())),
                ..Self::default()
            }
        }

        fn with_transfer_message(message: &str) -> Self {
            Self {
                transfer_message: Mutex::new(Some(message.to_string())),
                ..Self::default()
            }
        }

        fn with_users_result(result: UsersPlatformResult) -> Self {
            Self {
                users_result: Mutex::new(Some(result)),
                ..Self::default()
            }
        }

        fn with_users_error() -> Self {
            Self {
                users_error: Mutex::new(true),
                ..Self::default()
            }
        }

        fn with_groups_result(result: GroupsPlatformResult) -> Self {
            Self {
                groups_result: Mutex::new(Some(result)),
                ..Self::default()
            }
        }

        fn with_groups_error() -> Self {
            Self {
                groups_error: Mutex::new(true),
                ..Self::default()
            }
        }

        fn with_reports_result(result: ReportsPlatformResult) -> Self {
            Self {
                reports_result: Mutex::new(Some(result)),
                ..Self::default()
            }
        }

        fn with_reports_error() -> Self {
            Self {
                reports_error: Mutex::new(true),
                ..Self::default()
            }
        }

        fn with_config_result(result: ConfigPlatformResult) -> Self {
            Self {
                config_result: Mutex::new(Some(result)),
                ..Self::default()
            }
        }
    }

    #[async_trait]
    impl Platform for Arc<MockPlatform> {
        async fn upload(
            &self,
            _source: String,
            _target: String,
            _opts: CmdUploadOptions,
        ) -> Result<Option<String>, DcCmdError> {
            Ok(self.upload_message.lock().expect("lock poisoned").take())
        }

        async fn config(&self, _cmd: ConfigCommand) -> Result<ConfigPlatformResult, DcCmdError> {
            Ok(self
                .config_result
                .lock()
                .expect("lock poisoned")
                .take()
                .unwrap_or(ConfigPlatformResult::AuthTokenRemoved {
                    base_url: "https://example.com".to_string(),
                }))
        }

        async fn download(
            &self,
            source: String,
            target: String,
            _opts: CmdDownloadOptions,
        ) -> Result<DownloadOutcome, DcCmdError> {
            if *self.download_error.lock().expect("lock poisoned") {
                return Err(DcCmdError::InvalidArgument("download failed".to_string()));
            }
            self.download_calls
                .lock()
                .expect("lock poisoned")
                .push((source, target));
            Ok(self
                .download_outcome
                .lock()
                .expect("lock poisoned")
                .take()
                .unwrap_or_else(|| DownloadOutcome::success("/tmp/file.txt", 1)))
        }

        async fn transfer(
            &self,
            source: String,
            target: String,
            opts: CmdTransferOptions,
        ) -> Result<Option<String>, DcCmdError> {
            if *self.transfer_error.lock().expect("lock poisoned") {
                return Err(DcCmdError::InvalidArgument("transfer failed".to_string()));
            }
            self.transfer_calls
                .lock()
                .expect("lock poisoned")
                .push((source, target, opts));
            Ok(self.transfer_message.lock().expect("lock poisoned").take())
        }

        async fn copy_nodes(
            &self,
            _source: String,
            _target: String,
            _opts: CmdCopyOptions,
        ) -> Result<CopyNodesResult, DcCmdError> {
            if *self.copy_error.lock().expect("lock poisoned") {
                return Err(DcCmdError::InvalidArgument("copy failed".to_string()));
            }
            Ok(self
                .copy_result
                .lock()
                .expect("lock poisoned")
                .take()
                .unwrap_or(CopyNodesResult {
                    count_nodes: 1,
                    source_parent_path: "/".to_string(),
                    target_path: "/".to_string(),
                }))
        }

        async fn list_nodes(
            &self,
            _source: String,
            _opts: CmdListNodesOptions,
        ) -> Result<ListNodesResult, DcCmdError> {
            if *self.list_error.lock().expect("lock poisoned") {
                return Err(DcCmdError::InvalidArgument("list failed".to_string()));
            }
            if let Some(list_result) = self.list_result.lock().expect("lock poisoned").take() {
                return Ok(list_result);
            }
            Ok(ListNodesResult {
                list: empty_node_list(),
                node_path: Some("/".to_string()),
            })
        }

        async fn prepare_delete(
            &self,
            _source: String,
            opts: CmdDeleteOptions,
        ) -> Result<DeleteNodesPreparation, DcCmdError> {
            self.prepare_recursive
                .lock()
                .expect("lock poisoned")
                .push(opts.recursive());
            self.prepare_has_auth
                .lock()
                .expect("lock poisoned")
                .push(opts.auth().is_some());
            self.preparation
                .lock()
                .expect("lock poisoned")
                .take()
                .ok_or_else(|| DcCmdError::InvalidArgument("missing preparation".to_string()))
        }

        async fn delete_node(
            &self,
            _source: String,
            _opts: CmdDeleteOptions,
            node_id: u64,
        ) -> Result<(), DcCmdError> {
            self.deleted_nodes
                .lock()
                .expect("lock poisoned")
                .push(node_id);
            Ok(())
        }

        async fn delete_nodes(
            &self,
            _source: String,
            _opts: CmdDeleteOptions,
            node_ids: Vec<u64>,
        ) -> Result<(), DcCmdError> {
            self.deleted_batch_nodes
                .lock()
                .expect("lock poisoned")
                .push(node_ids);
            Ok(())
        }

        async fn create_container(
            &self,
            _source: String,
            _opts: CmdCreateContainerOptions,
        ) -> Result<String, DcCmdError> {
            if *self.create_error.lock().expect("lock poisoned") {
                return Err(DcCmdError::InvalidArgument(
                    "create container failed".to_string(),
                ));
            }
            Ok(self
                .create_message
                .lock()
                .expect("lock poisoned")
                .take()
                .unwrap_or_else(|| "created".to_string()))
        }

        async fn users(
            &self,
            _cmd: UsersCommand,
            _auth: Option<PasswordAuth>,
        ) -> Result<UsersPlatformResult, DcCmdError> {
            if *self.users_error.lock().expect("lock poisoned") {
                return Err(DcCmdError::InvalidArgument("users failed".to_string()));
            }
            if let Some(result) = self.users_result.lock().expect("lock poisoned").take() {
                return Ok(result);
            }
            Ok(UsersPlatformResult::Imported(
                crate::app::users::ImportUsersResult {
                    imported: 0,
                    failed: 0,
                    total: 0,
                },
            ))
        }

        async fn groups(
            &self,
            _cmd: GroupsCommand,
            _auth: Option<PasswordAuth>,
        ) -> Result<GroupsPlatformResult, DcCmdError> {
            if *self.groups_error.lock().expect("lock poisoned") {
                return Err(DcCmdError::InvalidArgument("groups failed".to_string()));
            }
            if let Some(result) = self.groups_result.lock().expect("lock poisoned").take() {
                return Ok(result);
            }
            Ok(GroupsPlatformResult::Listed {
                groups: RangedItems {
                    range: Range {
                        offset: 0,
                        limit: 500,
                        total: 0,
                    },
                    items: Vec::new(),
                },
                csv: false,
            })
        }

        async fn reports(
            &self,
            _cmd: ReportsCommand,
            _auth: Option<PasswordAuth>,
        ) -> Result<ReportsPlatformResult, DcCmdError> {
            if *self.reports_error.lock().expect("lock poisoned") {
                return Err(DcCmdError::InvalidArgument("reports failed".to_string()));
            }
            if let Some(result) = self.reports_result.lock().expect("lock poisoned").take() {
                return Ok(result);
            }
            Ok(ReportsPlatformResult::OperationTypes {
                operations: dco3::eventlog::LogOperationList {
                    operation_list: Vec::new(),
                },
            })
        }
    }

    #[derive(Default)]
    struct MockUi {
        warnings: Mutex<Vec<String>>,
        successes: Mutex<Vec<String>>,
        errors: Mutex<Vec<String>>,
        infos: Mutex<Vec<String>>,
        confirms: Mutex<Vec<bool>>,
        confirm_prompts: Mutex<Vec<String>>,
        printed_nodes: Mutex<Vec<(u64, bool, bool)>>,
        printed_users_csv: Mutex<Vec<bool>>,
        printed_user_info_ids: Mutex<Vec<u64>>,
        printed_groups_csv: Mutex<Vec<bool>>,
        printed_group_users_csv: Mutex<Vec<bool>>,
        printed_events_csv: Mutex<Vec<bool>>,
        printed_permissions_csv: Mutex<Vec<bool>>,
        printed_event_types_count: Mutex<u32>,
    }

    impl MockUi {
        fn with_confirms(confirms: Vec<bool>) -> Self {
            Self {
                confirms: Mutex::new(confirms),
                ..Self::default()
            }
        }
    }

    impl Ui for Arc<MockUi> {
        fn print_node(
            &self,
            node: &Node,
            long: bool,
            human_readable: bool,
        ) -> Result<(), DcCmdError> {
            self.printed_nodes
                .lock()
                .expect("lock poisoned")
                .push((node.id, long, human_readable));
            Ok(())
        }

        fn write_success(&self, message: &str) -> Result<(), DcCmdError> {
            self.successes
                .lock()
                .expect("lock poisoned")
                .push(message.to_string());
            Ok(())
        }

        fn write_warning(&self, message: &str) -> Result<(), DcCmdError> {
            self.warnings
                .lock()
                .expect("lock poisoned")
                .push(message.to_string());
            Ok(())
        }

        fn write_error(&self, message: &str) -> Result<(), DcCmdError> {
            self.errors
                .lock()
                .expect("lock poisoned")
                .push(message.to_string());
            Ok(())
        }

        fn write_info(&self, _message: &str) -> Result<(), DcCmdError> {
            self.infos
                .lock()
                .expect("lock poisoned")
                .push(_message.to_string());
            Ok(())
        }

        fn confirm(&self, prompt: &str) -> Result<bool, DcCmdError> {
            self.confirm_prompts
                .lock()
                .expect("lock poisoned")
                .push(prompt.to_string());
            let mut confirms = self.confirms.lock().expect("lock poisoned");
            if confirms.is_empty() {
                return Ok(true);
            }
            Ok(confirms.remove(0))
        }

        fn print_users(
            &self,
            _users: &RangedItems<dco3::users::UserItem>,
            csv: bool,
        ) -> Result<(), DcCmdError> {
            self.printed_users_csv
                .lock()
                .expect("lock poisoned")
                .push(csv);
            Ok(())
        }

        fn print_user_info(
            &self,
            user: crate::app::users::models::UserInfo,
        ) -> Result<(), DcCmdError> {
            self.printed_user_info_ids
                .lock()
                .expect("lock poisoned")
                .push(user.id);
            Ok(())
        }

        fn print_groups(
            &self,
            _groups: RangedItems<dco3::groups::Group>,
            csv: bool,
        ) -> Result<(), DcCmdError> {
            self.printed_groups_csv
                .lock()
                .expect("lock poisoned")
                .push(csv);
            Ok(())
        }

        fn print_group_users(
            &self,
            _pages: &[crate::app::groups::GroupUsersPage],
            csv: bool,
        ) -> Result<(), DcCmdError> {
            self.printed_group_users_csv
                .lock()
                .expect("lock poisoned")
                .push(csv);
            Ok(())
        }

        fn print_events(
            &self,
            _events: dco3::eventlog::LogEventList,
            csv: bool,
        ) -> Result<(), DcCmdError> {
            self.printed_events_csv
                .lock()
                .expect("lock poisoned")
                .push(csv);
            Ok(())
        }

        fn print_permissions(
            &self,
            _permissions: dco3::eventlog::AuditNodeList,
            csv: bool,
        ) -> Result<(), DcCmdError> {
            self.printed_permissions_csv
                .lock()
                .expect("lock poisoned")
                .push(csv);
            Ok(())
        }

        fn print_event_types(
            &self,
            _operations: dco3::eventlog::LogOperationList,
        ) -> Result<(), DcCmdError> {
            let mut count = self
                .printed_event_types_count
                .lock()
                .expect("lock poisoned");
            *count += 1;
            Ok(())
        }
    }

    fn empty_node_list() -> NodeList {
        RangedItems {
            range: Range {
                offset: 0,
                limit: 500,
                total: 0,
            },
            items: Vec::new(),
        }
    }

    fn node(id: u64, name: &str, node_type: NodeType, parent_path: &str) -> Node {
        Node {
            id,
            reference_id: None,
            node_type,
            name: name.to_string(),
            timestamp_creation: None,
            timestamp_modification: None,
            parent_id: None,
            parent_path: Some(parent_path.to_string()),
            created_at: None,
            created_by: None,
            updated_at: None,
            updated_by: None,
            expire_at: None,
            hash: None,
            file_type: None,
            media_type: None,
            size: None,
            classification: None,
            notes: None,
            permissions: None,
            inherit_permissions: None,
            is_encrypted: None,
            encryption_info: None,
            cnt_deleted_versions: None,
            cnt_comments: None,
            cnt_upload_shares: None,
            cnt_download_shares: None,
            recycle_bin_retention_period: None,
            has_activities_log: None,
            quota: None,
            is_favorite: None,
            branch_version: None,
            media_token: None,
            is_browsable: None,
            cnt_rooms: None,
            cnt_folders: None,
            cnt_files: None,
            auth_parent_id: None,
        }
    }

    #[tokio::test]
    async fn test_rm_uses_delete_options_and_deletes_single_node() {
        let platform = Arc::new(MockPlatform::with_preparation(
            DeleteNodesPreparation::SingleNode {
                node_id: 42,
                node_name: "file.txt".to_string(),
                node_type: NodeType::File,
            },
        ));
        let ui = Arc::new(MockUi::default());
        let app = App::new(platform.clone(), ui.clone());

        let opts = CmdDeleteOptions::new(
            true,
            Some(PasswordAuth::new(
                "test".to_string(),
                SecretString::new("pw".into()),
            )),
        );

        let outcome = app
            .execute(AppCommand::Rm {
                source: "example.com/test/file.txt".to_string(),
                opts,
            })
            .await
            .unwrap();

        assert_eq!(
            *platform.prepare_recursive.lock().expect("lock poisoned"),
            vec![true]
        );
        assert_eq!(
            *platform.prepare_has_auth.lock().expect("lock poisoned"),
            vec![true]
        );
        assert_eq!(
            *platform.deleted_nodes.lock().expect("lock poisoned"),
            vec![42]
        );
        assert_eq!(
            ui.successes.lock().expect("lock poisoned").as_slice(),
            ["Node file.txt deleted."]
        );
        assert_eq!(outcome.messages.len(), 1);
        assert_eq!(outcome.messages[0].kind, OutcomeMessageKind::Success);
    }

    #[tokio::test]
    async fn test_rm_room_cancelled_writes_error() {
        let platform = Arc::new(MockPlatform::with_preparation(
            DeleteNodesPreparation::SingleNode {
                node_id: 5,
                node_name: "room-a".to_string(),
                node_type: NodeType::Room,
            },
        ));
        let ui = Arc::new(MockUi::with_confirms(vec![false]));
        let app = App::new(platform.clone(), ui.clone());

        let outcome = app
            .execute(AppCommand::Rm {
                source: "example.com/room-a".to_string(),
                opts: CmdDeleteOptions::new(true, None),
            })
            .await
            .unwrap();

        assert!(platform
            .deleted_nodes
            .lock()
            .expect("lock poisoned")
            .is_empty());
        assert_eq!(
            ui.errors.lock().expect("lock poisoned").as_slice(),
            ["Deleting room not confirmed."]
        );
        assert_eq!(outcome.messages.len(), 1);
        assert_eq!(outcome.messages[0].kind, OutcomeMessageKind::Error);
    }

    #[tokio::test]
    async fn test_config_auth_rm_cancelled_before_platform_call() {
        let platform = Arc::new(MockPlatform::with_config_result(
            ConfigPlatformResult::AuthTokenRemoved {
                base_url: "https://example.com".to_string(),
            },
        ));
        let ui = Arc::new(MockUi::with_confirms(vec![false]));
        let app = App::new(platform.clone(), ui.clone());

        let outcome = app
            .execute(AppCommand::Config {
                cmd: ConfigCommand::Auth {
                    cmd: ConfigAuthCommand::Rm {
                        target: "example.com".to_string(),
                    },
                },
            })
            .await
            .unwrap();

        assert_eq!(
            ui.confirm_prompts.lock().expect("lock poisoned").as_slice(),
            ["Are you sure you want to remove the token?"]
        );
        assert!(ui.successes.lock().expect("lock poisoned").is_empty());
        assert!(outcome.messages.is_empty());
        assert!(platform
            .config_result
            .lock()
            .expect("lock poisoned")
            .is_some());
    }

    #[tokio::test]
    async fn test_config_missing_token_writes_error_outcome() {
        let platform = Arc::new(MockPlatform::with_config_result(
            ConfigPlatformResult::MissingToken {
                base_url: "https://example.com".to_string(),
            },
        ));
        let ui = Arc::new(MockUi::default());
        let app = App::new(platform, ui.clone());

        let outcome = app
            .execute(AppCommand::Config {
                cmd: ConfigCommand::Auth {
                    cmd: ConfigAuthCommand::Ls {
                        target: "example.com".to_string(),
                    },
                },
            })
            .await
            .unwrap();

        assert_eq!(
            ui.errors.lock().expect("lock poisoned").as_slice(),
            ["No token found for this DRACOON url: https://example.com."]
        );
        assert_eq!(outcome.messages.len(), 1);
        assert_eq!(outcome.messages[0].kind, OutcomeMessageKind::Error);
    }

    #[tokio::test]
    async fn test_config_crypto_missing_secret_writes_error_outcome() {
        let platform = Arc::new(MockPlatform::with_config_result(
            ConfigPlatformResult::MissingCryptoSecret,
        ));
        let ui = Arc::new(MockUi::default());
        let app = App::new(platform, ui.clone());

        let outcome = app
            .execute(AppCommand::Config {
                cmd: ConfigCommand::Crypto {
                    cmd: ConfigCryptoCommand::Ls {
                        target: "example.com".to_string(),
                    },
                },
            })
            .await
            .unwrap();

        assert_eq!(
            ui.errors.lock().expect("lock poisoned").as_slice(),
            ["No encryption secret found."]
        );
        assert_eq!(outcome.messages.len(), 1);
        assert_eq!(outcome.messages[0].kind, OutcomeMessageKind::Error);
    }

    #[tokio::test]
    async fn test_mkdir_alias_writes_deprecation_warning() {
        let platform = Arc::new(MockPlatform::with_create_message("Room team-a created."));
        let ui = Arc::new(MockUi::default());
        let app = App::new(platform, ui.clone());

        let outcome = app
            .execute(AppCommand::Mkdir {
                source: "example.com/rooms/team-a".to_string(),
                opts: CmdCreateContainerOptions::new(
                    CreateContainerType::Room,
                    Some(2),
                    None,
                    None,
                    None,
                    false,
                ),
                deprecated_alias: true,
            })
            .await
            .unwrap();

        assert_eq!(
            ui.warnings.lock().expect("lock poisoned").as_slice(),
            [MKROOM_DEPRECATION_WARNING]
        );
        assert_eq!(
            ui.successes.lock().expect("lock poisoned").as_slice(),
            ["Room team-a created."]
        );
        assert_eq!(outcome.messages.len(), 2);
        assert_eq!(outcome.messages[0].kind, OutcomeMessageKind::Warning);
        assert_eq!(outcome.messages[1].kind, OutcomeMessageKind::Success);
    }

    #[tokio::test]
    async fn test_mkdir_default_folder_writes_success_without_warning() {
        let platform = Arc::new(MockPlatform::with_create_message("Folder team-a created."));
        let ui = Arc::new(MockUi::default());
        let app = App::new(platform, ui.clone());

        let outcome = app
            .execute(AppCommand::Mkdir {
                source: "example.com/rooms/team-a".to_string(),
                opts: CmdCreateContainerOptions::new(
                    CreateContainerType::Folder,
                    None,
                    Some("notes".to_string()),
                    None,
                    None,
                    false,
                ),
                deprecated_alias: false,
            })
            .await
            .unwrap();

        assert!(ui.warnings.lock().expect("lock poisoned").is_empty());
        assert_eq!(
            ui.successes.lock().expect("lock poisoned").as_slice(),
            ["Folder team-a created."]
        );
        assert_eq!(outcome.messages.len(), 1);
        assert_eq!(outcome.messages[0].kind, OutcomeMessageKind::Success);
    }

    #[tokio::test]
    async fn test_ls_collects_info_outcome() {
        let platform = Arc::new(MockPlatform::default());
        let ui = Arc::new(MockUi::default());
        let app = App::new(platform, ui);

        let outcome = app
            .execute(AppCommand::Ls {
                source: "example.com/test".to_string(),
                opts: CmdListNodesOptions::new(
                    ListOptions::new(None, None, None, false, false),
                    false,
                    false,
                    false,
                    None,
                ),
            })
            .await
            .unwrap();

        assert_eq!(outcome.messages.len(), 1);
        assert_eq!(outcome.messages[0].kind, OutcomeMessageKind::Info);
    }

    #[tokio::test]
    async fn test_ls_passes_render_flags_to_ui() {
        let platform = Arc::new(MockPlatform::with_list_result(ListNodesResult {
            list: RangedItems {
                range: Range {
                    offset: 0,
                    limit: 500,
                    total: 1,
                },
                items: vec![node(11, "file-a.txt", NodeType::File, "/test/")],
            },
            node_path: Some("/test/".to_string()),
        }));
        let ui = Arc::new(MockUi::default());
        let app = App::new(platform, ui.clone());

        let outcome = app
            .execute(AppCommand::Ls {
                source: "example.com/test".to_string(),
                opts: CmdListNodesOptions::new(
                    ListOptions::new(None, None, None, false, false),
                    true,
                    true,
                    false,
                    None,
                ),
            })
            .await
            .unwrap();

        assert_eq!(
            ui.printed_nodes.lock().expect("lock poisoned").as_slice(),
            &[(11, true, true)]
        );
        assert_eq!(outcome.messages.len(), 1);
        assert_eq!(outcome.messages[0].kind, OutcomeMessageKind::Info);
        assert_eq!(outcome.messages[0].text, "Listed 1 node(s) in /test/.");
    }

    #[tokio::test]
    async fn test_ls_propagates_platform_error() {
        let platform = Arc::new(MockPlatform::with_list_error());
        let ui = Arc::new(MockUi::default());
        let app = App::new(platform, ui);

        let result = app
            .execute(AppCommand::Ls {
                source: "example.com/test".to_string(),
                opts: CmdListNodesOptions::new(
                    ListOptions::new(None, None, None, false, false),
                    false,
                    false,
                    false,
                    None,
                ),
            })
            .await;

        assert!(matches!(
            result,
            Err(DcCmdError::InvalidArgument(msg)) if msg == "list failed"
        ));
    }

    #[tokio::test]
    async fn test_cp_writes_success_outcome() {
        let platform = Arc::new(MockPlatform::with_copy_result(2, "/test/", "/target/"));
        let ui = Arc::new(MockUi::default());
        let app = App::new(platform, ui.clone());

        let outcome = app
            .execute(AppCommand::Cp {
                source: "example.com/test/*".to_string(),
                target: "/target/".to_string(),
                opts: CmdCopyOptions::new(None),
            })
            .await
            .unwrap();

        assert_eq!(
            ui.successes.lock().expect("lock poisoned").as_slice(),
            ["Copied 2 node(s) from /test/ to /target/."]
        );
        assert_eq!(outcome.messages.len(), 1);
        assert_eq!(outcome.messages[0].kind, OutcomeMessageKind::Success);
    }

    #[tokio::test]
    async fn test_cp_propagates_platform_error() {
        let platform = Arc::new(MockPlatform::with_copy_error());
        let ui = Arc::new(MockUi::default());
        let app = App::new(platform, ui);

        let result = app
            .execute(AppCommand::Cp {
                source: "example.com/test/*".to_string(),
                target: "/target/".to_string(),
                opts: CmdCopyOptions::new(None),
            })
            .await;

        assert!(matches!(
            result,
            Err(DcCmdError::InvalidArgument(msg)) if msg == "copy failed"
        ));
    }

    #[tokio::test]
    async fn test_download_delegates_to_platform() {
        let platform = Arc::new(MockPlatform::default());
        let ui = Arc::new(MockUi::default());
        let app = App::new(platform, ui.clone());

        let outcome = app
            .execute(AppCommand::Download {
                source: "example.com/room/file.txt".to_string(),
                target: "/tmp/file.txt".to_string(),
                opts: CmdDownloadOptions::new(false, None, None, None, None, false),
            })
            .await
            .unwrap();

        assert_eq!(
            ui.successes.lock().expect("lock poisoned").as_slice(),
            ["Downloaded 1 item(s) to /tmp/file.txt."]
        );
        assert_eq!(outcome.messages.len(), 1);
        assert_eq!(outcome.messages[0].kind, OutcomeMessageKind::Success);
        assert_eq!(
            app.platform
                .download_calls
                .lock()
                .expect("lock poisoned")
                .as_slice(),
            &[(
                "example.com/room/file.txt".to_string(),
                "/tmp/file.txt".to_string()
            )]
        );
    }

    #[tokio::test]
    async fn test_transfer_delegates_to_platform() {
        let platform = Arc::new(MockPlatform::default());
        let ui = Arc::new(MockUi::default());
        let app = App::new(platform.clone(), ui);

        app.execute(AppCommand::Transfer {
            source: "source.example.com/room/file.txt".to_string(),
            target: "target.example.com/room/".to_string(),
            opts: CmdTransferOptions::new(true, true, true, Some(2), Some("pw".to_string())),
        })
        .await
        .unwrap();

        let calls = platform.transfer_calls.lock().expect("lock poisoned");
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "source.example.com/room/file.txt");
        assert_eq!(calls[0].1, "target.example.com/room/");
        assert!(calls[0].2.overwrite);
        assert!(calls[0].2.keep_share_links);
        assert!(calls[0].2.share);
        assert_eq!(calls[0].2.classification, Some(2));
        assert_eq!(calls[0].2.share_password, Some("pw".to_string()));
    }

    #[tokio::test]
    async fn test_upload_share_message_uses_ui_success_channel() {
        let platform = Arc::new(MockPlatform::with_upload_message(
            "Shared file.txt.\n▶︎▶︎ https://example.com/public/download-shares/abc",
        ));
        let ui = Arc::new(MockUi::default());
        let app = App::new(platform, ui.clone());

        let outcome = app
            .execute(AppCommand::Upload {
                source: "/tmp/file.txt".to_string(),
                target: "example.com/room".to_string(),
                opts: CmdUploadOptions::new(
                    false, false, false, false, true, None, None, None, None, None,
                ),
            })
            .await
            .unwrap();

        assert_eq!(
            ui.successes.lock().expect("lock poisoned").as_slice(),
            ["Shared file.txt.\n▶︎▶︎ https://example.com/public/download-shares/abc"]
        );
        assert_eq!(outcome.messages.len(), 1);
        assert_eq!(outcome.messages[0].kind, OutcomeMessageKind::Success);
    }

    #[tokio::test]
    async fn test_transfer_share_message_uses_ui_success_channel() {
        let platform = Arc::new(MockPlatform::with_transfer_message(
            "Shared file.txt.\n▶︎▶︎ https://example.com/public/download-shares/abc",
        ));
        let ui = Arc::new(MockUi::default());
        let app = App::new(platform, ui.clone());

        let outcome = app
            .execute(AppCommand::Transfer {
                source: "source.example.com/room/file.txt".to_string(),
                target: "target.example.com/room/".to_string(),
                opts: CmdTransferOptions::new(false, false, true, None, None),
            })
            .await
            .unwrap();

        assert_eq!(
            ui.successes.lock().expect("lock poisoned").as_slice(),
            ["Shared file.txt.\n▶︎▶︎ https://example.com/public/download-shares/abc"]
        );
        assert_eq!(outcome.messages.len(), 1);
        assert_eq!(outcome.messages[0].kind, OutcomeMessageKind::Success);
    }

    #[tokio::test]
    async fn test_download_partial_failure_writes_warning_and_details() {
        let platform = Arc::new(MockPlatform::with_download_outcome(
            DownloadOutcome::from_failures(
                "/tmp",
                4,
                2,
                vec![
                    DownloadFailure {
                        node_id: Some(11),
                        node_name: "a.txt".to_string(),
                        target: "/tmp/a.txt".to_string(),
                        reason: "network".to_string(),
                    },
                    DownloadFailure {
                        node_id: Some(12),
                        node_name: "b.txt".to_string(),
                        target: "/tmp/b.txt".to_string(),
                        reason: "permission denied".to_string(),
                    },
                ],
            ),
        ));
        let ui = Arc::new(MockUi::default());
        let app = App::new(platform, ui.clone());

        let outcome = app
            .execute(AppCommand::Download {
                source: "example.com/room/*".to_string(),
                target: "/tmp/".to_string(),
                opts: CmdDownloadOptions::new(false, None, None, None, None, false),
            })
            .await
            .unwrap();

        assert_eq!(
            ui.warnings.lock().expect("lock poisoned").as_slice(),
            ["Downloaded 2/4 item(s) to /tmp; 2 failed."]
        );
        assert_eq!(
            ui.infos.lock().expect("lock poisoned").as_slice(),
            [
                "Failed a.txt -> /tmp/a.txt (network)",
                "Failed b.txt -> /tmp/b.txt (permission denied)"
            ]
        );
        assert_eq!(outcome.messages[0].kind, OutcomeMessageKind::Warning);
    }

    #[tokio::test]
    async fn test_download_all_failed_writes_error_and_truncated_details() {
        let platform = Arc::new(MockPlatform::with_download_outcome(
            DownloadOutcome::from_failures(
                "/tmp",
                5,
                0,
                vec![
                    DownloadFailure {
                        node_id: Some(1),
                        node_name: "a.txt".to_string(),
                        target: "/tmp/a.txt".to_string(),
                        reason: "io".to_string(),
                    },
                    DownloadFailure {
                        node_id: Some(2),
                        node_name: "b.txt".to_string(),
                        target: "/tmp/b.txt".to_string(),
                        reason: "io".to_string(),
                    },
                    DownloadFailure {
                        node_id: Some(3),
                        node_name: "c.txt".to_string(),
                        target: "/tmp/c.txt".to_string(),
                        reason: "io".to_string(),
                    },
                    DownloadFailure {
                        node_id: Some(4),
                        node_name: "d.txt".to_string(),
                        target: "/tmp/d.txt".to_string(),
                        reason: "io".to_string(),
                    },
                    DownloadFailure {
                        node_id: Some(5),
                        node_name: "e.txt".to_string(),
                        target: "/tmp/e.txt".to_string(),
                        reason: "io".to_string(),
                    },
                ],
            ),
        ));
        let ui = Arc::new(MockUi::default());
        let app = App::new(platform, ui.clone());

        let outcome = app
            .execute(AppCommand::Download {
                source: "example.com/room/*".to_string(),
                target: "/tmp/".to_string(),
                opts: CmdDownloadOptions::new(false, None, None, None, None, false),
            })
            .await
            .unwrap();

        assert_eq!(
            ui.errors.lock().expect("lock poisoned").as_slice(),
            ["Downloaded 0/5 item(s) to /tmp; 5 failed."]
        );
        assert_eq!(
            ui.infos.lock().expect("lock poisoned").as_slice(),
            [
                "Failed a.txt -> /tmp/a.txt (io)",
                "Failed b.txt -> /tmp/b.txt (io)",
                "Failed c.txt -> /tmp/c.txt (io)",
                "... and 2 more failed item(s)."
            ]
        );
        assert_eq!(outcome.messages[0].kind, OutcomeMessageKind::Error);
    }

    #[tokio::test]
    async fn test_download_propagates_platform_error() {
        let platform = Arc::new(MockPlatform::with_download_error());
        let ui = Arc::new(MockUi::default());
        let app = App::new(platform, ui);

        let result = app
            .execute(AppCommand::Download {
                source: "example.com/room/file.txt".to_string(),
                target: "/tmp/file.txt".to_string(),
                opts: CmdDownloadOptions::new(false, None, None, None, None, false),
            })
            .await;

        assert!(matches!(
            result,
            Err(DcCmdError::InvalidArgument(msg)) if msg == "download failed"
        ));
    }

    #[tokio::test]
    async fn test_transfer_propagates_platform_error() {
        let platform = Arc::new(MockPlatform::with_transfer_error());
        let ui = Arc::new(MockUi::default());
        let app = App::new(platform, ui);

        let result = app
            .execute(AppCommand::Transfer {
                source: "source.example.com/room/file.txt".to_string(),
                target: "target.example.com/room/".to_string(),
                opts: CmdTransferOptions::new(false, false, false, None, None),
            })
            .await;

        assert!(matches!(
            result,
            Err(DcCmdError::InvalidArgument(msg)) if msg == "transfer failed"
        ));
    }

    #[tokio::test]
    async fn test_rm_invalid_search_requires_recursive_writes_error() {
        let platform = Arc::new(MockPlatform::with_preparation(
            DeleteNodesPreparation::InvalidSearchRequiresRecursive,
        ));
        let ui = Arc::new(MockUi::default());
        let app = App::new(platform.clone(), ui.clone());

        let outcome = app
            .execute(AppCommand::Rm {
                source: "example.com/test/*".to_string(),
                opts: CmdDeleteOptions::new(false, None),
            })
            .await
            .unwrap();

        assert_eq!(
            ui.errors.lock().expect("lock poisoned").as_slice(),
            ["Deleting search results not allowed. Use --recursive flag to delete recursively."]
        );
        assert!(platform
            .deleted_nodes
            .lock()
            .expect("lock poisoned")
            .is_empty());
        assert!(platform
            .deleted_batch_nodes
            .lock()
            .expect("lock poisoned")
            .is_empty());
        assert_eq!(outcome.messages.len(), 1);
        assert_eq!(outcome.messages[0].kind, OutcomeMessageKind::Error);
    }

    #[tokio::test]
    async fn test_rm_container_requires_recursive_writes_error() {
        let platform = Arc::new(MockPlatform::with_preparation(
            DeleteNodesPreparation::ContainerRequiresRecursive,
        ));
        let ui = Arc::new(MockUi::default());
        let app = App::new(platform.clone(), ui.clone());

        let outcome = app
            .execute(AppCommand::Rm {
                source: "example.com/test/folder".to_string(),
                opts: CmdDeleteOptions::new(false, None),
            })
            .await
            .unwrap();

        assert_eq!(
            ui.errors.lock().expect("lock poisoned").as_slice(),
            ["Deleting non-empty folder or room not allowed. Use --recursive flag to delete recursively."]
        );
        assert!(platform
            .deleted_nodes
            .lock()
            .expect("lock poisoned")
            .is_empty());
        assert!(platform
            .deleted_batch_nodes
            .lock()
            .expect("lock poisoned")
            .is_empty());
        assert_eq!(outcome.messages.len(), 1);
        assert_eq!(outcome.messages[0].kind, OutcomeMessageKind::Error);
    }

    #[tokio::test]
    async fn test_rm_search_confirmed_deletes_batch() {
        let platform = Arc::new(MockPlatform::with_preparation(
            DeleteNodesPreparation::Search {
                node_ids: vec![11, 12, 13],
            },
        ));
        let ui = Arc::new(MockUi::with_confirms(vec![true]));
        let app = App::new(platform.clone(), ui);

        let outcome = app
            .execute(AppCommand::Rm {
                source: "example.com/test/*".to_string(),
                opts: CmdDeleteOptions::new(true, None),
            })
            .await
            .unwrap();

        assert_eq!(
            platform
                .deleted_batch_nodes
                .lock()
                .expect("lock poisoned")
                .as_slice(),
            &[vec![11, 12, 13]]
        );
        assert!(outcome.messages.is_empty());
    }

    #[tokio::test]
    async fn test_rm_search_cancelled_skips_batch_delete() {
        let platform = Arc::new(MockPlatform::with_preparation(
            DeleteNodesPreparation::Search {
                node_ids: vec![11, 12, 13],
            },
        ));
        let ui = Arc::new(MockUi::with_confirms(vec![false]));
        let app = App::new(platform.clone(), ui);

        let outcome = app
            .execute(AppCommand::Rm {
                source: "example.com/test/*".to_string(),
                opts: CmdDeleteOptions::new(true, None),
            })
            .await
            .unwrap();

        assert!(platform
            .deleted_batch_nodes
            .lock()
            .expect("lock poisoned")
            .is_empty());
        assert!(outcome.messages.is_empty());
    }

    #[tokio::test]
    async fn test_rm_room_confirmed_deletes_single_node() {
        let platform = Arc::new(MockPlatform::with_preparation(
            DeleteNodesPreparation::SingleNode {
                node_id: 77,
                node_name: "room-b".to_string(),
                node_type: NodeType::Room,
            },
        ));
        let ui = Arc::new(MockUi::with_confirms(vec![true]));
        let app = App::new(platform.clone(), ui.clone());

        let outcome = app
            .execute(AppCommand::Rm {
                source: "example.com/room-b".to_string(),
                opts: CmdDeleteOptions::new(true, None),
            })
            .await
            .unwrap();

        assert_eq!(
            platform
                .deleted_nodes
                .lock()
                .expect("lock poisoned")
                .as_slice(),
            &[77]
        );
        assert_eq!(
            ui.successes.lock().expect("lock poisoned").as_slice(),
            ["Node room-b deleted."]
        );
        assert_eq!(outcome.messages.len(), 1);
        assert_eq!(outcome.messages[0].kind, OutcomeMessageKind::Success);
    }

    #[tokio::test]
    async fn test_mkdir_propagates_platform_error() {
        let platform = Arc::new(MockPlatform::with_create_error());
        let ui = Arc::new(MockUi::default());
        let app = App::new(platform, ui);

        let result = app
            .execute(AppCommand::Mkdir {
                source: "example.com/rooms/team-a".to_string(),
                opts: CmdCreateContainerOptions::new(
                    CreateContainerType::Folder,
                    None,
                    None,
                    None,
                    None,
                    false,
                ),
                deprecated_alias: false,
            })
            .await;

        assert!(matches!(
            result,
            Err(DcCmdError::InvalidArgument(msg)) if msg == "create container failed"
        ));
    }

    #[tokio::test]
    async fn test_users_created_writes_success_and_info() {
        let platform = Arc::new(MockPlatform::with_users_result(
            UsersPlatformResult::Created {
                user_name: "alice".to_string(),
                user_id: 42,
                auth_method: "basic".to_string(),
            },
        ));
        let ui = Arc::new(MockUi::default());
        let app = App::new(platform, ui.clone());

        let outcome = app
            .execute(AppCommand::Users {
                auth: None,
                cmd: UsersCommand::Ls {
                    target: "example.com".to_string(),
                    filter: None,
                    offset: None,
                    limit: None,
                    all: false,
                    csv: false,
                },
            })
            .await
            .unwrap();

        assert_eq!(
            ui.successes.lock().expect("lock poisoned").as_slice(),
            ["User alice created"]
        );
        assert_eq!(ui.infos.lock().expect("lock poisoned").len(), 3);
        assert_eq!(outcome.messages.len(), 4);
    }

    #[tokio::test]
    async fn test_users_listed_calls_ui_printer() {
        let users = RangedItems {
            range: Range {
                offset: 0,
                limit: 500,
                total: 0,
            },
            items: Vec::new(),
        };
        let platform = Arc::new(MockPlatform::with_users_result(
            UsersPlatformResult::Listed { users, csv: true },
        ));
        let ui = Arc::new(MockUi::default());
        let app = App::new(platform, ui.clone());

        let outcome = app
            .execute(AppCommand::Users {
                auth: None,
                cmd: UsersCommand::Ls {
                    target: "example.com".to_string(),
                    filter: None,
                    offset: None,
                    limit: None,
                    all: false,
                    csv: true,
                },
            })
            .await
            .unwrap();

        assert_eq!(
            ui.printed_users_csv
                .lock()
                .expect("lock poisoned")
                .as_slice(),
            &[true]
        );
        assert!(outcome.messages.is_empty());
    }

    #[tokio::test]
    async fn test_users_removed_writes_success() {
        let platform = Arc::new(MockPlatform::with_users_result(
            UsersPlatformResult::Removed {
                message: "User alice deleted".to_string(),
            },
        ));
        let ui = Arc::new(MockUi::default());
        let app = App::new(platform, ui.clone());

        let outcome = app
            .execute(AppCommand::Users {
                auth: None,
                cmd: UsersCommand::Rm {
                    target: "example.com".to_string(),
                    user_name: Some("alice".to_string()),
                    user_id: None,
                },
            })
            .await
            .unwrap();

        assert_eq!(
            ui.successes.lock().expect("lock poisoned").as_slice(),
            ["User alice deleted"]
        );
        assert_eq!(outcome.messages.len(), 1);
        assert_eq!(outcome.messages[0].kind, OutcomeMessageKind::Success);
    }

    #[tokio::test]
    async fn test_users_imported_writes_summary_messages() {
        let platform = Arc::new(MockPlatform::with_users_result(
            UsersPlatformResult::Imported(crate::app::users::ImportUsersResult {
                imported: 3,
                failed: 1,
                total: 4,
            }),
        ));
        let ui = Arc::new(MockUi::default());
        let app = App::new(platform, ui.clone());

        let outcome = app
            .execute(AppCommand::Users {
                auth: None,
                cmd: UsersCommand::Import {
                    target: "example.com".to_string(),
                    source: "/tmp/users.csv".to_string(),
                    oidc_id: None,
                },
            })
            .await
            .unwrap();

        assert_eq!(
            ui.successes.lock().expect("lock poisoned").as_slice(),
            ["3 users imported"]
        );
        assert_eq!(
            ui.infos.lock().expect("lock poisoned").as_slice(),
            ["Import summary: 4 total, 1 failed."]
        );
        assert_eq!(outcome.messages.len(), 2);
    }

    #[tokio::test]
    async fn test_users_info_calls_ui_printer() {
        let platform = Arc::new(MockPlatform::with_users_result(UsersPlatformResult::Info {
            user: crate::app::users::models::UserInfo {
                id: 7,
                first_name: "Alice".to_string(),
                last_name: "Admin".to_string(),
                username: "alice".to_string(),
                email: Some("alice@example.com".to_string()),
                expire_at: None,
                is_locked: false,
                last_login_at: None,
            },
        }));
        let ui = Arc::new(MockUi::default());
        let app = App::new(platform, ui.clone());

        let outcome = app
            .execute(AppCommand::Users {
                auth: None,
                cmd: UsersCommand::Info {
                    target: "example.com".to_string(),
                    user_name: Some("alice".to_string()),
                    user_id: None,
                },
            })
            .await
            .unwrap();

        assert_eq!(
            ui.printed_user_info_ids
                .lock()
                .expect("lock poisoned")
                .as_slice(),
            &[7]
        );
        assert!(outcome.messages.is_empty());
    }

    #[tokio::test]
    async fn test_users_switch_auth_writes_success() {
        let platform = Arc::new(MockPlatform::with_users_result(
            UsersPlatformResult::SwitchedAuth(crate::app::users::SwitchAuthResult {
                current_method: "basic".to_string(),
                new_method: "openid".to_string(),
                updated_users: 5,
            }),
        ));
        let ui = Arc::new(MockUi::default());
        let app = App::new(platform, ui.clone());

        let outcome = app
            .execute(AppCommand::Users {
                auth: None,
                cmd: UsersCommand::SwitchAuth {
                    target: "example.com".to_string(),
                    current_method: "basic".to_string(),
                    new_method: "openid".to_string(),
                    current_oidc_id: None,
                    new_oidc_id: None,
                    current_ad_id: None,
                    new_ad_id: None,
                    filter: None,
                    login: None,
                },
            })
            .await
            .unwrap();

        assert_eq!(
            ui.successes.lock().expect("lock poisoned").as_slice(),
            ["Switched auth method from basic to openid for 5 users."]
        );
        assert_eq!(outcome.messages.len(), 1);
    }

    #[tokio::test]
    async fn test_users_enforce_mfa_writes_success_and_info() {
        let platform = Arc::new(MockPlatform::with_users_result(
            UsersPlatformResult::EnforcedMfa(crate::app::users::EnforceMfaResult {
                success_count: 8,
                failed_count: 2,
            }),
        ));
        let ui = Arc::new(MockUi::default());
        let app = App::new(platform, ui.clone());

        let outcome = app
            .execute(AppCommand::Users {
                auth: None,
                cmd: UsersCommand::EnforceMfa {
                    target: "example.com".to_string(),
                    auth_method: None,
                    filter: None,
                    auth_method_id: None,
                    group_id: None,
                },
            })
            .await
            .unwrap();

        assert_eq!(
            ui.successes.lock().expect("lock poisoned").as_slice(),
            ["Enforced MFA for 8 users successfully."]
        );
        assert_eq!(
            ui.infos.lock().expect("lock poisoned").as_slice(),
            ["Failed MFA updates: 2"]
        );
        assert_eq!(outcome.messages.len(), 2);
    }

    #[tokio::test]
    async fn test_users_propagates_platform_error() {
        let platform = Arc::new(MockPlatform::with_users_error());
        let ui = Arc::new(MockUi::default());
        let app = App::new(platform, ui);

        let result = app
            .execute(AppCommand::Users {
                auth: None,
                cmd: UsersCommand::Ls {
                    target: "example.com".to_string(),
                    filter: None,
                    offset: None,
                    limit: None,
                    all: false,
                    csv: false,
                },
            })
            .await;

        assert!(matches!(
            result,
            Err(DcCmdError::InvalidArgument(msg)) if msg == "users failed"
        ));
    }

    #[tokio::test]
    async fn test_groups_result_variants_are_handled() {
        let groups = RangedItems {
            range: Range {
                offset: 0,
                limit: 500,
                total: 0,
            },
            items: Vec::new(),
        };

        let platform = Arc::new(MockPlatform::with_groups_result(
            GroupsPlatformResult::Listed { groups, csv: true },
        ));
        let ui = Arc::new(MockUi::default());
        let app = App::new(platform, ui.clone());

        let outcome = app
            .execute(AppCommand::Groups {
                auth: None,
                cmd: GroupsCommand::Ls {
                    target: "example.com".to_string(),
                    filter: None,
                    offset: None,
                    limit: None,
                    all: false,
                    csv: true,
                },
            })
            .await
            .unwrap();

        assert_eq!(
            ui.printed_groups_csv
                .lock()
                .expect("lock poisoned")
                .as_slice(),
            &[true]
        );
        assert!(outcome.messages.is_empty());
    }

    #[tokio::test]
    async fn test_groups_created_and_removed_write_success() {
        let platform = Arc::new(MockPlatform::with_groups_result(
            GroupsPlatformResult::Created {
                group_name: "ops".to_string(),
                group_id: 77,
            },
        ));
        let ui = Arc::new(MockUi::default());
        let app = App::new(platform, ui.clone());

        let outcome = app
            .execute(AppCommand::Groups {
                auth: None,
                cmd: GroupsCommand::Create {
                    target: "example.com".to_string(),
                    name: "ops".to_string(),
                },
            })
            .await
            .unwrap();

        assert_eq!(
            ui.successes.lock().expect("lock poisoned").as_slice(),
            ["Group ops (77) created"]
        );
        assert_eq!(outcome.messages.len(), 1);
    }

    #[tokio::test]
    async fn test_groups_users_listed_calls_ui_printer() {
        let platform = Arc::new(MockPlatform::with_groups_result(
            GroupsPlatformResult::UsersListed {
                pages: Vec::new(),
                csv: false,
            },
        ));
        let ui = Arc::new(MockUi::default());
        let app = App::new(platform, ui.clone());

        let outcome = app
            .execute(AppCommand::Groups {
                auth: None,
                cmd: GroupsCommand::Users {
                    cmd: crate::command::GroupsUsersCommand::Ls {
                        target: "example.com".to_string(),
                        filter: None,
                        offset: None,
                        limit: None,
                        all: false,
                        csv: false,
                    },
                },
            })
            .await
            .unwrap();

        assert_eq!(
            ui.printed_group_users_csv
                .lock()
                .expect("lock poisoned")
                .as_slice(),
            &[false]
        );
        assert!(outcome.messages.is_empty());
    }

    #[tokio::test]
    async fn test_groups_user_added_writes_success() {
        let platform = Arc::new(MockPlatform::with_groups_result(
            GroupsPlatformResult::UserAdded {
                group_id: 9,
                user_id: 21,
            },
        ));
        let ui = Arc::new(MockUi::default());
        let app = App::new(platform, ui.clone());

        let outcome = app
            .execute(AppCommand::Groups {
                auth: None,
                cmd: GroupsCommand::Users {
                    cmd: crate::command::GroupsUsersCommand::Add {
                        target: "example.com".to_string(),
                        group_name: None,
                        group_id: Some(9),
                        user_name: None,
                        user_id: Some(21),
                    },
                },
            })
            .await
            .unwrap();

        assert_eq!(
            ui.successes.lock().expect("lock poisoned").as_slice(),
            ["User 21 added to group 9"]
        );
        assert_eq!(outcome.messages.len(), 1);
    }

    #[tokio::test]
    async fn test_groups_propagates_platform_error() {
        let platform = Arc::new(MockPlatform::with_groups_error());
        let ui = Arc::new(MockUi::default());
        let app = App::new(platform, ui);

        let result = app
            .execute(AppCommand::Groups {
                auth: None,
                cmd: GroupsCommand::Ls {
                    target: "example.com".to_string(),
                    filter: None,
                    offset: None,
                    limit: None,
                    all: false,
                    csv: false,
                },
            })
            .await;

        assert!(matches!(
            result,
            Err(DcCmdError::InvalidArgument(msg)) if msg == "groups failed"
        ));
    }

    #[tokio::test]
    async fn test_reports_events_calls_ui_printer() {
        let events: dco3::eventlog::LogEventList = RangedItems {
            range: Range {
                offset: 0,
                limit: 500,
                total: 0,
            },
            items: Vec::new(),
        };
        let platform = Arc::new(MockPlatform::with_reports_result(
            ReportsPlatformResult::Events { events, csv: true },
        ));
        let ui = Arc::new(MockUi::default());
        let app = App::new(platform, ui.clone());

        let outcome = app
            .execute(AppCommand::Reports {
                auth: None,
                cmd: ReportsCommand::Events {
                    target: "example.com".to_string(),
                    filter: None,
                    offset: None,
                    limit: None,
                    all: false,
                    csv: true,
                    operation_type: None,
                    user_id: None,
                    status: None,
                    start_date: None,
                    end_date: None,
                },
            })
            .await
            .unwrap();

        assert_eq!(
            ui.printed_events_csv
                .lock()
                .expect("lock poisoned")
                .as_slice(),
            &[true]
        );
        assert!(outcome.messages.is_empty());
    }

    #[tokio::test]
    async fn test_reports_permissions_and_operation_types_call_ui() {
        let permissions: dco3::eventlog::AuditNodeList = Vec::new();
        let platform = Arc::new(MockPlatform::with_reports_result(
            ReportsPlatformResult::Permissions {
                permissions,
                csv: false,
            },
        ));
        let ui = Arc::new(MockUi::default());
        let app = App::new(platform, ui.clone());

        let outcome = app
            .execute(AppCommand::Reports {
                auth: None,
                cmd: ReportsCommand::Permissions {
                    target: "example.com".to_string(),
                    filter: None,
                    offset: None,
                    limit: None,
                    all: false,
                    csv: false,
                },
            })
            .await
            .unwrap();

        assert_eq!(
            ui.printed_permissions_csv
                .lock()
                .expect("lock poisoned")
                .as_slice(),
            &[false]
        );
        assert!(outcome.messages.is_empty());

        let platform = Arc::new(MockPlatform::with_reports_result(
            ReportsPlatformResult::OperationTypes {
                operations: dco3::eventlog::LogOperationList {
                    operation_list: Vec::new(),
                },
            },
        ));
        let ui = Arc::new(MockUi::default());
        let app = App::new(platform, ui.clone());

        let outcome = app
            .execute(AppCommand::Reports {
                auth: None,
                cmd: ReportsCommand::OperationTypes {
                    target: "example.com".to_string(),
                },
            })
            .await
            .unwrap();

        assert_eq!(
            *ui.printed_event_types_count.lock().expect("lock poisoned"),
            1
        );
        assert!(outcome.messages.is_empty());
    }

    #[tokio::test]
    async fn test_reports_propagates_platform_error() {
        let platform = Arc::new(MockPlatform::with_reports_error());
        let ui = Arc::new(MockUi::default());
        let app = App::new(platform, ui);

        let result = app
            .execute(AppCommand::Reports {
                auth: None,
                cmd: ReportsCommand::OperationTypes {
                    target: "example.com".to_string(),
                },
            })
            .await;

        assert!(matches!(
            result,
            Err(DcCmdError::InvalidArgument(msg)) if msg == "reports failed"
        ));
    }
}

use console::Term;
use dco3::{
    eventlog::{AuditNodeList, LogEventList, LogOperationList},
    groups::Group,
    nodes::models::Node,
    users::UserItem,
    RangedItems,
};
use dialoguer::Confirm;

use crate::cli::format::{groups as groups_print, reports as reports_print, users as users_print};
use crate::{
    app::{groups::GroupUsersPage, users::models::UserInfo, Ui},
    core::{
        models::DcCmdError,
        utils::strings::{format_error_message, format_success_message, print_node},
    },
};

pub struct TermUi {
    out: Term,
    err: Term,
}

impl TermUi {
    pub fn new(out: Term, err: Term) -> Self {
        Self { out, err }
    }

    pub fn write_version(term: &Term) -> Result<(), DcCmdError> {
        term.write_line(Self::version_text().as_str())
            .map_err(|_| DcCmdError::IoError)
    }

    pub fn write_cli_error(term: &Term, err: &DcCmdError) -> Result<(), DcCmdError> {
        let message = format_error_message(&Self::cli_error_message(err));
        term.write_line(&message).map_err(|_| DcCmdError::IoError)
    }

    fn version_text() -> String {
        format!(
            "🚀 dccmd-rs {}\n▶︎ https://github.com/unbekanntes-pferd/dccmd-rs",
            env!("CARGO_PKG_VERSION")
        )
    }

    fn cli_error_message(err: &DcCmdError) -> String {
        match err {
            DcCmdError::InvalidUrl(url) => format!("Invalid URL: {url}"),
            DcCmdError::InvalidPath(path) => format!("Invalid path: {path}"),
            DcCmdError::IoError => "Error reading / writing content.".into(),
            DcCmdError::DracoonError(e) => format!("{e}"),
            DcCmdError::ConnectionFailed => "Connection failed.".into(),
            DcCmdError::CredentialDeletionFailed => "Credential deletion failed.".into(),
            DcCmdError::CredentialStorageFailed => "Credential store failed.".into(),
            DcCmdError::InvalidAccount => "Invalid account.".into(),
            DcCmdError::Unknown => "Unknown error.".into(),
            DcCmdError::DracoonS3Error(e) => format!("{e}"),
            DcCmdError::DracoonAuthError(e) => format!("{e}"),
            DcCmdError::InvalidArgument(msg) => msg.to_string(),
            DcCmdError::CommandFailed => "Command failed.".into(),
            DcCmdError::ConfigDirUnavailable => {
                "Config directory unavailable on this platform.".into()
            }
            DcCmdError::ConfigDirCreationFailed(msg) => {
                format!("Config directory creation failed: {msg}")
            }
            DcCmdError::LogFileCreationFailed => "Log file creation failed.".into(),
        }
    }
}

impl Ui for TermUi {
    fn print_node(&self, node: &Node, long: bool, human_readable: bool) -> Result<(), DcCmdError> {
        print_node(&self.out, node, Some(long), Some(human_readable));
        Ok(())
    }

    fn write_success(&self, message: &str) -> Result<(), DcCmdError> {
        let line = format_success_message(message);
        self.out.write_line(&line).map_err(|_| DcCmdError::IoError)
    }

    fn write_warning(&self, message: &str) -> Result<(), DcCmdError> {
        self.err
            .write_line(message)
            .map_err(|_| DcCmdError::IoError)
    }

    fn write_error(&self, message: &str) -> Result<(), DcCmdError> {
        let line = format_error_message(message);
        self.out.write_line(&line).map_err(|_| DcCmdError::IoError)
    }

    fn write_info(&self, message: &str) -> Result<(), DcCmdError> {
        self.out
            .write_line(message)
            .map_err(|_| DcCmdError::IoError)
    }

    fn confirm(&self, prompt: &str) -> Result<bool, DcCmdError> {
        Confirm::new()
            .with_prompt(prompt)
            .interact()
            .map_err(|_| DcCmdError::IoError)
    }

    fn print_users(&self, users: &RangedItems<UserItem>, csv: bool) -> Result<(), DcCmdError> {
        users_print::print_users(&self.out, users, csv)
    }

    fn print_user_info(&self, user: UserInfo) -> Result<(), DcCmdError> {
        users_print::print_user_info(&self.out, user)
    }

    fn print_groups(&self, groups: RangedItems<Group>, csv: bool) -> Result<(), DcCmdError> {
        groups_print::print_groups(&self.out, groups, csv)
    }

    fn print_group_users(&self, pages: &[GroupUsersPage], csv: bool) -> Result<(), DcCmdError> {
        for (idx, page) in pages.iter().enumerate() {
            groups_print::print_group_users(
                &self.out,
                page.users.clone(),
                &page.group,
                csv,
                idx == 0,
            )?;
        }
        Ok(())
    }

    fn print_events(&self, events: LogEventList, csv: bool) -> Result<(), DcCmdError> {
        reports_print::print_events(&self.out, events, csv)
    }

    fn print_permissions(&self, permissions: AuditNodeList, csv: bool) -> Result<(), DcCmdError> {
        reports_print::print_permissions(&self.out, permissions, csv)
    }

    fn print_event_types(&self, operations: LogOperationList) -> Result<(), DcCmdError> {
        reports_print::print_event_types(&self.out, operations)
    }
}

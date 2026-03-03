use crate::{
    app::nodes::command::{
        CmdCopyOptions, CmdCreateContainerOptions, CmdDeleteOptions, CmdDownloadOptions,
        CmdListNodesOptions, CmdTransferOptions, CmdUploadOptions,
    },
    core::models::PasswordAuth,
};

use super::{config::ConfigCommand, GroupsCommand, ReportsCommand, UsersCommand};

pub enum AppCommand {
    Config {
        cmd: ConfigCommand,
    },
    Upload {
        source: String,
        target: String,
        opts: CmdUploadOptions,
    },
    Download {
        source: String,
        target: String,
        opts: CmdDownloadOptions,
    },
    Transfer {
        source: String,
        target: String,
        opts: CmdTransferOptions,
    },
    Cp {
        source: String,
        target: String,
        opts: CmdCopyOptions,
    },
    Ls {
        source: String,
        opts: CmdListNodesOptions,
    },
    Rm {
        source: String,
        opts: CmdDeleteOptions,
    },
    Mkdir {
        source: String,
        opts: CmdCreateContainerOptions,
        deprecated_alias: bool,
    },
    Users {
        cmd: UsersCommand,
        auth: Option<PasswordAuth>,
    },
    Groups {
        cmd: GroupsCommand,
        auth: Option<PasswordAuth>,
    },
    Reports {
        cmd: ReportsCommand,
        auth: Option<PasswordAuth>,
    },
}

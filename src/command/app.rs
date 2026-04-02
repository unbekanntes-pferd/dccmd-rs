use crate::{
    app::nodes::command::{
        CmdCopyOptions, CmdCreateContainerOptions, CmdDeleteOptions, CmdDownloadOptions,
        CmdListNodesOptions, CmdTransferOptions, CmdUploadOptions,
    },
    app::requests::{ConfigRequest, GroupsRequest, ReportsRequest, UsersRequest},
    core::models::PasswordAuth,
};

pub enum AppCommand {
    Config {
        cmd: ConfigRequest,
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
        cmd: UsersRequest,
        auth: Option<PasswordAuth>,
    },
    Groups {
        cmd: GroupsRequest,
        auth: Option<PasswordAuth>,
    },
    Reports {
        cmd: ReportsRequest,
        auth: Option<PasswordAuth>,
    },
}

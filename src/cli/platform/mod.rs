mod config;
mod groups;
mod nodes;
mod reports;
mod users;

use async_trait::async_trait;

use crate::{
    app::{
        nodes::{
            command::{
                CmdCopyOptions, CmdCreateContainerOptions, CmdDeleteOptions, CmdDownloadOptions,
                CmdListNodesOptions, CmdTransferOptions, CmdUploadOptions,
            },
            download::{DownloadOutcome, NodesDownloadService, NoopTransferStateStore},
            filesystem::OSFileSystem,
            upload::NodesUploadService,
            CopyNodesResult, DeleteNodesPreparation, ListNodesResult,
        },
        results::{GroupsPlatformResult, ReportsPlatformResult, UsersPlatformResult},
        Platform,
    },
    cli::progress::IndicatifProgressReporter,
    command::{config::ConfigCommand, GroupsCommand, ReportsCommand, UsersCommand},
    core::models::{DcCmdError, PasswordAuth},
};

pub struct CliPlatform;

impl CliPlatform {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Platform for CliPlatform {
    async fn config(
        &self,
        cmd: ConfigCommand,
    ) -> Result<crate::app::results::ConfigPlatformResult, DcCmdError> {
        self.execute_config_cmd(cmd).await
    }

    async fn upload(
        &self,
        source: String,
        target: String,
        opts: CmdUploadOptions,
    ) -> Result<Option<String>, DcCmdError> {
        NodesUploadService::with_progress(std::sync::Arc::new(IndicatifProgressReporter::new()))
            .upload(source.into(), target, opts)
            .await
    }

    async fn download(
        &self,
        source: String,
        target: String,
        opts: CmdDownloadOptions,
    ) -> Result<DownloadOutcome, DcCmdError> {
        NodesDownloadService::with_progress(
            OSFileSystem,
            NoopTransferStateStore,
            std::sync::Arc::new(IndicatifProgressReporter::new()),
        )
        .download(source, target, opts)
        .await
    }

    async fn transfer(
        &self,
        source: String,
        target: String,
        opts: CmdTransferOptions,
    ) -> Result<Option<String>, DcCmdError> {
        self.execute_nodes_transfer(source, target, opts).await
    }

    async fn copy_nodes(
        &self,
        source: String,
        target: String,
        opts: CmdCopyOptions,
    ) -> Result<CopyNodesResult, DcCmdError> {
        self.execute_nodes_copy(source, target, opts).await
    }

    async fn list_nodes(
        &self,
        source: String,
        opts: CmdListNodesOptions,
    ) -> Result<ListNodesResult, DcCmdError> {
        self.execute_nodes_ls(source, opts).await
    }

    async fn prepare_delete(
        &self,
        source: String,
        opts: CmdDeleteOptions,
    ) -> Result<DeleteNodesPreparation, DcCmdError> {
        self.execute_nodes_prepare_rm(source, opts).await
    }

    async fn delete_node(
        &self,
        source: String,
        opts: CmdDeleteOptions,
        node_id: u64,
    ) -> Result<(), DcCmdError> {
        self.execute_nodes_rm_single(source, opts, node_id).await
    }

    async fn delete_nodes(
        &self,
        source: String,
        opts: CmdDeleteOptions,
        node_ids: Vec<u64>,
    ) -> Result<(), DcCmdError> {
        self.execute_nodes_rm_batch(source, opts, node_ids).await
    }

    async fn create_container(
        &self,
        source: String,
        opts: CmdCreateContainerOptions,
    ) -> Result<String, DcCmdError> {
        self.execute_nodes_mkdir(source, opts).await
    }

    async fn users(
        &self,
        cmd: UsersCommand,
        auth: Option<PasswordAuth>,
    ) -> Result<UsersPlatformResult, DcCmdError> {
        self.execute_users_cmd(cmd, auth).await
    }

    async fn groups(
        &self,
        cmd: GroupsCommand,
        auth: Option<PasswordAuth>,
    ) -> Result<GroupsPlatformResult, DcCmdError> {
        self.execute_groups_cmd(cmd, auth).await
    }

    async fn reports(
        &self,
        cmd: ReportsCommand,
        auth: Option<PasswordAuth>,
    ) -> Result<ReportsPlatformResult, DcCmdError> {
        self.execute_reports_cmd(cmd, auth).await
    }
}

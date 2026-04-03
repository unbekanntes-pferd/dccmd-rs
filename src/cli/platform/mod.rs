mod config;
mod groups;
mod nodes;
mod reports;
mod users;

use async_trait::async_trait;
use secrecy::SecretString;

use crate::{
    app::{
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
        results::{GroupsPlatformResult, ReportsPlatformResult, UsersPlatformResult},
        Platform,
    },
    cli::progress::IndicatifProgressReporter,
    core::models::{DcCmdError, PasswordAuth},
};

pub struct CliPlatform {
    password_auth: Option<PasswordAuth>,
    encryption_password: Option<SecretString>,
}

impl CliPlatform {
    pub fn new(
        password_auth: Option<PasswordAuth>,
        encryption_password: Option<SecretString>,
    ) -> Self {
        Self {
            password_auth,
            encryption_password,
        }
    }

    fn password_auth(&self) -> Option<PasswordAuth> {
        self.password_auth.clone()
    }

    fn encryption_password(&self) -> Option<SecretString> {
        self.encryption_password.clone()
    }

    fn progress_reporter(&self) -> std::sync::Arc<IndicatifProgressReporter> {
        std::sync::Arc::new(IndicatifProgressReporter::new())
    }
}

#[async_trait]
impl Platform for CliPlatform {
    async fn config(
        &self,
        cmd: ConfigRequest,
    ) -> Result<crate::app::results::ConfigPlatformResult, DcCmdError> {
        self.execute_config_cmd(cmd).await
    }

    async fn upload(
        &self,
        source: String,
        target: String,
        opts: CmdUploadOptions,
    ) -> Result<UploadOutcome, DcCmdError> {
        self.execute_nodes_upload(source, target, opts).await
    }

    async fn download(
        &self,
        source: String,
        target: String,
        opts: CmdDownloadOptions,
    ) -> Result<DownloadOutcome, DcCmdError> {
        self.execute_nodes_download(source, target, opts).await
    }

    async fn transfer(
        &self,
        source: String,
        target: String,
        opts: CmdTransferOptions,
    ) -> Result<TransferOutcome, DcCmdError> {
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

    async fn users(&self, cmd: UsersRequest) -> Result<UsersPlatformResult, DcCmdError> {
        self.execute_users_cmd(cmd).await
    }

    async fn groups(&self, cmd: GroupsRequest) -> Result<GroupsPlatformResult, DcCmdError> {
        self.execute_groups_cmd(cmd).await
    }

    async fn reports(&self, cmd: ReportsRequest) -> Result<ReportsPlatformResult, DcCmdError> {
        self.execute_reports_cmd(cmd).await
    }
}

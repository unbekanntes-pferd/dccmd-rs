use crate::{
    app::{
        auth::AuthService,
        nodes::{
            command::{
                CmdCopyOptions, CmdCreateContainerOptions, CmdDeleteOptions, CmdListNodesOptions,
                CmdTransferOptions,
            },
            transfer::{NodesTransferService, TransferOutcome},
            CopyNodesResult, DeleteNodesPreparation, ListNodesResult, NodesService,
        },
    },
    cli::progress::IndicatifProgressReporter,
    core::models::DcCmdError,
};

use super::CliPlatform;

impl CliPlatform {
    pub(super) async fn execute_nodes_copy(
        &self,
        source: String,
        target: String,
        opts: CmdCopyOptions,
    ) -> Result<CopyNodesResult, DcCmdError> {
        let dracoon = AuthService::new()
            .connect_client(&source, opts.auth, false)
            .await?;
        let service = NodesService::new(dracoon.clone());

        service
            .copy_nodes(&source, &target, dracoon.get_base_url().as_ref())
            .await
    }

    pub(super) async fn execute_nodes_ls(
        &self,
        source: String,
        opts: CmdListNodesOptions,
    ) -> Result<ListNodesResult, DcCmdError> {
        let dracoon = AuthService::new()
            .connect_client(&source, opts.auth(), false)
            .await?;
        let service = NodesService::new(dracoon.clone());

        service
            .list_nodes(
                &source,
                dracoon.get_base_url().as_ref(),
                opts.managed(),
                opts.list_opts(),
            )
            .await
    }

    pub(super) async fn execute_nodes_prepare_rm(
        &self,
        source: String,
        opts: CmdDeleteOptions,
    ) -> Result<DeleteNodesPreparation, DcCmdError> {
        let dracoon = AuthService::new()
            .connect_client(&source, opts.auth(), false)
            .await?;
        let service = NodesService::new(dracoon.clone());

        service
            .prepare_delete(&source, dracoon.get_base_url().as_ref(), opts.recursive())
            .await
    }

    pub(super) async fn execute_nodes_rm_single(
        &self,
        source: String,
        opts: CmdDeleteOptions,
        node_id: u64,
    ) -> Result<(), DcCmdError> {
        let dracoon = AuthService::new()
            .connect_client(&source, opts.auth(), false)
            .await?;
        let service = NodesService::new(dracoon);

        service.delete_node(node_id).await
    }

    pub(super) async fn execute_nodes_rm_batch(
        &self,
        source: String,
        opts: CmdDeleteOptions,
        node_ids: Vec<u64>,
    ) -> Result<(), DcCmdError> {
        let dracoon = AuthService::new()
            .connect_client(&source, opts.auth(), false)
            .await?;
        let service = NodesService::new(dracoon);

        service.delete_nodes(node_ids).await
    }

    pub(super) async fn execute_nodes_mkdir(
        &self,
        source: String,
        opts: CmdCreateContainerOptions,
    ) -> Result<String, DcCmdError> {
        let dracoon = AuthService::new()
            .connect_client(&source, opts.auth, false)
            .await?;
        let service = NodesService::new(dracoon.clone());

        service
            .create_container(
                &source,
                dracoon.get_base_url().as_ref(),
                opts.container_type,
                opts.classification,
                opts.notes,
                opts.admin_users,
                opts.inherit_permissions,
            )
            .await
    }

    pub(super) async fn execute_nodes_transfer(
        &self,
        source: String,
        target: String,
        opts: CmdTransferOptions,
    ) -> Result<TransferOutcome, DcCmdError> {
        NodesTransferService::with_progress(std::sync::Arc::new(IndicatifProgressReporter::new()))
            .transfer(source, target, opts)
            .await
    }
}

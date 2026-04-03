use dco3::Nodes;

use crate::{
    app::{
        auth::AuthService,
        nodes::{
            command::{
                CmdCopyOptions, CmdCreateContainerOptions, CmdDeleteOptions, CmdDownloadOptions,
                CmdListNodesOptions, CmdTransferOptions, CmdUploadOptions,
            },
            download::{
                DownloadOutcome, NodesDownloadService, NoopTransferStateStore,
                PreparedDownloadSource,
            },
            filesystem::OSFileSystem,
            transfer::{NodesTransferService, TransferOutcome},
            upload::{NodesUploadService, UploadOutcome},
            CopyNodesResult, DeleteNodesPreparation, ListNodesResult, NodesService,
        },
    },
    core::{models::DcCmdError, utils::strings::parse_path},
};

use super::CliPlatform;

impl CliPlatform {
    pub(super) async fn execute_nodes_copy(
        &self,
        source: String,
        target: String,
        _opts: CmdCopyOptions,
    ) -> Result<CopyNodesResult, DcCmdError> {
        let dracoon = AuthService::new()
            .connect_client(&source, self.password_auth(), false)
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
            .connect_client(&source, self.password_auth(), false)
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
            .connect_client(&source, self.password_auth(), false)
            .await?;
        let service = NodesService::new(dracoon.clone());

        service
            .prepare_delete(&source, dracoon.get_base_url().as_ref(), opts.recursive())
            .await
    }

    pub(super) async fn execute_nodes_rm_single(
        &self,
        source: String,
        _opts: CmdDeleteOptions,
        node_id: u64,
    ) -> Result<(), DcCmdError> {
        let dracoon = AuthService::new()
            .connect_client(&source, self.password_auth(), false)
            .await?;
        let service = NodesService::new(dracoon);

        service.delete_node(node_id).await
    }

    pub(super) async fn execute_nodes_rm_batch(
        &self,
        source: String,
        _opts: CmdDeleteOptions,
        node_ids: Vec<u64>,
    ) -> Result<(), DcCmdError> {
        let dracoon = AuthService::new()
            .connect_client(&source, self.password_auth(), false)
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
            .connect_client(&source, self.password_auth(), false)
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

    pub(super) async fn execute_nodes_download(
        &self,
        source: String,
        target: String,
        opts: CmdDownloadOptions,
    ) -> Result<DownloadOutcome, DcCmdError> {
        let service = NodesDownloadService::with_progress(
            OSFileSystem,
            NoopTransferStateStore,
            self.progress_reporter(),
        );

        if source.contains("/public/download-shares/") {
            let dracoon = AuthService::new().init_public(&source).await?;
            return service
                .download_public(&dracoon, source, target, opts)
                .await;
        }

        let auth_service = AuthService::new();
        let mut session = auth_service
            .connect(&source, self.password_auth(), true)
            .await?;

        let (parent_path, node_name, _) = parse_path(&source, session.base_url())
            .or(Err(DcCmdError::InvalidPath(source.clone())))?;
        let node_path = format!("{parent_path}{node_name}/");

        let lookup_api = session.client().clone();
        let node = if node_name.contains('*') {
            lookup_api.nodes().get_node_from_path(&parent_path).await?
        } else {
            lookup_api.nodes().get_node_from_path(&node_path).await?
        };

        let Some(node) = node else {
            return Err(DcCmdError::InvalidPath(source));
        };
        let requires_encryption = node.is_encrypted == Some(true);

        let prepared_source = if node_name.contains('*') {
            PreparedDownloadSource::SearchQuery {
                source,
                search_string: node_name,
                parent_path,
            }
        } else {
            PreparedDownloadSource::Node {
                source,
                node: Box::new(node),
            }
        };

        if requires_encryption {
            session = auth_service
                .ensure_encryption(session, self.encryption_password())
                .await?;
        }

        service
            .download_with_client(session.into_client(), prepared_source, target, opts)
            .await
    }

    pub(super) async fn execute_nodes_upload(
        &self,
        source: String,
        target: String,
        opts: CmdUploadOptions,
    ) -> Result<UploadOutcome, DcCmdError> {
        let service = NodesUploadService::with_progress(self.progress_reporter());
        let source_path = std::path::PathBuf::from(source);

        if target.contains("/public/upload-shares/") {
            let dracoon = AuthService::new().init_public(&target).await?;
            return service.upload_public(&dracoon, source_path, target).await;
        }

        let auth_service = AuthService::new();
        let mut dracoon = auth_service
            .connect_client(&target, self.password_auth(), true)
            .await?;

        let (parent_path, node_name, _) = parse_path(&target, dracoon.get_base_url().as_str())
            .or(Err(DcCmdError::InvalidPath(target.clone())))?;
        let node_path = format!("{parent_path}{node_name}/");

        let parent_node = dracoon
            .nodes()
            .get_node_from_path(&node_path)
            .await?
            .ok_or_else(|| DcCmdError::InvalidPath(target.clone()))?;

        if parent_node.is_encrypted == Some(true) {
            let base_url = dracoon.get_base_url().to_string();
            dracoon = auth_service
                .ensure_encryption_client(base_url, dracoon, self.encryption_password())
                .await?;
        }

        service
            .upload_with_client(&dracoon, source_path, &parent_node, opts)
            .await
    }

    pub(super) async fn execute_nodes_transfer(
        &self,
        source: String,
        target: String,
        opts: CmdTransferOptions,
    ) -> Result<TransferOutcome, DcCmdError> {
        NodesTransferService::with_progress(self.progress_reporter())
            .transfer(source, target, opts)
            .await
    }
}

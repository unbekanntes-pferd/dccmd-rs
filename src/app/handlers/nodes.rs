use dco3::nodes::models::NodeType;
use tracing::warn;

use crate::{
    app::{
        nodes::{
            command::{
                CmdCopyOptions, CmdCreateContainerOptions, CmdDeleteOptions, CmdDownloadOptions,
                CmdListNodesOptions, CmdTransferOptions, CmdUploadOptions,
            },
            DeleteNodesPreparation,
        },
        outcome::{CommandOutcome, OutcomeMessageKind},
        results::{AppPayload, AppResult, AppStatus},
        App, Platform, Ui,
    },
    core::models::DcCmdError,
};

impl<P: Platform, U: Ui> App<P, U> {
    pub(in crate::app) async fn handle_download(
        &self,
        source: String,
        target: String,
        opts: CmdDownloadOptions,
    ) -> Result<AppResult, DcCmdError> {
        let mut outcome = CommandOutcome::default();
        let download_outcome = self.platform.download(source, target, opts).await?;
        let status = if download_outcome.failed == 0 {
            AppStatus::Success
        } else if download_outcome.succeeded > 0 {
            AppStatus::PartialFailure
        } else {
            AppStatus::Failure
        };

        if download_outcome.failed == 0 {
            self.write_success(
                &mut outcome,
                &format!(
                    "Downloaded {} item(s) to {}.",
                    download_outcome.succeeded, download_outcome.target_root
                ),
            )?;
        } else {
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
                .take(super::super::DOWNLOAD_FAILURE_PREVIEW_LIMIT)
            {
                self.write_info(
                    &mut outcome,
                    &format!(
                        "Failed {} -> {} ({})",
                        failure.node_name, failure.target, failure.reason
                    ),
                )?;
            }

            if download_outcome.failures.len() > super::super::DOWNLOAD_FAILURE_PREVIEW_LIMIT {
                self.write_info(
                    &mut outcome,
                    &format!(
                        "... and {} more failed item(s).",
                        download_outcome.failures.len()
                            - super::super::DOWNLOAD_FAILURE_PREVIEW_LIMIT
                    ),
                )?;
            }
        }

        Ok(super::super::app_result_from_status(status, outcome)
            .with_payload(AppPayload::Download(download_outcome)))
    }

    pub(in crate::app) async fn handle_upload(
        &self,
        source: String,
        target: String,
        opts: CmdUploadOptions,
    ) -> Result<AppResult, DcCmdError> {
        let mut outcome = CommandOutcome::default();
        let upload_outcome = self.platform.upload(source, target, opts).await?;
        let status = if upload_outcome.failed == 0 {
            AppStatus::Success
        } else if upload_outcome.succeeded > 0 {
            AppStatus::PartialFailure
        } else {
            AppStatus::Failure
        };

        if let Some(message) = upload_outcome.share_message.as_deref() {
            self.write_success(&mut outcome, message)?;
        }

        if let Some(message) = upload_outcome.failure_message() {
            match status {
                AppStatus::Success => {}
                AppStatus::PartialFailure => self.write_warning(&mut outcome, &message)?,
                AppStatus::Failure => self.write_error(&mut outcome, &message)?,
            }
        }

        Ok(super::super::app_result_from_status(status, outcome)
            .with_payload(AppPayload::Upload(upload_outcome)))
    }

    pub(in crate::app) async fn handle_transfer(
        &self,
        source: String,
        target: String,
        opts: CmdTransferOptions,
    ) -> Result<AppResult, DcCmdError> {
        let mut outcome = CommandOutcome::default();
        let transfer_outcome = self.platform.transfer(source, target, opts).await?;
        if let Some(message) = transfer_outcome.share_message.as_deref() {
            self.write_success(&mut outcome, message)?;
        }
        Ok(AppResult::success(outcome).with_payload(AppPayload::Transfer(transfer_outcome)))
    }

    pub(in crate::app) async fn handle_copy(
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
        Ok(AppResult::success(outcome))
    }

    pub(in crate::app) async fn handle_list(
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

        Ok(AppResult::success(outcome))
    }

    pub(in crate::app) async fn handle_remove(
        &self,
        source: String,
        opts: CmdDeleteOptions,
    ) -> Result<AppResult, DcCmdError> {
        let mut outcome = CommandOutcome::default();
        let mut status = AppStatus::Success;
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
                status = AppStatus::Failure;
            }
            DeleteNodesPreparation::ContainerRequiresRecursive => {
                self.write_error(
                    &mut outcome,
                    "Deleting non-empty folder or room not allowed. Use --recursive flag to delete recursively.",
                )?;
                status = AppStatus::Failure;
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
                        return Ok(AppResult::failure(outcome));
                    }
                }

                self.platform.delete_node(source, opts, node_id).await?;
                self.write_success(&mut outcome, &format!("Node {node_name} deleted."))?;
            }
        }

        Ok(super::super::app_result_from_status(status, outcome))
    }

    pub(in crate::app) async fn handle_mkdir(
        &self,
        source: String,
        opts: CmdCreateContainerOptions,
        deprecated_alias: bool,
    ) -> Result<AppResult, DcCmdError> {
        let mut outcome = CommandOutcome::default();
        if deprecated_alias {
            warn!("{}", super::super::MKROOM_DEPRECATION_WARNING);
            self.write_warning(&mut outcome, super::super::MKROOM_DEPRECATION_WARNING)?;
        }

        let success_message = self.platform.create_container(source, opts).await?;
        self.write_success(&mut outcome, &success_message)?;
        Ok(AppResult::success(outcome))
    }
}

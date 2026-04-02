use std::{path::PathBuf, sync::Arc};

use files::{upload_file, upload_public_file};
use folders::upload_container;

use tracing::error;

use crate::app::{nodes::command::CmdUploadOptions, shares::DracoonDownloadShareLinkCreator};
use crate::{
    app::auth::AuthService,
    app::nodes::progress::ProgressReporter,
    core::{models::DcCmdError, utils::strings::parse_path},
};
use dco3::nodes::Nodes;

mod api;
mod files;
mod folders;

pub use self::files::UploadFailure;

const UPLOAD_FAILURE_PREVIEW_LIMIT: usize = 5;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UploadOutcome {
    pub requested_total: u64,
    pub succeeded: u64,
    pub failed: u64,
    pub failures: Vec<UploadFailure>,
    pub target_root: String,
    pub partial: bool,
    pub share_message: Option<String>,
}

impl UploadOutcome {
    pub fn success(target_root: impl Into<String>, count: u64) -> Self {
        Self {
            requested_total: count,
            succeeded: count,
            failed: 0,
            failures: Vec::new(),
            target_root: target_root.into(),
            partial: false,
            share_message: None,
        }
    }

    pub fn from_failures(
        target_root: impl Into<String>,
        requested_total: u64,
        succeeded: u64,
        failures: Vec<UploadFailure>,
    ) -> Self {
        let failed = failures.len() as u64;
        Self {
            requested_total,
            succeeded,
            failed,
            failures,
            target_root: target_root.into(),
            partial: failed > 0 && succeeded > 0,
            share_message: None,
        }
    }

    pub fn with_share_message(mut self, share_message: Option<String>) -> Self {
        self.share_message = share_message;
        self
    }

    pub fn failure_message(&self) -> Option<String> {
        if self.failed == 0 {
            return None;
        }

        let mut summary = format!(
            "Uploaded {}/{} file(s) to {}; {} failed.",
            self.succeeded, self.requested_total, self.target_root, self.failed
        );

        for failure in self.failures.iter().take(UPLOAD_FAILURE_PREVIEW_LIMIT) {
            summary.push_str(&format!("\nFailed {} ({})", failure.file, failure.reason));
        }

        if self.failures.len() > UPLOAD_FAILURE_PREVIEW_LIMIT {
            summary.push_str(&format!(
                "\n... and {} more failed file(s).",
                self.failures.len() - UPLOAD_FAILURE_PREVIEW_LIMIT
            ));
        }

        Some(summary)
    }
}

pub struct NodesUploadService {
    progress: Arc<dyn ProgressReporter>,
}

impl NodesUploadService {
    pub fn with_progress(progress: Arc<dyn ProgressReporter>) -> Self {
        Self { progress }
    }

    pub async fn upload(
        &self,
        source: PathBuf,
        target: String,
        opts: CmdUploadOptions,
    ) -> Result<UploadOutcome, DcCmdError> {
        // this is a public upload share
        match (target.contains("/public/upload-shares/"), source.is_file()) {
            (true, true) => {
                upload_public_file(source, target, self.progress.as_ref()).await?;
                return Ok(UploadOutcome::success("public upload share", 1));
            }
            (true, false) => {
                error!("Public upload shares only support file uploads.");
                return Err(DcCmdError::InvalidPath(
                    source.to_string_lossy().to_string(),
                ));
            }
            _ => (),
        }

        let auth = AuthService::new();
        let mut dracoon = auth
            .connect_client(&target, opts.auth.clone(), true)
            .await?;

        let (parent_path, node_name, _) = parse_path(&target, dracoon.get_base_url().as_str())
            .or(Err(DcCmdError::InvalidPath(target.clone())))?;
        let node_path = format!("{parent_path}{node_name}/");

        let parent_node = dracoon.nodes().get_node_from_path(&node_path).await?;

        let Some(parent_node) = parent_node else {
            error!("Target path not found: {}", target);
            return Err(DcCmdError::InvalidPath(target.clone()));
        };

        if parent_node.is_encrypted == Some(true) {
            let base_url = dracoon.get_base_url().to_string();
            dracoon = auth
                .ensure_encryption_client(base_url, dracoon, opts.encryption_password.clone())
                .await?;
        }

        if parent_node.is_encrypted.unwrap_or(false) && opts.share {
            error!("Parent node is encrypted. Cannot upload to encrypted nodes.");
            return Err(DcCmdError::InvalidArgument(
                "Sharing encrypted files currently not supported (remove --share flag)."
                    .to_string(),
            ));
        }

        match (source.is_file(), source.is_dir(), opts.recursive) {
            // is a file
            (true, _, _) => {
                let share_link_creator = DracoonDownloadShareLinkCreator::new(dracoon.clone());
                let share_message = upload_file(
                    &dracoon,
                    source,
                    &parent_node,
                    opts.clone(),
                    &share_link_creator,
                    self.progress.as_ref(),
                )
                .await?;
                Ok(UploadOutcome::success(parent_node.name.clone(), 1)
                    .with_share_message(share_message))
            }
            // is a directory and recursive flag is set
            (_, true, true) => {
                upload_container(
                    &dracoon,
                    source,
                    &parent_node,
                    &opts,
                    self.progress.as_ref(),
                )
                .await
            }
            // is a directory and recursive flag is not set
            (_, true, false) => Err(DcCmdError::InvalidArgument(
                "Container upload requires recursive flag".to_string(),
            )),
            // is neither a file nor a directory
            _ => Err(DcCmdError::InvalidPath(
                source.to_string_lossy().to_string(),
            )),
        }
    }
}

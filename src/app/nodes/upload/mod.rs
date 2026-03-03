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

mod files;
mod folders;

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
    ) -> Result<Option<String>, DcCmdError> {
        // this is a public upload share
        match (target.contains("/public/upload-shares/"), source.is_file()) {
            (true, true) => {
                upload_public_file(source, target, self.progress.as_ref()).await?;
                return Ok(None);
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
                return Ok(share_message);
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
                .await?;
            }
            // is a directory and recursive flag is not set
            (_, true, false) => {
                return Err(DcCmdError::InvalidArgument(
                    "Container upload requires recursive flag".to_string(),
                ));
            }
            // is neither a file nor a directory
            _ => {
                return Err(DcCmdError::InvalidPath(
                    source.to_string_lossy().to_string(),
                ));
            }
        }

        Ok(None)
    }
}

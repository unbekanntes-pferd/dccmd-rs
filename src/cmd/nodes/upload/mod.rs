use std::path::PathBuf;

use console::Term;
use files::{upload_file, upload_public_file};
use folders::upload_container;

use tracing::error;

use crate::cmd::{init_dracoon, init_encryption, models::DcCmdError, utils::strings::parse_path};
use dco3::nodes::Nodes;

mod files;
mod folders;

use super::models::CmdUploadOptions;

pub async fn upload(
    term: Term,
    source: PathBuf,
    target: String,
    opts: CmdUploadOptions,
) -> Result<(), DcCmdError> {
    // this is a public upload share
    match (target.contains("/public/upload-shares/"), source.is_file()) {
        (true, true) => return upload_public_file(source, target).await,
        (true, false) => {
            error!("Public upload shares only support file uploads.");
            return Err(DcCmdError::InvalidPath(
                source.to_string_lossy().to_string(),
            ));
        }
        _ => (),
    }

    let mut dracoon = init_dracoon(&target, opts.auth.clone(), true).await?;

    let (parent_path, node_name, _) = parse_path(&target, dracoon.get_base_url().as_str())
        .or(Err(DcCmdError::InvalidPath(target.clone())))?;
    let node_path = format!("{parent_path}{node_name}/");

    let parent_node = dracoon.nodes().get_node_from_path(&node_path).await?;

    let Some(parent_node) = parent_node else {
        error!("Target path not found: {}", target);
        return Err(DcCmdError::InvalidPath(target.clone()));
    };

    if parent_node.is_encrypted == Some(true) {
        dracoon = init_encryption(dracoon, opts.encryption_password.clone()).await?;
    }

    if parent_node.is_encrypted.unwrap_or(false) && opts.share {
        error!("Parent node is encrypted. Cannot upload to encrypted nodes.");
        return Err(DcCmdError::InvalidArgument(
            "Sharing encrypted files currently not supported (remove --share flag).".to_string(),
        ));
    }

    match (source.is_file(), source.is_dir(), opts.recursive) {
        // is a file
        (true, _, _) => {
            upload_file(term, &dracoon, source, &parent_node, opts.clone()).await?;
        }
        // is a directory and recursive flag is set
        (_, true, true) => {
            upload_container(&dracoon, source, &parent_node, &node_path, &opts).await?;
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

    Ok(())
}

use containers::download_container;
use files::{download_file, download_files, download_public_file};

use tracing::{debug, error, info};

use crate::cmd::{
    init_dracoon, init_encryption,
    models::{DcCmdError, ListOptions},
    nodes::{is_search_query, search_nodes},
    utils::strings::parse_path,
};

use dco3::nodes::{models::NodeType, Nodes};

use super::models::CmdDownloadOptions;

mod containers;
mod files;

pub async fn download(
    source: String,
    target: String,
    download_opts: CmdDownloadOptions,
) -> Result<(), DcCmdError> {
    debug!("Downloading {} to {}", source, target);
    debug!("Velocity: {}", download_opts.velocity.unwrap_or(1));

    // this is a public download share
    if source.contains("/public/download-shares/") {
        return download_public_file(source, target, download_opts).await;
    }

    let mut dracoon = init_dracoon(&source, download_opts.auth, true).await?;

    let (parent_path, node_name, _) = parse_path(&source, dracoon.get_base_url().as_ref())
        .or(Err(DcCmdError::InvalidPath(source.clone())))?;
    let node_path = format!("{parent_path}{node_name}/");

    let node = if is_search_query(&node_name) {
        debug!("Searching for query {}", node_name);
        debug!("Parent path {}", parent_path);
        dracoon.nodes().get_node_from_path(&parent_path).await?
    } else {
        dracoon.nodes().get_node_from_path(&node_path).await?
    };

    let Some(node) = node else {
        error!("Node not found");
        return Err(DcCmdError::InvalidPath(source.clone()));
    };

    if node.is_encrypted == Some(true) {
        dracoon = init_encryption(dracoon, download_opts.encryption_password).await?;
    }

    if is_search_query(&node_name) {
        info!("Attempting download of search query {}.", node_name);
        let files = search_nodes(
            &dracoon,
            &node_name,
            Some(&parent_path),
            &ListOptions::new(None, None, None, true, false),
        )
        .await?;
        let files = files.get_files();

        info!("Found {} files.", files.len());

        download_files(&dracoon, files, &target, None, download_opts.velocity).await
    } else {
        match node.node_type {
            NodeType::File => download_file(&dracoon, &node, &target).await,
            _ => {
                if download_opts.recursive {
                    download_container(
                        &dracoon,
                        &node,
                        &target,
                        download_opts.velocity,
                        download_opts.include_rooms,
                    )
                    .await
                } else {
                    Err(DcCmdError::InvalidArgument(
                        "Container download requires recursive flag".to_string(),
                    ))
                }
            }
        }
    }
}

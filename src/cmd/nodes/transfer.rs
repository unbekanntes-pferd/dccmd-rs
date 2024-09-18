use console::Term;
use dco3::auth::Connected;
use dco3::nodes::{FileMeta, Node, ResolutionStrategy, UploadOptions};
use dco3::{Download, Dracoon, Nodes, Upload};
use indicatif::{ProgressBar, ProgressStyle};
use tokio::io::{duplex, AsyncWriteExt, BufReader, BufWriter};
use tracing::{debug, error};

use crate::cmd::models::DcCmdError;
use crate::cmd::nodes::share::share_node;
use crate::cmd::utils::strings::{format_success_message, parse_path};
use crate::cmd::{init_dracoon, init_encryption};

use super::models::CmdTransferOptions;

const MAX_BUFFER_SIZE: usize = 64 * 1024;

pub async fn transfer_node(
    term: Term,
    source: String,
    target: String,
    opts: CmdTransferOptions,
) -> Result<(), DcCmdError> {
    let (mut source_dracoon, mut target_dracoon) = init_transfer_clients(&source, &target).await?;
    let (source_node, parent_node) =
        get_transfer_nodes(&source, &target, &source_dracoon, &target_dracoon).await?;

    if parent_node.is_encrypted == Some(true) {
        target_dracoon = init_encryption(target_dracoon, None).await?;
    }

    if source_node.is_encrypted == Some(true) {
        source_dracoon = init_encryption(source_dracoon, None).await?;
    }

    let target_dracoon_mv = target_dracoon.clone();

    if parent_node.is_encrypted.unwrap_or(false) && opts.share {
        error!("Parent node is encrypted. Cannot upload to encrypted nodes.");
        return Err(DcCmdError::InvalidArgument(
            "Sharing encrypted files currently not supported (remove --share flag).".to_string(),
        ));
    }

    let file_meta = FileMeta::builder(source_node.name.clone(), source_node.size.unwrap_or(0));

    let file_meta = if let Some(timestamp_modification) = source_node.timestamp_modification {
        file_meta.with_timestamp_modification(timestamp_modification)
    } else {
        file_meta
    };

    let file_meta = if let Some(timestamp_creation) = source_node.timestamp_creation {
        file_meta.with_timestamp_creation(timestamp_creation)
    } else {
        file_meta
    };

    let file_meta = file_meta.build();

    let resolution_strategy = if opts.overwrite {
        ResolutionStrategy::Overwrite
    } else {
        ResolutionStrategy::AutoRename
    };

    let progress_bar = ProgressBar::new(source_node.size.unwrap_or(0));
    progress_bar.set_style(
    ProgressStyle::default_bar()
    .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}) {msg}").unwrap()
    .progress_chars("=>-"),
);

    let upload_options = UploadOptions::builder(file_meta)
        .with_resolution_strategy(resolution_strategy)
        .with_keep_share_links(opts.keep_share_links)
        .with_classification(opts.classification.unwrap_or(1))
        .build();

    let progress_bar_mv = progress_bar.clone();

    let callback = move |read_bytes: u64, _total: u64| {
        progress_bar_mv.inc(read_bytes);
    };

    let (writer, reader) = duplex(MAX_BUFFER_SIZE);

    let download_task = tokio::spawn(async move {
        let mut buf_writer = BufWriter::new(writer);
        let res = source_dracoon
            .download(
                &source_node,
                &mut buf_writer,
                Some(Box::new(callback)),
                None,
            )
            .await
            .map_err(DcCmdError::from);

        // flushing writer and shut down
        let _ = buf_writer.flush().await;
        let _ = buf_writer.shutdown().await;

        res
    });

    let upload_task = tokio::spawn(async move {
        let buf_reader = BufReader::new(reader);
        target_dracoon_mv
            .upload(&parent_node, upload_options, buf_reader, None, None)
            .await
            .map_err(DcCmdError::from)
    });

    let (download_res, upload_res) = tokio::try_join!(download_task, upload_task)
        .unwrap_or((Err(DcCmdError::Unknown), Err(DcCmdError::Unknown)));

    download_res?;
    let node = upload_res?;

    if !node.is_encrypted.unwrap_or(false) && opts.share {
        let link = share_node(&target_dracoon, &node, opts.share_password).await?;
        let file_name = node.name.clone();
        let success_msg =
            format_success_message(format!("Shared {file_name}.\n▶︎▶︎ {link}").as_str());
        let success_msg = format!("\n{success_msg}");

        term.write_line(&success_msg)
            .expect("Error writing message to terminal.");
    }

    let msg = format!("Node {} uploaded from {source} to {target}.", node.name);
    progress_bar.finish_with_message(msg);

    Ok(())
}

async fn get_node_from_path(path: &str, dracoon: &Dracoon<Connected>) -> Result<Node, DcCmdError> {
    debug!("Base url: {}", dracoon.get_base_url());
    let (parent_path, node_name, _) = parse_path(path, dracoon.get_base_url().as_str())
        .or(Err(DcCmdError::InvalidPath(path.to_string())))?;
    debug!("Parent_path: {}, node name: {}", parent_path, node_name);
    let parent_node_path = format!("{parent_path}{node_name}/");
    debug!("Parent node path: {}", parent_node_path);

    let parent_node = dracoon
        .nodes()
        .get_node_from_path(&parent_node_path)
        .await?;

    let Some(parent_node) = parent_node else {
        error!("Target path not found: {}", path);
        return Err(DcCmdError::InvalidPath(path.to_string()));
    };

    Ok(parent_node)
}

async fn init_transfer_clients(
    source: &str,
    target: &str,
) -> Result<(Dracoon<Connected>, Dracoon<Connected>), DcCmdError> {
    let source_dracoon = init_dracoon(source, None, true).await?;
    let target_dracoon = init_dracoon(target, None, true).await?;

    Ok((source_dracoon, target_dracoon))
}

async fn get_transfer_nodes(
    source: &str,
    target: &str,
    source_dracoon: &Dracoon<Connected>,
    target_dracoon: &Dracoon<Connected>,
) -> Result<(Node, Node), DcCmdError> {
    let source_node = get_node_from_path(source, source_dracoon).await?;
    let target_node = get_node_from_path(target, target_dracoon).await?;

    Ok((source_node, target_node))
}

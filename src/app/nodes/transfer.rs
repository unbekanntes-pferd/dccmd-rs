use std::sync::Arc;

use dco3::{
    auth::Connected,
    nodes::{FileMeta, Node, ResolutionStrategy, UploadOptions},
    Download, Dracoon, Nodes, Upload,
};
use tokio::io::{duplex, AsyncWriteExt, BufReader, BufWriter};
use tokio::task::{JoinError, JoinHandle};
use tracing::{debug, error};

use crate::{
    app::{
        auth::AuthService,
        nodes::{
            command::CmdTransferOptions,
            progress::{start_progress_bar, ProgressReporter},
        },
        shares::{DownloadShareLinkCreator, DracoonDownloadShareLinkCreator},
    },
    core::{models::DcCmdError, utils::strings::parse_path},
};

const MAX_BUFFER_SIZE: usize = 64 * 1024;

pub struct NodesTransferService {
    progress: Arc<dyn ProgressReporter>,
}

impl NodesTransferService {
    pub fn with_progress(progress: Arc<dyn ProgressReporter>) -> Self {
        Self { progress }
    }

    pub async fn transfer(
        &self,
        source: String,
        target: String,
        opts: CmdTransferOptions,
    ) -> Result<Option<String>, DcCmdError> {
        let auth = AuthService::new();
        let (mut source_dracoon, mut target_dracoon) =
            init_transfer_clients(&auth, &source, &target).await?;
        let (source_node, parent_node) =
            get_transfer_nodes(&source, &target, &source_dracoon, &target_dracoon).await?;

        if parent_node.is_encrypted == Some(true) {
            let base_url = target_dracoon.get_base_url().to_string();
            target_dracoon = auth
                .ensure_encryption_client(base_url, target_dracoon, None)
                .await?;
        }

        if source_node.is_encrypted == Some(true) {
            let base_url = source_dracoon.get_base_url().to_string();
            source_dracoon = auth
                .ensure_encryption_client(base_url, source_dracoon, None)
                .await?;
        }

        let target_dracoon_mv = target_dracoon.clone();

        if parent_node.is_encrypted.unwrap_or(false) && opts.share {
            error!("Parent node is encrypted. Cannot upload to encrypted nodes.");
            return Err(DcCmdError::InvalidArgument(
                "Sharing encrypted files currently not supported (remove --share flag)."
                    .to_string(),
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

        let progress_bar =
            start_progress_bar(self.progress.as_ref(), source_node.size.unwrap_or(0), None);

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

        let node = await_transfer_tasks(download_task, upload_task).await?;

        let mut share_message = None;
        if !node.is_encrypted.unwrap_or(false) && opts.share {
            let share_link_creator = DracoonDownloadShareLinkCreator::new(target_dracoon.clone());
            let link = share_link_creator
                .create_download_share_link(&node, opts.share_password)
                .await?;
            let file_name = node.name.clone();
            share_message = Some(format!("Shared {file_name}.\n▶︎▶︎ {link}"));
        }

        let msg = format!("Node {} uploaded from {source} to {target}.", node.name);
        progress_bar.finish_with_message(&msg);

        Ok(share_message)
    }
}

async fn await_transfer_tasks(
    download_task: JoinHandle<Result<(), DcCmdError>>,
    upload_task: JoinHandle<Result<Node, DcCmdError>>,
) -> Result<Node, DcCmdError> {
    let (download_join_res, upload_join_res) = tokio::join!(download_task, upload_task);

    let download_res = map_transfer_task_join("download", download_join_res)?;
    let upload_res = map_transfer_task_join("upload", upload_join_res)?;

    download_res?;
    upload_res
}

fn map_transfer_task_join<T>(
    task_name: &str,
    join_result: Result<T, JoinError>,
) -> Result<T, DcCmdError> {
    join_result.map_err(|err| map_transfer_join_error(task_name, err))
}

fn map_transfer_join_error(task_name: &str, err: JoinError) -> DcCmdError {
    let reason = if err.is_cancelled() {
        "was cancelled".to_string()
    } else if err.is_panic() {
        "panicked".to_string()
    } else {
        format!("failed to join: {err}")
    };

    DcCmdError::InvalidArgument(format!("Transfer {task_name} task {reason}."))
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
    auth: &AuthService,
    source: &str,
    target: &str,
) -> Result<(Dracoon<Connected>, Dracoon<Connected>), DcCmdError> {
    let source_dracoon = auth.connect_client(source, None, true).await?;
    let target_dracoon = auth.connect_client(target, None, true).await?;

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

#[cfg(test)]
mod tests {
    use tokio::{
        task::JoinError,
        time::{sleep, Duration},
    };

    use super::map_transfer_join_error;
    use crate::core::models::DcCmdError;

    async fn cancelled_join_error() -> JoinError {
        let handle = tokio::spawn(async {
            sleep(Duration::from_millis(50)).await;
        });
        handle.abort();
        handle.await.expect_err("task should be cancelled")
    }

    async fn panic_join_error() -> JoinError {
        let handle = tokio::spawn(async {
            panic!("boom");
        });
        handle.await.expect_err("task should panic")
    }

    #[tokio::test]
    async fn test_map_transfer_join_error_for_cancelled_task() {
        let join_error = cancelled_join_error().await;
        let err = map_transfer_join_error("download", join_error);

        assert!(
            matches!(err, DcCmdError::InvalidArgument(message) if message == "Transfer download task was cancelled.")
        );
    }

    #[tokio::test]
    async fn test_map_transfer_join_error_for_panicked_task() {
        let join_error = panic_join_error().await;
        let err = map_transfer_join_error("upload", join_error);

        assert!(
            matches!(err, DcCmdError::InvalidArgument(message) if message == "Transfer upload task panicked.")
        );
    }
}

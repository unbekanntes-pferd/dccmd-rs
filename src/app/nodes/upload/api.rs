use std::{
    fs::Metadata,
    path::{Path, PathBuf},
    sync::Arc,
    time::SystemTime,
};

use async_trait::async_trait;
use dco3::{
    auth::Connected,
    nodes::{FileMeta, Node, ResolutionStrategy, UploadOptions},
    Dracoon, Upload,
};
use tracing::error;
use unicode_normalization::UnicodeNormalization;

use crate::core::{models::DcCmdError, utils::dates::to_datetime_utc};

pub type UploadProgressFn = Arc<dyn Fn(u64) + Send + Sync>;

#[async_trait]
pub trait UploadApi: Send + Sync {
    async fn upload_local_file(
        &self,
        source: PathBuf,
        target_node: &Node,
        classification: u8,
        overwrite: bool,
        keep_share_links: bool,
        on_progress: Option<UploadProgressFn>,
    ) -> Result<String, DcCmdError>;
}

#[async_trait]
impl UploadApi for Dracoon<Connected> {
    async fn upload_local_file(
        &self,
        source: PathBuf,
        target_node: &Node,
        classification: u8,
        overwrite: bool,
        keep_share_links: bool,
        on_progress: Option<UploadProgressFn>,
    ) -> Result<String, DcCmdError> {
        let file = tokio::fs::File::open(&source).await.map_err(|err| {
            error!("Error opening file: {}", err);
            DcCmdError::IoError
        })?;

        let file_meta = file.metadata().await.map_err(|err| {
            error!("Error getting file metadata for {:?}: {}", file, err);
            DcCmdError::IoError
        })?;

        if !file_meta.is_file() {
            return Err(DcCmdError::InvalidPath(
                source.to_string_lossy().to_string(),
            ));
        }

        let file_meta = get_file_meta(&file_meta, &source)?;
        let file_name = file_meta.name.clone();

        let resolution_strategy = if overwrite {
            ResolutionStrategy::Overwrite
        } else {
            ResolutionStrategy::AutoRename
        };

        let keep_share_links = match resolution_strategy {
            ResolutionStrategy::Overwrite => keep_share_links,
            _ => false,
        };

        let upload_options = UploadOptions::builder(file_meta)
            .with_classification(classification)
            .with_resolution_strategy(resolution_strategy)
            .with_keep_share_links(keep_share_links)
            .build();

        let reader = tokio::io::BufReader::new(file);
        let callback = on_progress.map(|progress_fn| {
            Box::new(move |progress: u64, _total: u64| {
                progress_fn(progress);
            }) as Box<dyn FnMut(u64, u64) + Send + Sync>
        });

        self.upload(target_node, upload_options, reader, callback, None)
            .await
            .map_err(DcCmdError::from)?;

        Ok(file_name)
    }
}

pub(super) fn get_file_meta(
    file_meta: &Metadata,
    file_path: &Path,
) -> Result<FileMeta, DcCmdError> {
    let file_name = file_path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .map(|n| n.nfc().collect::<String>())
        .ok_or(DcCmdError::InvalidPath(
            file_path.to_string_lossy().to_string(),
        ))?;

    let timestamp_modification = file_meta
        .modified()
        .or(Err(DcCmdError::IoError))
        .unwrap_or_else(|_| SystemTime::now());

    let timestamp_modification = to_datetime_utc(timestamp_modification);

    let timestamp_creation = file_meta
        .created()
        .or(Err(DcCmdError::IoError))
        .unwrap_or_else(|_| SystemTime::now());

    let timestamp_creation = to_datetime_utc(timestamp_creation);

    Ok(FileMeta::builder(file_name, file_meta.len())
        .with_timestamp_modification(timestamp_modification)
        .with_timestamp_creation(timestamp_creation)
        .build())
}

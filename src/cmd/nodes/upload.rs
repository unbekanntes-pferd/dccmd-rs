use std::{path::PathBuf, time::SystemTime};

use indicatif::{ProgressBar, ProgressStyle};

use crate::cmd::{models::DcCmdError, init_encryption, init_dracoon, utils::dates::to_datetime_utc};
use dco3::nodes::{models::{ResolutionStrategy, FileMeta, UploadOptions}, Upload, Nodes};

const DEFAULT_CHUNK_SIZE: usize = 1024 * 1024 * 5; // 5 MB

pub async fn upload(source: PathBuf, target: String, overwrite: bool, classification: Option<u8>) -> Result<(), DcCmdError> {
    let mut dracoon = init_dracoon(&target).await?;
    

    let parent_node = dracoon.get_node_from_path(&target).await?;

    let Some(parent_node) = parent_node else {
        return Err(DcCmdError::InvalidPath(target.clone()))
    };

    if parent_node.is_encrypted == Some(true) {
        dracoon = init_encryption(dracoon).await?;
    }

    let file = tokio::fs::File::open(&source)
        .await
        .or(Err(DcCmdError::IoError))?;

    let file_meta = file.metadata().await.or(Err(DcCmdError::IoError))?;

    if !file_meta.is_file() {
        return Err(DcCmdError::InvalidPath(source.to_string_lossy().to_string()));
    }

    let file_name = source
    .file_name()
    .expect("This is a file (handled above)")
    .to_owned()
    .to_string_lossy()
    .as_ref()
    .to_string();

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

    let file_meta = FileMeta::builder()
        .with_name(file_name.clone())
        .with_size(file_meta.len())
        .with_timestamp_modification(timestamp_modification)
        .with_timestamp_creation(timestamp_creation)
        .build();

    let progress_bar = ProgressBar::new(parent_node.size.unwrap_or(0));
    progress_bar.set_style(
        ProgressStyle::default_bar()
        .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}) {msg}").unwrap()
        .progress_chars("=>-"),
    );

    let progress_bar_mv = progress_bar.clone();

    let classification = classification.unwrap_or(2);
    let resolution_strategy = if overwrite {
        ResolutionStrategy::Overwrite
    } else {
        ResolutionStrategy::AutoRename
    };
    let keep_share_links = matches!(resolution_strategy, ResolutionStrategy::Overwrite);

    let upload_options = UploadOptions::builder()
    .with_classification(classification)
    .with_resolution_strategy(resolution_strategy)
    .with_keep_share_links(keep_share_links)
    .build();

    let reader = tokio::io::BufReader::new(file);

    dracoon.upload(
        file_meta,
        &parent_node,
        upload_options,
        reader,
        Some(Box::new(move |progress, total| {
            progress_bar_mv.set_message("Uploading");
            progress_bar_mv.set_length(total);
            progress_bar_mv.set_position(progress);
        })),
        Some(DEFAULT_CHUNK_SIZE)
    ).await?;

    progress_bar.finish_with_message(format!("Upload of {file_name} complete"));

    Ok(())
}
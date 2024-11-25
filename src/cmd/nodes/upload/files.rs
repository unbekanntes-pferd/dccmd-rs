use std::{
    collections::BTreeMap,
    fs::Metadata,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, AtomicUsize, Ordering},
        Arc,
    },
    time::SystemTime,
};

use console::Term;
use dco3::{
    auth::Connected,
    nodes::{FileMeta, Node, ResolutionStrategy, UploadOptions},
    Dracoon, Nodes, Public, PublicUpload, Upload,
};
use indicatif::{ProgressBar, ProgressStyle};
use tracing::{debug, error, info, warn};
use unicode_normalization::UnicodeNormalization;

use crate::cmd::{
    config::{DEFAULT_CHUNK_SIZE, DEFAULT_CONCURRENT_MULTIPLIER, MAX_VELOCITY, MIN_VELOCITY},
    init_public_dracoon,
    models::DcCmdError,
    nodes::{models::CmdUploadOptions, share::share_node},
    utils::{dates::to_datetime_utc, strings::format_success_message},
};

pub async fn upload_public_file(source: PathBuf, target: String) -> Result<(), DcCmdError> {
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

    let dracoon = init_public_dracoon(&target).await?;

    let access_key = target
        .split('/')
        .last()
        .ok_or(DcCmdError::InvalidPath(target.clone()))?;

    let upload_share = dracoon.public().get_public_upload_share(access_key).await?;

    let file_meta = get_file_meta(&file_meta, &source)?;

    let file_size = file_meta.size;

    let progress_bar = ProgressBar::new(file_size);
    progress_bar.set_style(
    ProgressStyle::default_bar()
    .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}) {msg}").unwrap()
    .progress_chars("=>-"),
);

    let upload_opts = UploadOptions::builder(file_meta).build();
    let buffer_size = calculate_buffer_size(file_size);
    let reader = tokio::io::BufReader::with_capacity(buffer_size, file);

    let progress_bar_mv = progress_bar.clone();

    progress_bar_mv.set_message("Uploading");
    progress_bar_mv.set_length(file_size);

    dracoon
        .public()
        .upload(
            access_key,
            upload_share,
            upload_opts,
            reader,
            Some(Box::new(move |progress, _| {
                progress_bar_mv.inc(progress);
            })),
            Some(DEFAULT_CHUNK_SIZE),
        )
        .await?;

    Ok(())
}

pub async fn upload_file(
    term: Term,
    dracoon: &Dracoon<Connected>,
    source: PathBuf,
    target_node: &Node,
    opts: CmdUploadOptions,
) -> Result<(), DcCmdError> {
    info!("Attempting upload of file: {}.", source.to_string_lossy());
    info!("Target node: {}.", target_node.name);
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

    let progress_bar = ProgressBar::new(file_meta.size);
    progress_bar.set_style(
    ProgressStyle::default_bar()
    .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}) {msg}").unwrap()
    .progress_chars("=>-"),
);

    let progress_bar_mv = progress_bar.clone();

    progress_bar_mv.set_message("Uploading");
    progress_bar_mv.set_length(file_meta.size);

    let classification = opts.classification.unwrap_or(2);
    let resolution_strategy = if opts.overwrite {
        ResolutionStrategy::Overwrite
    } else {
        ResolutionStrategy::AutoRename
    };

    // only keep share links if overwrite is set
    let keep_share_links = match resolution_strategy {
        ResolutionStrategy::Overwrite => opts.keep_share_links,
        _ => false,
    };

    let upload_options = UploadOptions::builder(file_meta)
        .with_classification(classification)
        .with_resolution_strategy(resolution_strategy)
        .with_keep_share_links(keep_share_links)
        .build();

    let reader = tokio::io::BufReader::new(file);

    let node = dracoon
        .upload(
            target_node,
            upload_options,
            reader,
            Some(Box::new(move |progress, _| {
                progress_bar_mv.inc(progress);
            })),
            Some(DEFAULT_CHUNK_SIZE),
        )
        .await?;

    progress_bar.finish_with_message(format!("Upload of {file_name} complete"));
    info!("Upload of {} complete.", source.to_string_lossy());

    let is_encrypted = node.is_encrypted.unwrap_or(false);

    if !is_encrypted && opts.share {
        let link = share_node(dracoon, &node, opts.share_password).await?;
        let success_msg =
            format_success_message(format!("Shared {file_name}.\n▶︎▶︎ {link}").as_str());
        let success_msg = format!("\n{success_msg}");

        term.write_line(&success_msg).or(Err(DcCmdError::IoError))?;
    }

    Ok(())
}

pub async fn upload_files(
    dracoon: &Dracoon<Connected>,
    parent_node: &Node,
    files: BTreeMap<PathBuf, (u64, u64)>,
    opts: CmdUploadOptions,
) -> Result<(), DcCmdError> {
    info!("Attempting upload of {} files.", files.len());

    // equals min. 5 concurrent, max. 50 concurrent requests
    let velocity = opts
        .velocity
        .unwrap_or(MIN_VELOCITY)
        .clamp(MIN_VELOCITY, MAX_VELOCITY);

    let concurrent_reqs = velocity * DEFAULT_CONCURRENT_MULTIPLIER;
    let semaphore = Arc::new(tokio::sync::Semaphore::new(concurrent_reqs as usize));

    let total_size = files.values().map(|(_, size)| size).sum::<u64>();

    let progress_bar = ProgressBar::new(total_size);
    progress_bar.set_style(
        ProgressStyle::default_bar()
        .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}) {msg}").unwrap()
        .progress_chars("=>-"),
    );

    progress_bar.set_length(total_size);
    let count_files = files.len();
    let message = format!("Uploading {count_files} files");
    progress_bar.set_message(message.clone());
    let remaining_files = Arc::new(AtomicU64::new(files.len() as u64));
    let uploaded_files = Arc::new(AtomicUsize::new(0));

    let files_iter: Vec<_> = files.into_iter().collect();

    let mut handles = Vec::new();

    for (source, (node_id, _)) in files_iter {
        let dracoon = dracoon.clone();
        let progress_bar = progress_bar.clone();
        let progress_bar_mv = progress_bar.clone();
        let progress_bar_inc = progress_bar.clone();
        let client = dracoon.clone();
        let remaining_files = remaining_files.clone();
        let uploaded_files = uploaded_files.clone();
        let semaphore = semaphore.clone();

        let upload_task = async move {
            let _permit = semaphore.acquire().await.map_err(|err| {
                error!("Error acquiring semaphore: {}", err);
                DcCmdError::IoError
            })?;

            debug!("Uploading file: {}", source.to_string_lossy());
            let file = tokio::fs::File::open(&source).await.map_err(|err| {
                error!("Error opening file: {}", err);
                DcCmdError::IoError
            })?;

            let parent_node = client.nodes().get_node(node_id).await?;

            let file_meta = file.metadata().await.or(Err(DcCmdError::IoError))?;
            let file_meta = get_file_meta(&file_meta, &source)?;

            let file_name = file_meta.name.clone();

            let classification = opts.classification.unwrap_or(2);
            let resolution_strategy = if opts.overwrite {
                ResolutionStrategy::Overwrite
            } else {
                ResolutionStrategy::AutoRename
            };

            // only keep share links if overwrite is set
            let keep_share_links = match resolution_strategy {
                ResolutionStrategy::Overwrite => opts.keep_share_links,
                _ => false,
            };

            let upload_options = UploadOptions::builder(file_meta)
                .with_classification(classification)
                .with_resolution_strategy(resolution_strategy)
                .with_keep_share_links(keep_share_links)
                .build();

            let reader = tokio::io::BufReader::new(file);

            match client
                .upload(
                    &parent_node,
                    upload_options,
                    reader,
                    Some(Box::new(move |progress: u64, _total: u64| {
                        progress_bar_mv.inc(progress);
                    })),
                    None,
                )
                .await
                .inspect_err(|_e| {
                    error!("Error uploading file: {}", file_name);
                }) {
                Ok(_) => {
                    _ = &remaining_files.fetch_sub(1, Ordering::Relaxed);
                    _ = &uploaded_files.fetch_add(1, Ordering::Relaxed);
                    debug!("Uploaded file: {}", file_name);
                    let message = format!(
                        "Uploading {} files",
                        &remaining_files.load(Ordering::Relaxed)
                    );
                    progress_bar_inc.set_message(message);
                }
                Err(e) => {
                    error!("Error uploading file: {file_name} ({e})");
                    return Err(e.into());
                }
            }

            Ok::<(), DcCmdError>(())
        };

        handles.push(tokio::spawn(upload_task));
    }

    for handle in handles {
        if let Err(e) = handle.await {
            error!("Error uploading file: {}", e);
            return Err(DcCmdError::IoError);
        }
    }

    let target = parent_node.name.clone();

    progress_bar.finish_with_message(format!("Upload to {target} complete"));
    let uploaded_files = uploaded_files.load(Ordering::Relaxed);

    info!("Upload of {uploaded_files} files to {target} complete.");

    if uploaded_files != count_files {
        warn!(
            "Failed to upload {} files to {target}.",
            count_files - uploaded_files
        );
    }

    Ok(())
}

fn get_file_meta(file_meta: &Metadata, file_path: &Path) -> Result<FileMeta, DcCmdError> {
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

fn calculate_buffer_size(file_size: u64) -> usize {
    const MEGABYTE: u64 = 1024 * 1024;
    match file_size {
        0..=MEGABYTE => 16 * 1024,
        size if size <= 10 * MEGABYTE => 64 * 1024,
        size if size <= 100 * MEGABYTE => 128 * 1024,
        _ => 256 * 1024,
    }
}

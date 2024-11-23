use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};

use dco3::{
    auth::Connected,
    nodes::{Node, NodesSearchFilter, NodesSearchSortBy},
    Download, Dracoon, ListAllParams, Nodes, Public, PublicDownload, SortOrder,
};
use indicatif::{ProgressBar, ProgressStyle};
use tracing::{debug, error, info, warn};

use crate::cmd::{
    config::{DEFAULT_CONCURRENT_MULTIPLIER, MAX_CONCURRENT_REQUESTS, MAX_VELOCITY, MIN_VELOCITY},
    init_public_dracoon,
    models::DcCmdError,
    nodes::models::CmdDownloadOptions,
};

pub async fn get_files(
    dracoon: &Dracoon<Connected>,
    parent_node: &Node,
) -> Result<Vec<Node>, DcCmdError> {
    let params = ListAllParams::builder()
        .with_filter(NodesSearchFilter::is_file())
        .with_sort(NodesSearchSortBy::size(SortOrder::Desc))
        .build();

    let mut files = dracoon
        .nodes()
        .search_nodes("*", Some(parent_node.id), Some(-1), Some(params))
        .await?;

    if files.range.total > 500 {
        let (tx, mut rx) = tokio::sync::mpsc::channel(MAX_CONCURRENT_REQUESTS);
        let semaphore = Arc::new(tokio::sync::Semaphore::new(MAX_CONCURRENT_REQUESTS));
        let mut handles = Vec::new();

        (500..=files.range.total).step_by(500).for_each(|offset| {
            let dracoon_client = dracoon.clone();
            let parent_node = parent_node.clone();
            let tx = tx.clone();
            let semaphore = semaphore.clone();
            let handle = tokio::spawn(async move {
                let _permit = semaphore.acquire().await.map_err(|_| {
                    error!("Error acquiring semaphore permit");
                    DcCmdError::IoError
                })?;
                let params = ListAllParams::builder()
                    .with_filter(NodesSearchFilter::is_file())
                    .with_sort(NodesSearchSortBy::size(SortOrder::Desc))
                    .with_offset(offset)
                    .build();

                match dracoon_client
                    .nodes()
                    .search_nodes("*", Some(parent_node.id), Some(-1), Some(params))
                    .await
                {
                    Ok(batch_items) => {
                        if let Err(e) = tx.send(batch_items.items).await {
                            error!("Error processing files: {}", e);
                            return Err(DcCmdError::IoError);
                        }
                    }
                    Err(e) => {
                        error!("Error getting files: {}", e);
                        return Err(e.into());
                    }
                }

                Ok(())
            });

            handles.push(handle);
        });

        drop(tx);

        while let Some(next_files) = rx.recv().await {
            files.items.extend(next_files);
        }

        for handle in handles {
            if let Err(e) = handle.await {
                error!("Error getting files: {}", e);
                return Err(DcCmdError::IoError);
            }
        }
    }

    let actual_count = files.items.len() as u64;
    if files.range.total != actual_count {
        warn!(
            "Total file count mismatch - expected: {}, actual: {}, difference: {} (check error logs)", 
            files.range.total,
            actual_count,
            files.range.total - actual_count
        );
    }

    Ok(files.get_files())
}

pub async fn download_public_file(
    source: String,
    target: String,
    download_opts: CmdDownloadOptions,
) -> Result<(), DcCmdError> {
    if download_opts.recursive {
        return Err(DcCmdError::InvalidArgument(
            "Recursive download not supported for public download shares".to_string(),
        ));
    }

    let access_key = source
        .split('/')
        .last()
        .ok_or(DcCmdError::InvalidPath(source.clone()))?;

    let dracoon = init_public_dracoon(&source).await?;

    let public_download_share = dracoon
        .public()
        .get_public_download_share(access_key)
        .await?;
    let file_name = public_download_share.file_name.clone();

    let original_target = target.to_string();

    // if own name provided - use it - otherwise use node name
    let target = if std::path::Path::new(&target).is_dir() {
        let path = std::path::Path::new(&target);
        let target = path.join(&public_download_share.file_name);

        let Some(target) = target.to_str() else {
            return Err(DcCmdError::InvalidPath(original_target));
        };

        target.to_string()
    } else {
        target.to_string()
    };

    let mut out_file = tokio::fs::File::create(target)
        .await
        .or(Err(DcCmdError::IoError))?;

    let progress_bar = ProgressBar::new(public_download_share.size);
    progress_bar.set_style(
    ProgressStyle::default_bar()
    .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}) {msg}").unwrap()
    .progress_chars("=>-"),
);

    progress_bar.set_length(public_download_share.size);

    let progress_bar_mv = progress_bar.clone();

    dracoon
        .public()
        .download(
            access_key,
            public_download_share.clone(),
            download_opts.share_password,
            &mut out_file,
            Some(Box::new(move |progress, _| {
                progress_bar_mv.set_message(public_download_share.clone().file_name);
                progress_bar_mv.inc(progress);
            })),
            None,
        )
        .await?;

    progress_bar.finish_with_message(format!("{file_name} complete"));

    info!("Download of public file {file_name} complete.");

    Ok(())
}

pub async fn download_file(
    dracoon: &Dracoon<Connected>,
    node: &Node,
    target: &str,
) -> Result<(), DcCmdError> {
    info!("Attempting download of node {}.", node.name);
    info!("Target: {}", target);

    let original_target = target.to_string();

    // if own name provided - use it - otherwise use node name
    let target = if std::path::Path::new(target).is_dir() {
        let path = std::path::Path::new(target);
        let target = path.join(node.name.clone());

        let Some(target) = target.to_str() else {
            return Err(DcCmdError::InvalidPath(original_target));
        };

        target.to_string()
    } else {
        target.to_string()
    };

    let mut out_file = tokio::fs::File::create(target)
        .await
        .or(Err(DcCmdError::IoError))?;

    let progress_bar = ProgressBar::new(node.size.unwrap_or(0));
    progress_bar.set_style(
        ProgressStyle::default_bar()
        .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}) {msg}").unwrap()
        .progress_chars("=>-"),
    );

    progress_bar.set_length(node.size.unwrap_or(0));
    let node_name = node.name.clone();
    progress_bar.set_message(node_name.clone());

    let progress_bar_mv = progress_bar.clone();

    dracoon
        .download(
            node,
            &mut out_file,
            Some(Box::new(move |progress, _| {
                progress_bar_mv.inc(progress);
            })),
            None,
        )
        .await?;

    progress_bar.finish_with_message(format!("{node_name} complete"));

    info!("Download of node {} complete.", node_name.clone());

    Ok(())
}

pub async fn download_files(
    dracoon: &Dracoon<Connected>,
    files: Vec<Node>,
    target: &str,
    targets: Option<HashMap<u64, String>>,
    velocity: Option<u8>,
) -> Result<(), DcCmdError> {
    info!("Attempting download of {} files.", files.len());
    info!("Target: {}", target);

    let velocity = velocity
        .unwrap_or(MIN_VELOCITY)
        .clamp(MIN_VELOCITY, MAX_VELOCITY);

    let concurrent_reqs = velocity * DEFAULT_CONCURRENT_MULTIPLIER;
    let semaphore = Arc::new(tokio::sync::Semaphore::new(concurrent_reqs as usize));

    let dracoon = dracoon.clone();

    let total_size = files.iter().map(|node| node.size.unwrap_or(0)).sum();
    let file_count = files.len();

    let progress_bar = ProgressBar::new(total_size);
    progress_bar.set_style(
        ProgressStyle::default_bar()
        .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}) {msg}").unwrap()
        .progress_chars("=>-"),
    );

    progress_bar.set_length(total_size);
    let message = format!("Downloading {} files", files.len());
    progress_bar.set_message(message.clone());
    let remaining_files = Arc::new(AtomicU64::new(files.len() as u64));
    let mut handles = Vec::new();

    for file in files {
        let dracoon_client = dracoon.clone();
        let target = target.to_string();
        debug!("Target: {}", target);
        let targets = targets.clone();

        let progress_bar_mv = progress_bar.clone();
        let progress_bar_inc = progress_bar.clone();
        let rm_files = remaining_files.clone();
        let semaphore = semaphore.clone();
        let download_task = async move {
            let _permit = semaphore.acquire().await.map_err(|_| {
                error!("Error acquiring semaphore permit");
                DcCmdError::IoError
            })?;

            let target = if let Some(targets) = targets {
                let target = targets.get(&file.id).expect("Target not found").clone();
                std::path::PathBuf::from(target)
            } else {
                let target = std::path::PathBuf::from(target);
                target.join(&file.name)
            };

            let mut out_file = tokio::fs::File::create(&target)
                .await
                .or(Err(DcCmdError::IoError))?;

            let node_name = file.name.clone();

            dracoon_client
                .download(
                    &file,
                    &mut out_file,
                    Some(Box::new(move |progress, _| {
                        progress_bar_mv.inc(progress);
                    })),
                    None,
                )
                .await
                .map_err(|e| {
                    error!("Error downloading file: {}", node_name);
                    error!("{:?}", e);
                    e
                })?;

            _ = &rm_files.fetch_sub(1, Ordering::Relaxed);
            let message = format!("Downloading {} files", &rm_files.load(Ordering::Relaxed));
            progress_bar_inc.set_message(message);
            Ok::<(), DcCmdError>(())
        };

        handles.push(tokio::spawn(download_task));
    }

    for handle in handles {
        if let Err(e) = handle.await {
            error!("Error uploading file: {}", e);
            return Err(DcCmdError::IoError);
        }
    }

    progress_bar.finish_with_message(format!("Download to {target} complete"));

    info!("Download of {file_count} files complete.");

    Ok(())
}

use std::{
    collections::HashMap,
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

use futures_util::future::join_all;
use indicatif::{ProgressBar, ProgressStyle};
use tracing::{debug, error, info};

use crate::cmd::{
    init_dracoon, init_encryption,
    models::{DcCmdError, PasswordAuth},
    nodes::{is_search_query, search_nodes},
    utils::strings::parse_path,
};

use dco3::{
    auth::Connected,
    nodes::{
        models::{filters::NodesSearchFilter, sorts::NodesSearchSortBy, Node, NodeType},
        Download, Nodes,
    },
    Dracoon, ListAllParams, SortOrder,
};

pub async fn download(
    source: String,
    target: String,
    velocity: Option<u8>,
    recursive: bool,
    auth: Option<PasswordAuth>,
    encryption_password: Option<String>,
) -> Result<(), DcCmdError> {
    debug!("Downloading {} to {}", source, target);
    debug!("Velocity: {}", velocity.unwrap_or(1));

    let mut dracoon = init_dracoon(&source, auth, true).await?;

    let (parent_path, node_name, _) = parse_path(&source, dracoon.get_base_url().as_ref())
        .or(Err(DcCmdError::InvalidPath(source.clone())))?;
    let node_path = format!("{parent_path}{node_name}/");

    let node = if is_search_query(&node_name) {
        debug!("Searching for query {}", node_name);
        debug!("Parent path {}", parent_path);
        dracoon.nodes.get_node_from_path(&parent_path).await?
    } else {
        dracoon.nodes.get_node_from_path(&node_path).await?
    };

    let Some(node) = node else {
        error!("Node not found");
        return Err(DcCmdError::InvalidPath(source.clone()));
    };

    if node.is_encrypted == Some(true) {
        dracoon = init_encryption(dracoon, encryption_password).await?;
    }

    if is_search_query(&node_name) {
        info!("Attempting download of search query {}.", node_name);
        let files =
            search_nodes(&dracoon, &node_name, Some(&parent_path), None, true, 0, 500).await?;
        let files = files.get_files();

        info!("Found {} files.", files.len());

        download_files(&mut dracoon, files, &target, None, velocity).await
    } else {
        match node.node_type {
            NodeType::File => download_file(&mut dracoon, &node, &target).await,
            _ => {
                if recursive {
                    download_container(&mut dracoon, &node, &target, velocity).await
                } else {
                    Err(DcCmdError::InvalidArgument(
                        "Container download requires recursive flag".to_string(),
                    ))
                }
            }
        }
    }
}

async fn download_file(
    dracoon: &mut Dracoon<Connected>,
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

    let progress_bar_mv = progress_bar.clone();

    let node_name = node.name.clone();
    let node_name_clone = node_name.clone();

    dracoon
        .download(
            node,
            &mut out_file,
            Some(Box::new(move |progress, total| {
                progress_bar_mv.set_message(node_name_clone.clone());
                progress_bar_mv.inc(progress);
            })),
        )
        .await?;

    progress_bar.finish_with_message(format!("{} complete", node_name.clone()));

    info!("Download of node {} complete.", node_name.clone());

    Ok(())
}

async fn download_files(
    dracoon: &mut Dracoon<Connected>,
    files: Vec<Node>,
    target: &str,
    targets: Option<HashMap<u64, String>>,
    velocity: Option<u8>,
) -> Result<(), DcCmdError> {
    info!("Attempting download of {} files.", files.len());
    info!("Target: {}", target);

    let velocity = velocity.unwrap_or(1).clamp(1, 10);

    let concurrent_reqs = velocity * 5;

    let dracoon = dracoon.clone();

    let total_size = files.iter().map(|node| node.size.unwrap_or(0)).sum();

    let progress_bar = ProgressBar::new(total_size);
    progress_bar.set_style(
        ProgressStyle::default_bar()
        .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}) {msg}").unwrap()
        .progress_chars("=>-"),
    );

    progress_bar.set_length(total_size);
    let message = format!("Downloading {} files", files.len());
    progress_bar.set_message(message.clone());
    let remaining_files = AtomicU64::new(files.len() as u64);

    for batch in files.chunks(concurrent_reqs.into()) {
        let mut download_reqs = vec![];
        for file in batch {
            let dracoon_client = dracoon.clone();
            let target = target.to_string();
            debug!("Target: {}", target);
            let targets = targets.clone();

            let progress_bar_mv = progress_bar.clone();
            let progress_bar_inc = progress_bar.clone();
            let rm_files = &remaining_files;
            let download_task = async move {
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
                        file,
                        &mut out_file,
                        Some(Box::new(move |progress, _| {
                            progress_bar_mv.inc(progress);
                        })),
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
                Ok(())
            };

            download_reqs.push(download_task);
        }

        let results: Vec<Result<(), DcCmdError>> = join_all(download_reqs).await;
        for result in results {
            if let Err(e) = result {
                error!("Error downloading file: {}", e);
            }
        }
    }

    progress_bar.finish_with_message(format!("Download to {target} complete"));

    info!("Download of {} files complete.", files.len());

    Ok(())
}

async fn download_container(
    dracoon: &mut Dracoon<Connected>,
    node: &Node,
    target: &str,
    velocity: Option<u8>,
) -> Result<(), DcCmdError> {
    info!("Attempting download of container {}.", node.name);
    info!("Target: {}", target);

    // indicate listing files and folders
    let progress_spinner = ProgressBar::new_spinner();
    progress_spinner.set_message("Listing files and folders...");
    progress_spinner.enable_steady_tick(Duration::from_millis(100));

    // first get all folders below parent
    let folders = get_folders(dracoon, node).await?;

    // create root directory on target
    let target = std::path::PathBuf::from(target);
    let target = target.clone().join(&node.name);
    std::fs::create_dir_all(&target).or(Err(DcCmdError::IoError))?;

    let base_path = node
        .clone()
        .parent_path
        .expect("Node has no parent path")
        .trim_end_matches('/')
        .to_string();

    // create all sub folders
    create_folders(&target, node, &base_path, folders)?;

    // get all files
    let files = get_files(dracoon, node).await?;

    // remove files in sub rooms
    let files = filter_files_in_sub_rooms(dracoon, node, files).await?;

    progress_spinner.finish_and_clear();

    // download all files
    let mut targets = HashMap::new();

    for file in &files {
        let file_target = target.clone();
        let file_base_path = file
            .clone()
            .parent_path
            .expect("File has no parent path")
            .trim_start_matches(&base_path)
            .to_string();
        let parent = format!("/{}", node.name.clone());
        let file_base_path = file_base_path.trim_start_matches(&parent);
        let file_base_path = file_base_path.trim_start_matches('/');
        let file_target = file_target.join(file_base_path);
        let target = file_target.join(&file.name);

        targets.insert(
            file.id,
            target.to_str().expect("Path has no content").to_string(),
        );
    }

    download_files(
        dracoon,
        files,
        target.to_str().expect("Path has no content"),
        Some(targets),
        velocity,
    )
    .await?;

    info!("Download of container {} complete.", node.name);

    Ok(())
}

async fn get_files(
    dracoon: &Dracoon<Connected>,
    parent_node: &Node,
) -> Result<Vec<Node>, DcCmdError> {
    // get all the files
    let params = ListAllParams::builder()
        .with_filter(NodesSearchFilter::is_file())
        .with_sort(NodesSearchSortBy::parent_path(SortOrder::Asc))
        .build();

    let mut files = dracoon
        .nodes
        .search_nodes("*", Some(parent_node.id), Some(-1), Some(params))
        .await?;

    // TODO: refactor to max velocity (currently all reqs are run in parallel)
    if files.range.total > 500 {
        let mut file_reqs = vec![];

        (500..files.range.total).step_by(500).for_each(|offset| {
            let dracoon_client = dracoon.clone();
            let parent_node = parent_node.clone();
            let get_task = async move {
                let params = ListAllParams::builder()
                    .with_filter(NodesSearchFilter::is_file())
                    .with_sort(NodesSearchSortBy::parent_path(SortOrder::Asc))
                    .with_offset(offset)
                    .build();

                let files = dracoon_client
                    .nodes
                    .search_nodes("*", Some(parent_node.id), Some(-1), Some(params))
                    .await?;

                Ok(files.items)
            };
            file_reqs.push(get_task);
        });

        let results: Vec<Result<Vec<Node>, DcCmdError>> = join_all(file_reqs).await;

        for result in results {
            match result {
                Ok(new_files) => files.items.extend(new_files),
                Err(e) => {
                    error!("Error getting files: {}", e);
                    return Err(e);
                }
            }
        }
    }

    Ok(files.get_files())
}

async fn filter_files_in_sub_rooms(
    dracoon: &Dracoon<Connected>,
    parent_node: &Node,
    files: Vec<Node>,
) -> Result<Vec<Node>, DcCmdError> {
    let params = ListAllParams::builder()
        .with_filter(NodesSearchFilter::is_room())
        .with_sort(NodesSearchSortBy::parent_path(dco3::SortOrder::Asc))
        .build();

    debug!("Total file count: {}", files.len());

    // ignore files in sub rooms
    let sub_rooms = dracoon
        .nodes
        .search_nodes("*", Some(parent_node.id), None, Some(params))
        .await?;

    // TODO: handle more than 500 sub rooms on first level
    let sub_room_paths = sub_rooms
        .get_rooms()
        .into_iter()
        .map(|r| format!("{}{}/", r.parent_path.unwrap_or_else(|| "/".into()), r.name))
        .collect::<Vec<_>>();

    Ok(files
        .into_iter()
        .filter(|f| {
            !sub_room_paths.iter().any(|p| {
                f.parent_path
                    .as_ref()
                    .unwrap_or(&String::new())
                    .starts_with(p)
            })
        })
        .collect::<Vec<_>>())
}

async fn get_folders(
    dracoon: &Dracoon<Connected>,
    parent_node: &Node,
) -> Result<Vec<Node>, DcCmdError> {
    let params = ListAllParams::builder()
        .with_filter(NodesSearchFilter::is_folder())
        .with_sort(NodesSearchSortBy::parent_path(SortOrder::Asc))
        .build();

    let mut folders = dracoon
        .nodes
        .search_nodes("*", Some(parent_node.id), Some(-1), Some(params))
        .await?;

    // TODO: refactor to max velocity (currently all reqs are run in parallel)
    if folders.range.total > 500 {
        let mut folder_reqs = vec![];

        (500..folders.range.total).step_by(500).for_each(|offset| {
            let dracoon_client = dracoon.clone();
            let parent_node = parent_node.clone();
            let get_task = async move {
                let params = ListAllParams::builder()
                    .with_filter(NodesSearchFilter::is_folder())
                    .with_sort(NodesSearchSortBy::parent_path(SortOrder::Asc))
                    .with_offset(500)
                    .build();

                let folders = dracoon_client
                    .nodes
                    .search_nodes("*", Some(parent_node.id), Some(-1), Some(params))
                    .await?;

                Ok(folders.items)
            };

            folder_reqs.push(get_task);
        });

        let results: Vec<Result<Vec<Node>, DcCmdError>> = join_all(folder_reqs).await;

        for result in results {
            match result {
                Ok(new_folders) => folders.items.extend(new_folders),
                Err(e) => {
                    error!("Error getting folders: {}", e);
                    return Err(e);
                }
            }
        }
    }

    Ok(folders.get_folders())
}

fn create_folders(
    target: &Path,
    parent_node: &Node,
    base_path: &str,
    folders: Vec<Node>,
) -> Result<(), DcCmdError> {
    // create all sub directories
    for folder in folders {
        let curr_target = target;

        let folder_base_path = folder
            .clone()
            .parent_path
            .expect("Folder has no parent path")
            .trim_start_matches(base_path)
            .to_string()
            .trim_start_matches('/')
            .to_string();
        let folder_base_path = folder_base_path
            .trim_start_matches(format!("{}/", parent_node.name).as_str())
            .to_string();
        debug!("Folder base path: {}", folder_base_path);
        let curr_target = curr_target.join(folder_base_path);
        let curr_target = curr_target.join(folder.name);

        std::fs::create_dir_all(&curr_target).map_err(|_| {
            error!("Error creating directory: {:?}", curr_target);
            DcCmdError::IoError
        })?;
    }

    Ok(())
}

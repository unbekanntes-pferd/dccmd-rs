use std::{collections::HashMap, path::Path, sync::Arc, time::Duration};

use dco3::{
    auth::Connected,
    nodes::{Node, NodeType, NodesSearchFilter, NodesSearchSortBy},
    Dracoon, ListAllParams, Nodes, SortOrder,
};
use indicatif::ProgressBar;
use tracing::{debug, error, info};

use crate::cmd::{
    config::MAX_CONCURRENT_REQUESTS,
    models::DcCmdError,
    nodes::download::files::{download_files, get_files},
};

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
            .unwrap_or("/".to_string())
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
        .nodes()
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

async fn get_containers(
    dracoon: &Dracoon<Connected>,
    parent_node: &Node,
    include_rooms: bool,
) -> Result<Vec<Node>, DcCmdError> {
    let filter = if include_rooms {
        NodesSearchFilter::is_types(vec![NodeType::Folder, NodeType::Room])
    } else {
        NodesSearchFilter::is_folder()
    };

    let params = ListAllParams::builder()
        .with_filter(filter.clone())
        .with_sort(NodesSearchSortBy::parent_path(SortOrder::Asc))
        .build();

    let mut folders = dracoon
        .nodes()
        .search_nodes("*", Some(parent_node.id), Some(-1), Some(params))
        .await?;

    if folders.range.total > 500 {
        let (tx, mut rx) = tokio::sync::mpsc::channel(MAX_CONCURRENT_REQUESTS);
        let semaphore = Arc::new(tokio::sync::Semaphore::new(MAX_CONCURRENT_REQUESTS));
        let mut handles = Vec::new();

        (500..=folders.range.total).step_by(500).for_each(|offset| {
            let tx = tx.clone();
            let dracoon_client = dracoon.clone();
            let parent_node = parent_node.clone();
            let filter = filter.clone();
            let semaphore = semaphore.clone();
            let handle = tokio::spawn(async move {
                let _permit = semaphore.acquire().await.map_err(|_| {
                    error!("Error acquiring semaphore permit");
                    DcCmdError::IoError
                })?;
                let params = ListAllParams::builder()
                    .with_filter(filter)
                    .with_sort(NodesSearchSortBy::parent_path(SortOrder::Asc))
                    .with_offset(offset)
                    .build();

                match dracoon_client
                    .nodes()
                    .search_nodes("*", Some(parent_node.id), Some(-1), Some(params))
                    .await
                {
                    Ok(batch_items) => {
                        if let Err(e) = tx.send(batch_items.items).await {
                            error!("Error sending folders: {}", e);
                            return Err(DcCmdError::IoError);
                        }

                        Ok(())
                    }
                    Err(e) => {
                        error!("Error getting folders: {}", e);
                        Err(e.into())
                    }
                }
            });
            handles.push(handle);
        });

        drop(tx);

        while let Some(next_folders) = rx.recv().await {
            folders.items.extend(next_folders);
        }

        for handle in handles {
            if let Err(e) = handle.await {
                error!("Error getting folders: {}", e);
                return Err(DcCmdError::IoError);
            }
        }
    }

    let node_filter = if include_rooms {
        |node: &Node| node.node_type == NodeType::Folder || node.node_type == NodeType::Room
    } else {
        |node: &Node| node.node_type == NodeType::Folder
    };

    Ok(folders
        .items
        .iter()
        .filter(|node| node_filter(node))
        .cloned()
        .collect())
}

pub async fn download_container(
    dracoon: &Dracoon<Connected>,
    node: &Node,
    target: &str,
    velocity: Option<u8>,
    include_rooms: bool,
) -> Result<(), DcCmdError> {
    info!("Attempting download of container {}.", node.name);
    info!("Target: {}", target);

    // indicate listing files and folders
    let progress_spinner = ProgressBar::new_spinner();
    progress_spinner.set_message("Listing files and folders...");
    progress_spinner.enable_steady_tick(Duration::from_millis(100));

    // first get all folders below parent
    let folders = get_containers(dracoon, node, include_rooms).await?;

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
    let files = if include_rooms {
        files
    } else {
        filter_files_in_sub_rooms(dracoon, node, files).await?
    };

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

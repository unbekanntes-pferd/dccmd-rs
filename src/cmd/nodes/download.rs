use std::collections::HashMap;

use futures_util::{future::join_all};
use indicatif::{ProgressBar, ProgressStyle};
use tracing::{debug, error};

use crate::{
    api::{
        auth::Connected,
        models::ListAllParams,
        nodes::{
            models::{Node, NodeType},
            Download, Nodes,
        },
        Dracoon,
    },
    cmd::{
        init_dracoon, init_encryption,
        models::DcCmdError,
        nodes::{is_search_query, search_nodes},
        utils::strings::parse_path,
    },
};

pub async fn download(
    source: String,
    target: String,
    velocity: Option<u8>,
    recursive: bool,
) -> Result<(), DcCmdError> {
    debug!("Downloading {} to {}", source, target);
    debug!("Velocity: {}", velocity.unwrap_or(1));

    let mut dracoon = init_dracoon(&source).await?;

    let (parent_path, node_name, _) = parse_path(&source, dracoon.get_base_url().as_ref())
        .or(Err(DcCmdError::InvalidPath(source.clone())))?;
    let node_path = format!("{parent_path}{node_name}/");

    let node = if is_search_query(&node_name) {
        debug!("Searching for query {}", node_name);
        debug!("Parent path {}", parent_path);
        dracoon.get_node_from_path(&parent_path).await?
    } else {
        dracoon.get_node_from_path(&node_path).await?
    };

    let Some(node) = node else {
        error!("Node not found");
        return Err(DcCmdError::InvalidPath(source.clone()))
    };

    if node.is_encrypted == Some(true) {
        dracoon = init_encryption(dracoon).await?;
    }

    if is_search_query(&node_name) {
        let files =
            search_nodes(&dracoon, &node_name, Some(&parent_path), None, true, 0, 500).await?;
        let files = files.get_files();

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
    let mut out_file = std::fs::File::create(target).or(Err(DcCmdError::IoError))?;

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

    Ok(())
}

async fn download_files(
    dracoon: &mut Dracoon<Connected>,
    files: Vec<Node>,
    target: &str,
    targets: Option<HashMap<u64, String>>,
    velocity: Option<u8>,
) -> Result<(), DcCmdError> {
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
    let mut remaining_files = files.len();

    for batch in files.chunks(concurrent_reqs.into()) {
        let mut download_reqs = vec![];
        for file in batch {
            let mut dracoon_client = dracoon.clone();
            let target = target.to_string();
            debug!("Target: {}", target);
            let targets = targets.clone();

            let progress_bar_mv = progress_bar.clone();
            let progress_bar_inc = progress_bar.clone();
            let download_task = async move {
                let target = if let Some(targets) = targets {
                    let target = targets.get(&file.id).expect("Target not found").clone();
                    std::path::PathBuf::from(target)
                } else {
                    let target = std::path::PathBuf::from(target);
                    target.join(&file.name)
                };

                let mut out_file = std::fs::File::create(&target).or(Err(DcCmdError::IoError))?;

                let node_name = file.name.clone();

                dracoon_client
                    .download(
                        file,
                        &mut out_file,
                        Some(Box::new(move |progress, _| {
                            progress_bar_mv.inc(progress);
                        })),
                    )
                    .await?;

                remaining_files -= 1;
                let message = format!("Downloading {remaining_files} files");
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

    Ok(())
}

#[allow(clippy::too_many_lines)] // TODO: refactor (e.g. fetch > 500 files, folders)
async fn download_container(
    dracoon: &mut Dracoon<Connected>,
    node: &Node,
    target: &str,
    velocity: Option<u8>,
) -> Result<(), DcCmdError> {
    // first get all folders below parent
    let params = ListAllParams::builder()
        .with_filter("type:eq:folder".into())
        .with_sort("parentPath:asc".into())
        .build();

    let mut folders = dracoon
        .search_nodes("*", Some(node.id), Some(-1), Some(params))
        .await?;

    if folders.range.total > 500 {
        for offset in (500..folders.range.total).into_iter().step_by(500) {
            let result = dracoon
                    .search_nodes(
                        "*",
                        Some(node.id),
                        Some(-1),
                        Some(
                            ListAllParams::builder()
                                .with_filter("type:eq:folder".into())
                                .with_sort("parentPath:asc".into())
                                .with_offset(offset)
                                .build(),
                        ),
                    )
                    .await?;

            folders.items.extend(result.items);
        }
    }

    let folders = folders.get_folders();

    // create a directory on target
    let target = std::path::PathBuf::from(target);
    let target = target.clone().join(&node.name);

    std::fs::create_dir_all(&target).or(Err(DcCmdError::IoError))?;

    let base_path = node
        .clone()
        .parent_path
        .expect("Node has no parent path")
        .trim_end_matches('/')
        .to_string();

    // create all other directories
    for folder in folders {
        let curr_target = target.clone();

        let folder_base_path = folder
            .clone()
            .parent_path
            .expect("Folder has no parent path")
            .trim_start_matches(&base_path)
            .to_string()
            .trim_start_matches('/')
            .to_string();
        let folder_base_path = folder_base_path
            .trim_start_matches(format!("{}/", node.name).as_str())
            .to_string();
        debug!("Folder base path: {}", folder_base_path);
        let curr_target = curr_target.join(folder_base_path);
        let curr_target = curr_target.join(folder.name);

        std::fs::create_dir_all(&curr_target).map_err(|_| {
            error!("Error creating directory: {:?}", curr_target);
            DcCmdError::IoError
        })?;
    }

    // get all the files
    let params = ListAllParams::builder()
        .with_filter("type:eq:file".into())
        .with_sort("parentPath:asc".into())
        .build();

    let mut files = dracoon
        .search_nodes("*", Some(node.id), Some(-1), Some(params))
        .await?;


    if files.range.total > 500 {

        for offset in (500..files.range.total).into_iter().step_by(500) {
            let result = dracoon
                    .search_nodes(
                        "*",
                        Some(node.id),
                        Some(-1),
                        Some(
                            ListAllParams::builder()
                                .with_filter("type:eq:file".into())
                                .with_sort("parentPath:asc".into())
                                .with_offset(offset)
                                .build(),
                        ),
                    )
                    .await?;

            files.items.extend(result.items);
        }
    }

    let files = files.get_files();

    let params = ListAllParams::builder()
        .with_filter("type:eq:room".into())
        .with_sort("parentPath:asc".into())
        .build();

    debug!("Total file count: {}", files.len());

    // ignore files in sub rooms
    let sub_rooms = dracoon
        .search_nodes("*", Some(node.id), Some(-1), Some(params))
        .await?;
    let sub_room_paths = sub_rooms
        .get_rooms()
        .into_iter()
        .map(|r| format!("{}{}/", r.parent_path.unwrap_or_else(|| "/".into()), r.name))
        .collect::<Vec<_>>();

    let files = files
        .into_iter()
        .filter(|f| {
            !sub_room_paths.iter().any(|p| {
                f.parent_path
                    .as_ref()
                    .unwrap_or(&String::new())
                    .starts_with(p)
            })
        })
        .collect::<Vec<_>>();

    debug!("File count (no rooms): {}", files.len());

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

    Ok(())
}

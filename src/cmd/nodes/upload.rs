use std::{
    collections::BTreeMap,
    fs::Metadata,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, SystemTime},
};

use async_recursion::async_recursion;
use console::Term;
use futures_util::future::join_all;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};

use tracing::{debug, error, info};

use crate::cmd::{
    init_dracoon, init_encryption,
    models::{DcCmdError, PasswordAuth},
    nodes::share::share_node,
    utils::{
        dates::to_datetime_utc,
        strings::{format_success_message, parse_path},
    },
};
use dco3::{
    auth::Connected,
    nodes::{
        models::{FileMeta, ResolutionStrategy, UploadOptions},
        CreateFolderRequest, Node, Nodes, Upload,
    },
    Dracoon, DracoonClientError, Folders,
};

// this is currently set low to display progress
// TODO: fix dco3 chunk progress for uploads
const DEFAULT_CHUNK_SIZE: usize = 1024 * 1024 * 5; // 5 MB

pub async fn upload(
    term: Term,
    source: PathBuf,
    target: String,
    options: super::models::UploadOptions,
    auth: Option<PasswordAuth>,
    encryption_password: Option<String>,
) -> Result<(), DcCmdError> {
    let mut dracoon = init_dracoon(&target, auth, true).await?;

    let (parent_path, node_name, _) = parse_path(&target, dracoon.get_base_url().as_str())
        .or(Err(DcCmdError::InvalidPath(target.clone())))?;
    let node_path = format!("{parent_path}{node_name}/");

    let parent_node = dracoon.nodes.get_node_from_path(&node_path).await?;

    let Some(parent_node) = parent_node else {
        error!("Target path not found: {}", target);
        return Err(DcCmdError::InvalidPath(target.clone()));
    };

    if parent_node.is_encrypted == Some(true) {
        dracoon = init_encryption(dracoon, encryption_password).await?;
    }

    if parent_node.is_encrypted.unwrap_or(false) && options.share {
        error!("Parent node is encrypted. Cannot upload to encrypted nodes.");
        return Err(DcCmdError::InvalidArgument(
            "Sharing encrypted files currently not supported (remove --share flag).".to_string(),
        ));
    }

    match (source.is_file(), source.is_dir(), options.recursive) {
        // is a file
        (true, _, _) => {
            upload_file(
                term,
                &dracoon,
                source,
                &parent_node,
                options.overwrite,
                options.classification,
                options.share,
            )
            .await?
        }
        // is a directory and recursive flag is set
        (_, true, true) => {
            upload_container(
                &dracoon,
                source,
                &parent_node,
                &node_path,
                options.overwrite,
                options.skip_root,
                options.classification,
                options.velocity,
            )
            .await?
        }
        // is a directory and recursive flag is not set
        (_, true, false) => {
            return Err(DcCmdError::InvalidArgument(
                "Container upload requires recursive flag".to_string(),
            ));
        }
        // is neither a file nor a directory
        _ => {
            return Err(DcCmdError::InvalidPath(
                source.to_string_lossy().to_string(),
            ));
        }
    }

    Ok(())
}

async fn upload_file(
    term: Term,
    dracoon: &Dracoon<Connected>,
    source: PathBuf,
    target_node: &Node,
    overwrite: bool,
    classification: Option<u8>,
    share: bool,
) -> Result<(), DcCmdError> {
    info!("Attempting upload of file: {}.", source.to_string_lossy());
    info!("Target node: {}.", target_node.name);
    let file = tokio::fs::File::open(&source)
        .await
        .or(Err(DcCmdError::IoError))?;

    let file_meta = file.metadata().await.or(Err(DcCmdError::IoError))?;

    if !file_meta.is_file() {
        return Err(DcCmdError::InvalidPath(
            source.to_string_lossy().to_string(),
        ));
    }

    let file_meta = get_file_meta(&file_meta, &source)?;
    let file_name = file_meta.0.clone();

    let progress_bar = ProgressBar::new(target_node.size.unwrap_or(0));
    progress_bar.set_style(
    ProgressStyle::default_bar()
    .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}) {msg}").unwrap()
    .progress_chars("=>-"),
);

    let progress_bar_mv = progress_bar.clone();

    progress_bar_mv.set_message("Uploading");
    progress_bar_mv.set_length(file_meta.1);

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

    let node = dracoon
        .upload(
            file_meta,
            target_node,
            upload_options,
            reader,
            Some(Box::new(move |progress, total| {
                progress_bar_mv.set_position(progress);
            })),
            Some(DEFAULT_CHUNK_SIZE),
        )
        .await?;

    progress_bar.finish_with_message(format!("Upload of {file_name} complete"));
    info!("Upload of {} complete.", source.to_string_lossy());

    let is_encrypted = node.is_encrypted.unwrap_or(false);

    if !is_encrypted && share {
        let link = share_node(dracoon, &node).await?;
        let success_msg =
            format_success_message(format!("Shared {}.\n▶︎▶︎ {}", file_name, link).as_str());
        let success_msg = format!("\n{}", success_msg);

        term.write_line(&success_msg)
            .expect("Error writing message to terminal.");
    }

    Ok(())
}

#[allow(clippy::too_many_lines, clippy::too_many_arguments)]
async fn upload_container(
    dracoon: &Dracoon<Connected>,
    source: PathBuf,
    target: &Node,
    target_parent: &str,
    overwrite: bool,
    skip_root: bool,
    classification: Option<u8>,
    velocity: Option<u8>,
) -> Result<(), DcCmdError> {
    info!("Attempting upload of folder: {}.", source.to_string_lossy());
    info!("Target node: {}.", target.name);

    // create folder first
    let name = source
        .file_name()
        .ok_or(DcCmdError::InvalidPath(
            source.to_string_lossy().to_string(),
        ))?
        .to_str()
        .ok_or(DcCmdError::InvalidPath(
            source.to_string_lossy().to_string(),
        ))?
        .to_string();

    if source.is_relative() {
        error!("Only absolute paths are supported.");
        return Err(DcCmdError::InvalidPath(
            source.to_string_lossy().to_string(),
        ));
    }

    let progress = MultiProgress::new();
    let progress_spinner = ProgressBar::new_spinner();
    progress_spinner.set_message("Creating folder structure...");
    progress_spinner.enable_steady_tick(Duration::from_millis(100));
    progress.add(progress_spinner);
    let parent_id = if skip_root {
        info!("Skipping root folder.");
        target.id
    } else {
        let root_folder = CreateFolderRequest::builder(&name, target.id).build();

        let root_folder = match dracoon.nodes.create_folder(root_folder).await {
            Ok(folder) => folder,
            Err(e) if e.is_conflict() => {
                let path = format!("{}{}", target_parent, name);
                debug!("Path: {}", path);
                dracoon
                    .nodes
                    .get_node_from_path(&path)
                    .await?
                    .ok_or_else(|| {
                        error!("Error creating root folder: {}", e);
                        e
                    })?
            }
            Err(e) => {
                error!("Error creating root folder: {}", e);
                return Err(e.into());
            }
        };

        root_folder.id
    };

    let (files, folders) =
        tokio::join!(list_files(source.clone()), list_directories(source.clone()));

    let files = files?;
    let folders = folders?;

    info!("Found {} files.", files.len());
    info!("Found {} folders.", folders.len());

    let progress_bar = ProgressBar::new(folders.len() as u64);
    progress_bar.set_style(
        ProgressStyle::default_bar()
        .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{human_len} ({per_sec}) {msg}").unwrap()
        .progress_chars("=>-"),
    );

    progress.add(progress_bar.clone());
    // sort the folders by depth
    let mut folders = folders
        .iter()
        .map(|folder| {
            let depth = folder.components().count() - 1; // remove root dir
            (folder, depth)
        })
        .collect::<Vec<_>>();

    folders.sort_by(|a, b| a.1.cmp(&b.1));

    // create HashMap of path and created node id
    let mut created_nodes = BTreeMap::new();
    let root_folder_path = format!("/{}", &name);
    created_nodes.insert(root_folder_path, parent_id);

    let root_depth_level = if folders.is_empty() {
        0
    } else {
        folders.first().expect("No folders found").1
    };

    let root_path = source.parent().unwrap_or_else(|| Path::new("/"));

    let mut all_depth_levels = folders.iter().map(|(_, depth)| depth).collect::<Vec<_>>();
    all_depth_levels.sort();

    // create folders
    let mut prev_depth = 0;
    let mut folder_reqs = Vec::new();

    for (folder, depth) in folders {
        if depth >= prev_depth {
            // execute all previous requests
            let created_folders = join_all(folder_reqs).await;
            let processed = created_folders.len();
            // return error if any of the folders failed to create
            update_folder_map(
                dracoon,
                created_folders,
                &mut created_nodes,
                target_parent,
                &name,
                folder,
                skip_root,
            )
            .await?;
            progress_bar.inc(processed as u64);
            prev_depth = depth;
            // reset folder_reqs
            folder_reqs = Vec::new();
        }

        let name = folder
            .file_name()
            .ok_or(DcCmdError::InvalidPath(
                folder.to_string_lossy().to_string(),
            ))?
            .to_str()
            .ok_or(DcCmdError::InvalidPath(
                folder.to_string_lossy().to_string(),
            ))?
            .to_string();

        let parent_id = if depth == root_depth_level {
            parent_id
        } else {
            // we need to find the parent id from the created_nodes map
            // we assume that the parent folder has already been created and is present in the map
            debug!("Processing sub folder: {}", folder.to_string_lossy());
            let parent_path = folder.parent().ok_or(DcCmdError::IoError)?.to_path_buf();
            let parent_path = parent_path.to_string_lossy();
            debug!("Parent path: {}", parent_path);
            let parent_path = parent_path.trim_start_matches('.');

            let root_path_str = root_path.to_string_lossy().to_string();
            debug!("Root path: {}", root_path_str);

            let parent_path = parent_path
                .strip_prefix(&root_path_str)
                .ok_or(DcCmdError::IoError)?;

            debug!("Parent path: {}", parent_path);
            debug!("{:?}", created_nodes);

            if overwrite {
                //TODO: broken - does not work, entry not present
                let path = format!("{}{}", target_parent, parent_path);
                let node = dracoon.nodes.get_node_from_path(&path).await?;
                node.ok_or(DcCmdError::Unknown)?.id
            } else {
                *created_nodes.get(parent_path).ok_or(DcCmdError::IoError)?
            }
        };

        let folder_req = CreateFolderRequest::builder(name, parent_id).build();
        folder_reqs.push(dracoon.nodes.create_folder(folder_req));
    }

    let created_folders = join_all(folder_reqs).await;
    let processed = created_folders.len();

    update_folder_map(
        dracoon,
        created_folders,
        &mut created_nodes,
        target_parent,
        &name,
        &source,
        skip_root,
    )
    .await?;

    progress_bar.inc(processed as u64);

    progress_bar.finish_with_message("Created folder structure.");
    info!("Created folder structure.");

    let file_map = create_file_map(files, &created_nodes, root_path)?;

    // upload files
    upload_files(
        dracoon,
        target,
        file_map,
        overwrite,
        classification,
        velocity,
    )
    .await?;

    info!("Upload of {} complete.", source.to_string_lossy());

    Ok(())
}

async fn update_folder_map(
    dracoon: &Dracoon<Connected>,
    folder_results: Vec<Result<Node, DracoonClientError>>,
    created_nodes: &mut BTreeMap<String, u64>,
    target_parent: &str,
    parent_name: &str,
    folder: &Path,
    skip_root: bool,
) -> Result<(), DcCmdError> {
    let folder_path = folder;

    debug!("Folder path: {}", folder_path.to_string_lossy());
    debug!("Target parent: {}", target_parent);
    debug!("Parent name: {}", parent_name);

    for folder in folder_results {
        let folder = match folder {
            Ok(folder) => folder,
            Err(e) if e.is_conflict() => {
                let error_details = match &e {
                    DracoonClientError::Http(e) => e,
                    _ => unreachable!("Error is http - checked and is conflict"),
                };

                let mut found_start = false;
                let mut result_path = PathBuf::new();

                let name = error_details.debug_info().ok_or(DcCmdError::IoError)?;
                let name = name
                    .split('\'')
                    .nth(1)
                    .map(|s| s.to_string())
                    .ok_or(DcCmdError::IoError)?;

                debug!("Name: {}", name);

                for component in folder_path.components() {
                    if let Some(segment) = component.as_os_str().to_str() {
                        if segment == parent_name {
                            found_start = true;
                        }
                        if found_start {
                            result_path.push(segment);
                        }
                        if segment == name {
                            break;
                        }
                    }
                }

                if found_start && result_path.ends_with(&name) {
                    result_path.pop();
                }

                let path = result_path.to_str().ok_or(DcCmdError::IoError)?;
                let path = format!("{}{}", target_parent, path);
                debug!("Path: {}", path);
                dracoon
                    .nodes
                    .get_node_from_path(&path)
                    .await?
                    .ok_or_else(|| {
                        error!("Error creating root folder: {}", e);
                        e
                    })?
            }
            Err(e) => {
                error!("Error creating root folder: {}", e);
                return Err(e.into());
            }
        };

        let folder_path = format!(
            "{}{}",
            folder.parent_path.unwrap_or("/".into()),
            folder.name
        );

        debug!("{}, {}", folder_path, folder.id);
        debug!("{}", target_parent);

        let target_parent = folder_path.trim_start_matches(target_parent);
        // ensure target parent starts with a slash
        let target_parent = if target_parent.starts_with('/') {
            target_parent.to_string()
        } else {
            format!("/{target_parent}")
        };

        let target_parent = if skip_root {
            format!("/{}{}", parent_name, target_parent)
        } else {
            target_parent
        };

        debug!("Target parent: {}", target_parent);

        created_nodes.insert(target_parent, folder.id);
    }

    Ok(())
}

fn create_file_map(
    files: Vec<PathBuf>,
    created_nodes: &BTreeMap<String, u64>,
    root_path: &Path,
) -> Result<BTreeMap<PathBuf, (u64, u64)>, DcCmdError> {
    files
        .into_iter()
        .map(|file| {
            // get relative path of file
            let file_rel_path = file
                .strip_prefix(root_path)
                .unwrap_or(file.as_ref())
                .parent()
                .unwrap_or(Path::new("/"));

            // ensure path starts with "/"
            let file_rel_path = if file_rel_path.is_absolute() {
                file_rel_path.to_path_buf()
            } else {
                Path::new("/").join(file_rel_path)
            };

            let file_rel_path = file_rel_path.to_string_lossy().to_string();

            // get node id of parent folder
            let node_id = *created_nodes
                .get(&file_rel_path)
                .ok_or(DcCmdError::IoError)?;

            // get file size
            let file_meta = std::fs::metadata(&file).map_err(|_| DcCmdError::IoError)?;
            let file_size = file_meta.len();

            Ok((file, (node_id, file_size)))
        })
        .collect::<Result<BTreeMap<PathBuf, (u64, u64)>, DcCmdError>>()
}

async fn upload_files(
    dracoon: &Dracoon<Connected>,
    parent_node: &Node,
    files: BTreeMap<PathBuf, (u64, u64)>,
    overwrite: bool,
    classification: Option<u8>,
    velocity: Option<u8>,
) -> Result<(), DcCmdError> {
    info!("Attempting upload of {} files.", files.len());

    let velocity = velocity.unwrap_or(1).clamp(1, 10);

    let concurrent_reqs = velocity * 5;

    let total_size = files.values().fold(0, |acc, (_, size)| acc + size);

    let progress_bar = ProgressBar::new(total_size);
    progress_bar.set_style(
        ProgressStyle::default_bar()
        .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}) {msg}").unwrap()
        .progress_chars("=>-"),
    );

    progress_bar.set_length(total_size);
    let message = format!("Uploading {} files", files.len());
    progress_bar.set_message(message.clone());
    let remaining_files = AtomicU64::new(files.len() as u64);

    let files_iter: Vec<_> = files.into_iter().collect();

    for batch in files_iter.chunks(concurrent_reqs.into()) {
        let mut file_reqs = Vec::new();

        for (source, (node_id, file_size)) in batch {
            let rm_files = &remaining_files;
            let progress_bar_mv = progress_bar.clone();
            let progress_bar_inc = progress_bar.clone();
            let client = dracoon.clone();

            let upload_task = async move {
                let file = tokio::fs::File::open(&source)
                    .await
                    .or(Err(DcCmdError::IoError))?;

                let parent_node = client.nodes.get_node(*node_id).await?;

                let file_meta = file.metadata().await.or(Err(DcCmdError::IoError))?;
                let file_meta = get_file_meta(&file_meta, source)?;

                let file_name = file_meta.0.clone();

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

                client
                    .upload(
                        file_meta,
                        &parent_node,
                        upload_options,
                        reader,
                        Some(Box::new(move |progress: u64, _total: u64| {
                            progress_bar_mv.inc(progress);
                        })),
                        None,
                    )
                    .await
                    .map_err(|e| {
                        error!("Error uploading file: {}", file_name);
                        e
                    })?;

                _ = &rm_files.fetch_sub(1, Ordering::Relaxed);
                let message = format!("Uploading {} files", &rm_files.load(Ordering::Relaxed));
                progress_bar_inc.set_message(message);

                Ok::<(), DcCmdError>(())
            };

            file_reqs.push(upload_task);
        }

        let results: Vec<Result<(), DcCmdError>> = join_all(file_reqs).await;
        for result in results {
            if let Err(e) = result {
                error!("Error uploading file: {}", e);
            }
        }
    }

    let target = parent_node.name.clone();

    progress_bar.finish_with_message(format!("Upload to {target} complete"));

    info!(
        "Upload of {} files to {} complete.",
        files_iter.len(),
        target
    );

    Ok(())
}

#[async_recursion]
async fn list_directories(root_path: PathBuf) -> Result<Vec<PathBuf>, DcCmdError> {
    let mut folder_paths: Vec<PathBuf> = Vec::new();

    let mut folders = tokio::fs::read_dir(root_path)
        .await
        .or(Err(DcCmdError::IoError))?;

    while let Some(entry) = folders.next_entry().await.or(Err(DcCmdError::IoError))? {
        let path = entry.path();
        if path.is_dir() {
            folder_paths.push(path.clone());
            let next_folders = list_directories(path).await?;
            folder_paths.extend(next_folders);
        }
    }

    Ok(folder_paths)
}

#[async_recursion]
async fn list_files(root_path: PathBuf) -> Result<Vec<PathBuf>, DcCmdError> {
    let mut file_paths: Vec<PathBuf> = Vec::new();

    let mut files = tokio::fs::read_dir(root_path)
        .await
        .or(Err(DcCmdError::IoError))?;

    while let Some(entry) = files.next_entry().await.or(Err(DcCmdError::IoError))? {
        let path = entry.path();
        if path.is_file() {
            file_paths.push(path.clone());
        } else if path.is_dir() {
            let next_files = list_files(path).await?;
            file_paths.extend(next_files);
        }
    }

    Ok(file_paths)
}

fn get_file_meta(file_meta: &Metadata, file_path: &Path) -> Result<FileMeta, DcCmdError> {
    let file_name = file_path
        .file_name()
        .ok_or(DcCmdError::InvalidPath(
            file_path.to_string_lossy().to_string(),
        ))?
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

    Ok(FileMeta::builder()
        .with_name(file_name)
        .with_size(file_meta.len())
        .with_timestamp_modification(timestamp_modification)
        .with_timestamp_creation(timestamp_creation)
        .build())
}

#[cfg(test)]
mod tests {

    use super::*;

    #[tokio::test]
    async fn test_list_directories() {
        let root_path = PathBuf::from("./src");
        let folders = list_directories(root_path).await.unwrap();
        assert_eq!(folders.len(), 4);
    }

    #[tokio::test]
    async fn test_list_files() {
        let root_path = PathBuf::from("./src/cmd/nodes");
        let files = list_files(root_path).await.unwrap();
        assert_eq!(files.len(), 5);
    }
}

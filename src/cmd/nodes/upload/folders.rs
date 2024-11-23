use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use async_recursion::async_recursion;
use dashmap::DashMap;
use dco3::{
    auth::Connected,
    nodes::{CreateFolderRequest, Node},
    Dracoon, Folders, Nodes,
};

use indicatif::{MultiProgress, ProgressBar, ProgressStyle};

use tracing::{debug, error, info};

use crate::cmd::{
    config::MAX_CONCURRENT_REQUESTS,
    models::DcCmdError,
    nodes::{models::CmdUploadOptions, upload::files::upload_files},
};

#[allow(clippy::too_many_lines)]
pub async fn upload_container(
    dracoon: &Dracoon<Connected>,
    source: PathBuf,
    target: &Node,
    target_parent: &str,
    opts: &CmdUploadOptions,
) -> Result<(), DcCmdError> {
    info!("Attempting upload of folder: {}.", source.to_string_lossy());
    info!("Target node: {}.", target.name);

    // create folder first
    let root_name = source
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
    let parent_id = if opts.skip_root {
        info!("Skipping root folder.");
        target.id
    } else {
        let root_folder = create_root_folder(dracoon, &root_name, target.id, target_parent).await?;
        root_folder.id
    };

    let (files, folders) = match tokio::try_join!(list_files(&source), list_directories(&source)) {
        Ok((files, folders)) => (files, folders),
        Err(e) => {
            error!("Error listing files and folders: {}", e);
            return Err(e);
        }
    };

    info!("Found {} files.", files.len());
    info!("Found {} folders.", folders.len());

    let progress_bar = ProgressBar::new(folders.len() as u64);
    progress_bar.set_style(
        ProgressStyle::default_bar()
        .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{human_len} ({per_sec}) {msg}").unwrap()
        .progress_chars("=>-"),
    );

    progress.add(progress_bar.clone());

    let folders = group_folders_by_depth(folders);

    let created_nodes = Arc::new(DashMap::new());
    let root_folder_path = format!("/{}", &root_name);

    created_nodes.insert(root_folder_path.clone(), parent_id);

    let semaphore = Arc::new(tokio::sync::Semaphore::new(MAX_CONCURRENT_REQUESTS));

    let root_path = source.parent().unwrap_or_else(|| Path::new("/")).to_owned();

    for depth_level in folders {
        let mut handles = Vec::new();

        for folder in depth_level {
            let semaphore = semaphore.clone();
            let dracoon = dracoon.clone();
            let created_nodes = created_nodes.clone();
            let target = target.clone();
            let progress_bar = progress_bar.clone();
            let root_path = root_path.clone();

            debug!("Created nodes: {:?}", created_nodes);

            let handle = tokio::spawn(async move {
                let _permit = semaphore.acquire().await;

                let (path, _) = folder;
                let parent_path = path.parent().unwrap_or_else(|| Path::new("/"));

                let parent_path = parent_path.to_string_lossy().to_string();
                let normalized_path = normalize_path(&path, &root_path);
                let normalized_parent = normalized_path.parent().unwrap_or_else(|| Path::new("/"));
                let normalized_parent = normalized_parent.to_string_lossy().to_string();
                debug!("Normalized path: {}", normalized_parent);
                debug!("Root and path: {:?} {:?}", root_path, path);
                let parent_id = created_nodes.get(&normalized_parent).ok_or_else(|| {
                    error!("Parent folder not found: {normalized_parent}");
                    DcCmdError::InvalidPath(parent_path.clone())
                })?;
                let name = path
                    .file_name()
                    .ok_or(DcCmdError::InvalidPath(path.to_string_lossy().to_string()))?
                    .to_str()
                    .ok_or(DcCmdError::InvalidPath(path.to_string_lossy().to_string()))?
                    .to_string();
                let folder = CreateFolderRequest::builder(&name, *parent_id).build();

                match dracoon.nodes().create_folder(folder).await {
                    Ok(folder) => {
                        let folder_path = format!("{normalized_parent}/{name}");
                        created_nodes.insert(folder_path, folder.id);
                        progress_bar.inc(1);
                    }
                    Err(e) if e.is_conflict() => {
                        let target_path = format!(
                            "{}{}",
                            target.parent_path.unwrap_or("/".into()),
                            target.name
                        );
                        let path = format!("{target_path}{normalized_parent}/{name}/");
                        let folder = dracoon
                            .nodes()
                            .get_node_from_path(&path)
                            .await?
                            .ok_or_else(|| {
                                error!("Conflict - folder not found: {path}");
                                e
                            })?;
                        let folder_path = format!("{normalized_parent}/{name}");
                        created_nodes.insert(folder_path, folder.id);
                        progress_bar.inc(1);
                    }
                    Err(e) => {
                        error!("Error creating folder: {}", e);
                        return Err(e.into());
                    }
                }

                Ok::<(), DcCmdError>(())
            });
            handles.push(handle);
        }

        for handle in handles {
            if let Err(e) = handle.await {
                error!("Error creating folder: {}", e);
            }
        }
    }

    progress_bar.finish_with_message("Created folder structure.");
    info!("Created folder structure.");
    let root_path = source.parent().unwrap_or_else(|| Path::new("/"));

    let file_map = create_file_map(files, created_nodes.clone(), root_path)?;

    // upload files
    upload_files(dracoon, target, file_map, opts.clone()).await?;

    info!("Upload of {} complete.", source.to_string_lossy());

    Ok(())
}

fn create_file_map(
    files: Vec<PathBuf>,
    created_nodes: Arc<DashMap<String, u64>>,
    root_path: &Path,
) -> Result<BTreeMap<PathBuf, (u64, u64)>, DcCmdError> {
    files
        .into_iter()
        .map(|file| {
            let file_rel_path = normalize_path(&file, root_path);

            let file_parent = file_rel_path.parent().unwrap_or_else(|| Path::new("/"));
            let file_parent = file_parent.to_string_lossy().to_string();

            // get node id of parent folder
            let node_id = *created_nodes.get(&file_parent).ok_or_else(|| {
                error!("Error getting node id for file path: {}", file_parent);
                debug!("Processed file: {}", file.to_string_lossy());
                debug!("Created nodes: {:?}", created_nodes);
                debug!("Root path: {}", root_path.to_string_lossy());
                DcCmdError::InvalidPath(file_parent)
            })?;

            // get file size
            let file_meta = std::fs::metadata(&file).map_err(|_| DcCmdError::IoError)?;
            let file_size = file_meta.len();

            Ok((file, (node_id, file_size)))
        })
        .collect::<Result<BTreeMap<PathBuf, (u64, u64)>, DcCmdError>>()
}

#[async_recursion]
async fn list_directories(root_path: &Path) -> Result<Vec<PathBuf>, DcCmdError> {
    let mut folder_paths: Vec<PathBuf> = Vec::new();

    let mut folders = tokio::fs::read_dir(root_path)
        .await
        .or(Err(DcCmdError::IoError))?;

    while let Some(entry) = folders.next_entry().await.or(Err(DcCmdError::IoError))? {
        let path = entry.path();
        if path.is_dir() {
            folder_paths.push(path.clone());
            let next_folders = list_directories(&path).await?;
            folder_paths.extend(next_folders);
        }
    }

    Ok(folder_paths)
}

#[async_recursion]
async fn list_files(root_path: &Path) -> Result<Vec<PathBuf>, DcCmdError> {
    let mut file_paths: Vec<PathBuf> = Vec::new();

    let mut files = tokio::fs::read_dir(root_path)
        .await
        .or(Err(DcCmdError::IoError))?;

    while let Some(entry) = files.next_entry().await.or(Err(DcCmdError::IoError))? {
        let path = entry.path();
        if path.is_file() {
            file_paths.push(path.clone());
        } else if path.is_dir() {
            let next_files = list_files(&path).await?;
            file_paths.extend(next_files);
        }
    }

    Ok(file_paths)
}

async fn create_root_folder(
    dracoon: &Dracoon<Connected>,
    name: &str,
    parent_id: u64,
    node_parent: &str,
) -> Result<Node, DcCmdError> {
    let root_folder = CreateFolderRequest::builder(name, parent_id).build();

    let root_folder = match dracoon.nodes().create_folder(root_folder).await {
        Ok(folder) => folder,
        Err(e) if e.is_conflict() => {
            let path = format!("{node_parent}{name}");
            debug!("Path: {}", path);
            dracoon
                .nodes()
                .get_node_from_path(&path)
                .await?
                .ok_or_else(|| {
                    error!("Failed to get path. Error creating root folder: {:?}", e);
                    e
                })?
        }
        Err(e) if e.is_conflict() => {
            let path = format!("{node_parent}{name}");
            debug!("Path: {}", path);
            dracoon
                .nodes()
                .get_node_from_path(&path)
                .await?
                .ok_or_else(|| {
                    error!(
                        "Failed to create folder (conflict). Error finding root folder: {:?}",
                        e
                    );
                    e
                })?
        }
        Err(e) => {
            error!("Not a conflict - error creating root folder: {:?}", e);
            debug!("Is conflict: {}", e.is_conflict());
            return Err(e.into());
        }
    };

    Ok(root_folder)
}

fn group_folders_by_depth(folders: Vec<PathBuf>) -> Vec<Vec<(PathBuf, usize)>> {
    let depth_map: BTreeMap<usize, Vec<_>> =
        folders.iter().fold(BTreeMap::new(), |mut acc, folder| {
            let depth = folder.components().count() - 1;
            acc.entry(depth).or_default().push((folder.clone(), depth));
            acc
        });

    depth_map.into_values().collect()
}

fn normalize_path(path: &Path, root_path: &Path) -> PathBuf {
    // Convert to forward-slash strings and remove drive letters if present
    let path_str = path
        .to_string_lossy()
        .replace('\\', "/")
        .split(':')
        .last()
        .unwrap_or("")
        .to_string();
    let root_str = root_path
        .to_string_lossy()
        .replace('\\', "/")
        .split(':')
        .last()
        .unwrap_or("")
        .to_string();

    // If path is same as root, return /
    if path_str == root_str {
        return PathBuf::from("/");
    }

    // Strip root prefix and ensure leading slash
    let normalized = path_str.strip_prefix(&root_str).unwrap_or(&path_str);
    PathBuf::from(&format!("/{}", normalized.trim_start_matches('/')))
}

#[cfg(test)]
mod tests {

    use super::*;

    #[tokio::test]
    async fn test_list_directories() {
        let root_path = PathBuf::from("./src");
        let folders = list_directories(&root_path).await.unwrap();
        assert_eq!(folders.len(), 9);
    }

    #[tokio::test]
    async fn test_list_files() {
        let root_path = PathBuf::from("./src/cmd/config");
        let files = list_files(&root_path).await.unwrap();
        assert_eq!(files.len(), 4);
    }

    #[test]
    fn test_group_folders_by_depth() {
        let folders = vec![
            PathBuf::from("/a/b/c"),
            PathBuf::from("/a/b/d"),
            PathBuf::from("/a/e"),
            PathBuf::from("/f"),
        ];

        let grouped = group_folders_by_depth(folders);

        assert_eq!(grouped.len(), 3);

        let first_depth = grouped.first().unwrap();
        assert_eq!(first_depth.len(), 1);
        assert_eq!(*first_depth.first().unwrap(), (PathBuf::from("/f"), 1));

        let second_depth = grouped.get(1).unwrap();
        assert_eq!(second_depth.len(), 1);
        assert_eq!(*second_depth.first().unwrap(), (PathBuf::from("/a/e"), 2));

        let third_depth = grouped.last().unwrap();
        assert_eq!(third_depth.len(), 2);
        assert_eq!(*third_depth.first().unwrap(), (PathBuf::from("/a/b/c"), 3));
        assert_eq!(*third_depth.last().unwrap(), (PathBuf::from("/a/b/d"), 3));
    }

    #[test]
    fn test_basic_path() {
        let root = PathBuf::from("/root");
        let path = PathBuf::from("/root/folder1/folder2");
        assert_eq!(
            normalize_path(&path, &root),
            PathBuf::from("/folder1/folder2")
        );
    }

    #[test]
    fn test_windows_path() {
        let root = PathBuf::from(r"C:\root");
        let path = PathBuf::from(r"C:\root\folder1\folder2");
        assert_eq!(
            normalize_path(&path, &root),
            PathBuf::from("/folder1/folder2")
        );
    }

    #[test]
    fn test_just_root() {
        let root = PathBuf::from("/root");
        let path = PathBuf::from("/root");
        assert_eq!(normalize_path(&path, &root), PathBuf::from("/"));
    }

    #[test]
    fn test_nested_paths() {
        let root = PathBuf::from("/root/base");
        let path = PathBuf::from("/root/base/folder1/folder2/folder3");
        assert_eq!(
            normalize_path(&path, &root),
            PathBuf::from("/folder1/folder2/folder3")
        );
    }

    #[test]
    fn test_already_normalized() {
        let root = PathBuf::from("/root");
        let path = PathBuf::from("/folder1/folder2");
        assert_eq!(
            normalize_path(&path, &root),
            PathBuf::from("/folder1/folder2")
        );
    }

    #[test]
    fn test_parent_paths() {
        let root = PathBuf::from("/root/base");
        let path = PathBuf::from("/root/base/folder1/folder2");
        let parent = path.parent().unwrap();
        assert_eq!(normalize_path(parent, &root), PathBuf::from("/folder1"));
    }
}

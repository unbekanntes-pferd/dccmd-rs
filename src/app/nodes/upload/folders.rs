use std::{
    collections::{BTreeMap, HashMap},
    path::{Path, PathBuf},
    sync::Arc,
};

use async_recursion::async_recursion;
use async_trait::async_trait;
use dashmap::DashMap;
use dco3::{
    auth::Connected,
    nodes::{models::NodeType, CreateFolderRequest, Node},
    Dracoon, Folders, ListAllParams, Nodes,
};
use futures_util::{stream, StreamExt};

use tracing::{debug, error, info};
use unicode_normalization::UnicodeNormalization;

use super::files::upload_files;
use crate::{
    app::nodes::{
        command::CmdUploadOptions,
        progress::{start_item_progress_bar, start_spinner, ProgressReporter},
        upload::UploadOutcome,
    },
    core::{
        constants::{MAX_CONCURRENT_REQUESTS, MIN_VELOCITY},
        models::DcCmdError,
    },
};

#[allow(clippy::too_many_lines)]
pub async fn upload_container(
    dracoon: &Dracoon<Connected>,
    source: PathBuf,
    target: &Node,
    opts: &CmdUploadOptions,
    progress: &dyn ProgressReporter,
) -> Result<UploadOutcome, DcCmdError> {
    info!("Attempting upload of folder: {}.", source.to_string_lossy());
    info!("Target node: {}.", target.name);

    // create folder first
    let root_name = source
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .map(|n| n.nfc().collect::<String>())
        .ok_or(DcCmdError::InvalidPath(
            source.to_string_lossy().to_string(),
        ))?;

    if source.is_relative() {
        error!("Only absolute paths are supported.");
        return Err(DcCmdError::InvalidPath(
            source.to_string_lossy().to_string(),
        ));
    }

    let progress_spinner = start_spinner(progress, "Creating folder structure...");
    let root_node = if opts.skip_root {
        info!("Skipping root folder.");
        target.clone()
    } else {
        create_root_folder(dracoon, &root_name, target.id).await?
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
    progress_spinner.finish_and_clear();

    let progress_bar =
        start_item_progress_bar(progress, folders.len() as u64, Some("Creating folders"));

    let folders = group_folders_by_depth(folders);

    let created_nodes = Arc::new(DashMap::new());
    let root_folder_path: String = format!("/{root_name}").nfc().collect();

    created_nodes.insert(root_folder_path.clone(), root_node);

    let velocity = opts
        .velocity
        .unwrap_or(MIN_VELOCITY)
        .clamp(MIN_VELOCITY, MAX_CONCURRENT_REQUESTS as u8);
    let concurrent_reqs = velocity as usize;

    let root_path = source.parent().unwrap_or_else(|| Path::new("/")).to_owned();

    for depth_level in folders {
        let depth_results = stream::iter(depth_level.into_iter().map(|folder| {
            let dracoon = dracoon.clone();
            let created_nodes = created_nodes.clone();
            let progress_bar = progress_bar.clone();
            let root_path = root_path.clone();
            let source = source.clone();

            async move {
                debug!("Created nodes: {:?}", created_nodes);

                let (path, _) = folder;
                let parent_path = path.parent().unwrap_or_else(|| Path::new("/"));

                let parent_path = parent_path.to_string_lossy().to_string();
                let parent_path = parent_path.nfc().collect::<String>();
                let normalized_path = normalize_path(&path, &root_path);
                let normalized_parent = normalized_path.parent().unwrap_or_else(|| Path::new("/"));
                let normalized_parent = normalized_parent.to_string_lossy().to_string();
                let normalized_parent = normalized_parent.nfc().collect::<String>();
                debug!("Normalized path: {}", normalized_parent);
                debug!("Root and path: {:?} {:?}", root_path, path);
                let parent_id = created_nodes
                    .get(&normalized_parent)
                    .map(|parent_node| parent_node.id)
                    .ok_or_else(|| {
                        error!("Parent folder not found: {normalized_parent}");
                        DcCmdError::InvalidPath(parent_path.clone())
                    })?;
                let name = path
                    .clone()
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .map(|n| n.nfc().collect::<String>())
                    .ok_or(DcCmdError::InvalidPath(
                        source.to_string_lossy().to_string(),
                    ))?;
                let folder = CreateFolderRequest::builder(&name, parent_id).build();

                match dracoon.nodes().create_folder(folder).await {
                    Ok(folder) => {
                        let folder_path = format!("{normalized_parent}/{name}").nfc().collect();
                        created_nodes.insert(folder_path, folder);
                        progress_bar.inc(1);
                    }
                    Err(e) if e.is_conflict() => {
                        let folder = find_existing_child_folder(&dracoon, parent_id, &name)
                            .await?
                            .ok_or_else(|| {
                                error!(
                                    "Conflict while creating folder '{name}' under parent {parent_id}, but existing folder was not found."
                                );
                                DcCmdError::InvalidPath(format!(
                                    "Folder '{name}' not found under parent id {parent_id}"
                                ))
                            })?;
                        let folder_path = format!("{normalized_parent}/{name}").nfc().collect();
                        created_nodes.insert(folder_path, folder);
                        progress_bar.inc(1);
                    }
                    Err(e) => {
                        error!("Error creating folder: {}", e);
                        return Err(e.into());
                    }
                }

                Ok::<(), DcCmdError>(())
            }
        }))
        .buffer_unordered(concurrent_reqs)
        .collect::<Vec<_>>()
        .await;

        for result in depth_results {
            result?;
        }
    }

    progress_bar.finish_with_message("Created folder structure.");
    info!("Created folder structure.");
    let root_path = source.parent().unwrap_or_else(|| Path::new("/"));

    let file_map = create_file_map(files, created_nodes.clone(), root_path)?;
    let parent_nodes = created_nodes
        .iter()
        .map(|entry| (entry.value().id, entry.value().clone()))
        .collect::<HashMap<_, _>>();

    // upload files
    let outcome = upload_files(
        dracoon,
        target,
        file_map,
        parent_nodes,
        opts.clone(),
        progress,
    )
    .await?;

    info!("Upload of {} complete.", source.to_string_lossy());

    Ok(outcome)
}

fn create_file_map(
    files: Vec<PathBuf>,
    created_nodes: Arc<DashMap<String, Node>>,
    root_path: &Path,
) -> Result<BTreeMap<PathBuf, (u64, u64)>, DcCmdError> {
    files
        .into_iter()
        .map(|file| {
            let file_rel_path = normalize_path(&file, root_path);

            let file_parent = file_rel_path.parent().unwrap_or_else(|| Path::new("/"));
            let file_parent = file_parent.to_string_lossy().nfc().collect::<String>();

            // get node id of parent folder
            let node_id = created_nodes
                .get(&file_parent)
                .map(|entry| entry.id)
                .ok_or_else(|| {
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
    let fs = TokioFsReader;
    list_directories_with(&fs, root_path).await
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FsEntryKind {
    File,
    Directory,
}

#[async_trait]
trait FsReader: Send + Sync {
    async fn entries(&self, root_path: &Path) -> Result<Vec<(PathBuf, FsEntryKind)>, DcCmdError>;
}

struct TokioFsReader;

#[async_trait]
impl FsReader for TokioFsReader {
    async fn entries(&self, root_path: &Path) -> Result<Vec<(PathBuf, FsEntryKind)>, DcCmdError> {
        let mut reader = tokio::fs::read_dir(root_path)
            .await
            .or(Err(DcCmdError::IoError))?;
        let mut entries = Vec::new();

        while let Some(entry) = reader.next_entry().await.or(Err(DcCmdError::IoError))? {
            let path = entry.path();
            let file_type = entry.file_type().await.or(Err(DcCmdError::IoError))?;
            if file_type.is_dir() {
                entries.push((path, FsEntryKind::Directory));
            } else if file_type.is_file() {
                entries.push((path, FsEntryKind::File));
            }
        }

        Ok(entries)
    }
}

#[async_recursion]
async fn list_directories_with(
    fs: &dyn FsReader,
    root_path: &Path,
) -> Result<Vec<PathBuf>, DcCmdError> {
    let mut folder_paths: Vec<PathBuf> = Vec::new();

    for (path, kind) in fs.entries(root_path).await? {
        if kind == FsEntryKind::Directory {
            folder_paths.push(path.clone());
            let next_folders = list_directories_with(fs, &path).await?;
            folder_paths.extend(next_folders);
        }
    }

    Ok(folder_paths)
}

#[async_recursion]
async fn list_files(root_path: &Path) -> Result<Vec<PathBuf>, DcCmdError> {
    let fs = TokioFsReader;
    list_files_with(&fs, root_path).await
}

#[async_recursion]
async fn list_files_with(fs: &dyn FsReader, root_path: &Path) -> Result<Vec<PathBuf>, DcCmdError> {
    let mut file_paths: Vec<PathBuf> = Vec::new();

    for (path, kind) in fs.entries(root_path).await? {
        if kind == FsEntryKind::File {
            file_paths.push(path.clone());
        } else if kind == FsEntryKind::Directory {
            let next_files = list_files_with(fs, &path).await?;
            file_paths.extend(next_files);
        }
    }

    Ok(file_paths)
}

async fn create_root_folder(
    dracoon: &Dracoon<Connected>,
    name: &str,
    parent_id: u64,
) -> Result<Node, DcCmdError> {
    let root_folder = CreateFolderRequest::builder(name, parent_id).build();

    let root_folder = match dracoon.nodes().create_folder(root_folder).await {
        Ok(folder) => folder,
        Err(e) if e.is_conflict() => {
            find_existing_child_folder(dracoon, parent_id, name)
                .await?
                .ok_or_else(|| {
                    error!(
                        "Conflict while creating root folder '{name}' under parent {parent_id}, but existing folder was not found."
                    );
                    DcCmdError::InvalidPath(format!(
                        "Folder '{name}' not found under parent id {parent_id}"
                    ))
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

async fn find_existing_child_folder(
    dracoon: &Dracoon<Connected>,
    parent_id: u64,
    folder_name: &str,
) -> Result<Option<Node>, DcCmdError> {
    let mut offset = 0_u64;
    loop {
        let params = ListAllParams::builder().with_offset(offset).build();
        let nodes = dracoon
            .nodes()
            .get_nodes(Some(parent_id), None, Some(params))
            .await?;

        if let Some(folder) = nodes
            .items
            .into_iter()
            .find(|node| node.node_type == NodeType::Folder && node.name == folder_name)
        {
            return Ok(Some(folder));
        }

        if nodes.range.limit == 0 {
            break;
        }

        let next_offset = offset.saturating_add(nodes.range.limit);
        if next_offset >= nodes.range.total || next_offset == offset {
            break;
        }
        offset = next_offset;
    }

    Ok(None)
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
    // Normalize Windows paths: replace `\` with `/` and strip drive letters (if any)
    let path_str = path
        .to_string_lossy()
        .replace('\\', "/")
        .split(':')
        .next_back() // Remove drive letters, e.g., "C:"
        .unwrap_or("")
        .nfc() // Normalize to NFC
        .collect::<String>();

    let root_str = root_path
        .to_string_lossy()
        .replace('\\', "/")
        .split(':')
        .next_back()
        .unwrap_or("")
        .nfc()
        .collect::<String>();

    // Special case: if the path matches the root, return "/"
    if path_str == root_str {
        return PathBuf::from("/");
    }

    // Strip the root prefix and normalize components
    let stripped = path_str
        .strip_prefix(&root_str)
        .unwrap_or(&path_str)
        .trim_start_matches('/'); // Remove leading slash after stripping

    let normalized = stripped
        .split('/') // Split path into components
        .map(|component| component.nfc().collect::<String>()) // Normalize each component to NFC
        .collect::<Vec<_>>()
        .join("/"); // Rebuild the normalized path

    PathBuf::from(format!("/{normalized}"))
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, sync::Mutex};

    use super::*;

    struct MockFsReader {
        entries: HashMap<PathBuf, Vec<(PathBuf, FsEntryKind)>>,
        calls: Mutex<Vec<PathBuf>>,
    }

    impl MockFsReader {
        fn new(entries: HashMap<PathBuf, Vec<(PathBuf, FsEntryKind)>>) -> Self {
            Self {
                entries,
                calls: Mutex::new(Vec::new()),
            }
        }

        fn calls(&self) -> Vec<PathBuf> {
            self.calls.lock().expect("lock poisoned").clone()
        }
    }

    #[async_trait]
    impl FsReader for MockFsReader {
        async fn entries(
            &self,
            root_path: &Path,
        ) -> Result<Vec<(PathBuf, FsEntryKind)>, DcCmdError> {
            self.calls
                .lock()
                .expect("lock poisoned")
                .push(root_path.to_path_buf());
            Ok(self.entries.get(root_path).cloned().unwrap_or_default())
        }
    }

    #[tokio::test]
    async fn test_list_directories_uses_mock_fs_tree() {
        let root = PathBuf::from("/root");
        let first = PathBuf::from("/root/first");
        let second = PathBuf::from("/root/second");
        let nested = PathBuf::from("/root/first/nested");
        let mut entries = HashMap::new();
        entries.insert(
            root.clone(),
            vec![
                (first.clone(), FsEntryKind::Directory),
                (second.clone(), FsEntryKind::Directory),
                (PathBuf::from("/root/file.txt"), FsEntryKind::File),
            ],
        );
        entries.insert(
            first.clone(),
            vec![
                (nested.clone(), FsEntryKind::Directory),
                (PathBuf::from("/root/first/a.txt"), FsEntryKind::File),
            ],
        );
        entries.insert(second.clone(), vec![]);
        entries.insert(nested.clone(), vec![]);

        let fs = MockFsReader::new(entries);

        let folders = list_directories_with(&fs, &root).await.unwrap();
        assert_eq!(folders, vec![first, nested, second]);
        assert_eq!(
            fs.calls(),
            vec![
                root,
                PathBuf::from("/root/first"),
                PathBuf::from("/root/first/nested"),
                PathBuf::from("/root/second")
            ]
        );
    }

    #[tokio::test]
    async fn test_list_files_uses_mock_fs_tree() {
        let root = PathBuf::from("/root");
        let first = PathBuf::from("/root/first");
        let second = PathBuf::from("/root/second");
        let nested = PathBuf::from("/root/first/nested");
        let file_root = PathBuf::from("/root/file.txt");
        let file_first = PathBuf::from("/root/first/a.txt");
        let file_nested = PathBuf::from("/root/first/nested/b.txt");
        let file_second = PathBuf::from("/root/second/c.txt");

        let mut entries = HashMap::new();
        entries.insert(
            root.clone(),
            vec![
                (first.clone(), FsEntryKind::Directory),
                (second.clone(), FsEntryKind::Directory),
                (file_root.clone(), FsEntryKind::File),
            ],
        );
        entries.insert(
            first.clone(),
            vec![
                (nested.clone(), FsEntryKind::Directory),
                (file_first.clone(), FsEntryKind::File),
            ],
        );
        entries.insert(
            nested.clone(),
            vec![(file_nested.clone(), FsEntryKind::File)],
        );
        entries.insert(
            second.clone(),
            vec![(file_second.clone(), FsEntryKind::File)],
        );

        let fs = MockFsReader::new(entries);

        let files = list_files_with(&fs, &root).await.unwrap();
        assert_eq!(files, vec![file_nested, file_first, file_second, file_root]);
        assert_eq!(
            fs.calls(),
            vec![
                root,
                PathBuf::from("/root/first"),
                PathBuf::from("/root/first/nested"),
                PathBuf::from("/root/second"),
            ]
        );
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

    #[test]
    fn test_mac_path_normalization() {
        let root = PathBuf::from("/root");

        // Simulate a macOS-style NFD input path
        let path_nfd = PathBuf::from("/root/fo\u{308}lde\u{301}r"); // "földér" in NFD

        // Normalize the path
        let normalized = normalize_path(&path_nfd, &root);

        // The output should normalize to NFC
        assert_eq!(
            normalized,
            PathBuf::from("/földér") // "földér" in NFC
        );
    }
}

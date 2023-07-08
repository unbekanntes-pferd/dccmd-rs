use std::{collections::HashMap, fs::Metadata, path::PathBuf, time::SystemTime};

use async_recursion::async_recursion;
use futures_util::future::join_all;
use indicatif::{ProgressBar, ProgressStyle};

use tracing::error;

use crate::cmd::{
    init_dracoon, init_encryption,
    models::DcCmdError,
    utils::{dates::to_datetime_utc, strings::parse_path},
};
use dco3::{
    auth::Connected,
    nodes::{
        models::{FileMeta, ResolutionStrategy, UploadOptions},
        CreateFolderRequest, Node, Nodes, Upload,
    },
    Dracoon, Folders,
};

// this is currently set low to display progress
// TODO: fix dco3 chunk progress for uploads
const DEFAULT_CHUNK_SIZE: usize = 1024 * 1024 * 5; // 5 MB

pub async fn upload(
    source: PathBuf,
    target: String,
    overwrite: bool,
    classification: Option<u8>,
) -> Result<(), DcCmdError> {
    let mut dracoon = init_dracoon(&target).await?;

    let (parent_path, node_name, _) = parse_path(&target, dracoon.get_base_url().as_str())
        .or(Err(DcCmdError::InvalidPath(target.clone())))?;
    let node_path = format!("{parent_path}{node_name}/");

    let parent_node = dracoon.get_node_from_path(&node_path).await?;

    let Some(parent_node) = parent_node else {
        error!("Target path not found: {}", target);
        return Err(DcCmdError::InvalidPath(target.clone()))
    };

    if parent_node.is_encrypted == Some(true) {
        dracoon = init_encryption(dracoon).await?;
    }

    if source.is_file() {
        upload_file(
            &mut dracoon,
            source.clone(),
            &parent_node,
            overwrite,
            classification,
        )
        .await?;
    } else if source.is_dir() {
        upload_container(&mut dracoon, source.clone(), &parent_node).await?;
    } else {
        return Err(DcCmdError::InvalidPath(
            source.to_string_lossy().to_string(),
        ));
    }

    Ok(())
}

async fn upload_file(
    dracoon: &mut Dracoon<Connected>,
    source: PathBuf,
    target_node: &Node,
    overwrite: bool,
    classification: Option<u8>,
) -> Result<(), DcCmdError> {
    let file = tokio::fs::File::open(&source)
        .await
        .or(Err(DcCmdError::IoError))?;

    let file_meta = file.metadata().await.or(Err(DcCmdError::IoError))?;

    if !file_meta.is_file() {
        return Err(DcCmdError::InvalidPath(
            source.to_string_lossy().to_string(),
        ));
    }

    let file_meta = get_file_meta(&file_meta, source.clone())?;
    let file_name = file_meta.0.clone();

    let progress_bar = ProgressBar::new(target_node.size.unwrap_or(0));
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

    dracoon
        .upload(
            file_meta,
            target_node,
            upload_options,
            reader,
            Some(Box::new(move |progress, total| {
                progress_bar_mv.set_message("Uploading");
                progress_bar_mv.set_length(total);
                progress_bar_mv.set_position(progress);
            })),
            Some(DEFAULT_CHUNK_SIZE),
        )
        .await?;

    progress_bar.finish_with_message(format!("Upload of {file_name} complete"));

    Ok(())
}

async fn upload_container(
    dracoon: &mut Dracoon<Connected>,
    source: PathBuf,
    target: &Node,
) -> Result<(), DcCmdError> {
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

    let folder = CreateFolderRequest::builder(name, target.id).build();
    let folder = dracoon.create_folder(folder).await?;

    let (files, folders) = tokio::join!(
        list_files(source.clone()),
        list_directories(source.clone())
    );

    let files = files?;
    let folders = folders?;

    // sort the folders by depth 
    let mut folders = folders
        .iter()
        .map(|folder| {
            let depth = folder.components().count();
            (folder, depth)
        })
        .collect::<Vec<_>>();

    folders.sort_by(|a, b| a.1.cmp(&b.1));

    // get all depth levels in folders
    let mut depth_levels = folders
        .iter()
        .map(|folder| folder.1)
        .collect::<Vec<_>>();
    depth_levels.sort();

    // create HashMap of path and created node id
    let mut created_nodes = HashMap::new();

    // create folders
    for depth_level in depth_levels {
        let folders = folders
            .iter()
            .filter(|folder| folder.1 == depth_level)
            .map(|folder| folder.0)
            .collect::<Vec<_>>();

        // check if first level
        if depth_level == 1 {
            let mut folder_reqs = Vec::new();
            for folder in folders {
                let folder = CreateFolderRequest::builder(
                    folder
                        .file_name()
                        .ok_or(DcCmdError::InvalidPath(
                            folder.to_string_lossy().to_string(),
                        ))?
                        .to_str()
                        .ok_or(DcCmdError::InvalidPath(
                            folder.to_string_lossy().to_string(),
                        ))?
                        .to_string(),
                    target.id
                    ).build();

                let folder_req = dracoon.create_folder(folder);
                folder_reqs.push(folder_req);
        }

        let folders = join_all(folder_reqs).await;

        // return error if any of the folders failed to create
        for folder in folders {
            let folder = folder?;
            let folder_path = format!("{}{}", folder.parent_path.unwrap_or("/".into()), folder.name);

            // truncate path based on source to a relative path

            // upload ./a/b/c
            // in DRACOON /some/other/path/target_path

            // TODO: fix path 

            created_nodes.insert(folder_path, folder.id);
        }

    }
}

    // upload files

    Ok(())
}

async fn upload_files(
    dracoon: &mut Dracoon<Connected>,
    files: HashMap<PathBuf, u64>,
) -> Result<(), DcCmdError> {
    // TODO: implement bulk upload
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

fn get_file_meta(file_meta: &Metadata, file_path: PathBuf) -> Result<FileMeta, DcCmdError> {
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
        println!("{:?}", folders);
        assert_eq!(folders.len(), 3);
    }

    #[tokio::test]
    async fn test_list_files() {
        let root_path = PathBuf::from("./src/cmd/nodes");
        let files = list_files(root_path).await.unwrap();
        println!("{:?}", files);
        assert_eq!(files.len(), 3);
    }
}

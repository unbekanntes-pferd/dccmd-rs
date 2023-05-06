use std::collections::HashMap;

use futures_util::future::join_all;
use indicatif::{ProgressBar, ProgressStyle, MultiProgress};
use tracing::{debug, error};

use crate::{cmd::{models::DcCmdError, init_dracoon, init_encryption, nodes::{is_search_query, search_nodes}, utils::strings::parse_path}, api::{nodes::{Nodes, models::{NodeType, Node}, Download}, auth::Connected, Dracoon, models::ListAllParams}};

pub async fn download(source: String, target: String, velocity: Option<usize>) -> Result<(), DcCmdError> {
    debug!("Downloading {} to {}", source, target);
    let mut dracoon = init_dracoon(&source).await?;

    let (parent_path, node_name, _) = parse_path(&source, dracoon.get_base_url().as_ref()).or(Err(DcCmdError::InvalidPath(source.clone())))?;
    let node_path  = format!("{}{}/", parent_path, node_name);

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
        let files = search_nodes(&dracoon, &node_name, Some(&parent_path), None, true, 0, 500).await?;
        let files = files.get_files();

        download_files(&mut dracoon, files, &target, None, velocity).await
    } else {
        match node.node_type {
        NodeType::File => download_file(&mut dracoon, &node, &target).await,
        _ => download_container(&mut dracoon, &node, &target).await,
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

    let progress_bar_mv = progress_bar.clone();

    let node_name = node.name.clone();
    let node_name_clone = node_name.clone();

    dracoon
        .download(
            node,
            &mut out_file,
            Some(Box::new(move |progress, total| {
                progress_bar_mv.set_message(format!("{}", node_name_clone.clone()));
                progress_bar_mv.set_length(total);
                progress_bar_mv.set_position(progress);
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
    velocity: Option<usize>
) -> Result<(), DcCmdError> {

    let mut velocity = velocity.unwrap_or(1);

    if velocity < 1 {
        velocity = 1;
    } else if velocity > 10 {
        velocity = 10;
    }

    let concurrent_reqs  = velocity * 5;
    
    let dracoon = dracoon.clone();

    let multi_progress = MultiProgress::new();

    for batch in files.chunks(concurrent_reqs) {
        let mut download_reqs = vec![];
        for file in batch {
            let mp = multi_progress.clone();
            let mut dracoon_client = dracoon.clone();
            let target = target.to_string();
            debug!("Target: {}", target);
            let targets = targets.clone();
            
            download_reqs.push(async move  {

                let target = if targets.is_some() {

                    let targets = targets.clone().unwrap();
                    let target = targets.get(&file.id).expect("Target not found");
                    debug!("Target: {:?}", target);
                    std::path::PathBuf::from(target)
                } else {
                    // join path and file name as PathBuf
                    let target = std::path::PathBuf::from(target);
                    let target = target.join(&file.name);
                    debug!("Target: {:?}", target);
                    target
                };


                let mut out_file = std::fs::File::create(target).or(Err(DcCmdError::IoError))?;
    
                let progress_bar = ProgressBar::new(file.size.unwrap_or(0));
                progress_bar.set_style(
                    ProgressStyle::default_bar()
                    .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}) {msg}").unwrap()
                    .progress_chars("=>-"),
                );
    
                let progress_bar_mv = progress_bar.clone();

                mp.add(progress_bar.clone());
    
                let node_name = file.name.clone();
    
                dracoon_client
                    .download(
                        &file,
                        &mut out_file,
                        Some(Box::new(move |progress, total| {
                            progress_bar_mv.set_message("Downloading");
                            progress_bar_mv.set_length(total);
                            progress_bar_mv.set_position(progress);
                        })),
                    )
                    .await?;
    
                progress_bar.finish_with_message(format!("Download of {node_name} complete"));
    
                Ok(())
    
            } );
        }
    
        let results: Vec<Result<(), DcCmdError>> = join_all(download_reqs).await;    
        for result in results {
        if let Err(e) = result {
            error!("Error downloading file: {}", e);
        }
    }

    }

    Ok(())
}

async fn download_container(
    dracoon: &mut Dracoon<Connected>,
    node: &Node,
    target: &str,
) -> Result<(), DcCmdError> {

    // first get all folders below parent
    let params = ListAllParams::builder()
        .with_filter("type:eq:folder".into())
        .with_sort("parentPath:asc".into())
        .build();

    let folders = dracoon.search_nodes("*", Some(node.id), Some(-1), Some(params)).await?;
    let folders = folders.get_folders();

    // create a directory on target
    let target = std::path::PathBuf::from(target);
    let target = target.clone().join(&node.name);
    
    std::fs::create_dir_all(&target).or(Err(DcCmdError::IoError))?;

    let base_path = node.clone().parent_path.expect("Node has no parent path").trim_end_matches("/").to_string();

    // create all other directories
    for folder in folders {

        let target = std::path::PathBuf::from(target.clone());
        debug!("Target: {:?}", target);

        let folder_base_path = folder.clone().parent_path.expect("Folder has no parent path").trim_start_matches(&base_path).to_string().trim_start_matches("/").to_string();
        let target = target.join(folder_base_path);

        debug!("Target: {:?}", target);

    
        std::fs::create_dir_all(&target).map_err(|_| 
            {
            error!("Error creating directory: {:?}", target);
            DcCmdError::IoError
            }
        )?;
    }

    // get all the files

    let params = ListAllParams::builder()
        .with_filter("type:eq:file".into())
        .with_sort("parentPath:asc".into())
        .build();

    let files = dracoon.search_nodes("*", Some(node.id), Some(-1), Some(params)).await?;

    let files = files.get_files();

    // download all files

    let mut targets = HashMap::new();

    for file in &files {
        let file_target = target.clone();
        let file_base_path = file.clone().parent_path.expect("File has no parent path").trim_start_matches(&base_path).to_string();
        let parent = format!("/{}", node.name.clone());
        let file_base_path = file_base_path.trim_start_matches(&parent);
        let target = file_target.join(&file.name);

        debug!("Target: {:?}", target);
        debug!("file_base_path: {:?}", file_base_path);
        debug!("file_target: {:?}", file_target);

        targets.insert(file.id, target.to_str().expect("Path has no content").to_string());
    }

    download_files(dracoon, files, target.to_str().expect("Path has no content"), Some(targets), None).await?;

    Ok(())
}
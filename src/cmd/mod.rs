use std::{path::PathBuf, time::SystemTime};

use console::Term;
use dialoguer::Confirm;
use futures_util::future::join_all;
use indicatif::{ProgressBar, ProgressStyle};
use tracing::debug;

use self::{
    credentials::{get_dracoon_crypto_env, get_dracoon_env, set_dracoon_crypto_env},
    models::DcCmdError,
    utils::{
        dates::to_datetime_utc,
        strings::{format_error_message, format_success_message},
    },
};
use crate::{
    api::{
        auth::{Connected, OAuth2Flow},
        constants::get_client_credentials,
        models::ListAllParams,
        nodes::{
            models::{CreateFolderRequest, FileMeta, Node, NodeType, UploadOptions, ResolutionStrategy},
            Download, Folders, Nodes, Upload,
        },
        Dracoon, DracoonBuilder,
    },
    cmd::{
        credentials::set_dracoon_env,
        utils::strings::{build_node_path, parse_node_path, print_node},
    },
};

pub mod credentials;
pub mod models;
pub mod utils;

pub async fn download(source: String, target: String) -> Result<(), DcCmdError> {
    debug!("Downloading {} to {}", source, target);
    let mut dracoon = init_dracoon(&source).await?;

    let node = dracoon.get_node_from_path(&source).await?;

    let Some(node) = node else {
        return Err(DcCmdError::InvalidPath(source.clone()))
    };

    if node.is_encrypted == Some(true) {
        dracoon = init_encryption(dracoon).await?;
    }

    match node.node_type {
        NodeType::File => download_file(&mut dracoon, &node, &target).await,
        _ => unimplemented!(),
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

    dracoon
        .download(
            node,
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
}

async fn download_container(
    dracoon: &mut Dracoon<Connected>,
    node: &Node,
    target: &str,
) -> Result<(), DcCmdError> {
    // first get a list of all files recursively via search
    let params = ListAllParams::builder()
        .with_filter("type:eq:file".into())
        .build();

    let mut files = dracoon
        .search_nodes("*", Some(node.id), Some(-1), Some(params))
        .await?;

    if files.range.total > 500 {
        let mut offset = 500;
        let limit = 500;
        let mut futures = vec![];

        while offset < files.range.total {
            let params = ListAllParams::builder()
                .with_filter("type:eq:file".into())
                .with_offset(offset)
                .with_limit(limit)
                .build();
            let next_files_req = dracoon.search_nodes("*", Some(node.id), Some(-1), Some(params));
            futures.push(next_files_req);
            offset += limit;
        }

        let mut next_files_items = vec![];

        // parallelize the requests in batches of 20

        //TODO: refactor this to process only a given batch size in parallel instead of all at once
        let results = join_all(futures).await;
        for result in results {
            debug!("Result: {:?}", result);
            let next_files = result?.items;
            next_files_items.extend(next_files);
        }
        files.items.append(&mut next_files_items);
    }

    // then get a list of all containers recursively via search
    let params = ListAllParams::builder()
    .with_filter("type:eq:folder:room".into())
    .build();

    let mut containers = dracoon
        .search_nodes("*", Some(node.id), Some(-1), Some(params))
        .await?;

    if containers.range.total > 500 {
        let mut offset = 500;
        let limit = 500;
        let mut futures = vec![];

        while offset < containers.range.total {
            let params = ListAllParams::builder()
                .with_filter("type:eq:folder:room".into())
                .with_offset(offset)
                .with_limit(limit)
                .build();

            let next_containers_req =
                dracoon.search_nodes("*", Some(node.id), Some(-1), Some(params));
            futures.push(next_containers_req);
            offset += limit;
        }

        let mut next_containers_items = vec![];
        let results = join_all(futures).await;
        for result in results {
            debug!("Result: {:?}", result);
            let next_containers = result?.items;
            next_containers_items.extend(next_containers);
        }
        containers.items.append(&mut next_containers_items);
    }

    // then recreate the structure in the target directory, beginning with creating the top container
    // TODO

    // then download all files into the target directories
    // TODO

    Ok(())
}

pub async fn upload(source: PathBuf, target: String, overwrite: bool, classification: Option<u8>) -> Result<(), DcCmdError> {
    let mut dracoon = init_dracoon(&target).await?;

    let parent_node = dracoon.get_node_from_path(&target).await?;

    let Some(parent_node) = parent_node else {
        return Err(DcCmdError::InvalidPath(target.clone()))
    };

    if parent_node.is_encrypted == Some(true) {
        dracoon = init_encryption(dracoon).await?;
    }

    let file = tokio::fs::File::open(&source)
        .await
        .or(Err(DcCmdError::IoError))?;

    let file_meta = file.metadata().await.or(Err(DcCmdError::IoError))?;

    if !file_meta.is_file() {
        // TODO: FIX correct path output
        return Err(DcCmdError::InvalidPath("some string".to_string()));
    }

    let file_name = source
    .file_name()
    .expect("This is a file (handled above)")
    .to_owned()
    .to_string_lossy()
    .as_ref()
    .to_string();

    let timestamp_modification = file_meta
        .modified()
        .or(Err(DcCmdError::IoError))
        .unwrap_or(SystemTime::now());

    let timestamp_modification = to_datetime_utc(timestamp_modification);

    let timestamp_creation = file_meta
        .created()
        .or(Err(DcCmdError::IoError))
        .unwrap_or(SystemTime::now());

    let timestamp_creation = to_datetime_utc(timestamp_creation);

    let file_meta = FileMeta::builder()
        .with_name(file_name.clone())
        .with_size(file_meta.len())
        .with_timestamp_modification(timestamp_modification)
        .with_timestamp_creation(timestamp_creation)
        .build();

    let progress_bar = ProgressBar::new(parent_node.size.unwrap_or(0));
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

    dracoon.upload(
        file_meta,
        &parent_node,
        upload_options,
        reader,
        Some(Box::new(move |progress, total| {
            progress_bar_mv.set_message("Uploading");
            progress_bar_mv.set_length(total);
            progress_bar_mv.set_position(progress);
        })),
    ).await?;

    progress_bar.finish_with_message(format!("Upload of {file_name} complete"));

    Ok(())
}

/// initializes a dracoon client with encryption enabled (plain keypair ready to use)
async fn init_encryption(
    mut dracoon: Dracoon<Connected>,
) -> Result<Dracoon<Connected>, DcCmdError> {
    let (secret, store) =
        if let Ok(secret) = get_dracoon_crypto_env(dracoon.get_base_url().as_ref()) {
            (secret, false)
        } else {
            let secret = dialoguer::Password::new()
                .with_prompt("Please enter your encryption secret")
                .interact()
                .or(Err(DcCmdError::IoError))?;
            (secret, true)
        };

    let keypair = dracoon.get_keypair(Some(&secret)).await?;

    if store {
        set_dracoon_crypto_env(dracoon.get_base_url().as_ref(), &secret)?;
    }

    Ok(dracoon)
}

pub async fn get_nodes(
    term: Term,
    source: String,
    long: Option<bool>,
    human_readable: Option<bool>,
    managed: Option<bool>,
    all: Option<bool>,
) -> Result<(), DcCmdError> {
    debug!("Fetching node list from {}", source);
    let dracoon = init_dracoon(&source).await?;

    let (parent_path, node_name, depth) =
        parse_node_path(&source, dracoon.get_base_url().as_ref())?;
    let node_path = build_node_path((parent_path.clone(), node_name.clone(), depth));

    debug!("Parent path: {}", parent_path);
    debug!("Node name: {}", node_name);
    debug!("Depth: {}", depth);

    let all = all.unwrap_or(false);

    let node_list = if parent_path.as_str() == "/" {
        // this is the root node
        debug!("Fetching root node list");
        let mut node_list = dracoon.get_nodes(None, managed, None).await?;

        if all && node_list.range.total > 500 {
            let mut offset = 500;
            let limit = 500;
            let mut futures = vec![];

            while offset < node_list.range.total {
                let params = ListAllParams::builder()
                    .with_offset(offset)
                    .with_limit(limit)
                    .build();

                let next_node_list_req = dracoon.get_nodes(None, managed, Some(params));
                futures.push(next_node_list_req);
                offset += limit;
            }

            let mut next_node_list_items = vec![];
            let results = join_all(futures).await;
            for result in results {
                debug!("Result: {:?}", result);
                let next_node_list = result?.items;
                next_node_list_items.extend(next_node_list);
            }
            node_list.items.append(&mut next_node_list_items);
        }

        node_list
    } else {
        // this is a sub node
        debug!("Fetching node list from path {}", node_path);
        let node = dracoon.get_node_from_path(&node_path).await?;

        let Some(node) = node else {
                return Err(DcCmdError::InvalidPath(source.clone()))
            };

        let mut node_list = dracoon.get_nodes(Some(node.id), managed, None).await?;

        if all && node_list.range.total > 500 {
            let mut offset = 500;
            let limit = 500;

            while offset < node_list.range.total {
                let mut futures = vec![];

                while offset < node_list.range.total {

                    let params = ListAllParams::builder()
                        .with_offset(offset)
                        .with_limit(limit)
                        .build();

                    let next_node_list_req =
                        dracoon.get_nodes(Some(node.id), managed, Some(params));
                    futures.push(next_node_list_req);
                    offset += limit;
                }

                let mut next_node_list_items = vec![];

                let results = join_all(futures).await;
                for result in results {
                    let next_node_list = result?.items;
                    next_node_list_items.extend(next_node_list);
                }
                node_list.items.append(&mut next_node_list_items);
            }
        }

        node_list
    };

    node_list
        .items
        .iter()
        .for_each(|node| print_node(&term, node, long, human_readable));

    Ok(())
}

async fn init_dracoon(url_path: &str) -> Result<Dracoon<Connected>, DcCmdError> {
    let (client_id, client_secret) = get_client_credentials();
    let base_url = parse_base_url(url_path.to_string())?;

    let mut dracoon = DracoonBuilder::new()
        .with_base_url(base_url.clone())
        .with_client_id(client_id)
        .with_client_secret(client_secret)
        .build()?;

    let dracoon = if let Ok(refresh_token) = get_dracoon_env(&base_url) {
         
            dracoon
                .connect(OAuth2Flow::RefreshToken(refresh_token))
                .await?
        } else {
            debug!("No refresh token stored for {}", base_url);
            println!("Please log in via browser (open url): ");
            println!("{}", dracoon.get_authorize_url());
            println!("Please enter authorization code: ");
            let mut auth_code = String::new();
            std::io::stdin()
                .read_line(&mut auth_code)
                .expect("Error parsing user input (auth code).");

            let dracoon = dracoon
                .connect(OAuth2Flow::AuthCodeFlow(auth_code.trim_end().into()))
                .await?;

            set_dracoon_env(&base_url, dracoon.get_refresh_token())?;

            dracoon
        };

    debug!("Successfully authenticated to {}", base_url);

    Ok(dracoon)
}

fn parse_base_url(url_str: String) -> Result<String, DcCmdError> {
    if url_str.starts_with("http://") {
        return Err(DcCmdError::InvalidUrl(url_str));
    };

    let url_str = if url_str.starts_with("https://") {
        url_str
    } else {
        format!("https://{url_str}")
    };

    let uri_fragments: Vec<&str> = url_str[8..].split('/').collect();

    match uri_fragments.len() {
        2.. => Ok(format!("https://{}", uri_fragments[0])),
        _ => Err(DcCmdError::InvalidUrl(url_str)),
    }
}

pub fn handle_error(term: &Term, err: &DcCmdError) {
    let err_msg = get_error_message(err);
    let err_msg = format_error_message(&err_msg);

    term.write_line(&err_msg)
        .expect("Error writing error message to terminal.");
}

fn get_error_message(err: &DcCmdError) -> String {
    match err {
        DcCmdError::InvalidUrl(url) => format!("Invalid URL: {url}"),
        DcCmdError::InvalidPath(path) => format!("Invalid path: {path}"),
        DcCmdError::IoError => "Error reading / writing content.".into(),
        DcCmdError::DracoonError(e) => format!("{e}"),
        DcCmdError::ConnectionFailed => "Connection failed.".into(),
        DcCmdError::CredentialDeletionFailed => "Credential deletion failed.".into(),
        DcCmdError::CredentialStorageFailed => "Credential store failed.".into(),
        DcCmdError::InvalidAccount => "Invalid account.".into(),
        DcCmdError::Unknown => "Unknown error.".into(),
        DcCmdError::DracoonS3Error(e) => format!("{e}"),
        DcCmdError::DracoonAuthError(e) => format!("{e}"),
    }
}

pub async fn delete_node(
    term: Term,
    source: String,
    recursive: Option<bool>,
) -> Result<(), DcCmdError> {
    let dracoon = init_dracoon(&source).await?;
    let (parent_path, node_name, depth) =
        parse_node_path(&source, dracoon.get_base_url().as_ref())?;
    let node_path = build_node_path((parent_path.clone(), node_name.clone(), depth));
    let node = dracoon
        .get_node_from_path(&node_path)
        .await?
        .ok_or(DcCmdError::InvalidPath(source.clone()))?;

    // check if recursive flag is set
    let recursive = recursive.unwrap_or(false);

    // if node type is folder or room and not recursive, abort
    if !recursive && (node.node_type == NodeType::Folder || node.node_type == NodeType::Room) {
        let msg = format_error_message("Deleting non-empty folder or room not allowed. Use --recursive flag to delete recursively.");
        term.write_line(&msg)
            .expect("Error writing message to terminal.");
        return Ok(());
    }

    // define async block to delete node
    let delete_node = async {
        dracoon.delete_node(node.id).await?;
        let msg = format!("Node {node_name} deleted.");
        let msg = format_success_message(&msg);
        term.write_line(&msg)
            .expect("Error writing message to terminal.");
        Ok(())
    };

    // check if node is a room
    match node.node_type {
        NodeType::Room => {
            // ask for confirmation if node is a room
            let confirmed = Confirm::new()
                .with_prompt(format!("Do you really want to delete room {node_name}?"))
                .interact()
                .expect("Error reading user input.");

            if confirmed {
                delete_node.await
            } else {
                let msg = format_error_message("Deleting room not confirmed.");
                term.write_line(&msg)
                    .expect("Error writing message to terminal.");
                Ok(())
            }
        }
        _ => delete_node.await,
    }
}

pub async fn create_folder(
    term: Term,
    source: String,
    classification: Option<u8>,
    notes: Option<String>,
) -> Result<(), DcCmdError> {
    let dracoon = init_dracoon(&source).await?;
    let (parent_path, node_name, _) =
        parse_node_path(&source, dracoon.get_base_url().as_ref())?;

    let parent_node = dracoon
        .get_node_from_path(&parent_path)
        .await?
        .ok_or(DcCmdError::InvalidPath(source.clone()))?;

    let req = CreateFolderRequest::builder(node_name.clone(), parent_node.id);

    let req = match classification {
        Some(classification) => req.with_classification(classification),
        None => req,
    };

    let req = match notes {
        Some(notes) => req.with_notes(notes),
        None => req,
    };

    let req = req.build();

    let folder = dracoon.create_folder(req).await?;

    let msg = format!("Folder {node_name} created.");
    let msg = format_success_message(&msg);
    term.write_line(&msg)
        .expect("Error writing message to terminal.");

    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base_url_parse_https() {
        let base_url = parse_base_url("https://bla.dracoon.com/bla/somefile.pdf".into()).unwrap();
        assert_eq!(base_url, "https://bla.dracoon.com");
    }

    #[test]
    fn test_base_url_parse_no_https() {
        let base_url = parse_base_url("bla.dracoon.com/bla/somefile.pdf".into()).unwrap();
        assert_eq!(base_url, "https://bla.dracoon.com");
    }

    #[test]
    fn test_base_url_parse_invalid_path() {
        let base_url = parse_base_url("bla.dracoon.com".into());
        assert_eq!(
            base_url,
            Err(DcCmdError::InvalidUrl("https://bla.dracoon.com".into()))
        );
    }
}

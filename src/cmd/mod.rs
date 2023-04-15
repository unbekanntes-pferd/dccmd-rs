use console::Term;
use dialoguer::Confirm;
use futures_util::{stream::FuturesUnordered, StreamExt};
use indicatif::{ProgressBar, ProgressStyle};
use tracing::debug;

use self::{
    credentials::get_dracoon_env,
    models::DcCmdError,
    utils::strings::{format_error_message, format_success_message},
};
use crate::{
    api::{
        auth::{Connected, OAuth2Flow},
        constants::get_client_credentials,
        models::ListAllParams,
        nodes::{models::{NodeType, CreateFolderRequest}, Download, Nodes, Folders},
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
    let dracoon = init_dracoon(&source).await?;

    let node = dracoon.get_node_from_path(&source).await?;

    let Some(node) = node else {
        return Err(DcCmdError::InvalidPath(source.clone()))
    };

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
            &node,
            &mut out_file,
            Some(Box::new(move |progress, total| {
                progress_bar_mv.set_message("Downloading");
                progress_bar_mv.set_length(total);
                progress_bar_mv.set_position(progress);
            })),
        )
        .await?;

    progress_bar.finish_with_message(format!("Download of {} complete", node_name));

    Ok(())
}

pub fn upload(source: String, target: String) {
    todo!()
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
        parse_node_path(&source, &dracoon.get_base_url().to_string())?;
    let node_path = build_node_path((parent_path.clone(), node_name.clone(), depth));

    debug!("Parent path: {}", parent_path);
    debug!("Node name: {}", node_name);
    debug!("Depth: {}", depth);

    let all = all.unwrap_or(false);

    let node_list = match parent_path.as_str() {
        "/" => {
            // this is the root node
            debug!("Fetching root node list");
            let mut node_list = dracoon.get_nodes(None, managed, None).await?;

            if all && node_list.range.total > 500 {
                let mut offset = 500;
                let limit = 500;
                let mut futures = FuturesUnordered::new();

                while offset < node_list.range.total {
                    let next_node_list_req = dracoon.get_nodes(
                        None,
                        managed,
                        Some(ListAllParams {
                            offset: Some(offset),
                            limit: Some(limit),
                            filter: None,
                            sort: None,
                        }),
                    );
                    futures.push(next_node_list_req);
                    offset += limit;
                }

                let mut results = vec![];
                while let Some(next_node_list) = futures.next().await {
                    results.push(next_node_list?.items);
                    if futures.len() < 10 {
                        break;
                    }
                }

                while let Some(next_node_list) = futures.next().await {
                    results.push(next_node_list?.items);
                }

                node_list
                    .items
                    .reserve(results.iter().map(|v| v.len()).sum());
                for mut next_node_list_items in results {
                    node_list.items.append(&mut next_node_list_items);
                }
            }

            node_list
        }
        _ => {
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
                    let mut next_node_list = dracoon
                        .get_nodes(
                            None,
                            managed,
                            Some(ListAllParams {
                                offset: Some(offset),
                                limit: Some(limit),
                                filter: None,
                                sort: None,
                            }),
                        )
                        .await?;
                    node_list.items.append(&mut next_node_list.items);
                    offset += limit;
                }
            }

            node_list
        }
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

    let dracoon = match get_dracoon_env(&base_url) {
        Ok(refresh_token) => {
            dracoon
                .connect(OAuth2Flow::RefreshToken(refresh_token))
                .await?
        }
        Err(_) => {
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
        }
    };

    debug!("Successfully authenticated to {}", base_url);

    Ok(dracoon)
}

fn parse_base_url(url_str: String) -> Result<String, DcCmdError> {
    if url_str.starts_with("http://") {
        return Err(DcCmdError::InvalidUrl(url_str));
    };

    let url_str = match url_str.starts_with("https://") {
        true => url_str,
        false => format!("https://{url_str}"),
    };

    let uri_fragments: Vec<&str> = url_str[8..].split('/').collect();

    match uri_fragments.len() {
        2.. => Ok(format!("https://{}", uri_fragments[0])),
        _ => Err(DcCmdError::InvalidUrl(url_str)),
    }
}

pub fn handle_error(term: Term, err: DcCmdError) -> () {
    let err_msg = get_error_message(&err);
    let err_msg = format_error_message(&err_msg);

    term.write_line(&err_msg)
        .expect("Error writing error message to terminal.");
}

fn get_error_message(err: &DcCmdError) -> String {
    match err {
        DcCmdError::InvalidUrl(url) => format!("Invalid URL: {}", url),
        DcCmdError::InvalidPath(path) => format!("Invalid path: {}", path),
        DcCmdError::IoError => "Error reading / writing content.".into(),
        DcCmdError::DracoonError(e) => format!("{}", e),
        DcCmdError::ConnectionFailed => "Connection failed.".into(),
        DcCmdError::CredentialDeletionFailed => "Credential deletion failed.".into(),
        DcCmdError::CredentialStorageFailed => "Credential store failed.".into(),
        DcCmdError::InvalidAccount => "Invalid account.".into(),
        DcCmdError::Unknown => "Unknown error.".into(),
        DcCmdError::DracoonS3Error(e) => format!("{}", e),
        DcCmdError::DracoonAuthError(e) => format!("{}", e),
    }
}

pub async fn delete_node(term: Term, source: String) -> Result<(), DcCmdError> {

    let dracoon = init_dracoon(&source).await?;
    let (parent_path, node_name, depth) = parse_node_path(&source, &dracoon.get_base_url().to_string())?;
    let node_path = build_node_path((parent_path.clone(), node_name.clone(), depth));
    let node = dracoon.get_node_from_path(&node_path).await?.ok_or(DcCmdError::InvalidPath(source.clone()))?;

    // define async block to delete node
    let delete_node = async {
        dracoon.delete_node(node.id).await?;
        let msg = format!("Node {} deleted.", node_name);
        let msg = format_success_message(&msg);
        term.write_line(&msg).expect("Error writing message to terminal.");
        Ok(())
    };
    
    // check if node is a room
    match node.node_type {
        NodeType::Room => {
            // ask for confirmation if node is a room
            let confirmed = Confirm::new()
                .with_prompt(format!("Do you really want to delete room {}?", node_name))
                .interact()
                .expect("Error reading user input.");

            if confirmed {
                delete_node.await
            } else {
                let msg = format_error_message("Deleting room not confirmed.");
                term.write_line(&msg).expect("Error writing message to terminal.");
                Ok(())
            }
        }
        _ => delete_node.await,
    }

    }

pub async fn create_folder(term: Term, source: String, classification: Option<u8>, notes: Option<String>) -> Result<(), DcCmdError> {
    let dracoon = init_dracoon(&source).await?;
    let (parent_path, node_name, _) = parse_node_path(&source, &dracoon.get_base_url().to_string())?;

    let parent_node = dracoon.get_node_from_path(&parent_path).await?.ok_or(DcCmdError::InvalidPath(source.clone()))?;

    let req = CreateFolderRequest::new(node_name.clone(), parent_node.id);

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

    let msg = format!("Folder {} created.", node_name);
    let msg = format_success_message(&msg);
    term.write_line(&msg).expect("Error writing message to terminal.");

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
            Err(DcCmdError::InvalidUrl("bla.dracoon.com".into()))
        );
    }
}

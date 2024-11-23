use std::sync::Arc;

use console::Term;
use dialoguer::Confirm;
use models::{CmdCopyOptions, CmdListNodesOptions};

use tracing::{debug, error, info};

use crate::cmd::{
    init_dracoon,
    users::UserCommandHandler,
    utils::strings::{build_node_path, parse_path, print_node},
};

use dco3::{
    auth::Connected,
    nodes::{
        models::{CreateFolderRequest, NodeList, NodeType},
        rooms::models::CreateRoomRequest,
        Folders, Nodes, Rooms,
    },
    Dracoon,
};

use self::models::CmdMkRoomOptions;

use super::{
    config::MAX_CONCURRENT_REQUESTS,
    models::{build_params, DcCmdError, ListOptions, PasswordAuth},
    utils::strings::{format_error_message, format_success_message},
};

pub mod download;
pub mod models;
mod share;
pub mod transfer;
pub mod upload;

#[allow(clippy::module_name_repetitions)]
pub async fn list_nodes(
    term: Term,
    source: String,
    opts: CmdListNodesOptions,
) -> Result<(), DcCmdError> {
    let dracoon = init_dracoon(&source, opts.auth(), false).await?;

    let (parent_path, node_name, depth) = parse_path(&source, dracoon.get_base_url().as_ref())?;
    let node_path = build_node_path((parent_path.clone(), node_name.clone(), depth));

    // only provide a path if not the root node
    let node_path = if node_path == "//" {
        None
    } else {
        Some(node_path.as_str())
    };

    let node_list = if is_search_query(&node_name) {
        debug!("Searching for nodes with query {}", node_name);
        search_nodes(&dracoon, &node_name, Some(&parent_path), opts.list_opts()).await?
    } else {
        debug!("Fetching node list from path {}", node_path.unwrap_or("/"));
        get_nodes(&dracoon, node_path, Some(opts.managed()), opts.list_opts()).await?
    };

    node_list
        .items
        .iter()
        .for_each(|node| print_node(&term, node, Some(opts.long()), Some(opts.human_readable())));

    info!("Listed nodes in: {}", node_path.unwrap_or("/"));
    info!("Total nodes: {}", node_list.range.total);
    info!("Offset: {}", node_list.range.offset);
    info!("Limit: {}", node_list.range.limit);

    Ok(())
}

fn is_search_query(query: &str) -> bool {
    query.contains('*')
}

async fn get_nodes(
    dracoon: &Dracoon<Connected>,
    node_path: Option<&str>,
    managed: Option<bool>,
    opts: &ListOptions,
) -> Result<NodeList, DcCmdError> {
    let parent_id = if let Some(node_path) = node_path {
        let node = dracoon.nodes().get_node_from_path(node_path).await?;

        let Some(node) = node else {
            return Err(DcCmdError::InvalidPath(node_path.to_string()));
        };

        Some(node.id)
    } else {
        None
    };

    let offset = opts.offset().unwrap_or(0);
    let limit = u64::from(opts.limit().unwrap_or(500))
        .try_into()
        .map_err(|_| {
            DcCmdError::InvalidArgument("Limit must be a positive integer.".to_string())
        })?;

    let params = build_params(opts.filter(), offset, Some(limit))?;

    let mut node_list = dracoon
        .nodes()
        .get_nodes(parent_id, managed, Some(params))
        .await?;

    if opts.all() && node_list.range.total > 500 {
        let (tx, mut rx) = tokio::sync::mpsc::channel(MAX_CONCURRENT_REQUESTS);
        let semaphore = Arc::new(tokio::sync::Semaphore::new(MAX_CONCURRENT_REQUESTS));
        let mut handles = Vec::new();

        (500..=node_list.range.total)
            .step_by(500)
            .for_each(|offset| {
                let tx = tx.clone();
                let semaphore = semaphore.clone();
                let dracoon = dracoon.clone();
                let opts = opts.clone();

                let handle = tokio::spawn(async move {
                    let _permit = semaphore.acquire().await.map_err({
                        error!("Failed to acquire semaphore permit.");
                        |_| DcCmdError::IoError
                    })?;
                    let params = build_params(opts.filter(), offset, None)?;

                    match dracoon
                        .nodes()
                        .get_nodes(parent_id, managed, Some(params))
                        .await
                    {
                        Ok(node_list) => {
                            if let Err(e) = tx.send(node_list.items).await {
                                error!("Failed to send node list: {}", e);
                                return Err(DcCmdError::IoError);
                            }

                            Ok::<(), DcCmdError>(())
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

        while let Some(result) = rx.recv().await {
            node_list.items.extend(result);
        }

        for handle in handles {
            if let Err(e) = handle.await {
                error!("Failed to join task: {}", e);
                return Err(DcCmdError::IoError);
            }
        }
    }
    Ok(node_list)
}

async fn search_nodes(
    dracoon: &Dracoon<Connected>,
    search_string: &str,
    node_path: Option<&str>,
    opts: &ListOptions,
) -> Result<NodeList, DcCmdError> {
    let parent_id = if let Some(node_path) = node_path {
        let node = dracoon.nodes().get_node_from_path(node_path).await?;

        let Some(node) = node else {
            return Err(DcCmdError::InvalidPath(node_path.to_string()));
        };

        Some(node.id)
    } else {
        None
    };

    let params = build_params(
        opts.filter(),
        opts.offset().unwrap_or(0),
        Some(
            u64::from(opts.limit().unwrap_or(500))
                .try_into()
                .map_err(|_| {
                    DcCmdError::InvalidArgument("Limit must be a positive integer.".to_string())
                })?,
        ),
    )?;

    let mut node_list = dracoon
        .nodes()
        .search_nodes(search_string, parent_id, Some(0), Some(params))
        .await?;

    if opts.all() && node_list.range.total > 500 {
        let (tx, mut rx) = tokio::sync::mpsc::channel(MAX_CONCURRENT_REQUESTS);
        let semaphore = Arc::new(tokio::sync::Semaphore::new(MAX_CONCURRENT_REQUESTS));
        let mut handles = Vec::new();

        (500..=node_list.range.total)
            .step_by(500)
            .for_each(|offset| {
                let tx = tx.clone();
                let semaphore = semaphore.clone();
                let dracoon = dracoon.clone();
                let opts = opts.clone();
                let search_string = search_string.to_string();

                let handle = tokio::spawn(async move {
                    let _permit = semaphore.acquire().await.map_err({
                        error!("Failed to acquire semaphore permit.");
                        |_| DcCmdError::IoError
                    })?;
                    let params = build_params(opts.filter(), offset, None)?;

                    match dracoon
                        .nodes()
                        .search_nodes(&search_string, parent_id, Some(0), Some(params))
                        .await
                    {
                        Ok(node_list) => {
                            if let Err(e) = tx.send(node_list.items).await {
                                error!("Failed to send node list: {}", e);
                                return Err(DcCmdError::IoError);
                            }

                            Ok::<(), DcCmdError>(())
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

        while let Some(result) = rx.recv().await {
            node_list.items.extend(result);
        }

        for handle in handles {
            if let Err(e) = handle.await {
                error!("Failed to join task: {}", e);
                return Err(DcCmdError::IoError);
            }
        }
    }
    Ok(node_list)
}

pub async fn delete_node(
    term: Term,
    source: String,
    recursive: Option<bool>,
    auth: Option<PasswordAuth>,
) -> Result<(), DcCmdError> {
    let dracoon = init_dracoon(&source, auth, false).await?;
    let (parent_path, node_name, depth) = parse_path(&source, dracoon.get_base_url().as_ref())?;
    let is_search_query = is_search_query(&node_name);
    // check if recursive flag is set
    let recursive = recursive.unwrap_or(false);

    match (recursive, is_search_query) {
        (true, true) => return delete_node_content(&dracoon, &node_name, parent_path).await,
        (false, true) => {
            let msg = format_error_message(
                "Deleting search results not allowed. Use --recursive flag to delete recursively.",
            );
            error!("{}", msg);
            term.write_line(&msg).map_err(|_| DcCmdError::IoError)?;
            return Ok(());
        }
        _ => (),
    }

    let node_path = build_node_path((parent_path.clone(), node_name.clone(), depth));
    let node = dracoon
        .nodes()
        .get_node_from_path(&node_path)
        .await?
        .ok_or(DcCmdError::InvalidPath(source.clone()))?;

    // if node type is folder or room and not recursive, abort
    if !recursive && (node.node_type == NodeType::Folder || node.node_type == NodeType::Room) {
        let msg = format_error_message("Deleting non-empty folder or room not allowed. Use --recursive flag to delete recursively.");
        error!("{}", msg);
        term.write_line(&msg).map_err(|_| DcCmdError::IoError)?;
        return Ok(());
    }

    // define async block to delete node
    let delete_node = async {
        dracoon.nodes().delete_node(node.id).await?;
        let msg = format!("Node {node_name} deleted.");
        info!("{}", msg);
        let msg = format_success_message(&msg);
        term.write_line(&msg).map_err(|_| DcCmdError::IoError)?;
        Ok(())
    };

    // check if node is a room
    match node.node_type {
        NodeType::Room => {
            // ask for confirmation if node is a room
            let confirmed = Confirm::new()
                .with_prompt(format!("Do you really want to delete room {node_name}?"))
                .interact()
                .map_err(|_| DcCmdError::IoError)?;

            if confirmed {
                delete_node.await
            } else {
                let msg = format_error_message("Deleting room not confirmed.");
                error!("{}", msg);
                term.write_line(&msg).map_err(|_| DcCmdError::IoError)?;
                Ok(())
            }
        }
        _ => delete_node.await,
    }
}

async fn delete_node_content(
    dracoon: &Dracoon<Connected>,
    search: &str,
    parent_path: String,
) -> Result<(), DcCmdError> {
    let nodes = search_nodes(
        dracoon,
        search,
        Some(&parent_path),
        &ListOptions::new(None, None, None, true, false),
    )
    .await?;
    let node_ids = nodes
        .items
        .iter()
        .filter(|node| node.node_type != NodeType::Room)
        .map(|node| node.id)
        .collect::<Vec<u64>>();

    // ask for confirmation and provide info about number of items to delete
    let confirmed = Confirm::new()
        .with_prompt(format!(
            "Do you really want to delete {} items?",
            node_ids.len()
        ))
        .interact()
        .or(Err(DcCmdError::IoError))?;

    if confirmed {
        dracoon.nodes().delete_nodes(node_ids.into()).await?;
    }

    Ok(())
}

pub async fn create_folder(
    term: Term,
    source: String,
    classification: Option<u8>,
    notes: Option<String>,
    auth: Option<PasswordAuth>,
) -> Result<(), DcCmdError> {
    let dracoon = init_dracoon(&source, auth, false).await?;
    let (parent_path, node_name, _) = parse_path(&source, dracoon.get_base_url().as_ref())?;

    debug!("parent_path: {}", parent_path);
    debug!("base_url: {}", dracoon.get_base_url().as_ref());

    let parent_node = dracoon
        .nodes()
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

    let _folder = dracoon.nodes().create_folder(req).await?;

    let msg = format!("Folder {node_name} created.");
    info!("{}", msg);
    let msg = format_success_message(&msg);
    term.write_line(&msg).map_err(|_| DcCmdError::IoError)?;

    Ok(())
}

pub async fn create_room(
    term: Term,
    source: String,
    opts: CmdMkRoomOptions,
) -> Result<(), DcCmdError> {
    let dracoon = init_dracoon(&source, opts.auth, false).await?;
    let (parent_path, node_name, _) = parse_path(&source, dracoon.get_base_url().as_ref())?;

    let parent_node = dracoon
        .nodes()
        .get_node_from_path(&parent_path)
        .await?
        .ok_or(DcCmdError::InvalidPath(source.clone()))?;

    if parent_node.node_type != NodeType::Room {
        return Err(DcCmdError::InvalidPath(source.clone()));
    }

    let classification = opts.classification.unwrap_or(2);

    let req = match opts.admin_users {
        Some(users) => {
            let handler = UserCommandHandler::new_from_client(dracoon.clone(), term.clone());
            let (tx, mut rx) = tokio::sync::mpsc::channel(MAX_CONCURRENT_REQUESTS);
            let semaphore = Arc::new(tokio::sync::Semaphore::new(MAX_CONCURRENT_REQUESTS));
            let mut handles = Vec::new();

            for user in users {
                let tx = tx.clone();
                let semaphore = semaphore.clone();
                let handler = handler.clone();
                let handle = tokio::spawn(async move {
                    let _permit = semaphore.acquire().await.map_err(|_| {
                        error!("Error acquiring semaphore permit");
                        DcCmdError::IoError
                    })?;
                    let user = handler.find_user_by_username(&user).await?;
                    if let Err(e) = tx.send(user).await {
                        error!("Failed to send user: {}", e);
                    }

                    Ok::<(), DcCmdError>(())
                });

                handles.push(handle);
            }

            drop(tx);

            let mut admin_users = Vec::new();
            while let Some(result) = rx.recv().await {
                admin_users.push(result);
            }

            for handle in handles {
                if let Err(e) = handle.await {
                    error!("Error fetching users: {}", e);
                    return Err(DcCmdError::IoError);
                }
            }

            let admin_users: Vec<_> = admin_users.iter().map(|user| user.id).collect();

            if admin_users.is_empty() {
                return Err(DcCmdError::InvalidArgument(
                    "No valid admin users provided.".to_string(),
                ));
            }

            CreateRoomRequest::builder(&node_name.clone())
                .with_parent_id(parent_node.id)
                .with_classification(classification)
                .with_inherit_permissions(opts.inherit_permissions)
                .with_admin_ids(admin_users)
                .build()
        }
        None => CreateRoomRequest::builder(&node_name.clone())
            .with_parent_id(parent_node.id)
            .with_classification(classification)
            .with_inherit_permissions(true)
            .build(),
    };

    let _room = dracoon.nodes().create_room(req).await?;

    let msg = format!("Room {node_name} created.");
    info!("{}", msg);
    let msg = format_success_message(&msg);
    term.write_line(&msg).map_err(|_| DcCmdError::IoError)?;

    Ok(())
}

pub async fn copy_nodes(
    term: Term,
    source: String,
    target: String,
    opts: CmdCopyOptions,
) -> Result<(), DcCmdError> {
    let client = init_dracoon(&source, opts.auth, false).await?;

    let (source_parent_path, source_node_name, source_depth) =
        parse_path(&source, client.get_base_url().as_ref())?;

    let source_path = build_node_path((
        source_parent_path.clone(),
        source_node_name.clone(),
        source_depth,
    ));

    let nodes = if source.contains('*') {
        let nodes = search_nodes(
            &client,
            &source_node_name,
            Some(&source_parent_path),
            &ListOptions::new(None, None, None, true, false),
        )
        .await?;

        nodes.items
    } else {
        let source_node = client
            .nodes()
            .get_node_from_path(&source_path)
            .await?
            .ok_or(DcCmdError::InvalidPath(source.clone()))?;

        vec![source_node]
    };

    let source_node_ids = nodes.iter().map(|node| node.id).collect::<Vec<u64>>();
    let count_nodes = source_node_ids.len();

    let target_node = client
        .nodes()
        .get_node_from_path(&target)
        .await?
        .ok_or(DcCmdError::InvalidPath(target.clone()))?;

    client
        .nodes()
        .copy_nodes(source_node_ids.into(), target_node.id)
        .await?;

    let msg = format!("Copied {count_nodes} node(s) from {source_parent_path} to {target}.");
    info!("{}", msg);
    let msg = format_success_message(&msg);
    term.write_line(&msg).map_err(|_| DcCmdError::IoError)?;

    Ok(())
}

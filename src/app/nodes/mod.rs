pub mod api;
pub mod command;
mod common;
pub mod download;
pub mod filesystem;
pub mod progress;
pub mod transfer;
pub mod upload;

use dco3::nodes::{models::NodeType, NodeList};

use crate::{
    app::{nodes::api::NodesApi, users::api::UsersApi},
    command::CreateContainerType,
    core::{
        models::{DcCmdError, ListOptions},
        utils::strings::{build_node_path, parse_path},
    },
};
use common::{is_search_query, NodesPathPaginationHelper};

pub struct ListNodesResult {
    pub list: NodeList,
    pub node_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CopyNodesResult {
    pub count_nodes: usize,
    pub source_parent_path: String,
    pub target_path: String,
}

pub enum DeleteNodesPreparation {
    InvalidSearchRequiresRecursive,
    ContainerRequiresRecursive,
    Search {
        node_ids: Vec<u64>,
    },
    SingleNode {
        node_id: u64,
        node_name: String,
        node_type: NodeType,
    },
}

pub struct NodesService<C: NodesApi + UsersApi> {
    api: C,
    path_pagination: NodesPathPaginationHelper,
}

impl<C: NodesApi + UsersApi> NodesService<C> {
    pub fn new(api: C) -> Self {
        Self {
            api,
            path_pagination: NodesPathPaginationHelper::new(),
        }
    }

    pub async fn list_nodes(
        &self,
        source: &str,
        base_url: &str,
        managed: bool,
        opts: &ListOptions,
    ) -> Result<ListNodesResult, DcCmdError> {
        let (parent_path, node_name, depth) = parse_path(source, base_url)?;
        let node_path = build_node_path((parent_path.clone(), node_name.clone(), depth));

        let node_path = if node_path == "//" {
            None
        } else {
            Some(node_path)
        };

        let node_list = if is_search_query(&node_name) {
            self.search_nodes_with_opts(&node_name, Some(&parent_path), opts)
                .await?
        } else {
            self.get_nodes(node_path.as_deref(), Some(managed), opts)
                .await?
        };

        Ok(ListNodesResult {
            list: node_list,
            node_path,
        })
    }

    pub async fn copy_nodes(
        &self,
        source: &str,
        target: &str,
        base_url: &str,
    ) -> Result<CopyNodesResult, DcCmdError> {
        let (source_parent_path, source_node_name, source_depth) = parse_path(source, base_url)?;

        let source_path = build_node_path((
            source_parent_path.clone(),
            source_node_name.clone(),
            source_depth,
        ));

        let source_nodes = if is_search_query(&source_node_name) {
            self.search_nodes_all_pages(&source_node_name, Some(&source_parent_path))
                .await?
                .items
        } else {
            let source_node = self
                .api
                .get_node_from_path(&source_path)
                .await?
                .ok_or_else(|| DcCmdError::InvalidPath(source.to_string()))?;
            vec![source_node]
        };

        let source_node_ids = source_nodes
            .iter()
            .map(|node| node.id)
            .collect::<Vec<u64>>();
        let count_nodes = source_node_ids.len();

        let (target_parent_path, target_node_name, target_depth) = parse_path(target, base_url)?;
        let target_path = build_node_path((target_parent_path, target_node_name, target_depth));

        let target_node = self
            .api
            .get_node_from_path(&target_path)
            .await?
            .ok_or_else(|| DcCmdError::InvalidPath(target.to_string()))?;

        self.api.copy_nodes(source_node_ids, target_node.id).await?;

        Ok(CopyNodesResult {
            count_nodes,
            source_parent_path,
            target_path: target.to_string(),
        })
    }

    pub async fn prepare_delete(
        &self,
        source: &str,
        base_url: &str,
        recursive: bool,
    ) -> Result<DeleteNodesPreparation, DcCmdError> {
        let (parent_path, node_name, depth) = parse_path(source, base_url)?;

        if is_search_query(&node_name) {
            if !recursive {
                return Ok(DeleteNodesPreparation::InvalidSearchRequiresRecursive);
            }

            let node_list = self
                .search_nodes_all_pages(&node_name, Some(&parent_path))
                .await?;
            let node_ids = node_list
                .items
                .into_iter()
                .filter(|node| node.node_type != NodeType::Room)
                .map(|node| node.id)
                .collect::<Vec<_>>();

            return Ok(DeleteNodesPreparation::Search { node_ids });
        }

        let node_path = build_node_path((parent_path, node_name.clone(), depth));
        let node = self
            .api
            .get_node_from_path(&node_path)
            .await?
            .ok_or_else(|| DcCmdError::InvalidPath(source.to_string()))?;

        if !recursive && (node.node_type == NodeType::Folder || node.node_type == NodeType::Room) {
            return Ok(DeleteNodesPreparation::ContainerRequiresRecursive);
        }

        Ok(DeleteNodesPreparation::SingleNode {
            node_id: node.id,
            node_name,
            node_type: node.node_type,
        })
    }

    pub async fn delete_node(&self, node_id: u64) -> Result<(), DcCmdError> {
        self.api.delete_node(node_id).await
    }

    pub async fn delete_nodes(&self, node_ids: Vec<u64>) -> Result<(), DcCmdError> {
        self.api.delete_nodes(node_ids).await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn create_container(
        &self,
        source: &str,
        base_url: &str,
        container_type: CreateContainerType,
        classification: Option<u8>,
        notes: Option<String>,
        admin_users: Option<Vec<String>>,
        inherit_permissions: bool,
    ) -> Result<String, DcCmdError> {
        let (parent_path, node_name, _) = parse_path(source, base_url)?;

        let parent_node = self
            .api
            .get_node_from_path(&parent_path)
            .await?
            .ok_or_else(|| DcCmdError::InvalidPath(source.to_string()))?;

        match container_type {
            CreateContainerType::Folder => {
                if admin_users.is_some() || inherit_permissions {
                    return Err(DcCmdError::InvalidArgument(
                        "Room-only options are not supported when creating folders.".to_string(),
                    ));
                }

                self.api
                    .create_folder(&node_name, parent_node.id, classification, notes)
                    .await?;

                Ok(format!("Folder {node_name} created."))
            }
            CreateContainerType::Room => {
                if notes.is_some() {
                    return Err(DcCmdError::InvalidArgument(
                        "Notes are only supported when creating folders.".to_string(),
                    ));
                }

                if parent_node.node_type != NodeType::Room {
                    return Err(DcCmdError::InvalidPath(source.to_string()));
                }

                let classification = classification.unwrap_or(2);

                let admin_ids = if let Some(admin_users) = admin_users {
                    let mut admin_ids = Vec::with_capacity(admin_users.len());
                    for user_name in admin_users {
                        let user_id = self.api.find_user_id_by_username(&user_name).await?;
                        admin_ids.push(user_id);
                    }

                    if admin_ids.is_empty() {
                        return Err(DcCmdError::InvalidArgument(
                            "No valid admin users provided.".to_string(),
                        ));
                    }

                    Some(admin_ids)
                } else {
                    None
                };

                let inherit_permissions =
                    effective_room_inherit_permissions(admin_ids.is_some(), inherit_permissions);

                self.api
                    .create_room(
                        &node_name,
                        parent_node.id,
                        classification,
                        inherit_permissions,
                        admin_ids,
                    )
                    .await?;

                Ok(format!("Room {node_name} created."))
            }
        }
    }

    async fn get_nodes(
        &self,
        node_path: Option<&str>,
        managed: Option<bool>,
        opts: &ListOptions,
    ) -> Result<NodeList, DcCmdError> {
        let parent_id = self
            .path_pagination
            .resolve_parent_id(&self.api, node_path)
            .await?;
        self.path_pagination
            .get_nodes_with_opts(&self.api, parent_id, managed, opts)
            .await
    }

    async fn search_nodes_with_opts(
        &self,
        search_string: &str,
        node_path: Option<&str>,
        opts: &ListOptions,
    ) -> Result<NodeList, DcCmdError> {
        let parent_id = self
            .path_pagination
            .resolve_parent_id(&self.api, node_path)
            .await?;
        self.path_pagination
            .search_nodes_with_opts(&self.api, search_string, parent_id, Some(0), opts)
            .await
    }

    async fn search_nodes_all_pages(
        &self,
        search_string: &str,
        node_path: Option<&str>,
    ) -> Result<NodeList, DcCmdError> {
        let parent_id = self
            .path_pagination
            .resolve_parent_id(&self.api, node_path)
            .await?;
        self.path_pagination
            .search_nodes_all_pages(&self.api, search_string, parent_id, Some(0))
            .await
    }
}

fn effective_room_inherit_permissions(has_admin_users: bool, inherit_permissions: bool) -> bool {
    if has_admin_users {
        inherit_permissions
    } else {
        true
    }
}

#[cfg(test)]
mod tests;

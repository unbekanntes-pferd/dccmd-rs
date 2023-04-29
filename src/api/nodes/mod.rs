use self::models::{
    CreateFolderRequest, FileMeta, Node, NodeList, DownloadProgressCallback, TransferNodesRequest,
    UpdateFolderRequest, UploadOptions, UploadProgressCallback,
};
use super::{auth::errors::DracoonClientError, models::ListAllParams};
use async_trait::async_trait;
use std::io::Write;
use tokio::io::{AsyncRead, BufReader};

pub mod download;
pub mod folders;
pub mod models;
pub mod nodes;
pub mod rooms;
pub mod upload;

#[async_trait]
pub trait Nodes {
    /// Returns a list of nodes
    async fn get_nodes(
        &self,
        parent_id: Option<u64>,
        room_manager: Option<bool>,
        params: Option<ListAllParams>,
    ) -> Result<NodeList, DracoonClientError>;

    /// Searches for a node by path
    /// Returns the node if found (or None if not found)
    async fn get_node_from_path(&self, path: &str) -> Result<Option<Node>, DracoonClientError>;

    /// Searches for nodes by search string
    async fn search_nodes(
        &self,
        search_string: &str,
        parent_id: Option<u64>,
        depth_level: Option<i8>,
        params: Option<ListAllParams>,
    ) -> Result<NodeList, DracoonClientError>;

    /// Returns a node by id
    async fn get_node(&self, node_id: u64) -> Result<Node, DracoonClientError>;

    /// Deletes a node by id
    async fn delete_node(&self, node_id: u64) -> Result<(), DracoonClientError>;

    /// Deletes multiple nodes by ids
    async fn delete_nodes(&self, node_ids: Vec<u64>) -> Result<(), DracoonClientError>;

    /// Move nodes to a target parent node
    async fn move_nodes(
        &self,
        req: TransferNodesRequest,
        target_parent_id: u64,
    ) -> Result<Node, DracoonClientError>;

    /// Copy nodes to a target parent node
    async fn copy_nodes(
        &self,
        req: TransferNodesRequest,
        target_parent_id: u64,
    ) -> Result<Node, DracoonClientError>;
}

#[async_trait]
pub trait Folders {
    /// Creates a folder with given params in the given parent node
    async fn create_folder(&self, req: CreateFolderRequest) -> Result<Node, DracoonClientError>;

    /// Updates a folder with given params by id
    async fn update_folder(
        &self,
        folder_id: u64,
        req: UpdateFolderRequest,
    ) -> Result<Node, DracoonClientError>;
}

#[async_trait]
pub trait Rooms {
    async fn create_room(&self, parent_id: u64, name: &str) -> Result<Node, DracoonClientError>;

    async fn update_room(&self, node_id: u64, name: &str) -> Result<Node, DracoonClientError>;

    async fn config_room(&self, node_id: u64, name: &str) -> Result<Node, DracoonClientError>;

    async fn encrypt_room(&self, node_id: u64, name: &str) -> Result<Node, DracoonClientError>;

    async fn get_room_groups(&self, node_id: u64) -> Result<Node, DracoonClientError>;

    async fn update_room_groups(
        &self,
        node_id: u64,
        name: &str,
    ) -> Result<Node, DracoonClientError>;

    async fn delete_room_groups(
        &self,
        node_id: u64,
        name: &str,
    ) -> Result<Node, DracoonClientError>;

    async fn get_room_users(&self, node_id: u64) -> Result<Node, DracoonClientError>;

    async fn update_room_users(&self, node_id: u64, name: &str)
        -> Result<Node, DracoonClientError>;

    async fn delete_room_users(&self, node_id: u64, name: &str)
        -> Result<Node, DracoonClientError>;
}

#[async_trait]
pub trait Download {
    /// Downloads a file (node) to the given writer buffer
    async fn download<'w>(
        &'w mut self,
        node: &Node,
        writer: &'w mut (dyn Write + Send),
        mut callback: Option<DownloadProgressCallback>,
    ) -> Result<(), DracoonClientError>;
}

#[async_trait]
pub trait Upload<R: AsyncRead> {
    /// Uploads a file (buffer reader) with given file meta info to the given parent node
    async fn upload<'r>(
        &'r self,
        file_meta: FileMeta,
        parent_node: &Node,
        upload_options: UploadOptions,
        mut reader: BufReader<R>,
        mut callback: Option<UploadProgressCallback>,
        chunk_size: Option<usize>
    ) -> Result<Node, DracoonClientError>;
}

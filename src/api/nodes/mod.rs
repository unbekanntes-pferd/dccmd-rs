use self::{models::{NodeList, Node, ProgressCallback, FileMeta, UploadOptions}};
use super::{auth::errors::DracoonClientError, models::ListAllParams};
use async_trait::async_trait;
use tokio::io::AsyncRead;
use std::io::{Write, Read};

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

    /// Searches for a node by path (returns 404 if no node was found)
    async fn get_node_from_path(&self, path: &str) -> Result<Node, DracoonClientError>;
}

#[async_trait]
pub trait Download {
    /// Downloads a file (node) to the given writer buffer
    async fn download<'w>(&'w self, node: &Node, writer: &'w mut (dyn Write + Send), mut callback: Option<ProgressCallback>) -> Result<(), DracoonClientError>;
}


#[async_trait]
pub trait Upload {
    /// Uploads a file (buffer reader) with given file meta info to the given parent node
    async fn upload<'r>(&'r self, file_meta: FileMeta, parent_node: &Node, upload_options: UploadOptions, reader: &'r mut (dyn AsyncRead + Send), mut callback: Option<ProgressCallback>) -> Result<Node, DracoonClientError>;
}

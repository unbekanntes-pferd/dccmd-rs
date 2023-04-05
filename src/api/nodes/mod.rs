use self::models::{NodeList, Node};
use super::{auth::errors::DracoonClientError, models::ListAllParams};
use async_trait::async_trait;
use std::io::Write;

pub mod download;
pub mod folders;
pub mod models;
pub mod nodes;
pub mod rooms;
pub mod upload;

#[async_trait]
pub trait Nodes {
    async fn get_nodes(
        &self,
        parent_id: Option<u64>,
        room_manager: Option<bool>,
        params: Option<ListAllParams>,
    ) -> Result<NodeList, DracoonClientError>;

    async fn get_node_from_path(&self, path: &str) -> Result<Node, DracoonClientError>;
}

#[async_trait]
pub trait Download {
    async fn download<'w>(&'w self, node: &Node, writer: &'w mut (dyn Write + Send)) -> Result<(), DracoonClientError>;
}

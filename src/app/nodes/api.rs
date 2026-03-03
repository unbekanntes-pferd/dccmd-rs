use async_trait::async_trait;
use dco3::{
    auth::Connected,
    nodes::{
        models::{CreateFolderRequest, NodeList},
        rooms::models::CreateRoomRequest,
    },
    Dracoon, Folders, ListAllParams, Nodes, Rooms,
};

use crate::core::models::DcCmdError;

#[async_trait]
pub trait NodesApi: Send + Sync {
    async fn get_node_from_path(
        &self,
        node_path: &str,
    ) -> Result<Option<dco3::nodes::Node>, DcCmdError>;

    async fn get_nodes(
        &self,
        parent_id: Option<u64>,
        managed: Option<bool>,
        params: Option<ListAllParams>,
    ) -> Result<NodeList, DcCmdError>;

    async fn search_nodes(
        &self,
        search_string: &str,
        parent_id: Option<u64>,
        depth_level: Option<i8>,
        params: Option<ListAllParams>,
    ) -> Result<NodeList, DcCmdError>;

    async fn delete_node(&self, node_id: u64) -> Result<(), DcCmdError>;

    async fn delete_nodes(&self, node_ids: Vec<u64>) -> Result<(), DcCmdError>;

    async fn copy_nodes(&self, node_ids: Vec<u64>, target_parent_id: u64)
        -> Result<(), DcCmdError>;

    async fn create_folder(
        &self,
        node_name: &str,
        parent_id: u64,
        classification: Option<u8>,
        notes: Option<String>,
    ) -> Result<(), DcCmdError>;

    async fn create_room(
        &self,
        node_name: &str,
        parent_id: u64,
        classification: u8,
        inherit_permissions: bool,
        admin_ids: Option<Vec<u64>>,
    ) -> Result<(), DcCmdError>;
}

#[async_trait]
impl NodesApi for Dracoon<Connected> {
    async fn get_node_from_path(
        &self,
        node_path: &str,
    ) -> Result<Option<dco3::nodes::Node>, DcCmdError> {
        self.nodes()
            .get_node_from_path(node_path)
            .await
            .map_err(Into::into)
    }

    async fn get_nodes(
        &self,
        parent_id: Option<u64>,
        managed: Option<bool>,
        params: Option<ListAllParams>,
    ) -> Result<NodeList, DcCmdError> {
        self.nodes()
            .get_nodes(parent_id, managed, params)
            .await
            .map_err(Into::into)
    }

    async fn search_nodes(
        &self,
        search_string: &str,
        parent_id: Option<u64>,
        depth_level: Option<i8>,
        params: Option<ListAllParams>,
    ) -> Result<NodeList, DcCmdError> {
        self.nodes()
            .search_nodes(search_string, parent_id, depth_level, params)
            .await
            .map_err(Into::into)
    }

    async fn delete_node(&self, node_id: u64) -> Result<(), DcCmdError> {
        self.nodes().delete_node(node_id).await.map_err(Into::into)
    }

    async fn delete_nodes(&self, node_ids: Vec<u64>) -> Result<(), DcCmdError> {
        self.nodes()
            .delete_nodes(node_ids.into())
            .await
            .map_err(Into::into)
    }

    async fn copy_nodes(
        &self,
        node_ids: Vec<u64>,
        target_parent_id: u64,
    ) -> Result<(), DcCmdError> {
        self.nodes()
            .copy_nodes(node_ids.into(), target_parent_id)
            .await
            .map(|_| ())
            .map_err(Into::into)
    }

    async fn create_folder(
        &self,
        node_name: &str,
        parent_id: u64,
        classification: Option<u8>,
        notes: Option<String>,
    ) -> Result<(), DcCmdError> {
        let req = CreateFolderRequest::builder(node_name.to_string(), parent_id);
        let req = if let Some(classification) = classification {
            req.with_classification(classification)
        } else {
            req
        };
        let req = if let Some(notes) = notes {
            req.with_notes(notes)
        } else {
            req
        };

        self.nodes()
            .create_folder(req.build())
            .await
            .map(|_| ())
            .map_err(Into::into)
    }

    async fn create_room(
        &self,
        node_name: &str,
        parent_id: u64,
        classification: u8,
        inherit_permissions: bool,
        admin_ids: Option<Vec<u64>>,
    ) -> Result<(), DcCmdError> {
        let req = CreateRoomRequest::builder(node_name)
            .with_parent_id(parent_id)
            .with_classification(classification)
            .with_inherit_permissions(inherit_permissions);
        let req = if let Some(admin_ids) = admin_ids {
            req.with_admin_ids(admin_ids)
        } else {
            req
        };

        self.nodes()
            .create_room(req.build())
            .await
            .map(|_| ())
            .map_err(Into::into)
    }
}

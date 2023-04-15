use async_trait::async_trait;
use reqwest::header;

use crate::api::{
    auth::{errors::DracoonClientError, Connected},
    constants::{DRACOON_API_PREFIX, FOLDERS_BASE, NODES_BASE},
    utils::FromResponse,
    Dracoon,
};

use super::{
    models::{CreateFolderRequest, Node, UpdateFolderRequest},
    Folders,
};

#[async_trait]
impl Folders for Dracoon<Connected> {
    async fn create_folder(&self, req: CreateFolderRequest) -> Result<Node, DracoonClientError> {
        let url_part = format!("/{}/{}/{}", DRACOON_API_PREFIX, NODES_BASE, FOLDERS_BASE);

        let api_url = self.build_api_url(&url_part);
        let response = self
            .client
            .http
            .post(api_url)
            .header(header::AUTHORIZATION, self.get_auth_header().await?)
            .header(header::CONTENT_TYPE, "application/json")
            .json(&req)
            .send()
            .await?;

        Node::from_response(response).await
    }

    async fn update_folder(&self, folder_id: u64, req: UpdateFolderRequest) -> Result<Node, DracoonClientError> {
        let url_part = format!("/{}/{}/{}/{}", DRACOON_API_PREFIX, NODES_BASE, FOLDERS_BASE, folder_id);

        let api_url = self.build_api_url(&url_part);

        let response = self
            .client
            .http
            .put(api_url)
            .header(header::AUTHORIZATION, self.get_auth_header().await?)
            .header(header::CONTENT_TYPE, "application/json")
            .json(&req)
            .send()
            .await?;

        Node::from_response(response).await
    }
}

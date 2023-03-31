use async_trait::async_trait;
use reqwest::header;

use crate::api::constants::DRACOON_API_PREFIX;

use self::models::NodeList;

use super::{
    auth::{errors::DracoonClientError, Connected},
    constants::GET_NODES,
    models::ListAllParams,
    Dracoon,
};

pub mod models;

#[async_trait]
pub trait Nodes {
    async fn get_nodes(
        &self,
        params: Option<ListAllParams>,
    ) -> Result<NodeList, DracoonClientError>;
}

#[async_trait]
impl Nodes for Dracoon<Connected> {
    async fn get_nodes(
        &self,
        params: Option<ListAllParams>,
    ) -> Result<NodeList, DracoonClientError> {
        let params = params.unwrap_or_default();
        let url_part = format!(
            "/{}/{}/{}",
            DRACOON_API_PREFIX,
            GET_NODES,
            String::from(params)
        );
        let api_url = self.build_api_url(&url_part);

        let auth_header = format!(
            "Bearer {}",
            self.client
                .connection
                .as_ref()
                .expect("Connected client has a connection")
                .access_token
        );

        let response = self
            .client
            .http
            .get(api_url)
            .header(header::AUTHORIZATION, auth_header)
            .header(header::CONTENT_TYPE, "application/json")
            .send()
            .await?;

        NodeList::from_response(response).await
    }
}


impl Dracoon<Connected> {
    
}
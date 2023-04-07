use async_trait::async_trait;
use reqwest::header;
use tracing::debug;

use crate::{
    api::{
        auth::{errors::DracoonClientError, Connected, models::DracoonErrorResponse},
        constants::{DRACOON_API_PREFIX, NODES_BASE, NODES_SEARCH},
        models::ListAllParams,
        Dracoon,
    },
    cmd::utils::strings::parse_node_path,
};

use super::{
    models::{Node, NodeList},
    Nodes,
};

#[async_trait]
impl Nodes for Dracoon<Connected> {
    async fn get_nodes(
        &self,
        parent_id: Option<u64>,
        room_manager: Option<bool>,
        params: Option<ListAllParams>,
    ) -> Result<NodeList, DracoonClientError> {
        let params = params.unwrap_or_default();
        let url_part = format!(
            "/{}/{}",
            DRACOON_API_PREFIX,
            NODES_BASE
        );

        let mut api_url = self.build_api_url(&url_part);

        api_url.query_pairs_mut()
       .extend_pairs(params.limit.map(|v| ("limit", v.to_string())))
       .extend_pairs(params.offset.map(|v| ("offset", v.to_string())))
       .extend_pairs(params.sort.map(|v| ("sort_by", v.to_string())))
       .extend_pairs(params.filter.map(|v| ("filter", v.to_string())))
       .extend_pairs(room_manager.map(|v| ("room_manager", v.to_string())))
       .extend_pairs(parent_id.map(|v| ("parent_id", v.to_string())))
       .finish();


        let response = self
            .client
            .http
            .get(api_url)
            .header(header::AUTHORIZATION, self.get_auth_header().await?)
            .header(header::CONTENT_TYPE, "application/json")
            .send()
            .await?;

        NodeList::from_response(response).await
    }

    async fn get_node_from_path(&self, path: &str) -> Result<Node, DracoonClientError> {
        let url_part = format!("/{}/{}/{}", DRACOON_API_PREFIX, NODES_BASE, NODES_SEARCH);

        debug!("Looking up node - path: {}", path);
        
        let base_url = self.client.get_base_url().to_string();
        let base_url = base_url.trim_start_matches("https://");
        let base_url = base_url.trim_end_matches('/');

        debug!("Base url: {}", base_url);
        let (parent_path, name, depth) =
            parse_node_path(path, base_url).or(Err(DracoonClientError::InvalidUrl))?;

        debug!("Looking up node - parent_path: {}", parent_path);
        debug!("Parsed name: {}", name);
        debug!("Calculated depth: {}", depth);

        let mut api_url = self.build_api_url(&url_part);

        api_url.query_pairs_mut()
            .append_pair("search_string", &name)
            .append_pair("depth_level", &depth.to_string())
            .append_pair("filter", &format!("parentPath:eq:{}", parent_path))
            .finish();
        

        let response = self
            .client
            .http
            .get(api_url)
            .header(header::AUTHORIZATION, self.get_auth_header().await?)
            .header(header::CONTENT_TYPE, "application/json")
            .send()
            .await?;

        let nodes = NodeList::from_response(response).await?;

        debug!("Found {} nodes", nodes.items.len());

        match nodes.items.len() {
            1 => Ok(nodes.items.into_iter().next().ok_or(DracoonClientError::Http(DracoonErrorResponse::new(404, "Not found")))?),
            _ => Err(DracoonClientError::Http(DracoonErrorResponse::new(404, "Not found"))),  
        }

    }
}

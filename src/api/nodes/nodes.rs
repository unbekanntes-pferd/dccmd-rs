#![allow(clippy::module_inception)]

use async_trait::async_trait;
use reqwest::header;
use tracing::{debug, error};

use crate::api::{
    auth::{errors::DracoonClientError, Connected},
    constants::{DRACOON_API_PREFIX, NODES_BASE, NODES_COPY, NODES_MOVE, NODES_SEARCH},
    models::ListAllParams,
    utils::FromResponse,
    Dracoon,
};

use super::{
    models::{DeleteNodesRequest, Node, NodeList, TransferNodesRequest},
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
        let url_part = format!("/{DRACOON_API_PREFIX}/{NODES_BASE}");

        let mut api_url = self.build_api_url(&url_part);

        api_url
            .query_pairs_mut()
            .extend_pairs(params.limit.map(|v| ("limit", v.to_string())))
            .extend_pairs(params.offset.map(|v| ("offset", v.to_string())))
            .extend_pairs(params.sort.map(|v| ("sort", v)))
            .extend_pairs(params.filter.map(|v| ("filter", v)))
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

    async fn get_node_from_path(&self, path: &str) -> Result<Option<Node>, DracoonClientError> {
        let url_part = format!("/{DRACOON_API_PREFIX}/{NODES_BASE}/{NODES_SEARCH}");

        debug!("Looking up node - path: {}", path);

        let (parent_path, name, depth) = parse_node_path(path).map_err(|_| {
            error!("Failed to parse path: {}", path);
            DracoonClientError::InvalidPath(path.to_string())
        })?;

        debug!("Looking up node - parent_path: {}", parent_path);
        debug!("Parsed name: {}", name);
        debug!("Calculated depth: {}", depth);

        let mut api_url = self.build_api_url(&url_part);

        api_url
            .query_pairs_mut()
            .append_pair("search_string", &name)
            .append_pair("depth_level", &depth.to_string())
            .append_pair("filter", &format!("parentPath:eq:{parent_path}"))
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
            1 => Ok(nodes.items.into_iter().next()),
            _ => Ok(None),
        }
    }

    async fn get_node(&self, node_id: u64) -> Result<Node, DracoonClientError> {
        let url_part = format!("/{DRACOON_API_PREFIX}/{NODES_BASE}/{node_id}");

        let api_url = self.build_api_url(&url_part);

        let response = self
            .client
            .http
            .get(api_url)
            .header(header::AUTHORIZATION, self.get_auth_header().await?)
            .header(header::CONTENT_TYPE, "application/json")
            .send()
            .await?;

        Node::from_response(response).await
    }

    async fn search_nodes(
        &self,
        search_string: &str,
        parent_id: Option<u64>,
        depth_level: Option<i8>,
        params: Option<ListAllParams>,
    ) -> Result<NodeList, DracoonClientError> {
        let params = params.unwrap_or_default();
        let url_part = format!("/{DRACOON_API_PREFIX}/{NODES_BASE}/{NODES_SEARCH}");

        let mut api_url = self.build_api_url(&url_part);

        api_url
            .query_pairs_mut()
            .append_pair("search_string", search_string)
            .extend_pairs(depth_level.map(|v| ("depth_level", v.to_string())))
            .extend_pairs(params.limit.map(|v| ("limit", v.to_string())))
            .extend_pairs(params.offset.map(|v| ("offset", v.to_string())))
            .extend_pairs(params.sort.map(|v| ("sort_by", v)))
            .extend_pairs(params.filter.map(|v| ("filter", v)))
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

    async fn delete_node(&self, node_id: u64) -> Result<(), DracoonClientError> {
        let url_part = format!("/{DRACOON_API_PREFIX}/{NODES_BASE}/{node_id}");

        let api_url = self.build_api_url(&url_part);

        let response = self
            .client
            .http
            .delete(api_url)
            .header(header::AUTHORIZATION, self.get_auth_header().await?)
            .header(header::CONTENT_TYPE, "application/json")
            .send()
            .await?;

        if response.status().is_server_error() || response.status().is_client_error() {
            return Err(DracoonClientError::from_response(response)
                .await
                .expect("Could not parse error response"));
        }

        Ok(())
    }

    async fn delete_nodes(&self, node_ids: Vec<u64>) -> Result<(), DracoonClientError> {
        let url_part = format!("/{DRACOON_API_PREFIX}/{NODES_BASE}");

        let api_url = self.build_api_url(&url_part);

        let del_node_req: DeleteNodesRequest = node_ids.into();

        let response = self
            .client
            .http
            .delete(api_url)
            .header(header::AUTHORIZATION, self.get_auth_header().await?)
            .header(header::CONTENT_TYPE, "application/json")
            .json(&del_node_req)
            .send()
            .await?;

        if response.status().is_server_error() || response.status().is_client_error() {
            return Err(DracoonClientError::from_response(response)
                .await
                .expect("Could not parse error response"));
        }

        Ok(())
    }

    async fn move_nodes(
        &self,
        req: TransferNodesRequest,
        target_parent_id: u64,
    ) -> Result<Node, DracoonClientError> {
        let url_part =
            format!("/{DRACOON_API_PREFIX}/{NODES_BASE}/{target_parent_id}/{NODES_MOVE}");

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

    async fn copy_nodes(
        &self,
        req: TransferNodesRequest,
        target_parent_id: u64,
    ) -> Result<Node, DracoonClientError> {
        let url_part =
            format!("/{DRACOON_API_PREFIX}/{NODES_BASE}/{target_parent_id}/{NODES_COPY}");

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
}

type ParsedPath = (String, String, u64);
pub fn parse_node_path(path: &str) -> Result<ParsedPath, DracoonClientError> {
    if path == "/" {
        return Ok((String::from("/"), String::new(), 0));
    }
    
    let path_parts: Vec<&str> = path.trim_end_matches('/').split('/').collect();
    let name = String::from(*path_parts.last().ok_or(DracoonClientError::InvalidPath(path.to_string()))?);
    let parent_path = format!("{}/", path_parts[..path_parts.len() - 1].join("/"));
    let depth = path_parts.len().saturating_sub(2) as u64;
    
    Ok((parent_path, name, depth))
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_folder_path() {
        let path = "/test/folder/";
        let (parent_path, name, depth) = parse_node_path(path).unwrap();
        assert_eq!("/test/", parent_path);
        assert_eq!("folder", name);
        assert_eq!(1, depth);
    }

    #[test]
    fn test_parse_folder_path_deep() {
        let path = "/test/folder/sub1/";
        let (parent_path, name, depth) = parse_node_path(path).unwrap();
        assert_eq!("/test/folder/", parent_path);
        assert_eq!("sub1", name);
        assert_eq!(2, depth);
    }

    #[test]
    fn test_parse_folder_path_deeper() {
        let path = "/test/folder/sub1/sub2/sub3/";
        let (parent_path, name, depth) = parse_node_path(path).unwrap();
        assert_eq!("/test/folder/sub1/sub2/", parent_path);
        assert_eq!("sub3", name);
        assert_eq!(4, depth);
    }

    #[test]
    fn test_parse_folder_path_no_trail_slash() {
        let path = "/test/folder";
        let (parent_path, name, depth) = parse_node_path(path).unwrap();
        assert_eq!("/test/", parent_path);
        assert_eq!("folder", name);
        assert_eq!(1, depth);
    }

    #[test]
    fn test_file_path() {
        let path = "/test/folder/file.txt";
        let (parent_path, name, depth) = parse_node_path(path).unwrap();
        assert_eq!("/test/folder/", parent_path);
        assert_eq!("file.txt", name);
        assert_eq!(2, depth);
    }

    #[test]
    fn test_root_path() {
        let path = "/";
        let (parent_path, name, depth) = parse_node_path(path).unwrap();
        assert_eq!("/", parent_path);
        assert_eq!("", name);
        assert_eq!(0, depth);
    }
}
use async_trait::async_trait;
use reqwest::header;

use crate::api::{
    auth::{errors::DracoonClientError, Connected},
    constants::{DRACOON_API_PREFIX, ROOMS_BASE, NODES_BASE, ROOMS_CONFIG, ROOMS_USERS, ROOMS_GROUPS, ROOMS_ENCRYPT},
    models::ListAllParams,
    Dracoon, utils::FromResponse,
};

use self::models::{
    ConfigRoomRequest, CreateRoomRequest, EncryptRoomRequest, RoomGroupList,
    RoomGroupsAddBatchRequest, RoomGroupsDeleteBatchRequest, RoomUserList,
    RoomUsersAddBatchRequest, RoomUsersDeleteBatchRequest, UpdateRoomRequest,
};

use super::{models::Node, Rooms};

pub mod models;

#[async_trait]
impl Rooms for Dracoon<Connected> {
    async fn create_room(
        &self,
        create_room_req: CreateRoomRequest,
    ) -> Result<Node, DracoonClientError> {
        let url_part = format!("/{DRACOON_API_PREFIX}/{NODES_BASE}/{ROOMS_BASE}");
        let api_url = self.build_api_url(&url_part);

        let response = self
            .client
            .http
            .post(api_url)
            .header(header::AUTHORIZATION, self.get_auth_header().await?)
            .header(header::CONTENT_TYPE, "application/json")
            .json(&create_room_req)
            .send()
            .await?;
        
        Node::from_response(response).await
    }
    async fn update_room(
        &self,
        room_id: u64,
        update_room_req: UpdateRoomRequest,
    ) -> Result<Node, DracoonClientError> {
        let url_part = format!("/{DRACOON_API_PREFIX}/{NODES_BASE}/{ROOMS_BASE}/{room_id}");
        let api_url = self.build_api_url(&url_part);

        let response = self
            .client
            .http
            .put(api_url)
            .header(header::AUTHORIZATION, self.get_auth_header().await?)
            .header(header::CONTENT_TYPE, "application/json")
            .json(&update_room_req)
            .send()
            .await?;

        Node::from_response(response).await

    }
    async fn config_room(
        &self,
        room_id: u64,
        config_room_req: ConfigRoomRequest,
    ) -> Result<Node, DracoonClientError> {

        let url_part = format!("/{DRACOON_API_PREFIX}/{NODES_BASE}/{ROOMS_BASE}/{room_id}/{ROOMS_CONFIG}");
        let api_url = self.build_api_url(&url_part);

        let response = self
            .client
            .http
            .put(api_url)
            .header(header::AUTHORIZATION, self.get_auth_header().await?)
            .header(header::CONTENT_TYPE, "application/json")
            .json(&config_room_req)
            .send()
            .await?;

        Node::from_response(response).await
    }
    async fn encrypt_room(
        &self,
        room_id: u64,
        encrypt_room_req: EncryptRoomRequest,
    ) -> Result<Node, DracoonClientError> {
        let url_part = format!("/{DRACOON_API_PREFIX}/{NODES_BASE}/{ROOMS_BASE}/{room_id}/{ROOMS_ENCRYPT}");
        let api_url = self.build_api_url(&url_part);

        let response = self
            .client
            .http
            .put(api_url)
            .header(header::AUTHORIZATION, self.get_auth_header().await?)
            .header(header::CONTENT_TYPE, "application/json")
            .json(&encrypt_room_req)
            .send()
            .await?;


        Node::from_response(response).await
    }
    async fn get_room_groups(
        &self,
        room_id: u64,
        params: Option<ListAllParams>,
    ) -> Result<RoomGroupList, DracoonClientError> {
        let url_part = format!("/{DRACOON_API_PREFIX}/{NODES_BASE}/{ROOMS_BASE}/{room_id}/{ROOMS_GROUPS}");
        let mut api_url = self.build_api_url(&url_part);

        let params = params.unwrap_or_default();
        let filters = params.filter_to_string();
        let sorts = params.sort_to_string();

        api_url.query_pairs_mut()
        .extend_pairs(params.limit.map(|limit| ("limit", limit.to_string())))
        .extend_pairs(params.offset.map(|offset| ("offset", offset.to_string())))
        .extend_pairs(params.filter.map(|filter| ("filter", filters)))
        .extend_pairs(params.sort.map(|sort| ("sort", sorts)))
        .finish();

        let response = self
            .client
            .http
            .get(api_url)
            .header(header::AUTHORIZATION, self.get_auth_header().await?)
            .send()
            .await?;

        RoomGroupList::from_response(response).await

    
    }
    async fn update_room_groups(
        &self,
        room_id: u64,
        room_groups_update_req: RoomGroupsAddBatchRequest,
    ) -> Result<(), DracoonClientError> {
        let url_part = format!("/{DRACOON_API_PREFIX}/{NODES_BASE}/{ROOMS_BASE}/{room_id}/{ROOMS_GROUPS}");
        let api_url = self.build_api_url(&url_part);

        let response = self
            .client
            .http
            .put(api_url)
            .header(header::AUTHORIZATION, self.get_auth_header().await?)
            .header(header::CONTENT_TYPE, "application/json")
            .json(&room_groups_update_req)
            .send()
            .await?;

        if response.status().is_client_error() || response.status().is_server_error() {
            return Err(DracoonClientError::from_response(response).await?);
        }

        Ok(())

    }
    async fn delete_room_groups(
        &self,
        room_id: u64,
        room_groups_del_req: RoomGroupsDeleteBatchRequest,
    ) -> Result<(), DracoonClientError> {
        let url_part = format!("/{DRACOON_API_PREFIX}/{NODES_BASE}/{ROOMS_BASE}/{room_id}/{ROOMS_GROUPS}");
        let api_url = self.build_api_url(&url_part);

        let response = self
            .client
            .http
            .delete(api_url)
            .header(header::AUTHORIZATION, self.get_auth_header().await?)
            .header(header::CONTENT_TYPE, "application/json")
            .json(&room_groups_del_req)
            .send()
            .await?;

        if response.status().is_client_error() || response.status().is_server_error() {
            return Err(DracoonClientError::from_response(response).await?);
        }

        Ok(())
    }
    async fn get_room_users(
        &self,
        room_id: u64,
        params: Option<ListAllParams>,
    ) -> Result<RoomUserList, DracoonClientError> {
        let url_part = format!("/{DRACOON_API_PREFIX}/{NODES_BASE}/{ROOMS_BASE}/{room_id}/{ROOMS_USERS}");
        let mut api_url = self.build_api_url(&url_part);

        let params = params.unwrap_or_default();

        let filters = params.filter_to_string();
        let sorts = params.sort_to_string();

        api_url.query_pairs_mut()
        .extend_pairs(params.limit.map(|limit| ("limit", limit.to_string())))
        .extend_pairs(params.offset.map(|offset| ("offset", offset.to_string())))
        .extend_pairs(params.filter.map(|filter| ("filter", filters)))
        .extend_pairs(params.sort.map(|sort| ("sort", sorts)))
        .finish();

        let response = self
            .client
            .http
            .get(api_url)
            .header(header::AUTHORIZATION, self.get_auth_header().await?)
            .send()
            .await?;

        RoomUserList::from_response(response).await
 
    }
    async fn update_room_users(
        &self,
        room_id: u64,
        room_users_update_req: RoomUsersAddBatchRequest,
    ) -> Result<(), DracoonClientError> {
        let url_part = format!("/{DRACOON_API_PREFIX}/{NODES_BASE}/{ROOMS_BASE}/{room_id}/{ROOMS_USERS}");
        let api_url = self.build_api_url(&url_part);

        let response = self
            .client
            .http
            .put(api_url)
            .header(header::AUTHORIZATION, self.get_auth_header().await?)
            .header(header::CONTENT_TYPE, "application/json")
            .json(&room_users_update_req)
            .send()
            .await?;

        if response.status().is_client_error() || response.status().is_server_error() {
            return Err(DracoonClientError::from_response(response).await?);
        }

        Ok(())
  
    }
    async fn delete_room_users(
        &self,
        room_id: u64,
        room_users_del_req: RoomUsersDeleteBatchRequest,
    ) -> Result<(), DracoonClientError> {
        let url_part = format!("/{DRACOON_API_PREFIX}/{NODES_BASE}/{ROOMS_BASE}/{room_id}/{ROOMS_USERS}");
        let api_url = self.build_api_url(&url_part);

        let response = self
            .client
            .http
            .delete(api_url)
            .header(header::AUTHORIZATION, self.get_auth_header().await?)
            .header(header::CONTENT_TYPE, "application/json")
            .json(&room_users_del_req)
            .send()
            .await?;


        if response.status().is_client_error() || response.status().is_server_error() {
            return Err(DracoonClientError::from_response(response).await?);
        }

        Ok(())
    }
}

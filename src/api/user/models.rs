use async_trait::async_trait;
use dco3_crypto::UserKeyPairContainer;
use reqwest::Response;
use serde::{Deserialize, Serialize};

use crate::api::{utils::{FromResponse, parse_body}, auth::{errors::DracoonClientError, models::DracoonErrorResponse}};

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
#[allow(non_snake_case)]
pub struct UserAccount {
    id: u64,
    first_name: String,
    last_name: String,
    user_name: String,
    is_locked: bool,
    has_manageable_rooms: bool,
    user_roles: RoleList,
    language: String,
    auth_data: UserAuthData,
    must_set_email: Option<bool>,
    needs_to_accept_EULA: Option<bool>,
    expire_at: Option<String>,
    is_encryption_enabled: Option<bool>,
    last_login_success_at: Option<String>,
    last_login_fail_at: Option<String>,
    email: Option<String>,
    phone: Option<String>,
    home_room_id: Option<u64>,
    user_groups: Vec<UserGroup>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UserAuthData {
    method: String,
    login: Option<String>,
    passowrd: Option<String>,
    must_change_password: Option<bool>,
    ad_config_id: Option<u64>,
    oidc_config_id: Option<u64>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Right {
    id: u64,
    name: String,
    description: String,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Role {
    id: u64,
    name: String,
    description: String,
    items: Option<Vec<Right>>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RoleList {
    roles: Vec<Role>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UserGroup {
    id: u64,
    is_member: bool,
    name: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
#[allow(non_snake_case)]
pub struct UpdateUserAccountRequest {
    user_name: Option<String>,
    accept_EULA: Option<bool>,
    first_name: Option<String>,
    last_name: Option<String>,
    email: Option<String>,
    phone: Option<String>,
    language: Option<String>,
}

#[async_trait]
impl FromResponse for UserAccount {
    async fn from_response(response: Response) -> Result<Self, DracoonClientError> {

        parse_body::<Self, DracoonErrorResponse>(response).await
        
    }
}

#[async_trait]
impl FromResponse for UserKeyPairContainer {
    async fn from_response(response: Response) -> Result<Self, DracoonClientError> {

        parse_body::<Self, DracoonErrorResponse>(response).await
        
    }
}
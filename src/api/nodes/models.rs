#![allow(dead_code, unused_imports)]

use crate::{
    api::{
        auth::{errors::DracoonClientError, models::DracoonErrorResponse},
        models::Range,
    },
    cmd::utils::parse_body,
};
use reqwest::{Response, StatusCode};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct NodeList {
    pub range: Range,
    pub items: Vec<Node>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Node {
    pub id: u64,
    pub reference_id: Option<u64>,
    pub r#type: String,
    pub name: String,
    pub timestamp_creation: Option<String>,
    pub timestamp_modification: Option<String>,
    pub parent_id: Option<u64>,
    pub created_at: Option<String>,
    pub created_by: Option<UserInfo>,
    pub updated_at: Option<String>,
    pub updated_by: Option<UserInfo>,
    pub expire_at: Option<String>,
    pub hash: Option<String>,
    pub file_type: Option<String>,
    pub media_type: Option<String>,
    pub size: Option<u64>,
    pub classification: Option<u64>,
    pub notes: Option<String>,
    pub permissions: Option<NodePermissions>,
    pub inherit_permissions: Option<bool>,
    pub is_encrypted: Option<bool>,
    pub encryption_info: Option<EncryptionInfo>,
    pub cnt_deleted_versions: Option<u64>,
    pub cnt_comments: Option<u64>,
    pub cnt_upload_shares: Option<u64>,
    pub cnt_download_shares: Option<u64>,
    pub recycle_bin_retention_period: Option<u64>,
    pub has_activities_log: Option<bool>,
    pub quota: Option<u64>,
    pub is_favorite: Option<bool>,
    pub branch_version: Option<u64>,
    pub media_token: Option<String>,
    pub is_browsable: Option<bool>,
    pub cnt_rooms: Option<u64>,
    pub cnt_folders: Option<u64>,
    pub cnt_files: Option<u64>,
    pub auth_parent_id: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodePermissions {
    manage: bool,
    read: bool,
    create: bool,
    change: bool,
    delete: bool,
    manage_download_share: bool,
    manage_upload_share: bool,
    read_recycle_bin: bool,
    restore_recycle_bin: bool,
    delete_recycle_bin: bool,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EncryptionInfo {
    user_key_state: String,
    room_key_state: String,
    data_space_key_state: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserInfo {
    id: u64,
    user_type: String,
    avatar_uuid: String,
    first_name: String,
    last_name: String,
    email: Option<String>,
}

impl NodeList {
    pub async fn from_response(res: Response) -> Result<Self, DracoonClientError> {
        parse_body::<Self, DracoonErrorResponse>(res).await
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct DownloadUrlResponse {
    pub download_url: String,
}


impl DownloadUrlResponse {
    pub async fn from_response(res: Response) -> Result<Self, DracoonClientError> {
        parse_body::<Self, DracoonErrorResponse>(res).await
    }
}
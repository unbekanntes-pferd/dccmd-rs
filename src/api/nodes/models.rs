#![allow(dead_code, unused_imports)]

use crate::{
    api::{
        auth::{errors::DracoonClientError, models::DracoonErrorResponse},
        models::Range, utils::parse_body,
        utils::FromResponse
    }

};
use async_trait::async_trait;
use dco3_crypto::FileKey;
use reqwest::{Response, StatusCode};
use serde::{Deserialize, Serialize};

/// A callback function that is called after each chunk is processed (upload and download)
pub type ProgressCallback = Box<dyn FnMut(u64, u64) + Send + Sync>;

/// file meta information (name, size)
pub type FileMeta = (String, u64);


/// A list of nodes in DRACOON - GET /nodes
#[derive(Debug, Serialize, Deserialize)]
pub struct NodeList {
    pub range: Range,
    pub items: Vec<Node>,
}

/// A node in DRACOON - GET /nodes/{nodeId}
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

/// DRACOOON node permissions
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

/// DRACOOON encryption info (rescue keys)
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EncryptionInfo {
    user_key_state: String,
    room_key_state: String,
    data_space_key_state: String,
}

/// DRACOON user info on nodes (created_by, updated_by)
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserInfo {
    id: u64,
    user_type: String,
    avatar_uuid: String,
    first_name: Option<String>,
    last_name: Option<String>,
    email: Option<String>,
}

#[async_trait]
impl FromResponse for NodeList {
    /// transforms a response into a NodeList
    async fn from_response(res: Response) -> Result<Self, DracoonClientError> {
        parse_body::<Self, DracoonErrorResponse>(res).await
    }
}

/// Response for download url of a node - POST /nodes/files/{nodeId}/download
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DownloadUrlResponse {
    pub download_url: String,
}

#[async_trait]
impl FromResponse for DownloadUrlResponse {
    /// transforms a response into a DownloadUrlResponse
    async fn from_response(res: Response) -> Result<Self, DracoonClientError> {
        parse_body::<Self, DracoonErrorResponse>(res).await
    }
}


/// Error response for S3 requests (XML)
#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub struct S3XmlError {
    code: Option<String>,
    request_id: Option<String>,
    host_id: Option<String>,
    message: Option<String>,
    argument_name: Option<String>,
}

/// Error response for S3 requests
#[derive(Debug, PartialEq)]
pub struct S3ErrorResponse {
    pub status: StatusCode,
    pub error: S3XmlError,
}

impl S3ErrorResponse {
    /// transforms a S3XmlError into a S3ErrorResponse
    pub fn from_xml_error(status: StatusCode, error: S3XmlError) -> Self {
        Self { status, error }
    }
}

#[async_trait]
impl FromResponse for FileKey { 
  /// transforms a response into a FileKey
  async fn from_response(res: Response) -> Result<Self, DracoonClientError> {
    parse_body::<Self, DracoonErrorResponse>(res).await
  }
}


#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateFileUploadResponse {
    pub upload_url: Option<String>,
    pub upload_id: String,
    pub token: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PresignedUrl {
    pub url: String,
    pub part_number: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PresignedUrlList {
    pub urls: Vec<PresignedUrl>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct S3FileUploadStatus {
    pub status: String,
    pub node: Option<Node>,
    pub error_details: Option<DracoonErrorResponse>
}
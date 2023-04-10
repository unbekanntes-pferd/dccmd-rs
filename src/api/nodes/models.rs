#![allow(dead_code, unused_imports)]

use std::fmt::Debug;
use std::fmt::Formatter;

use crate::api::{
    auth::{errors::DracoonClientError, models::DracoonErrorResponse},
    models::{ObjectExpiration, Range},
    utils::parse_body,
    utils::FromResponse,
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use dco3_crypto::FileKey;
use reqwest::{Response, StatusCode};
use serde::{Deserialize, Serialize};

/// A callback function that is called after each chunk is processed (upload and download)
pub type ProgressCallback = Box<dyn FnMut(u64, u64) + Send + Sync>;

/// file meta information (name, size, timestamp creation, timestamp modification)
#[derive(Debug, Clone)]
pub struct FileMeta(pub String, pub u64, pub Option<DateTime<Utc>>, pub Option<DateTime<Utc>>);

/// upload options (expiration, classification)
#[derive(Debug, Clone)]
pub struct UploadOptions(pub Option<ObjectExpiration>, pub Option<u64>);

impl Default for UploadOptions {
    fn default() -> Self {
        Self(None, None)
    }
}

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

#[async_trait]
impl FromResponse for CreateFileUploadResponse {
    /// transforms a response into a CreateFileUploadResponse
    async fn from_response(res: Response) -> Result<Self, DracoonClientError> {
        parse_body::<Self, DracoonErrorResponse>(res).await
    }
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

#[async_trait]
impl FromResponse for PresignedUrlList {
    /// transforms a response into a PresignedUrlList
    async fn from_response(res: Response) -> Result<Self, DracoonClientError> {
        parse_body::<Self, DracoonErrorResponse>(res).await
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct S3FileUploadStatus {
    pub status: S3UploadStatus,
    pub node: Option<Node>,
    pub error_details: Option<DracoonErrorResponse>,
}

#[derive(Debug, Deserialize)]
pub enum S3UploadStatus {
    #[serde(rename = "transfer")]
    Transfer,
    #[serde(rename = "finishing")]
    Finishing,
    #[serde(rename = "done")]
    Done,
    #[serde(rename = "error")]
    Error,
}

#[async_trait]
impl FromResponse for S3FileUploadStatus {
    /// transforms a response into a S3FileUploadStatus
    async fn from_response(res: Response) -> Result<Self, DracoonClientError> {
        parse_body::<Self, DracoonErrorResponse>(res).await
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
#[allow(non_snake_case)]
pub struct CreateFileUploadRequest {
    parent_id: u64,
    name: String,
    size: Option<u64>,
    classification: Option<u64>,
    expiration: Option<ObjectExpiration>,
    direct_S3_upload: Option<bool>,
    timestamp_creation: Option<String>,
    timestamp_modification: Option<String>,
}

impl CreateFileUploadRequest {
    pub fn new(parent_id: u64, name: String) -> CreateFileUploadRequestBuilder {
        CreateFileUploadRequestBuilder {
            parent_id,
            name,
            size: None,
            classification: None,
            expiration: None,
            direct_s3_upload: Some(true),
            timestamp_creation: None,
            timestamp_modification: None,
        }
    }
}

pub struct CreateFileUploadRequestBuilder {
    parent_id: u64,
    name: String,
    size: Option<u64>,
    classification: Option<u64>,
    expiration: Option<ObjectExpiration>,
    direct_s3_upload: Option<bool>,
    timestamp_creation: Option<String>,
    timestamp_modification: Option<String>,
}

impl CreateFileUploadRequestBuilder {
    pub fn with_size(mut self, size: u64) -> Self {
        self.size = Some(size);
        self
    }

    pub fn with_classification(mut self, classification: u64) -> Self {
        self.classification = Some(classification);
        self
    }

    pub fn with_expiration(mut self, expiration: ObjectExpiration) -> Self {
        self.expiration = Some(expiration);
        self
    }
    pub fn with_timestamp_creation(mut self, timestamp_creation: DateTime<Utc>) -> Self {
        self.timestamp_creation = Some(timestamp_creation.to_rfc3339());
        self
    }
    pub fn with_timestamp_modification(mut self, timestamp_modification: DateTime<Utc>) -> Self {
        self.timestamp_modification = Some(timestamp_modification.to_rfc3339());
        self
    }
    pub fn build(self) -> CreateFileUploadRequest {
        CreateFileUploadRequest {
            parent_id: self.parent_id,
            name: self.name,
            size: self.size,
            classification: self.classification,
            expiration: self.expiration,
            direct_S3_upload: self.direct_s3_upload,
            timestamp_creation: self.timestamp_creation,
            timestamp_modification: self.timestamp_modification,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneratePresignedUrlsRequest {
    size: u64,
    first_part_number: u32,
    last_part_number: u32,
}

impl GeneratePresignedUrlsRequest {
    pub fn new(size: u64, first_part_number: u32, last_part_number: u32) -> Self {
        Self {
            size,
            first_part_number,
            last_part_number,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompleteS3FileUploadRequest {
    parts: Vec<S3FileUploadPart>,
    resolution_strategy: Option<ResolutionStrategy>,
    file_name: Option<String>,
    keep_share_links: Option<bool>,
    file_key: Option<FileKey>,
}

pub struct CompleteS3FileUploadRequestBuilder {
    parts: Vec<S3FileUploadPart>,
    resolution_strategy: Option<ResolutionStrategy>,
    file_name: Option<String>,
    keep_share_links: Option<bool>,
    file_key: Option<FileKey>,
}

impl CompleteS3FileUploadRequest {
    pub fn new(parts: Vec<S3FileUploadPart>) -> CompleteS3FileUploadRequestBuilder {
        CompleteS3FileUploadRequestBuilder {
            parts,
            resolution_strategy: None,
            file_name: None,
            keep_share_links: None,
            file_key: None,
        }
    }
}

impl CompleteS3FileUploadRequestBuilder {
    pub fn with_resolution_strategy(mut self, resolution_strategy: ResolutionStrategy) -> Self {
        self.resolution_strategy = Some(resolution_strategy);
        self
    }

    pub fn with_file_name(mut self, file_name: String) -> Self {
        self.file_name = Some(file_name);
        self
    }

    pub fn with_keep_share_links(mut self, keep_share_links: bool) -> Self {
        self.keep_share_links = Some(keep_share_links);
        self
    }

    pub fn with_file_key(mut self, file_key: FileKey) -> Self {
        self.file_key = Some(file_key);
        self
    }

    pub fn build(self) -> CompleteS3FileUploadRequest {
        CompleteS3FileUploadRequest {
            parts: self.parts,
            resolution_strategy: self.resolution_strategy,
            file_name: self.file_name,
            keep_share_links: self.keep_share_links,
            file_key: self.file_key,
        }
    }
}

#[derive(Debug, Serialize)]
pub enum ResolutionStrategy {
    #[serde(rename = "autorename")]
    AutoRename,
    #[serde(rename = "overwrite")]
    Overwrite,
    #[serde(rename = "fail")]
    Fail,
}

#[derive(Debug, Serialize)]
pub struct S3FileUploadPart {
    part_number: u32,
    etag: String,
}

impl S3FileUploadPart {
    pub fn new(part_number: u32, etag: String) -> Self {
        Self { part_number, etag }
    }
}

#![allow(dead_code, unused_imports)]

use std::fmt::Debug;
use std::fmt::Display;
use std::fmt::Formatter;
use std::sync::Arc;
use std::sync::Mutex;

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

/// A callback function that is called after each chunk is processed (download)
pub type DownloadProgressCallback = Box<dyn FnMut(u64, u64) + Send + Sync>;

/// A callback function that is called after each chunk is processed (upload)
pub type UploadProgressCallback = Box<dyn FnMut(u64, u64) + Send + Sync>;

/// A callback function (thread-safe) that can be cloned and called from multiple threads (upload)
pub struct CloneableUploadProgressCallback(Arc<Mutex<UploadProgressCallback>>);

impl Clone for CloneableUploadProgressCallback {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl CloneableUploadProgressCallback {
    pub fn new<F>(callback: F) -> Self
    where
        F: 'static + FnMut(u64, u64) + Send + Sync,
    {
        Self(Arc::new(Mutex::new(Box::new(callback))))
    }

    pub fn call(&self, bytes_read: u64, total_size: u64) {
        (self.0.lock().unwrap())(bytes_read, total_size);
    }
}


/// file meta information (name, size, timestamp creation, timestamp modification)
#[derive(Debug, Clone)]
pub struct FileMeta(
    pub String,
    pub u64,
    pub Option<DateTime<Utc>>,
    pub Option<DateTime<Utc>>,
);

pub struct FileMetaBuilder {
    name: Option<String>,
    size: Option<u64>,
    timestamp_creation: Option<DateTime<Utc>>,
    timestamp_modification: Option<DateTime<Utc>>,
}

impl FileMeta {
    pub fn builder() -> FileMetaBuilder {
        FileMetaBuilder::new()
    }
}

impl FileMetaBuilder {
    pub fn new() -> Self {
        Self {
            name: None,
            size: None,
            timestamp_creation: None,
            timestamp_modification: None,
        }
    }

    pub fn with_name(mut self, name: String) -> Self {
        self.name = Some(name);
        self
    }

    pub fn with_size(mut self, size: u64) -> Self {
        self.size = Some(size);
        self
    }

    pub fn with_timestamp_creation(mut self, timestamp_creation: DateTime<Utc>) -> Self {
        self.timestamp_creation = Some(timestamp_creation);
        self
    }

    pub fn with_timestamp_modification(mut self, timestamp_modification: DateTime<Utc>) -> Self {
        self.timestamp_modification = Some(timestamp_modification);
        self
    }

    pub fn build(self) -> FileMeta {
        FileMeta(
            self.name.unwrap(),
            self.size.unwrap(),
            self.timestamp_creation,
            self.timestamp_modification,
        )
    }
}

/// upload options (expiration, classification, keep share links, resolution strategy)
#[derive(Debug, Clone, Default)]
pub struct UploadOptions(pub Option<ObjectExpiration>, pub Option<u8>, pub Option<bool>, pub Option<ResolutionStrategy>);

impl UploadOptions {
    pub fn builder() -> UploadOptionsBuilder {
        UploadOptionsBuilder::new()
    }

}

pub struct UploadOptionsBuilder {
    expiration: Option<ObjectExpiration>,
    classification: Option<u8>,
    keep_share_links: Option<bool>,
    resolution_strategy: Option<ResolutionStrategy>,
}

impl UploadOptionsBuilder {
    pub fn new() -> Self {
        Self {
            expiration: None,
            classification: None,
            keep_share_links: None,
            resolution_strategy: None,
        }
    }

    pub fn with_expiration(mut self, expiration: ObjectExpiration) -> Self {
        self.expiration = Some(expiration);
        self
    }

    pub fn with_classification(mut self, classification: u8) -> Self {
        self.classification = Some(classification);
        self
    }

    pub fn with_keep_share_links(mut self, keep_share_links: bool) -> Self {
        self.keep_share_links = Some(keep_share_links);
        self
    }

    pub fn with_resolution_strategy(mut self, resolution_strategy: ResolutionStrategy) -> Self {
        self.resolution_strategy = Some(resolution_strategy);
        self
    }

    pub fn build(self) -> UploadOptions {
        UploadOptions(self.expiration, self.classification, self.keep_share_links, self.resolution_strategy)
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
    #[serde(rename = "type")]
    pub node_type: NodeType,
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

#[async_trait]
impl FromResponse for Node {

    async fn from_response(response: Response) -> Result<Self, DracoonClientError> {
        parse_body::<Self, DracoonErrorResponse>(response).await
}
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub enum NodeType {
    #[serde(rename = "room")]
    Room,
    #[serde(rename = "folder")]
    Folder,
    #[serde(rename = "file")]
    File,
}

/// DRACOOON node permissions
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(clippy::struct_excessive_bools)]
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

impl ToString for NodePermissions {
    fn to_string(&self) -> String {
        let mapping = [
            (self.manage, 'm'),
            (self.read, 'r'),
            (self.create, 'w'),
            (self.change, 'c'),
            (self.delete, 'd'),
            (self.manage_download_share, 'm'),
            (self.manage_upload_share, 'm'),
            (self.read_recycle_bin, 'r'),
            (self.restore_recycle_bin, 'r'),
            (self.delete_recycle_bin, 'd'),
        ];

        let mut perms = String::with_capacity(mapping.len());

        for (i, &(flag, ch)) in mapping.iter().enumerate() {
            perms.push(if flag { ch } else { '-' });

            // Add a dash after the "delete" permission
            if i == 4 {
                perms.push('-');
            }
        }

        perms
    }
}

/// DRACOOON encryption info (rescue keys)
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EncryptionInfo {
    user_key_state: String,
    room_key_state: String,
    data_space_key_state: String,
}

/// DRACOON user info on nodes (`created_by`, `updated_by`)
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserInfo {
    pub id: u64,
    pub user_type: String,
    pub avatar_uuid: String,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub email: Option<String>,
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

impl Display for S3ErrorResponse {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Error: {} ({})",
            self.error.message.as_ref().unwrap_or(&String::from("Unknown S3 error")),
            self.status,
        )
    }
}

impl S3ErrorResponse {
    /// transforms a `S3XmlError` into a `S3ErrorResponse`
    pub fn from_xml_error(status: StatusCode, error: S3XmlError) -> Self {
        Self { status, error }
    }
}

#[async_trait]
impl FromResponse for FileKey {
    /// transforms a response into a `FileKey`
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
    /// transforms a response into a `CreateFileUploadResponse`
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
    /// transforms a response into a `PresignedUrlList`
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
    /// transforms a response into a `S3FileUploadStatus`
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
    classification: Option<u8>,
    expiration: Option<ObjectExpiration>,
    direct_S3_upload: Option<bool>,
    timestamp_creation: Option<String>,
    timestamp_modification: Option<String>,
}

impl CreateFileUploadRequest {
    pub fn builder(parent_id: u64, name: String) -> CreateFileUploadRequestBuilder {
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
    classification: Option<u8>,
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

    pub fn with_classification(mut self, classification: u8) -> Self {
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
    pub fn builder(parts: Vec<S3FileUploadPart>) -> CompleteS3FileUploadRequestBuilder {
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

#[derive(Debug, Serialize, Clone)]
pub enum ResolutionStrategy {
    #[serde(rename = "autorename")]
    AutoRename,
    #[serde(rename = "overwrite")]
    Overwrite,
    #[serde(rename = "fail")]
    Fail,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct S3FileUploadPart {
    part_number: u32,
    part_etag: String,
}

impl S3FileUploadPart {
    pub fn new(part_number: u32, part_etag: String) -> Self {
        Self { part_number, part_etag }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteNodesRequest {
    node_ids: Vec<u64>
}

impl From<Vec<u64>> for DeleteNodesRequest {
    fn from(node_ids: Vec<u64>) -> Self {
        Self { node_ids }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferNodesRequest {
    items: Vec<TransferNode>,
    resolution_strategy: Option<ResolutionStrategy>,
    keep_share_links: Option<bool>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferNode {
    id: u64,
    name: Option<String>,
    timestamp_creation: Option<String>,
    timestamp_modification: Option<String>,
}

impl From<u64> for TransferNode {
    fn from(node_id: u64) -> Self {
        Self {
            id: node_id,
            name: None,
            timestamp_creation: None,
            timestamp_modification: None,
        }
    }
}

impl From<Vec<u64>> for TransferNodesRequest {
    fn from(node_ids: Vec<u64>) -> Self {
        Self {
            items: node_ids.into_iter().map(std::convert::Into::into).collect(),
            resolution_strategy: None,
            keep_share_links: None,
        }
    }
}

pub struct TransferNodesRequestBuilder {
    items: Vec<TransferNode>,
    resolution_strategy: Option<ResolutionStrategy>,
    keep_share_links: Option<bool>,
}

impl TransferNodesRequest {
    pub fn builder(items: Vec<TransferNode>) -> TransferNodesRequestBuilder {
        TransferNodesRequestBuilder {
            items,
            resolution_strategy: None,
            keep_share_links: None,
        }
    }

    pub fn new_from_ids(node_ids: Vec<u64>) -> TransferNodesRequestBuilder {
        TransferNodesRequestBuilder {
            items: node_ids.into_iter().map(std::convert::Into::into).collect(),
            resolution_strategy: None,
            keep_share_links: None,
        }
    }

    pub fn with_resolution_strategy(mut self, resolution_strategy: ResolutionStrategy) -> Self {
        self.resolution_strategy = Some(resolution_strategy);
        self
    }

    pub fn with_keep_share_links(mut self, keep_share_links: bool) -> Self {
        self.keep_share_links = Some(keep_share_links);
        self
    }

    pub fn build(self) -> TransferNodesRequest {
        TransferNodesRequest {
            items: self.items,
            resolution_strategy: self.resolution_strategy,
            keep_share_links: self.keep_share_links,
        }
    }
}

pub struct TransferNodeBuilder {
    id: u64,
    name: Option<String>,
    timestamp_creation: Option<String>,
    timestamp_modification: Option<String>,
}

impl TransferNode {
    pub fn builder(id: u64) -> TransferNodeBuilder {
        TransferNodeBuilder {
            id,
            name: None,
            timestamp_creation: None,
            timestamp_modification: None,
        }

    }

    pub fn with_name(mut self, name: String) -> Self {
        self.name = Some(name);
        self
    }

    pub fn with_timestamp_creation(mut self, timestamp_creation: String) -> Self {
        self.timestamp_creation = Some(timestamp_creation);
        self
    }

    pub fn with_timestamp_modification(mut self, timestamp_modification: String) -> Self {
        self.timestamp_modification = Some(timestamp_modification);
        self
    }

    pub fn build(self) -> TransferNode {
        TransferNode {
            id: self.id,
            name: self.name,
            timestamp_creation: self.timestamp_creation,
            timestamp_modification: self.timestamp_modification,
        }
    }
}


#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateFolderRequest {
    name: String,
    parent_id: u64,
    notes: Option<String>,
    timestamp_creation: Option<String>,
    timestamp_modification: Option<String>,
    classification: Option<u8>
}

pub struct CreateFolderRequestBuilder {
    name: String,
    parent_id: u64,
    notes: Option<String>,
    timestamp_creation: Option<String>,
    timestamp_modification: Option<String>,
    classification: Option<u8>
}

impl CreateFolderRequest {
    pub fn builder(name: String, parent_id: u64) -> CreateFolderRequestBuilder {
        CreateFolderRequestBuilder {
            name,
            parent_id,
            notes: None,
            timestamp_creation: None,
            timestamp_modification: None,
            classification: None,
        }
    }

}

impl CreateFolderRequestBuilder {
    pub fn with_notes(mut self, notes: String) -> Self {
        self.notes = Some(notes);
        self
    }

    pub fn with_timestamp_creation(mut self, timestamp_creation: String) -> Self {
        self.timestamp_creation = Some(timestamp_creation);
        self
    }

    pub fn with_timestamp_modification(mut self, timestamp_modification: String) -> Self {
        self.timestamp_modification = Some(timestamp_modification);
        self
    }

    pub fn with_classification(mut self, classification: u8) -> Self {
        self.classification = Some(classification);
        self
    }

    pub fn build(self) -> CreateFolderRequest {
        CreateFolderRequest {
            name: self.name,
            parent_id: self.parent_id,
            notes: self.notes,
            timestamp_creation: self.timestamp_creation,
            timestamp_modification: self.timestamp_modification,
            classification: self.classification,
        }
}
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateFolderRequest {
    name: Option<String>,
    notes: Option<String>,
    timestamp_creation: Option<String>,
    timestamp_modification: Option<String>,
    classification: Option<u8>
}

pub struct UpdateFolderRequestBuilder {
    name: Option<String>,
    notes: Option<String>,
    timestamp_creation: Option<String>,
    timestamp_modification: Option<String>,
    classification: Option<u8>
}

impl UpdateFolderRequest {
    pub fn builder() -> UpdateFolderRequestBuilder {
        UpdateFolderRequestBuilder {
            name: None,
            notes: None,
            timestamp_creation: None,
            timestamp_modification: None,
            classification: None,
        }
    }
}

impl UpdateFolderRequestBuilder {
    pub fn with_name(mut self, name: String) -> Self {
        self.name = Some(name);
        self
    }

    pub fn with_notes(mut self, notes: String) -> Self {
        self.notes = Some(notes);
        self
    }

    pub fn with_timestamp_creation(mut self, timestamp_creation: String) -> Self {
        self.timestamp_creation = Some(timestamp_creation);
        self
    }

    pub fn with_timestamp_modification(mut self, timestamp_modification: String) -> Self {
        self.timestamp_modification = Some(timestamp_modification);
        self
    }

    pub fn with_classification(mut self, classification: u8) -> Self {
        self.classification = Some(classification);
        self
    }

    pub fn build(self) -> UpdateFolderRequest {
        UpdateFolderRequest {
            name: self.name,
            notes: self.notes,
            timestamp_creation: self.timestamp_creation,
            timestamp_modification: self.timestamp_modification,
            classification: self.classification,
        }
    }
}


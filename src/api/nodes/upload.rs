use super::{
    models::{
        CompleteS3FileUploadRequest, CreateFileUploadRequest, CreateFileUploadResponse, FileMeta,
        GeneratePresignedUrlsRequest, Node, PresignedUrlList, ProgressCallback, S3FileUploadStatus,
        UploadOptions,
    },
    Upload,
};
use crate::api::{
    auth::{errors::DracoonClientError, Connected},
    constants::{
        CHUNK_SIZE, DRACOON_API_PREFIX, FILES_BASE, FILES_S3_COMPLETE, FILES_S3_URLS, FILES_UPLOAD,
        NODES_BASE,
    },
    models::ObjectExpiration,
    utils::FromResponse,
    Dracoon,
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use tokio::io::AsyncRead;
use std::{io::Read};

#[async_trait]
impl<C: UploadInternal + Sync> Upload for C {
    async fn upload<'r>(
        &'r self,
        file_meta: FileMeta,
        parent_node: &Node,
        upload_options: UploadOptions,
        reader: &'r mut (dyn AsyncRead + Send),
        callback: Option<ProgressCallback>,
    ) -> Result<Node, DracoonClientError> {
        match parent_node.is_encrypted {
            Some(encrypted) => {
                if encrypted {
                    self.upload_to_s3_encrypted(
                        file_meta,
                        parent_node,
                        upload_options,
                        reader,
                        callback,
                    )
                    .await
                } else {
                    self.upload_to_s3_unencrypted(
                        file_meta,
                        parent_node,
                        upload_options,
                        reader,
                        callback,
                    )
                    .await
                }
            }
            None => {
                self.upload_to_s3_unencrypted(
                    file_meta,
                    parent_node,
                    upload_options,
                    reader,
                    callback,
                )
                .await
            }
        }
    }
}

#[async_trait]
trait UploadInternal {
    async fn create_upload_channel(
        &self,
        req: CreateFileUploadRequest,
    ) -> Result<CreateFileUploadResponse, DracoonClientError>;

    async fn create_s3_upload_urls(
        &self,
        upload_id: String,
        req: GeneratePresignedUrlsRequest,
    ) -> Result<PresignedUrlList, DracoonClientError>;

    async fn upload_to_s3_unencrypted(
        &self,
        file_meta: FileMeta,
        parent_node: &Node,
        upload_options: UploadOptions,
        reader: &mut (dyn AsyncRead + Send),
        mut callback: Option<ProgressCallback>,
    ) -> Result<Node, DracoonClientError>;
    async fn upload_to_s3_encrypted(
        &self,
        file_meta: FileMeta,
        parent_node: &Node,
        upload_options: UploadOptions,
        reader: &mut (dyn AsyncRead + Send),
        mut callback: Option<ProgressCallback>,
    ) -> Result<Node, DracoonClientError>;

    async fn finalize_upload(
        &self,
        upload_id: String,
        req: CompleteS3FileUploadRequest,
    ) -> Result<(), DracoonClientError>;

    async fn get_upload_status(
        &self,
        upload_id: String,
    ) -> Result<S3FileUploadStatus, DracoonClientError>;
}

#[async_trait]
impl UploadInternal for Dracoon<Connected> {
    async fn create_upload_channel(
        &self,
        req: CreateFileUploadRequest,
    ) -> Result<CreateFileUploadResponse, DracoonClientError> {
        let url_part = format!(
            "{}/{}/{}/{}",
            DRACOON_API_PREFIX, NODES_BASE, FILES_BASE, FILES_UPLOAD
        );

        let api_url = self.build_api_url(&url_part);
        let res = self.client.http.post(api_url).json(&req).send().await?;

        CreateFileUploadResponse::from_response(res).await
    }

    async fn create_s3_upload_urls(
        &self,
        upload_id: String,
        req: GeneratePresignedUrlsRequest,
    ) -> Result<PresignedUrlList, DracoonClientError> {
        let url_part = format!(
            "{}/{}/{}/{}/{}/{}",
            DRACOON_API_PREFIX, NODES_BASE, FILES_BASE, FILES_UPLOAD, upload_id, FILES_S3_URLS
        );
        let api_url = self.build_api_url(&url_part);
        let res = self.client.http.post(api_url).json(&req).send().await?;

        PresignedUrlList::from_response(res).await
    }

    async fn finalize_upload(
        &self,
        upload_id: String,
        req: CompleteS3FileUploadRequest,
    ) -> Result<(), DracoonClientError> {
        let url_part = format!(
            "{}/{}/{}/{}/{}/{}",
            DRACOON_API_PREFIX, NODES_BASE, FILES_BASE, FILES_UPLOAD, upload_id, FILES_S3_COMPLETE
        );
        let api_url = self.build_api_url(&url_part);
        let res = self.client.http.put(api_url).json(&req).send().await?;
        todo!()
    }

    /// requests S3 upload status from DRACOON
    async fn get_upload_status(
        &self,
        upload_id: String,
    ) -> Result<S3FileUploadStatus, DracoonClientError> {
        let url_part = format!(
            "{}/{}/{}/{}/{}",
            DRACOON_API_PREFIX, NODES_BASE, FILES_BASE, FILES_UPLOAD, upload_id
        );
        let api_url = self.build_api_url(&url_part);
        let res = self.client.http.get(api_url).send().await?;

        S3FileUploadStatus::from_response(res).await
    }

    async fn upload_to_s3_unencrypted(
        &self,
        file_meta: FileMeta,
        parent_node: &Node,
        upload_options: UploadOptions,
        reader: &mut (dyn AsyncRead + Send),
        mut callback: Option<ProgressCallback>,
    ) -> Result<Node, DracoonClientError> {
        // parse upload options
        let (classification, timestamp_creation, timestamp_modification, expiration) =
            parse_upload_options(&file_meta, &upload_options);

        // create upload channel
        let req = CreateFileUploadRequest::new(parent_node.id, file_meta.0)
            .with_classification(classification)
            .with_size(file_meta.1)
            .with_timestamp_modification(timestamp_modification)
            .with_timestamp_creation(timestamp_creation)
            .with_expiration(expiration)
            .build();

        let upload_channel = self.create_upload_channel(req).await?;
        let upload_id = upload_channel.upload_id;
        let mut buffer = [0u8; CHUNK_SIZE];

        // Initialize a variable to keep track of the number of bytes read
        let mut bytes_read = 0u64;

        loop {
            // Read the next chunk of data from the reader into the buffer
            let num_bytes = match reader.read(&mut buffer) {
                Ok(n) if n > 0 => n,
                Ok(_) => break,                                    // end of file
                Err(e) => return Err(DracoonClientError::IoError), // handle error
            };

            // Create a slice of the buffer that contains only the data that was read
            let data = &buffer[..num_bytes];
            let chunk_len = data.len() as u64;
            let req = GeneratePresignedUrlsRequest::new(chunk_len, 1, 1);

            let s3_url = self.create_s3_upload_urls(upload_id.clone(), req).await?;

            // Update the number of bytes read so far
            bytes_read += num_bytes as u64;

            let s3_url = &s3_url
                .urls
                .iter()
                .next()
                .expect("S3 url creation failed")
                .url;

            // upload to s3
            let res = self
                .client
                .http
                .put(s3_url)
                .body(data.to_vec())
                .send()
                .await?;

            // If a progress callback function was provided, call it with the current progress
            if let Some(ref mut cb) = callback {
                cb(bytes_read as u64, file_meta.1);
            }
        }

        todo!()
    }

    async fn upload_to_s3_encrypted(
        &self,
        file_meta: FileMeta,
        parent_node: &Node,
        upload_options: UploadOptions,
        reader: &mut (dyn AsyncRead + Send),
        mut callback: Option<ProgressCallback>,
    ) -> Result<Node, DracoonClientError> {
        todo!()
    }
}

/// helper to parse upload options (file meta and upload options)
fn parse_upload_options(
    file_meta: &FileMeta,
    upload_options: &UploadOptions,
) -> (u64, DateTime<Utc>, DateTime<Utc>, ObjectExpiration) {
    let classification = upload_options.1.unwrap_or(2);
    let timestamp_modification = file_meta.3.unwrap_or(Utc::now());
    let timestamp_creation = file_meta.2.unwrap_or(Utc::now());
    let expiration = upload_options.clone().0.unwrap_or_default();

    (
        classification,
        timestamp_creation,
        timestamp_modification,
        expiration,
    )
}

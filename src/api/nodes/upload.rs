use std::{cmp::min, pin::Pin};

use super::{
    models::{
        CompleteS3FileUploadRequest, CreateFileUploadRequest, CreateFileUploadResponse, FileMeta,
        GeneratePresignedUrlsRequest, Node, PresignedUrlList, ProgressCallback, S3FileUploadStatus,
        UploadOptions, PresignedUrl,
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
use futures_util::StreamExt;
use reqwest::Body;
use tokio::io::AsyncRead;
use tokio_util::io::ReaderStream;

#[async_trait]
impl<C: UploadInternal<R> + Sync, R: AsyncRead + Sync + Send> Upload<R> for C {
    async fn upload<'r>(
        &'r self,
        file_meta: FileMeta,
        parent_node: &Node,
        upload_options: UploadOptions,
        stream: &'r mut ReaderStream<R>,
        callback: Option<ProgressCallback>,
    ) -> Result<Node, DracoonClientError> {
        match parent_node.is_encrypted {
            Some(encrypted) => {
                if encrypted {
                    self.upload_to_s3_encrypted(
                        file_meta,
                        parent_node,
                        upload_options,
                        stream,
                        callback,
                    )
                    .await
                } else {
                    self.upload_to_s3_unencrypted(
                        file_meta,
                        parent_node,
                        upload_options,
                        stream,
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
                    stream,
                    callback,
                )
                .await
            }
        }
    }
}

#[async_trait]
trait UploadInternal<R: AsyncRead> {
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
        reader: &mut ReaderStream<R>,
        mut callback: Option<ProgressCallback>,
    ) -> Result<Node, DracoonClientError>;
    async fn upload_to_s3_encrypted(
        &self,
        file_meta: FileMeta,
        parent_node: &Node,
        upload_options: UploadOptions,
        reader: &mut ReaderStream<R>,
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
impl<R: AsyncRead + Sync + Send + Unpin> UploadInternal<R> for Dracoon<Connected> {
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
        stream: &mut ReaderStream<R>,
        mut callback: Option<ProgressCallback>,
    ) -> Result<Node, DracoonClientError> {
        // parse upload options
        let (classification, timestamp_creation, timestamp_modification, expiration) =
            parse_upload_options(&file_meta, &upload_options);
        
        let fm = file_meta.clone();

        // create upload channel
        let req = CreateFileUploadRequest::new(parent_node.id, fm.0)
            .with_classification(classification)
            .with_size(fm.1)
            .with_timestamp_modification(timestamp_modification)
            .with_timestamp_creation(timestamp_creation)
            .with_expiration(expiration)

            .build();

        let upload_channel =
            <Dracoon<Connected> as UploadInternal<R>>::create_upload_channel::<'_, '_>(self, req)
                .await?;
            


        // Initialize a variable to keep track of the number of bytes read
        let mut bytes_read = 0u64;
        let fm = &file_meta.clone();
        // Create an async stream from the reader
        let async_stream = async_stream::stream! {

            while let Some(chunk) = stream.next().await {
                if let Ok(chunk) = &chunk {
                    let processed = min(bytes_read + (chunk.len() as u64), fm.1);
                    bytes_read = processed;

                    if let Some(cb) = &mut callback {
                        cb(bytes_read, fm.1);
                    }
                }
                yield chunk
            }

        let (count_urls, last_chunk_size) = calculate_s3_url_count(fm.1, CHUNK_SIZE as u64);

        match count_urls {
            1 => {
                // only one request for small files
                let req = GeneratePresignedUrlsRequest::new(fm.1, 1, 1);
                let url = <Dracoon<Connected> as UploadInternal<R>>::create_s3_upload_urls::<'_, '_>(self, upload_channel.upload_id, req).await;

            },
            _ => {
                // first request for all urls except the last one
                let req = GeneratePresignedUrlsRequest::new(CHUNK_SIZE as u64, 1, count_urls);
            }
        }

        


        };

        todo!()
    }

    async fn upload_to_s3_encrypted(
        &self,
        file_meta: FileMeta,
        parent_node: &Node,
        upload_options: UploadOptions,
        reader: &mut ReaderStream<R>,
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

/// helper to calculate the number of S3 urls and the size of the last chunk
fn calculate_s3_url_count(total_size: u64, chunk_size: u64) -> (u32, u64) {
    let mut urls = total_size / chunk_size;
    if total_size % chunk_size != 0 {
        urls += 1;
    }

    // return url count and last chunk size
    (urls.try_into().expect("overflow size to chunk"), total_size % chunk_size)
}

async fn upload_stream_to_s3<'a, R>(
    stream: ReaderStream<R>,
    url: &PresignedUrl,
    file_meta: FileMeta,
    mut callback: Option<ProgressCallback>,
)  -> Result<(), DracoonClientError> where R: AsyncRead + Unpin + Send  + Sync + 'static {

    let mut boxed_stream = Box::pin(stream);

    // Initialize a variable to keep track of the number of bytes read
    let mut bytes_read = 0u64;
    // Create an async stream from the reader
    let async_stream = async_stream::stream! {

        while let Some(chunk) = boxed_stream.next().await {
            if let Ok(chunk) = &chunk {
                let processed = min(bytes_read + (chunk.len() as u64), file_meta.1);
                bytes_read = processed;

                if let Some(cb) = &mut callback {
                    cb(bytes_read, file_meta.1);
                }
            }
            yield chunk
        }
    };

    let body = Body::wrap_stream(async_stream);

    let res = reqwest::Client::new()
        .put(&url.url)
        .body(body)
        .send()
        .await?;

    Ok(())

}

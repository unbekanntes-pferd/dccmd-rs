use std::{cmp::min, time::Duration};

use super::{
    models::{
        CompleteS3FileUploadRequest, CreateFileUploadRequest, CreateFileUploadResponse, FileMeta,
        GeneratePresignedUrlsRequest, Node, PresignedUrl, PresignedUrlList, ProgressCallback,
        ResolutionStrategy, S3FileUploadStatus, S3UploadStatus, UploadOptions,
    },
    Upload,
};
use crate::api::{
    auth::{errors::DracoonClientError, Connected},
    constants::{
        CHUNK_SIZE, DRACOON_API_PREFIX, FILES_BASE, FILES_S3_COMPLETE, FILES_S3_URLS, FILES_UPLOAD,
        NODES_BASE, POLLING_START_DELAY,
    },
    models::ObjectExpiration,
    nodes::models::S3FileUploadPart,
    utils::{build_s3_error, FromResponse},
    Dracoon,
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use futures_util::StreamExt;
use reqwest::{header, Body};
use tokio::io::AsyncRead;
use tokio_util::io::ReaderStream;

#[async_trait]
impl<C: UploadInternal<R> + Sync, R: AsyncRead + Sync + Send + 'static> Upload<R> for C {
    async fn upload<'r>(
        &'r self,
        file_meta: FileMeta,
        parent_node: &Node,
        upload_options: UploadOptions,
        stream: ReaderStream<R>,
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
        create_file_upload_req: CreateFileUploadRequest,
    ) -> Result<CreateFileUploadResponse, DracoonClientError>;

    async fn create_s3_upload_urls(
        &self,
        upload_id: String,
        generate_urls_req: GeneratePresignedUrlsRequest,
    ) -> Result<PresignedUrlList, DracoonClientError>;

    async fn upload_to_s3_unencrypted(
        &self,
        file_meta: FileMeta,
        parent_node: &Node,
        upload_options: UploadOptions,
        mut stream: ReaderStream<R>,
        mut callback: Option<ProgressCallback>,
    ) -> Result<Node, DracoonClientError>;
    async fn upload_to_s3_encrypted(
        &self,
        file_meta: FileMeta,
        parent_node: &Node,
        upload_options: UploadOptions,
        mut stream: ReaderStream<R>,
        mut callback: Option<ProgressCallback>,
    ) -> Result<Node, DracoonClientError>;

    async fn finalize_upload(
        &self,
        upload_id: String,
        complete_file_upload_req: CompleteS3FileUploadRequest,
    ) -> Result<(), DracoonClientError>;

    async fn get_upload_status(
        &self,
        upload_id: String,
    ) -> Result<S3FileUploadStatus, DracoonClientError>;
}

#[async_trait]
impl<R: AsyncRead + Sync + Send + Unpin + 'static> UploadInternal<R> for Dracoon<Connected> {
    async fn create_upload_channel(
        &self,
        create_file_upload_req: CreateFileUploadRequest,
    ) -> Result<CreateFileUploadResponse, DracoonClientError> {
        let url_part = format!("{DRACOON_API_PREFIX}/{NODES_BASE}/{FILES_BASE}/{FILES_UPLOAD}");

        let api_url = self.build_api_url(&url_part);
        let res = self
            .client
            .http
            .post(api_url)
            .header(header::AUTHORIZATION, self.get_auth_header().await?)
            .header(header::CONTENT_TYPE, "application/json")
            .json(&create_file_upload_req)
            .send()
            .await?;

        CreateFileUploadResponse::from_response(res).await
    }

    async fn create_s3_upload_urls(
        &self,
        upload_id: String,
        generate_urls_req: GeneratePresignedUrlsRequest,
    ) -> Result<PresignedUrlList, DracoonClientError> {
        let url_part = format!(
            "{DRACOON_API_PREFIX}/{NODES_BASE}/{FILES_BASE}/{FILES_UPLOAD}/{upload_id}/{FILES_S3_URLS}"
        );
        let api_url = self.build_api_url(&url_part);
        let res = self
            .client
            .http
            .post(api_url)
            .header(header::AUTHORIZATION, self.get_auth_header().await?)
            .header(header::CONTENT_TYPE, "application/json")
            .json(&generate_urls_req)
            .send()
            .await?;

        PresignedUrlList::from_response(res).await
    }

    async fn finalize_upload(
        &self,
        upload_id: String,
        complete_file_upload_req: CompleteS3FileUploadRequest,
    ) -> Result<(), DracoonClientError> {
        let url_part = format!(
            "{DRACOON_API_PREFIX}/{NODES_BASE}/{FILES_BASE}/{FILES_UPLOAD}/{upload_id}/{FILES_S3_COMPLETE}"
        );
        let api_url = self.build_api_url(&url_part);
        let res = self
            .client
            .http
            .put(api_url)
            .header(header::AUTHORIZATION, self.get_auth_header().await?)
            .header(header::CONTENT_TYPE, "application/json")
            .json(&complete_file_upload_req)
            .send()
            .await?;

        if res.status().is_success() {
            Ok(())
        } else {
            Err(DracoonClientError::from_response(res).await?)
        }
    }

    /// requests S3 upload status from DRACOON
    async fn get_upload_status(
        &self,
        upload_id: String,
    ) -> Result<S3FileUploadStatus, DracoonClientError> {
        let url_part =
            format!("{DRACOON_API_PREFIX}/{NODES_BASE}/{FILES_BASE}/{FILES_UPLOAD}/{upload_id}");
        let api_url = self.build_api_url(&url_part);
        let res = self
            .client
            .http
            .get(api_url)
            .header(header::AUTHORIZATION, self.get_auth_header().await?)
            .header(header::CONTENT_TYPE, "application/json")
            .send()
            .await?;

        S3FileUploadStatus::from_response(res).await
    }

    #[allow(clippy::single_match_else)]
    async fn upload_to_s3_unencrypted(
        &self,
        file_meta: FileMeta,
        parent_node: &Node,
        upload_options: UploadOptions,
        stream: ReaderStream<R>,
        callback: Option<ProgressCallback>,
    ) -> Result<Node, DracoonClientError> {
        // parse upload options
        let (classification, timestamp_creation, timestamp_modification, expiration) =
            parse_upload_options(&file_meta, &upload_options);

        let fm = file_meta.clone();

        // create upload channel
        let file_upload_req = CreateFileUploadRequest::builder(parent_node.id, fm.0)
            .with_classification(classification)
            .with_size(fm.1)
            .with_timestamp_modification(timestamp_modification)
            .with_timestamp_creation(timestamp_creation)
            .with_expiration(expiration)
            .build();

        let upload_channel = <Dracoon<Connected> as UploadInternal<R>>::create_upload_channel::<
            '_,
            '_,
        >(self, file_upload_req)
        .await?;

        // Initialize a variable to keep track of the number of bytes read
        let mut bytes_read = 0u64;
        let fm = &file_meta.clone();

        let (count_urls, last_chunk_size) = calculate_s3_url_count(fm.1, CHUNK_SIZE as u64);

        println!("file meta: {:?}", fm);

        // only one request for small files
        let url_req = GeneratePresignedUrlsRequest::new(fm.1, 1, 1);
        let url = <Dracoon<Connected> as UploadInternal<R>>::create_s3_upload_urls::<'_, '_>(
            self,
            upload_channel.upload_id.clone(),
            url_req,
        )
        .await?;
        let url = url.urls.first().expect("Creating S3 url failed");
        let e_tag = upload_stream_to_s3(stream, url, file_meta, callback).await?;

        let s3_parts = vec![S3FileUploadPart::new(1, e_tag)];

        // finalize upload
        let complete_upload_req = CompleteS3FileUploadRequest::builder(s3_parts)
            .with_resolution_strategy(ResolutionStrategy::Overwrite)
            .with_keep_share_links(true)
            .build();

        <Dracoon<Connected> as UploadInternal<R>>::finalize_upload::<'_, '_>(
            self,
            upload_channel.upload_id.clone(),
            complete_upload_req,
        )
        .await?;

        // get upload status
        // return node if upload is done
        // return error if upload failed
        // polling with exponential backoff
        let mut sleep_duration = POLLING_START_DELAY;
        loop {
            let status_response = <Dracoon<Connected> as UploadInternal<R>>::get_upload_status(self, upload_channel.upload_id.clone()).await?;
            
            match status_response.status {
                S3UploadStatus::Done => {
                    return Ok(status_response
                        .node
                        .expect("Node must be set if status is done"));
                }
                S3UploadStatus::Error => {
                    return Err(DracoonClientError::Http(
                        status_response
                            .error_details
                            .expect("Error message must be set if status is error"),
                    ));
                }
                _ => {
                    tokio::time::sleep(Duration::from_millis(sleep_duration)).await;
                    sleep_duration *= 2;
                }
            }
        }
    }

    async fn upload_to_s3_encrypted(
        &self,
        file_meta: FileMeta,
        parent_node: &Node,
        upload_options: UploadOptions,
        stream: ReaderStream<R>,
        callback: Option<ProgressCallback>,
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
    (
        urls.try_into().expect("overflow size to chunk"),
        total_size % chunk_size,
    )
}

async fn upload_stream_to_s3<'a, R>(
    mut stream: ReaderStream<R>,
    url: &PresignedUrl,
    file_meta: FileMeta,
    mut callback: Option<ProgressCallback>,
) -> Result<String, DracoonClientError>
where
    R: AsyncRead + Unpin + Send + Sync + 'static,
{
    // Initialize a variable to keep track of the number of bytes read
    let mut bytes_read = 0u64;
    let file_size = file_meta.1;
    // Create an async stream from the reader
    let async_stream = async_stream::stream! {

        while let Some(chunk) = stream.next().await {
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
        .header(header::CONTENT_LENGTH, file_size)
        .send()
        .await?;

    // handle error
    if res.error_for_status_ref().is_err() {
        let error = build_s3_error(res).await;
        return Err(error);
    }

    let e_tag_header = res
        .headers()
        .get("etag")
        .expect("ETag header missing")
        .to_str()
        .expect("ETag header invalid");
    let e_tag = e_tag_header.trim_start_matches('"').trim_end_matches('"');

    Ok(e_tag.to_string())
}

use super::{
    models::{DownloadUrlResponse, Node, ProgressCallback},
    Download,
};
use crate::api::{
    auth::{errors::DracoonClientError, Connected},
    constants::{CHUNK_SIZE, DRACOON_API_PREFIX, FILES_BASE, NODES_BASE, NODES_DOWNLOAD_URL, FILES_FILE_KEY},
    utils::{build_s3_error, FromResponse},
    Dracoon,
};
use async_trait::async_trait;
use dco3_crypto::FileKey;
use futures_util::TryStreamExt;
use reqwest::header::{self, CONTENT_LENGTH, RANGE};
use std::{cmp::min, io::Write};
use tracing::debug;

#[async_trait]
impl<T: DownloadInternal + Sync> Download for T {
    async fn download<'w>(
        &'w self,
        node: &Node,
        writer: &'w mut (dyn Write + Send),
        callback: Option<ProgressCallback>,
    ) -> Result<(), DracoonClientError> {
        let download_url_response = self.get_download_url(node.id).await?;

        match node.is_encrypted {
            Some(encrypted) => {
                if encrypted {
                    self.download_encrypted(
                        &download_url_response.download_url,
                        writer,
                        node.size,
                        callback,
                    )
                    .await
                } else {
                    self.download_unencrypted(
                        &download_url_response.download_url,
                        writer,
                        node.size,
                        callback,
                    )
                    .await
                }
            }
            None => {
                self.download_unencrypted(
                    &download_url_response.download_url,
                    writer,
                    node.size,
                    callback,
                )
                .await
            }
        }
    }
}

#[async_trait]
trait DownloadInternal {
    async fn get_download_url(
        &self,
        node_id: u64,
    ) -> Result<DownloadUrlResponse, DracoonClientError>;

    async fn get_file_key(&self, node_id: u64) -> Result<FileKey, DracoonClientError>;

    async fn download_unencrypted(
        &self,
        url: &str,
        writer: &mut (dyn Write + Send),
        size: Option<u64>,
        mut callback: Option<ProgressCallback>,
    ) -> Result<(), DracoonClientError>;

    async fn download_encrypted(
        &self,
        url: &str,
        writer: &mut (dyn Write + Send),
        size: Option<u64>,
        mut callback: Option<ProgressCallback>,
    ) -> Result<(), DracoonClientError>;
}

#[async_trait]
impl DownloadInternal for Dracoon<Connected> {
    async fn get_download_url(
        &self,
        node_id: u64,
    ) -> Result<DownloadUrlResponse, DracoonClientError> {
        let url_part = format!(
            "{}/{}/{}/{}/{}",
            DRACOON_API_PREFIX, NODES_BASE, FILES_BASE, node_id, NODES_DOWNLOAD_URL
        );

        let api_url = self.build_api_url(&url_part);

        let response = self
            .client
            .http
            .post(api_url)
            .header(header::AUTHORIZATION, self.get_auth_header().await?)
            .header(header::CONTENT_TYPE, "application/json")
            .send()
            .await?;

        DownloadUrlResponse::from_response(response).await
    }

    async fn download_unencrypted(
        &self,
        url: &str,
        writer: &mut (dyn Write + Send),
        size: Option<u64>,
        mut callback: Option<ProgressCallback>,
    ) -> Result<(), DracoonClientError> {
        // get content length from header
        let content_length = self
            .client
            .http
            .head(url)
            .send()
            .await?
            .headers()
            .get(CONTENT_LENGTH)
            .and_then(|val| val.to_str().ok())
            .and_then(|val| val.parse().ok())
            .unwrap_or(0);

        // if size is given, use it
        let content_length = size.unwrap_or(content_length);

        // offset (in bytes)
        let mut downloaded_bytes = 0u64;

        debug!("Content length: {}", content_length);

        // loop until all bytes are downloaded
        while downloaded_bytes < content_length {
            // calculate range
            let start = downloaded_bytes;
            let end = min(start + CHUNK_SIZE as u64 - 1, content_length - 1);
            let range = format!("bytes={}-{}", start, end);

            // get chunk
            let response = self
                .client
                .http
                .get(url)
                .header(RANGE, range)
                .send()
                .await?;

            // handle error
            if !response.error_for_status_ref().is_ok() {
                let error = build_s3_error(response).await;
                return Err(error);
            }

            // write chunk to writer
            let mut stream = response.bytes_stream();

            while let Some(chunk) = stream.try_next().await? {
                let len = chunk.len() as u64;
                writer.write(&chunk).or(Err(DracoonClientError::IoError))?;
                downloaded_bytes += len;

                // call progress callback if provided
                if let Some(ref mut callback) = callback {
                    callback(downloaded_bytes, content_length);
                }
                if downloaded_bytes >= content_length {
                    break;
                }
            }
        }

        Ok(())
    }

    async fn download_encrypted(
        &self,
        url: &str,
        writer: &mut (dyn Write + Send),
        size: Option<u64>,
        callback: Option<ProgressCallback>,
    ) -> Result<(), DracoonClientError> {
        todo!()
    }

    async fn get_file_key(&self, node_id: u64) -> Result<FileKey, DracoonClientError> {
        
        let url_part = format!(
            "{}/{}/{}/{}/{}",
            DRACOON_API_PREFIX, NODES_BASE, FILES_BASE, node_id, FILES_FILE_KEY
        );

        let response = self
            .client
            .http
            .get(self.build_api_url(&url_part))
            .header(header::AUTHORIZATION, self.get_auth_header().await?)
            .send()
            .await?;

        FileKey::from_response(response).await
    }
}

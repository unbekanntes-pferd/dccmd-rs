use super::{
    models::{DownloadUrlResponse, Node},
    Download,
};
use crate::api::{
    auth::{errors::DracoonClientError, Connected},
    constants::{CHUNK_SIZE, DRACOON_API_PREFIX, FILES_BASE, NODES_BASE, NODES_DOWNLOAD_URL},
    nodes::models::{S3ErrorResponse, S3XmlError},
    Dracoon,
};
use async_trait::async_trait;
use reqwest::header::{self, CONTENT_LENGTH, RANGE};
use serde_xml_rs::from_str;
use std::{cmp::min, io::Write};
use tracing::debug;

#[async_trait]
impl<T: DownloadInternal + Sync> Download for T {
    async fn download<'w>(
        &'w self,
        node: &Node,
        writer: &'w mut (dyn Write + Send),
    ) -> Result<(), DracoonClientError> {
        let download_url_response = self.get_download_url(node.id).await?;

        match node.is_encrypted {
            Some(val) => {
                if val {
                    self.download_encrypted(&download_url_response.download_url, writer)
                        .await
                } else {
                    self.download_unencrypted(
                        &download_url_response.download_url,
                        node.size,
                        writer,
                    )
                    .await
                }
            }
            None => {
                self.download_unencrypted(&download_url_response.download_url, node.size, writer)
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

    async fn download_unencrypted(
        &self,
        url: &str,
        size: Option<u64>,
        writer: &mut (dyn Write + Send),
    ) -> Result<(), DracoonClientError>;

    async fn download_encrypted(
        &self,
        url: &str,
        writer: &mut (dyn Write + Send),
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
        size: Option<u64>,
        writer: &mut (dyn Write + Send),
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
                let status = &response.status();
                let text = response.text().await?;
                let error: S3XmlError = from_str(&text).expect("Valid S3 XML error");
                let err_response = S3ErrorResponse::from_xml_error(status.clone(), error);
                return Err(DracoonClientError::S3Error(err_response));
            }

            // write chunk to writer
            let chunk = response.bytes().await?;
            writer.write(&chunk).or(Err(DracoonClientError::IoError))?;

            // update offset
            let offset = min(downloaded_bytes + (chunk.len() as u64), content_length);
            downloaded_bytes = offset;
        }

        Ok(())
    }

    async fn download_encrypted(
        &self,
        url: &str,
        writer: &mut (dyn Write + Send),
    ) -> Result<(), DracoonClientError> {
        todo!()
    }
}

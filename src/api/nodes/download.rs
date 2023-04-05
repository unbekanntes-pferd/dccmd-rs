use super::{
    models::{DownloadUrlResponse, Node},
    Download,
};
use crate::api::{
    auth::{errors::DracoonClientError, Connected},
    constants::{DRACOON_API_PREFIX, FILES_BASE, NODES_BASE, NODES_DOWNLOAD_URL},
    Dracoon,
};
use async_trait::async_trait;
use futures_util::StreamExt;
use reqwest::header;
use std::{cmp::min, io::Write};

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
                    self.download_unencrypted(&download_url_response.download_url, writer)
                        .await
                }
            }
            None => {
                self.download_unencrypted(&download_url_response.download_url, writer)
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
            DRACOON_API_PREFIX,
            NODES_BASE,
            FILES_BASE,
            node_id,
            NODES_DOWNLOAD_URL
        );

        let api_url = self.build_api_url(&url_part);

        let response = self
            .client
            .http
            .get(api_url)
            .header(header::AUTHORIZATION, self.get_auth_header())
            .header(header::CONTENT_TYPE, "application/json")
            .send()
            .await?;

        DownloadUrlResponse::from_response(response).await
    }

    async fn download_unencrypted(
        &self,
        url: &str,
        writer: &mut (dyn Write + Send),
    ) -> Result<(), DracoonClientError> {
        let response = self.client.http.get(url).send().await?;

        let total_size = response.content_length().unwrap_or(0);
        let mut downloaded_bytes = 0u64;
        let mut stream = response.bytes_stream();

        while let Some(item) = stream.next().await {
            let chunk = item?;
            writer.write(&chunk).or(Err(DracoonClientError::IoError))?;
            let offset = min(downloaded_bytes + (chunk.len() as u64), total_size);
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

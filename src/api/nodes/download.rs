use super::{
    models::{DownloadUrlResponse, Node, ProgressCallback},
    Download,
};
use crate::api::{
    auth::{errors::DracoonClientError, Connected},
    constants::{
        CHUNK_SIZE, DRACOON_API_PREFIX, FILES_BASE, FILES_FILE_KEY, NODES_BASE, NODES_DOWNLOAD_URL,
    },
    utils::{build_s3_error, FromResponse},
    Dracoon,
};
use async_trait::async_trait;
use dco3_crypto::{FileKey, DracoonCrypto, DracoonRSACrypto, Decrypter, ChunkedEncryption};
use futures_util::TryStreamExt;
use reqwest::header::{self, CONTENT_LENGTH, RANGE};
use std::{cmp::min, io::Write};
use tracing::debug;

#[async_trait]
impl<T: DownloadInternal + Sync + Send> Download for T {
    async fn download<'w>(
        &'w mut self,
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
                        node.id,
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
        &mut self,
        url: &str,
        node_id: u64,
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
            "{DRACOON_API_PREFIX}/{NODES_BASE}/{FILES_BASE}/{node_id}/{NODES_DOWNLOAD_URL}"
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
            let range = format!("bytes={start}-{end}");

            // get chunk
            let response = self
                .client
                .http
                .get(url)
                .header(RANGE, range)
                .send()
                .await?;

            // handle error
            if response.error_for_status_ref().is_err() {
                let error = build_s3_error(response).await;
                return Err(error);
            }

            // write chunk to writer
            let mut stream = response.bytes_stream();

            while let Some(chunk) = stream.try_next().await? {
                let len = chunk.len() as u64;
                writer.write_all(&chunk).or(Err(DracoonClientError::IoError))?;
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
        &mut self,
        url: &str,
        node_id: u64,
        writer: &mut (dyn Write + Send),
        size: Option<u64>,
        mut callback: Option<ProgressCallback>,
    ) -> Result<(), DracoonClientError> {
        // get file key
        let file_key = self.get_file_key(node_id).await?;

        let keypair = self.get_keypair(None).await?;
 
        let plain_key = DracoonCrypto::decrypt_file_key(file_key, keypair)?;


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

        // this is safe, because the maximum size of a file (encrypted) is 60 GB
        #[allow(clippy::cast_possible_truncation)]
        let mut buffer = vec![0u8; content_length as usize];

        let mut crypter = DracoonCrypto::decrypter(plain_key, &mut buffer)?;


        // offset (in bytes)
        let mut downloaded_bytes = 0u64;

        debug!("Content length: {}", content_length);

        // loop until all bytes are downloaded
        while downloaded_bytes < content_length {
            // calculate range
            let start = downloaded_bytes;
            let end = min(start + CHUNK_SIZE as u64 - 1, content_length - 1);
            let range = format!("bytes={start}-{end}");

            // get chunk
            let response = self
                .client
                .http
                .get(url)
                .header(RANGE, range)
                .send()
                .await?;

            // handle error
            if response.error_for_status_ref().is_err() {
                let error = build_s3_error(response).await;
                return Err(error);
            }

            // write chunk to writer
            let mut stream = response.bytes_stream();

            while let Some(chunk) = stream.try_next().await? {
                let len = chunk.len() as u64;

                crypter.update(&chunk)?;
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

        crypter.finalize()?;

        writer.write_all(&buffer).or(Err(DracoonClientError::IoError))?;
        Ok(())
    }

    async fn get_file_key(&self, node_id: u64) -> Result<FileKey, DracoonClientError> {
        let url_part = format!(
            "{DRACOON_API_PREFIX}/{NODES_BASE}/{FILES_BASE}/{node_id}/{FILES_FILE_KEY}"
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

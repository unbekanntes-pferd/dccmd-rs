use std::{cmp::min, time::Duration};

use super::{
    models::{
        CloneableUploadProgressCallback, CompleteS3FileUploadRequest, CreateFileUploadRequest,
        CreateFileUploadResponse, FileMeta, GeneratePresignedUrlsRequest, MissingKeysResponse,
        Node, PresignedUrl, PresignedUrlList, ResolutionStrategy, S3FileUploadStatus,
        S3UploadStatus, UploadOptions, UploadProgressCallback, UserFileKeySetBatchRequest,
    },
    Upload,
};
use crate::api::{
    auth::{errors::DracoonClientError, Connected},
    constants::{
        CHUNK_SIZE, DRACOON_API_PREFIX, FILES_BASE, FILES_KEYS, FILES_S3_COMPLETE, FILES_S3_URLS,
        FILES_UPLOAD, MISSING_FILE_KEYS, MISSING_KEYS_BATCH, NODES_BASE, POLLING_START_DELAY,
    },
    models::ObjectExpiration,
    nodes::models::{S3FileUploadPart, UserFileKeySetRequest},
    utils::{build_s3_error, FromResponse},
    Dracoon,
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use dco3_crypto::{ChunkedEncryption, DracoonCrypto, DracoonRSACrypto, Encrypter};
use futures_util::{Stream, StreamExt};
use reqwest::{header, Body};
use tokio::io::{AsyncRead, AsyncReadExt, BufReader};

#[async_trait]
impl<C: UploadInternal<R> + Sync + Send, R: AsyncRead + Sync + Send + 'static> Upload<R> for C {
    async fn upload<'r>(
        &'r mut self,
        file_meta: FileMeta,
        parent_node: &Node,
        upload_options: UploadOptions,
        reader: BufReader<R>,
        callback: Option<UploadProgressCallback>,
        chunk_size: Option<usize>,
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
                        chunk_size,
                    )
                    .await
                } else {
                    self.upload_to_s3_unencrypted(
                        file_meta,
                        parent_node,
                        upload_options,
                        reader,
                        callback,
                        chunk_size,
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
                    chunk_size,
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
        reader: BufReader<R>,
        mut callback: Option<UploadProgressCallback>,
        chunk_size: Option<usize>,
    ) -> Result<Node, DracoonClientError>;
    async fn upload_to_s3_encrypted(
        &mut self,
        file_meta: FileMeta,
        parent_node: &Node,
        upload_options: UploadOptions,
        reader: BufReader<R>,
        mut callback: Option<UploadProgressCallback>,
        chunk_size: Option<usize>,
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
    async fn upload_stream_to_s3<'a>(
        &self,
        mut stream: impl Stream<Item = Result<bytes::Bytes, impl std::error::Error + Send + Sync + 'static>>
            + Sync
            + Send
            + Unpin
            + 'static,
        url: &PresignedUrl,
        file_meta: FileMeta,
        chunk_size: usize,
        curr_pos: Option<u64>,
        callback: Option<CloneableUploadProgressCallback>,
    ) -> Result<String, DracoonClientError>;

    async fn get_missing_file_keys(
        &self,
        file_id: u64,
    ) -> Result<MissingKeysResponse, DracoonClientError>;

    async fn set_file_keys(
        &self,
        keys_batch_req: UserFileKeySetBatchRequest,
    ) -> Result<(), DracoonClientError>;
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
    #[allow(clippy::too_many_lines)]
    async fn upload_to_s3_unencrypted(
        &self,
        file_meta: FileMeta,
        parent_node: &Node,
        upload_options: UploadOptions,
        mut reader: BufReader<R>,
        callback: Option<UploadProgressCallback>,
        chunk_size: Option<usize>,
    ) -> Result<Node, DracoonClientError> {
        // parse upload options
        let (
            classification,
            timestamp_creation,
            timestamp_modification,
            expiration,
            resolution_strategy,
            keep_share_links,
        ) = parse_upload_options(&file_meta, &upload_options);

        let fm = file_meta.clone();

        let chunk_size = chunk_size.unwrap_or(CHUNK_SIZE);

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

        let fm = &file_meta.clone();
        let mut s3_parts = Vec::new();

        let (count_urls, last_chunk_size) = calculate_s3_url_count(fm.1, chunk_size as u64);
        let mut url_part: u32 = 1;

        let cloneable_callback = callback.map(CloneableUploadProgressCallback::new);

        if count_urls > 1 {
            while url_part < count_urls {
                let mut buffer = vec![0; chunk_size];

                match reader.read_exact(&mut buffer).await {
                    Ok(0) => break,
                    Ok(n) => {
                        buffer.truncate(n);
                        let chunk = bytes::Bytes::from(buffer);

                        let stream: async_stream::__private::AsyncStream<
                            Result<bytes::Bytes, std::io::Error>,
                            _,
                        > = async_stream::stream! {
                            yield Ok(chunk);
                        };

                        let url_req = GeneratePresignedUrlsRequest::new(
                            n.try_into().expect("size not larger than 32 MB"),
                            url_part,
                            url_part,
                        );
                        let url =
                            <Dracoon<Connected> as UploadInternal<R>>::create_s3_upload_urls::<
                                '_,
                                '_,
                            >(
                                self, upload_channel.upload_id.clone(), url_req
                            )
                            .await?;
                        let url = url.urls.first().expect("Creating S3 url failed");

                        // truncation is safe because chunk_size is 32 MB
                        #[allow(clippy::cast_possible_truncation, clippy::cast_lossless)]
                        let curr_pos: u64 = ((url_part - 1) * (chunk_size as u32)) as u64;

                        let e_tag = <Dracoon<Connected> as UploadInternal<R>>::upload_stream_to_s3(
                            self,
                            Box::pin(stream),
                            url,
                            file_meta.clone(),
                            n,
                            Some(curr_pos),
                            cloneable_callback.clone(),
                        )
                        .await?;

                        s3_parts.push(S3FileUploadPart::new(url_part, e_tag));
                        url_part += 1;
                    }
                    Err(err) => return Err(DracoonClientError::IoError),
                }
            }
        }

        // upload last chunk
        let mut buffer = vec![
            0;
            last_chunk_size
                .try_into()
                .expect("size not larger than 32 MB")
        ];
        match reader.read_exact(&mut buffer).await {
            Ok(0) => unreachable!("last chunk is empty"),
            Ok(n) => {
                buffer.truncate(n);
                let chunk = bytes::Bytes::from(buffer);
                let stream: async_stream::__private::AsyncStream<
                    Result<bytes::Bytes, std::io::Error>,
                    _,
                > = async_stream::stream! {
                    // TODO: chunk stream for better progress
                    yield Ok(chunk);

                };

                let url_req = GeneratePresignedUrlsRequest::new(
                    n.try_into().expect("size not larger than 32 MB"),
                    url_part,
                    url_part,
                );
                let url =
                    <Dracoon<Connected> as UploadInternal<R>>::create_s3_upload_urls::<'_, '_>(
                        self,
                        upload_channel.upload_id.clone(),
                        url_req,
                    )
                    .await?;
                let url = url.urls.first().expect("Creating S3 url failed");

                // truncation is safe because chunk_size is 32 MB
                #[allow(clippy::cast_possible_truncation, clippy::cast_lossless)]
                let curr_pos: u64 = ((url_part - 1) * (CHUNK_SIZE as u32)) as u64;

                let e_tag = <Dracoon<Connected> as UploadInternal<R>>::upload_stream_to_s3(
                    self,
                    Box::pin(stream),
                    url,
                    file_meta.clone(),
                    n,
                    Some(curr_pos),
                    cloneable_callback.clone(),
                )
                .await?;

                s3_parts.push(S3FileUploadPart::new(url_part, e_tag));
            }
            Err(err) => return Err(DracoonClientError::IoError),
        }

        // finalize upload
        let complete_upload_req = CompleteS3FileUploadRequest::builder(s3_parts)
            .with_resolution_strategy(resolution_strategy)
            .with_keep_share_links(keep_share_links)
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
            let status_response = <Dracoon<Connected> as UploadInternal<R>>::get_upload_status(
                self,
                upload_channel.upload_id.clone(),
            )
            .await?;

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

    #[allow(clippy::too_many_lines)]
    async fn upload_to_s3_encrypted(
        &mut self,
        file_meta: FileMeta,
        parent_node: &Node,
        upload_options: UploadOptions,
        mut reader: BufReader<R>,
        callback: Option<UploadProgressCallback>,
        chunk_size: Option<usize>,
    ) -> Result<Node, DracoonClientError> {
        let keypair = self.get_keypair(None).await?.clone();

        let chunk_size = chunk_size.unwrap_or(CHUNK_SIZE);

        let mut crypto_buff =
            vec![0u8; file_meta.1.try_into().expect("size not larger than 32 MB")];
        let mut read_buff = vec![0u8; file_meta.1.try_into().expect("size not larger than 32 MB")];
        let mut crypter = DracoonCrypto::encrypter(&mut crypto_buff)?;

        while let Ok(chunk) = reader.read(&mut read_buff).await {
            if chunk == 0 {
                break;
            }
            crypter.update(&read_buff[..chunk])?;
        }
        crypter.finalize()?;
        // drop the read buffer after completing the encryption
        drop(read_buff);

        //TODO: rewrite without buffer clone
        let enc_bytes = crypter.get_message().clone();

        assert_eq!(enc_bytes.len() as u64, file_meta.1);

        let mut crypto_reader = BufReader::new(enc_bytes.as_slice());
        let plain_file_key = crypter.get_plain_file_key();
        let file_key = DracoonCrypto::encrypt_file_key(plain_file_key.clone(), keypair)?;
        // drop the crypto buffer (enc bytes are still in the reader)
        drop(crypto_buff);

        let (
            classification,
            timestamp_creation,
            timestamp_modification,
            expiration,
            resolution_strategy,
            keep_share_links,
        ) = parse_upload_options(&file_meta, &upload_options);

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

        let fm = &file_meta.clone();
        let mut s3_parts = Vec::new();

        let (count_urls, last_chunk_size) = calculate_s3_url_count(fm.1, chunk_size as u64);
        let mut url_part: u32 = 1;

        let cloneable_callback = callback.map(CloneableUploadProgressCallback::new);

        if count_urls > 1 {
            while url_part < count_urls {
                let mut buffer = vec![0; chunk_size];

                match crypto_reader.read_exact(&mut buffer).await {
                    Ok(0) => break,
                    Ok(n) => {
                        let chunk_len = n;
                        buffer.truncate(chunk_len);
                        let chunk = bytes::Bytes::from(buffer);

                        let stream: async_stream::__private::AsyncStream<
                            Result<bytes::Bytes, std::io::Error>,
                            _,
                        > = async_stream::stream! {
                            yield Ok(chunk);
                        };

                        let url_req = GeneratePresignedUrlsRequest::new(
                            chunk_len.try_into().expect("size not larger than 32 MB"),
                            url_part,
                            url_part,
                        );
                        let url =
                            <Dracoon<Connected> as UploadInternal<R>>::create_s3_upload_urls::<
                                '_,
                                '_,
                            >(
                                self, upload_channel.upload_id.clone(), url_req
                            )
                            .await?;
                        let url = url.urls.first().expect("Creating S3 url failed");
                        
                        // truncation is safe because chunk_size is 32 MB
                        #[allow(clippy::cast_possible_truncation, clippy::cast_lossless)]
                        let curr_pos: u64 = ((url_part - 1) * (chunk_size as u32)) as u64;

                        let e_tag = <Dracoon<Connected> as UploadInternal<R>>::upload_stream_to_s3(
                            self,
                            Box::pin(stream),
                            url,
                            file_meta.clone(),
                            chunk_len,
                            Some(curr_pos),
                            cloneable_callback.clone(),
                        )
                        .await?;

                        s3_parts.push(S3FileUploadPart::new(url_part, e_tag));
                        url_part += 1;
                    }
                    Err(err) => return Err(DracoonClientError::IoError),
                }
            }
        }

        // upload last chunk
        let mut buffer = vec![
            0;
            last_chunk_size
                .try_into()
                .expect("size not larger than 32 MB")
        ];
        match crypto_reader.read_exact(&mut buffer).await {
            Ok(0) => unreachable!("last chunk is empty"),
            Ok(n) => {
                buffer.truncate(n);
                let chunk = bytes::Bytes::from(buffer);
                let stream: async_stream::__private::AsyncStream<
                    Result<bytes::Bytes, std::io::Error>,
                    _,
                > = async_stream::stream! {
                    // TODO: chunk stream for better progress
                    yield Ok(chunk);

                };

                let url_req = GeneratePresignedUrlsRequest::new(
                    n.try_into().expect("size not larger than 32 MB"),
                    url_part,
                    url_part,
                );
                let url =
                    <Dracoon<Connected> as UploadInternal<R>>::create_s3_upload_urls::<'_, '_>(
                        self,
                        upload_channel.upload_id.clone(),
                        url_req,
                    )
                    .await?;
                let url = url.urls.first().expect("Creating S3 url failed");

                // truncation is safe because chunk_size is 32 MB
                #[allow(clippy::cast_possible_truncation, clippy::cast_lossless)]
                let curr_pos: u64 = ((url_part - 1) * (CHUNK_SIZE as u32)) as u64;

                let e_tag = <Dracoon<Connected> as UploadInternal<R>>::upload_stream_to_s3(
                    self,
                    Box::pin(stream),
                    url,
                    file_meta.clone(),
                    n,
                    Some(curr_pos),
                    cloneable_callback.clone(),
                )
                .await?;

                s3_parts.push(S3FileUploadPart::new(url_part, e_tag));
            }

            Err(err) => return Err(DracoonClientError::IoError),
        }

        // finalize upload
        let complete_upload_req = CompleteS3FileUploadRequest::builder(s3_parts)
            .with_resolution_strategy(resolution_strategy)
            .with_keep_share_links(keep_share_links)
            .with_file_key(file_key)
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
            let status_response = <Dracoon<Connected> as UploadInternal<R>>::get_upload_status(
                self,
                upload_channel.upload_id.clone(),
            )
            .await?;

            match status_response.status {
                S3UploadStatus::Done => {
                    // fetch missing keys (limit 50)
                    let missing_keys =
                        <Dracoon<Connected> as UploadInternal<R>>::get_missing_file_keys(
                            self,
                            status_response
                                .node
                                .as_ref()
                                .expect("Node must be set if status is done")
                                .id,
                        )
                        .await?;

                    // encrypt plain file key for each user
                    let key_reqs = missing_keys
                        .users
                        .into_iter()
                        .flat_map::<Result<UserFileKeySetRequest, DracoonClientError>, _>(|user| {
                            let user_id = user.id;
                            let file_id = status_response
                                .node
                                .as_ref()
                                .expect("Node must be set if status is done")
                                .id;
                            let public_key = user.public_key_container;
                            let file_key = DracoonCrypto::encrypt_file_key(
                                plain_file_key.clone(),
                                public_key,
                            )?;
                            let set_key_req =
                                UserFileKeySetRequest::new(file_id, user_id, file_key);
                            Ok(set_key_req)
                        })
                        .collect::<Vec<_>>();

                    drop(plain_file_key);

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

    async fn upload_stream_to_s3<'a>(
        &self,
        mut stream: impl Stream<Item = Result<bytes::Bytes, impl std::error::Error + Send + Sync + 'static>>
            + Sync
            + Send
            + Unpin
            + 'static,
        url: &PresignedUrl,
        file_meta: FileMeta,
        chunk_size: usize,
        curr_pos: Option<u64>,
        callback: Option<CloneableUploadProgressCallback>,
    ) -> Result<String, DracoonClientError> {
        // Initialize a variable to keep track of the number of bytes read
        let mut bytes_read = curr_pos.unwrap_or(0);
        let file_size = file_meta.1;
        // Create an async stream from the reader
        let async_stream = async_stream::stream! {

            while let Some(chunk) = stream.next().await {
                if let Ok(chunk) = &chunk {
                    let processed = min(bytes_read + (chunk.len() as u64), file_meta.1);
                    bytes_read = processed;

                    if let Some(cb) = callback.clone() {
                        cb.call(bytes_read, file_meta.1);
                    }
                }
                yield chunk
            }
        };

        let body = Body::wrap_stream(async_stream);

        let res = self
            .client
            .http
            .put(&url.url)
            .body(body)
            .header(header::CONTENT_LENGTH, chunk_size)
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

    async fn get_missing_file_keys(
        &self,
        file_id: u64,
    ) -> Result<MissingKeysResponse, DracoonClientError> {
        let url_part = format!("{DRACOON_API_PREFIX}/{NODES_BASE}/{MISSING_FILE_KEYS}");

        let mut api_url = self.build_api_url(&url_part);

        api_url
            .query_pairs_mut()
            .append_pair("file_id", file_id.to_string().as_str())
            .append_pair("limit", MISSING_KEYS_BATCH.to_string().as_str())
            .finish();

        let response = self
            .client
            .http
            .get(api_url)
            .header(header::AUTHORIZATION, self.get_auth_header().await?)
            .send()
            .await?;

        MissingKeysResponse::from_response(response).await
    }

    async fn set_file_keys(
        &self,
        keys_batch_req: UserFileKeySetBatchRequest,
    ) -> Result<(), DracoonClientError> {
        let url_part = format!("{DRACOON_API_PREFIX}/{NODES_BASE}/{FILES_BASE}/{FILES_KEYS}");

        let api_url = self.build_api_url(&url_part);

        let response = self
            .client
            .http
            .post(api_url)
            .header(header::AUTHORIZATION, self.get_auth_header().await?)
            .json(&keys_batch_req)
            .send()
            .await?;

        if response.status().is_server_error() || response.status().is_client_error() {
            return Err(DracoonClientError::from_response(response)
                .await
                .expect("Could not parse error response"));
        }

        Ok(())
    }
}
/// helper to parse upload options (file meta and upload options)
fn parse_upload_options(
    file_meta: &FileMeta,
    upload_options: &UploadOptions,
) -> (
    u8,
    DateTime<Utc>,
    DateTime<Utc>,
    ObjectExpiration,
    ResolutionStrategy,
    bool,
) {
    let classification = upload_options.1.unwrap_or(2);
    let timestamp_modification = file_meta.3.unwrap_or(Utc::now());
    let timestamp_creation = file_meta.2.unwrap_or(Utc::now());
    let expiration = upload_options.clone().0.unwrap_or_default();
    let resolution_strategy = upload_options
        .3
        .as_ref()
        .unwrap_or(&ResolutionStrategy::AutoRename);
    let keep_share_links = upload_options.2.unwrap_or(false);

    (
        classification,
        timestamp_creation,
        timestamp_modification,
        expiration,
        resolution_strategy.clone(),
        keep_share_links,
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

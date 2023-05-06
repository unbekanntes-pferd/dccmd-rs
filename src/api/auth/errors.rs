use async_trait::async_trait;
use dco3_crypto::DracoonCryptoError;
use reqwest::{Error as ReqError, Response};
use thiserror::Error;

use crate::api::{nodes::models::S3ErrorResponse, utils::FromResponse};

use super::models::{DracoonAuthErrorResponse, DracoonErrorResponse};

#[derive(Debug, Error)]
pub enum DracoonClientError {
    #[error("Client id required")]
    MissingClientId,
    #[error("Client secret required")]
    MissingClientSecret,
    #[error("Base url required")]
    MissingBaseUrl,
    #[error("Invalid DRACOON url")]
    InvalidUrl(String),
    #[error("Invalid DRACOON path")]
    InvalidPath(String),
    #[error("Connection to DRACOON failed")]
    ConnectionFailed,
    #[error("Unknown error")]
    Unknown,
    #[error("Internal error")]
    Internal,
    #[error("HTTP error")]
    Http(DracoonErrorResponse),
    #[error("S3 error")]
    S3Error(Box<S3ErrorResponse>),
    #[error("Authentication error")]
    Auth(DracoonAuthErrorResponse),
    #[error("IO error")]
    IoError,
    #[error("Crypto error")]
    CryptoError(DracoonCryptoError),
    #[error("Missing encryption secret")]
    MissingEncryptionSecret,
}

impl From<ReqError> for DracoonClientError {
    fn from(value: ReqError) -> Self {
        if value.is_builder() {
            return DracoonClientError::Internal;
        }

        DracoonClientError::Unknown
    }
}

#[async_trait]
impl FromResponse for DracoonClientError {
    async fn from_response(value: Response) -> Result<Self, DracoonClientError> {

        if !value.status().is_success() {
            let error = value.json::<DracoonErrorResponse>().await?;
            return Ok(DracoonClientError::Http(error));
        }

        Err(DracoonClientError::Unknown)
    }
}

impl From<DracoonCryptoError> for DracoonClientError {
    fn from(value: DracoonCryptoError) -> Self {
        DracoonClientError::CryptoError(value)
    }
}

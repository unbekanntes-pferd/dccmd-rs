use reqwest::Error as ReqError;
use thiserror::Error;

use crate::api::nodes::models::S3ErrorResponse;

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
    InvalidUrl,
    #[error("Connection to DRACOON failed")]
    ConnectionFailed,
    #[error("Unknown error")]
    Unknown,
    #[error("Internal error")]
    Internal,
    #[error("HTTP error")]
    Http(DracoonErrorResponse),
    #[error("S3 error")]
    S3Error(S3ErrorResponse),
    #[error("Authentication error")]
    Auth(DracoonAuthErrorResponse),
    #[error("IO error")]
    IoError,
}

impl From<ReqError> for DracoonClientError {
    fn from(value: ReqError) -> Self {
        if value.is_builder() {
            return DracoonClientError::Internal;
        }

        DracoonClientError::Unknown
    }
}

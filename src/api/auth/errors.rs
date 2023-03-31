use thiserror::Error;
use reqwest::Error as ReqError;

use super::models::{DracoonErrorResponse, DracoonAuthErrorResponse};

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
    #[error("Authentication error")]
    Auth(DracoonAuthErrorResponse)
}

impl From<ReqError> for DracoonClientError {
    fn from(value: ReqError) -> Self {

        if value.is_builder() {
            return DracoonClientError::Internal
        }


        DracoonClientError::Unknown

    }
}
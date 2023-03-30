use thiserror::Error;

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
    Unknown
}

use clap::Parser;
use thiserror::Error;

use crate::api::errors::DracoonClientError;


#[derive(Debug, PartialEq, Error)]
pub enum DcCmdError {
    #[error("Connection to DRACOON failed")]
    ConnectionFailed,
    #[error("Unknown error")]
    Unknown,
    #[error("Invalid DRACOON url format")]
    InvalidUrl,
    #[error("Saving DRACOON credentials failed")]
    CredentialStorageFailed,
    #[error("Deleting DRACOON credentials failed")]
    CredentialDeletionFailed,
    #[error("DRACOON account not found")]
    InvalidAccount
}


impl From<DracoonClientError> for DcCmdError {
    fn from(value: DracoonClientError) -> Self {
        match value {
            DracoonClientError::ConnectionFailed => DcCmdError::ConnectionFailed,
            _ => DcCmdError::Unknown
        }
    }
}

#[derive(Parser)]
#[clap(rename_all = "kebab-case", about="DRACOON Commander (dccmd-rs)")]
pub enum DcCmd {
    Upload {
        source: String,
        target: String
    },
    Download {
        source: String,
        target: String
    },
}
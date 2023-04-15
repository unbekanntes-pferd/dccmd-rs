use clap::Parser;
use thiserror::Error;

use crate::api::{
    auth::{
        errors::DracoonClientError,
        models::{DracoonAuthErrorResponse, DracoonErrorResponse},
    },
    nodes::models::S3ErrorResponse,
};

#[derive(Debug, PartialEq, Error)]
pub enum DcCmdError {
    #[error("Connection to DRACOON failed")]
    ConnectionFailed,
    #[error("Unknown error")]
    Unknown,
    #[error("Invalid DRACOON url format")]
    InvalidUrl(String),
    #[error("Invalid DRACOON path")]
    InvalidPath(String),
    #[error("Saving DRACOON credentials failed")]
    CredentialStorageFailed,
    #[error("Deleting DRACOON credentials failed")]
    CredentialDeletionFailed,
    #[error("DRACOON account not found")]
    InvalidAccount,
    #[error("DRACOON HTTP API error")]
    DracoonError(DracoonErrorResponse),
    #[error("DRACOON HTTP S3 error")]
    DracoonS3Error(S3ErrorResponse),
    #[error("DRACOON HTTP authentication error")]
    DracoonAuthError(DracoonAuthErrorResponse),
    #[error("IO error")]
    IoError,
}

impl From<DracoonClientError> for DcCmdError {
    fn from(value: DracoonClientError) -> Self {
        match value {
            DracoonClientError::ConnectionFailed => DcCmdError::ConnectionFailed,
            DracoonClientError::Http(err) => DcCmdError::DracoonError(err),
            DracoonClientError::Auth(err) => DcCmdError::DracoonAuthError(err),
            DracoonClientError::InvalidUrl(url)=> DcCmdError::InvalidUrl(url),
            DracoonClientError::IoError => DcCmdError::IoError,
            DracoonClientError::S3Error(err) => DcCmdError::DracoonS3Error(err),
            _ => DcCmdError::Unknown,
        }
    }
}

#[derive(Parser)]
#[clap(rename_all = "kebab-case", about = "DRACOON Commander (dccmd-rs)")]
pub struct DcCmd {
    #[clap(subcommand)]
    pub cmd: DcCmdCommand,
}

#[derive(Parser)]
pub enum DcCmdCommand {
    Upload {
        source: String,
        target: String,
    },
    Download {
        source: String,
        target: String,
    },
    Ls {
        source: String,

        #[clap(short, long)]
        long: bool,

        #[clap(short='r', long)]
        human_readable: bool,

        #[clap(long)]
        managed: bool,

        #[clap(long)]
        all: bool,
    },
}

#[derive(Clone, Copy)]
pub enum PrintFormat {
    Pretty,
    Csv,
}

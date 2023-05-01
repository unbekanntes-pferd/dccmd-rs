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
    DracoonS3Error(Box<S3ErrorResponse>),
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
            DracoonClientError::InvalidUrl(url) => DcCmdError::InvalidUrl(url),
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

    #[clap(long)]
    pub debug: bool,
}

#[derive(Parser)]
pub enum DcCmdCommand {
    /// Upload a file to DRACOON
    Upload {
        /// Source file path
        source: String,

        /// Target file path in DRACOON
        target: String,

        /// Overwrite existing file in DRACOON
        #[clap(long)]
        overwrite: bool,
        
        /// classification of the node (1-4)
        #[clap(long)]
        classification: Option<u8>,

    },
    /// download a file from DRACOON to target
    Download {
        /// Source file path in DRACOON
        source: String,
        /// Target file path
        target: String,
    },
    Ls {
        /// Source file path in DRACOON
        source: String,

        /// Print node information (details)
        #[clap(short, long)]
        long: bool,

         /// human readable node size
        #[clap(short = 'r', long)]
        human_readable: bool,

        /// Display nodes as room manager / room admin
        #[clap(long)]
        managed: bool,
        
        /// fetch all nodes (default: 500)
        #[clap(long)]
        all: bool,
    },
    
    /// Create a folder in DRACOON
    Mkdir {
        /// Source file path in DRACOON
        source: String,
        
        /// classification of the node (1-4)
        #[clap(long)]
        classification: Option<u8>,

        /// Notes
        #[clap(long)]
        notes: Option<String>,
    },
    
    /// Create a room in DRACOON
    Mkroom {
        /// Source file path in DRACOON
        source: String,
    },

    /// Delete a node in DRACOON
    Rm {
        /// Source file path in DRACOON
        source: String,
        
        /// recursive delete (mandatory for rooms / folders)
        #[clap(short, long)]
        recursive: bool,
    },
}

#[derive(Clone, Copy)]
pub enum PrintFormat {
    Pretty,
    Csv,
}

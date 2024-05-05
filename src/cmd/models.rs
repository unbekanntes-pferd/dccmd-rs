use clap::Parser;
use thiserror::Error;

use dco3::{
    auth::{
        errors::DracoonClientError,
        models::{DracoonAuthErrorResponse, DracoonErrorResponse},
    },
    nodes::models::S3ErrorResponse,
};

use super::config::models::{ConfigAuthCommand, ConfigCryptoCommand};

// represents password flow
#[derive(Clone)]
pub struct PasswordAuth(pub String, pub String);

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
    #[error("Invalid argument")]
    InvalidArgument(String),
    #[error("Log file creation failed")]
    LogFileCreationFailed,
}

impl From<DracoonClientError> for DcCmdError {
    fn from(value: DracoonClientError) -> Self {
        match value {
            DracoonClientError::ConnectionFailed(_) => DcCmdError::ConnectionFailed,
            DracoonClientError::Http(err) => DcCmdError::DracoonError(err),
            DracoonClientError::Auth(err) => DcCmdError::DracoonAuthError(err),
            DracoonClientError::InvalidUrl(url) => DcCmdError::InvalidUrl(url),
            DracoonClientError::IoError => DcCmdError::IoError,
            DracoonClientError::S3Error(err) => DcCmdError::DracoonS3Error(err),
            DracoonClientError::MissingArgument => {
                DcCmdError::InvalidArgument("Missing argument (password set?)".to_string())
            },
            DracoonClientError::CryptoError(_) => {
                DcCmdError::InvalidArgument(("Wrong encryption secret.").to_string())
            },  
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

    #[clap(long)]
    pub log_file_out: bool,

    #[clap(long)]
    pub log_file_path: Option<String>,

    /// optional username
    #[clap(long)]
    pub username: Option<String>,

    /// optional password
    #[clap(long)]
    pub password: Option<String>,

    /// optional encryption password
    #[clap(long)]
    pub encryption_password: Option<String>,
}

#[derive(Parser)]
pub enum DcCmdCommand {
    /// Upload a file or folder to DRACOON
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

        #[clap(long, short)]
        velocity: Option<u8>,

        /// recursive upload
        #[clap(short, long)]
        recursive: bool,

        /// skip root
        #[clap(long)]
        skip_root: bool,

        /// share upload
        #[clap(long)]
        share: bool,

        #[clap(long)]
        share_password: Option<String>,
    },
    /// Download a file or container from DRACOON to target
    Download {
        /// Source file path in DRACOON
        source: String,
        /// Target file path
        target: String,

        #[clap(long, short)]
        velocity: Option<u8>,

        /// recursive download
        #[clap(short, long)]
        recursive: bool,

        #[clap(long)]
        share_password: Option<String>,
    },
    /// List nodes in DRACOON
    Ls {
        /// Source file path in DRACOON
        source: String,

        /// Print node information (details)
        #[clap(short, long)]
        long: bool,

        /// human readable node size
        #[clap(short = 'r', long)]
        human_readable: bool,

        /// skip n nodes (default offset: 0)
        #[clap(short, long)]
        offset: Option<u32>,

        /// limit n nodes (default limit: 500)
        #[clap(long)]
        limit: Option<u32>,

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

    /// Create a room in DRACOON (inhherits permissions from parent)
    Mkroom {
        /// Source file path in DRACOON
        source: String,

        /// admin usernames
        #[clap(long, short)]
        admin_users: Option<Vec<String>>,

        /// classification of the node (1-4)
        #[clap(long)]
        classification: Option<u8>,

        /// inherit permissions from parent room
        #[clap(long)]
        inherit_permissions: bool,
    },

    /// Delete a node in DRACOON
    Rm {
        /// Source file path in DRACOON
        source: String,

        /// recursive delete (mandatory for rooms / folders)
        #[clap(short, long)]
        recursive: bool,
    },

    /// Manage users in DRACOON
    Users {
        #[clap(subcommand)]
        cmd: UserCommand,
    },

    /// Configure DRACOON Commander
    Config {
        #[clap(subcommand)]
        cmd: ConfigCommand,
    },

    /// Print current dccmd-rs version
    Version,
}

#[derive(Parser)]
pub enum UserCommand {
    /// List users in DRACOON
    Ls {
        /// DRACOON url
        target: String,

        /// search filter (username, first name, last name)
        #[clap(long)]
        search: Option<String>,

        /// skip n users (default offset: 0)
        #[clap(short, long)]
        offset: Option<u32>,

        /// limit n users (default limit: 500)
        #[clap(long)]
        limit: Option<u32>,

        /// fetch all users (default: 500)
        #[clap(long)]
        all: bool,

        /// print user information in CSV format
        #[clap(long)]
        csv: bool,
    },

    /// Create a user in DRACOON
    Create {
        /// DRACOON url
        target: String,

        /// User first name
        #[clap(long, short)]
        first_name: String,

        /// User last name
        #[clap(long, short)]
        last_name: String,

        /// User email
        #[clap(long, short)]
        email: String,

        /// Login (for OIDC)
        #[clap(long)]
        login: Option<String>,

        /// OIDC config id
        #[clap(long)]
        oidc_id: Option<u32>,

        /// OIDC config id
        #[clap(long)]
        mfa_enforced: bool,
    },

    /// delete a user in DRACOON
    Rm {
        /// DRACOON url
        target: String,

        /// User login
        #[clap(long, short)]
        user_name: Option<String>,

        #[clap(long)]
        user_id: Option<u64>,
    },

    /// import users from CSV file into DRACOON
    Import {
        /// DRACOON url
        target: String,

        /// Source file path
        source: String,

        /// OIDC config id
        #[clap(long)]
        oidc_id: Option<u32>,
    },

    /// print user information in DRACOON
    Info {
        /// DRACOON url
        target: String,

        /// User login
        #[clap(long, short)]
        user_name: Option<String>,

        #[clap(long)]
        user_id: Option<u64>,
    },
}

#[derive(Parser)]
pub enum ConfigCommand {
    /// Manage DRACOON Commander auth credentials (refresh token)
    Auth {
        #[clap(subcommand)]
        cmd: ConfigAuthCommand,
    },

    /// Manage DRACOON Commander encryption credentials (encryption secret)
    Crypto {
        #[clap(subcommand)]
        cmd: ConfigCryptoCommand,
    },
}

#[derive(Clone, Copy)]
pub enum PrintFormat {
    Pretty,
    Csv,
}

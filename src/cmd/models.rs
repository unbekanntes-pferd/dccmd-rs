use clap::Parser;
use thiserror::Error;

use dco3::{
    auth::{
        errors::DracoonClientError,
        models::{DracoonAuthErrorResponse, DracoonErrorResponse},
    },
    nodes::models::S3ErrorResponse,
    FilterOperator, FilterQueryBuilder, ListAllParams,
};

use super::{
    config::models::{ConfigAuthCommand, ConfigCryptoCommand},
    groups::GroupsUsersCommand,
};

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
            }
            DracoonClientError::CryptoError(_) => {
                DcCmdError::InvalidArgument(("Wrong encryption secret.").to_string())
            }
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

        /// Preserve Download Share Links and point them to the new node in DRACOON
        #[clap(long)]
        keep_share_links: bool,

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
    /// Transfer files across DRACOON instances
    Transfer {
        /// Source file path in DRACOON
        source: String,

        /// Target file path in DRACOON
        target: String,

        /// Overwrite existing file in DRACOON
        #[clap(long)]
        overwrite: bool,

        /// Preserve Download Share Links and point them to the new node in DRACOON
        #[clap(long)]
        keep_share_links: bool,

        /// classification of the node (1-4)
        #[clap(long)]
        classification: Option<u8>,
        
        /// share upload
        #[clap(long)]
        share: bool,

        #[clap(long)]
        share_password: Option<String>,
    },
    /// List nodes in DRACOON
    Ls {
        /// Source file path in DRACOON
        source: String,

        /// Filter nodes (e.g. by name)
        #[clap(long)]
        filter: Option<String>,

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
        cmd: UsersCommand,
    },

    /// Manage groups in DRACOON
    Groups {
        #[clap(subcommand)]
        cmd: GroupsCommand,
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
pub enum UsersCommand {
    /// List users in DRACOON
    Ls {
        /// DRACOON url
        target: String,

        /// search filter (e.g. username, first name, last name)
        #[clap(long)]
        filter: Option<String>,

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
pub enum GroupsCommand {
    /// List groups in DRACOON
    Ls {
        /// DRACOON url
        target: String,

        /// search filter (e.g. group name)
        #[clap(long)]
        filter: Option<String>,

        /// skip n groups (default offset: 0)
        #[clap(short, long)]
        offset: Option<u32>,

        /// limit n groups (default limit: 500)
        #[clap(long)]
        limit: Option<u32>,

        /// fetch all groups (default: 500)
        #[clap(long)]
        all: bool,

        /// print user information in CSV format
        #[clap(long)]
        csv: bool,
    },

    /// Create a group in DRACOON
    Create {
        /// DRACOON url
        target: String,

        /// Group name
        #[clap(long, short)]
        name: String,
    },

    /// delete a group in DRACOON
    Rm {
        /// DRACOON url
        target: String,

        ///  Group name
        #[clap(long, short)]
        group_name: Option<String>,

        /// Group id
        #[clap(long)]
        group_id: Option<u64>,
    },

    Users {
        #[clap(subcommand)]
        cmd: GroupsUsersCommand,
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

pub struct ListOptions {
    filter: Option<String>,
    offset: Option<u32>,
    limit: Option<u32>,
    all: bool,
    csv: bool,
}

impl ListOptions {
    pub fn new(
        filter: Option<String>,
        offset: Option<u32>,
        limit: Option<u32>,
        all: bool,
        csv: bool,
    ) -> Self {
        Self {
            filter,
            offset,
            limit,
            all,
            csv,
        }
    }

    pub fn filter(&self) -> &Option<String> {
        &self.filter
    }

    pub fn offset(&self) -> Option<u32> {
        self.offset
    }

    pub fn limit(&self) -> Option<u32> {
        self.limit
    }

    pub fn all(&self) -> bool {
        self.all
    }

    pub fn csv(&self) -> bool {
        self.csv
    }
}

pub(crate) trait ToFilterOperator {
    fn to_filter_operator(&self) -> Result<FilterOperator, DcCmdError>;
}

impl ToFilterOperator for &str {
    fn to_filter_operator(&self) -> Result<FilterOperator, DcCmdError> {
        match *self {
            "eq" => Ok(FilterOperator::Eq),
            "neq" => Ok(FilterOperator::Neq),
            "cn" => Ok(FilterOperator::Cn),
            "ge" => Ok(FilterOperator::Ge),
            "le" => Ok(FilterOperator::Le),
            _ => Err(DcCmdError::InvalidArgument(format!(
                "Invalid filter operator: {}",
                self
            ))),
        }
    }
}

pub fn build_params(
    filter: &Option<String>,
    offset: u64,
    limit: u64,
) -> Result<ListAllParams, DcCmdError> {
    if let Some(search) = filter {
        let params = {
            let mut parts = search.split(':');

            let error_msg = format!(
                "Invalid filter query ({}) Expected format: field:operator:value",
                search
            );
            let field = parts
                .next()
                .ok_or(DcCmdError::InvalidArgument(error_msg.clone()))?;
            let operator = parts
                .next()
                .ok_or(DcCmdError::InvalidArgument(error_msg.clone()))?
                .to_filter_operator()?;
            let value = parts.next().ok_or(DcCmdError::InvalidArgument(error_msg))?;

            let filter = FilterQueryBuilder::new()
                .with_field(field)
                .with_operator(operator)
                .with_value(value)
                .try_build()?;

            ListAllParams::builder()
                .with_filter(filter)
                .with_offset(offset)
                .with_limit(limit)
                .build()
        };

        Ok(params)
    } else {
        Ok(ListAllParams::builder()
            .with_offset(offset)
            .with_limit(limit)
            .build())
    }
}

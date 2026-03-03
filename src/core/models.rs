use secrecy::{ExposeSecret, SecretString};
use thiserror::Error;

use dco3::{
    auth::models::{DracoonAuthErrorResponse, DracoonErrorResponse},
    errors::DracoonClientError,
    nodes::models::S3ErrorResponse,
    FilterOperator, FilterQueryBuilder, ListAllParams,
};

// represents password flow
#[derive(Clone)]
pub struct PasswordAuth {
    username: String,
    password: SecretString,
}

impl PasswordAuth {
    pub fn new(username: String, password: SecretString) -> Self {
        Self { username, password }
    }

    pub fn username(&self) -> &str {
        self.username.as_str()
    }

    pub fn password_str(&self) -> &str {
        self.password.expose_secret()
    }
}

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
    #[error("Command failed")]
    CommandFailed,
    #[error("Config directory unavailable on this platform")]
    ConfigDirUnavailable,
    #[error("Config directory creation failed: {0}")]
    ConfigDirCreationFailed(String),
    #[error("Log file creation failed")]
    LogFileCreationFailed,
}

impl From<DracoonClientError> for DcCmdError {
    fn from(value: DracoonClientError) -> Self {
        (&value).into()
    }
}

impl From<&DracoonClientError> for DcCmdError {
    fn from(value: &DracoonClientError) -> Self {
        match value {
            DracoonClientError::ConnectionFailed(_) => DcCmdError::ConnectionFailed,
            DracoonClientError::Http(err) => DcCmdError::DracoonError(err.clone()),
            DracoonClientError::Auth(err) => DcCmdError::DracoonAuthError(err.clone()),
            DracoonClientError::InvalidUrl(url) => DcCmdError::InvalidUrl(url.clone()),
            DracoonClientError::IoError => DcCmdError::IoError,
            DracoonClientError::S3Error(err) => DcCmdError::DracoonS3Error(err.clone()),
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

#[derive(Clone, Copy)]
pub enum PrintFormat {
    Pretty,
    Csv,
}

#[derive(Clone, Default)]
pub struct ListOptions {
    filter: Option<String>,
    offset: Option<u64>,
    limit: Option<u32>,
    all: bool,
    csv: bool,
}

impl ListOptions {
    pub fn new(
        filter: Option<String>,
        offset: Option<u64>,
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

    pub fn offset(&self) -> Option<u64> {
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
                "Invalid filter operator: {self}"
            ))),
        }
    }
}

pub fn build_params(
    filter: &Option<String>,
    offset: u64,
    limit: Option<u32>,
) -> Result<ListAllParams, DcCmdError> {
    if let Some(search) = filter {
        let params = {
            let mut parts = search.split(':');

            let error_msg =
                format!("Invalid filter query ({search}) Expected format: field:operator:value");
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

            let params = ListAllParams::builder()
                .with_filter(filter)
                .with_offset(offset);

            let params = if let Some(limit) = limit {
                params.with_limit(limit as u64)
            } else {
                params
            };

            params.build()
        };

        Ok(params)
    } else {
        let params = ListAllParams::builder().with_offset(offset);

        let params = if let Some(limit) = limit {
            params.with_limit(limit as u64)
        } else {
            params
        };

        Ok(params.build())
    }
}

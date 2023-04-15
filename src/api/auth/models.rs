use std::fmt::{Formatter, Display};

use url::ParseError;

use chrono::Utc;
use reqwest::{Response, StatusCode};
use serde::{Deserialize, Serialize};

use crate::{api::{constants::{GRANT_TYPE_REFRESH_TOKEN, GRANT_TYPE_AUTH_CODE}, utils::parse_body}};

use super::{errors::DracoonClientError, Connection};


/// represents form data payload for OAuth2 password flow
#[derive(Debug, Serialize, Deserialize)]
pub struct OAuth2PasswordFlow {
    pub username: String,
    pub password: String,
    pub grant_type: String,
}

/// represents form data payload for OAuth2 authorization code flow
#[derive(Debug, Serialize, Deserialize)]
pub struct OAuth2AuthCodeFlow {
    pub client_id: String,
    pub client_secret: String,
    pub grant_type: String,
    pub code: String,
    pub redirect_uri: String,
}

impl OAuth2AuthCodeFlow {
    /// creates a new authorization code flow payload
    pub fn new(client_id: &str, client_secret: &str, code: &str, redirect_uri: &str) -> Self {
        Self {
            client_id: client_id.to_string(),
            client_secret: client_secret.to_string(),
            grant_type: GRANT_TYPE_AUTH_CODE.to_string(),
            code: code.to_string(),
            redirect_uri: redirect_uri.to_string(),
        }
    }
}

/// represents form data payload for OAuth2 refresh token flow
#[derive(Debug, Serialize, Deserialize)]
pub struct OAuth2RefreshTokenFlow {
    client_id: String,
    client_secret: String,
    grant_type: String,
    refresh_token: String,
}

impl OAuth2RefreshTokenFlow {
    /// creates a new refresh token flow payload
    pub fn new(client_id: &str, client_secret: &str, refresh_token: &str) -> Self {
        Self {
            client_id: client_id.to_string(),
            client_secret: client_secret.to_string(),
            grant_type: GRANT_TYPE_REFRESH_TOKEN.to_string(),
            refresh_token: refresh_token.to_string(),
        }
    }
}

/// represents form data payload for OAuth2 token revoke
#[derive(Debug, Serialize, Deserialize)]
struct OAuth2TokenRevoke {
    client_id: String,
    client_secret: String,
    token_type_hint: String,
    token: String,
}

/// DRACOON OAuth2 token response
#[derive(Debug, Serialize, Deserialize)]
pub struct OAuth2TokenResponse {
    access_token: String,
    refresh_token: String,
    token_type: Option<String>,
    expires_in: u64,
    expires_in_inactive: Option<u64>,
    scope: Option<String>,
}

/// DRACOON http error response
#[derive(Deserialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DracoonErrorResponse {
    code: i32,
    message: String,
    debug_info: Option<String>,
    error_code: Option<i32>,
}

impl Display for DracoonErrorResponse {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Error: {} ({})", self.message, self.code)
    }
}

impl DracoonErrorResponse {
    /// creates a DRACOON compatible error type
    pub fn new(code: i32, message: &str) -> Self {
        Self {
            code,
            message: message.to_string(),
            debug_info: None,
            error_code: None,
        }
    }
}

/// DRACOON OAuth2 error response
#[derive(Deserialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DracoonAuthErrorResponse {
    error: String,
    error_description: String,
}


impl Display for DracoonAuthErrorResponse {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Error: {} ({})", self.error_description, self.error)
    }
}

impl OAuth2TokenResponse {
    /// transforms a response into a DRACOON OAuth2 token response
    /// on error will return a DRACOON auth error response
    pub async fn from_response(res: Response) -> Result<Self, DracoonClientError> {
        parse_body::<Self, DracoonAuthErrorResponse>(res).await
    }
}

/// represents the state of a status code
///  - Ok: 2xx
/// - Error: 4xx or 5xx
pub enum StatusCodeState {
    Ok(StatusCode),
    Error(StatusCode)
}

impl From<StatusCode> for StatusCodeState {
    /// transforms a status code into a status code state
    fn from(value: StatusCode) -> Self {
        match value {
            StatusCode::OK => StatusCodeState::Ok(value),
            StatusCode::CREATED => StatusCodeState::Ok(value),
            StatusCode::ACCEPTED => StatusCodeState::Ok(value),
            StatusCode::NO_CONTENT => StatusCodeState::Ok(value),
            _ => StatusCodeState::Error(value),
        }
    }
}

impl From<OAuth2TokenResponse> for Connection {
    /// transforms a OAuth2 token response into a connection for the client
    fn from(value: OAuth2TokenResponse) -> Self {
        Self {
            connected_at: Utc::now(),
            access_token: value.access_token,
            refresh_token: value.refresh_token,
            expires_in: value.expires_in.try_into().expect("only positive numbers"),
        }
    }
}

impl From<DracoonAuthErrorResponse> for DracoonClientError {
    /// transforms a DRACOON auth error response into a DRACOON client error
    fn from(value: DracoonAuthErrorResponse) -> Self {
        Self::Auth(value)
    }
}

impl From<DracoonErrorResponse> for DracoonClientError {
    /// transforms a DRACOON error response into a DRACOON client error
    fn from(value: DracoonErrorResponse) -> Self {
        Self::Http(value)
    }
}


impl From<ParseError> for DracoonClientError {
    /// transforms a URL parse error into a DRACOON client error
    fn from(value: ParseError) -> Self {

        Self::InvalidUrl("parsing url failed (invalid)".into())
    }
}
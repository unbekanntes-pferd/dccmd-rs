use url::ParseError;

use chrono::Utc;
use reqwest::{Response, StatusCode};
use serde::{Deserialize, Serialize};

use crate::cmd::utils::parse_body;

use super::{errors::DracoonClientError, Connection};

/// OAuth2 flow structs (form data for POST to token (revoke) url)
#[derive(Debug, Serialize, Deserialize)]
pub struct OAuth2PasswordFlow {
    pub username: String,
    pub password: String,
    pub grant_type: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OAuth2AuthCodeFlow {
    pub client_id: String,
    pub client_secret: String,
    pub grant_type: String,
    pub code: String,
    pub redirect_uri: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct OAuth2RefreshTokenFlow {
    client_id: String,
    client_secret: String,
    grant_type: String,
    refresh_token: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct OAuth2TokenRevoke {
    client_id: String,
    client_secret: String,
    token_type_hint: String,
    token: String,
}

/// DRACOON token response
#[derive(Debug, Serialize, Deserialize)]
pub struct OAuth2TokenResponse {
    access_token: String,
    refresh_token: String,
    token_type: Option<String>,
    expires_in: u64,
    expires_in_inactive: Option<u64>,
    scope: Option<String>,
}

#[derive(Deserialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DracoonErrorResponse {
    code: i32,
    message: String,
    debug_info: Option<String>,
    error_code: Option<i32>,
}

impl DracoonErrorResponse {
    pub fn new(code: i32, message: &str) -> Self {
        Self {
            code,
            message: message.to_string(),
            debug_info: None,
            error_code: None,
        }
    }
}

#[derive(Deserialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DracoonAuthErrorResponse {
    error: String,
    error_description: String,
}

impl OAuth2TokenResponse {
    pub async fn from_response(res: Response) -> Result<Self, DracoonClientError> {
        parse_body::<Self, DracoonAuthErrorResponse>(res).await
    }
}

pub enum StatusCodeState {
    Ok(StatusCode),
    Error(StatusCode)
}

impl From<StatusCode> for StatusCodeState {
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
    fn from(value: DracoonAuthErrorResponse) -> Self {
        Self::Auth(value)
    }
}

impl From<DracoonErrorResponse> for DracoonClientError {
    fn from(value: DracoonErrorResponse) -> Self {
        Self::Http(value)
    }
}


impl From<ParseError> for DracoonClientError {
    fn from(value: ParseError) -> Self {
        Self::InvalidUrl
    }
}
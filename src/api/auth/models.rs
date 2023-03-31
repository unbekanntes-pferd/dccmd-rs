use chrono::Utc;
use reqwest::{Response, StatusCode};
use serde::{Deserialize, Serialize};
use tracing::debug;

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

#[derive(Deserialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DracoonAuthErrorResponse {
    error: String,
    error_description: String,
}

impl OAuth2TokenResponse {
    pub async fn from_response(res: Response) -> Result<Self, DracoonClientError> {

        debug!("{:?}", res);

        match res.status() {
            StatusCode::OK => Ok(res.json::<OAuth2TokenResponse>().await?),
            _ => Err(DracoonClientError::Auth(
                res.json::<DracoonAuthErrorResponse>().await?,
            )),
        }
    }
}

impl From<OAuth2TokenResponse> for Connection {
    fn from(value: OAuth2TokenResponse) -> Self {
        Self {
            connected_at: Utc::now(),
            access_token: value.access_token,
            refresh_token: value.refresh_token,
            expires_in: value.expires_in.try_into().expect("only positive numbers")
        }
    }
}

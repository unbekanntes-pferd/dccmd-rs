use chrono::{DateTime, Utc};
use reqwest::{header, header::HeaderMap, Client, Url};
use std::marker::PhantomData;

use base64;

pub mod errors;
pub mod models;

use crate::api::{
    auth::models::{OAuth2AuthCodeFlow, OAuth2TokenResponse},
    constants::{DRACOON_TOKEN_URL, GRANT_TYPE_AUTH_CODE},
};

use self::errors::DracoonClientError;
use super::constants::APP_USER_AGENT;

pub enum OAuth2Flow {
    PasswordFlow(String, String),
    AuthCodeFlow(String),
    RefreshToken(String),
}

// states of a client
pub struct Connected;
pub struct Disconnected;

pub struct Connection {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: u32,
    pub connected_at: DateTime<Utc>,
}

pub struct DracoonClient<State = Disconnected> {
    base_url: Url,
    redirect_uri: Option<Url>,
    client_id: String,
    client_secret: String,
    pub http: Client,
    connection: Option<Connection>,
    connected: PhantomData<State>,
}

pub struct DracoonClientBuilder {
    base_url: Option<String>,
    redirect_uri: Option<String>,
    client_id: Option<String>,
    client_secret: Option<String>,
}

impl DracoonClientBuilder {
    pub fn new() -> Self {
        Self {
            base_url: None,
            redirect_uri: None,
            client_id: None,
            client_secret: None,
        }
    }
    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = Some(base_url.into());
        self
    }

    pub fn with_redirect_uri(mut self, redirect_uri: impl Into<String>) -> Self {
        self.redirect_uri = Some(redirect_uri.into());
        self
    }

    pub fn with_client_id(mut self, client_id: impl Into<String>) -> Self {
        self.client_id = Some(client_id.into());
        self
    }

    pub fn with_client_secret(mut self, client_secret: impl Into<String>) -> Self {
        self.client_secret = Some(client_secret.into());
        self
    }

    pub fn build(self) -> Result<DracoonClient<Disconnected>, DracoonClientError> {
        let http = Client::builder().user_agent(APP_USER_AGENT).build()?;

        let Some(base_url) = self.base_url.clone() else {
            return Err(DracoonClientError::MissingBaseUrl)
        };

        let base_url = Url::parse(&base_url)?;

        let Some(client_id) = self.client_id else {
            return Err(DracoonClientError::MissingClientId)
        };

        let Some(client_secret) = self.client_secret else {
            return Err(DracoonClientError::MissingClientSecret)
        };

        let redirect_uri = match self.redirect_uri {
            Some(url) => Url::parse(&url)?,
            None => Url::parse(&format!(
                "{}/oauth/callback",
                self.base_url.expect("missing base url already checked")
            ))?,
        };

        Ok(DracoonClient {
            base_url,
            redirect_uri: Some(redirect_uri),
            client_id,
            client_secret,
            connection: None,
            connected: PhantomData,
            http,
        })
    }
}

impl DracoonClient<Disconnected> {
    pub async fn connect(
        self,
        oauth_flow: OAuth2Flow,
    ) -> Result<DracoonClient<Connected>, DracoonClientError> {
        let connection = match oauth_flow {
            OAuth2Flow::PasswordFlow(username, password) => {
                self.connect_password_flow(&username, &password).await?
            }
            OAuth2Flow::AuthCodeFlow(code) => self.connect_authcode_flow(&code).await?,
            OAuth2Flow::RefreshToken(token) => self.connect_refresh_token(&token).await?,
        };

        let mut headers = HeaderMap::new();
        headers.insert(
            header::AUTHORIZATION,
            connection
                .access_token
                .parse()
                .expect("correct access token format"),
        );

        let http = Client::builder()
            .user_agent(APP_USER_AGENT)
            .default_headers(headers)
            .build()?;

        Ok(DracoonClient {
            client_id: self.client_id,
            client_secret: self.client_secret,
            connection: Some(connection),
            base_url: self.base_url,
            redirect_uri: self.redirect_uri,
            connected: PhantomData,
            http,
        })
    }

    fn client_credentials(&self) -> String {
        let client_credentials = format!("{}:{}", &self.client_id, &self.client_secret);

        base64::encode(client_credentials)
    }

    pub fn get_authorize_url(&mut self) -> String {
        let default_redirect = self
            .base_url
            .join("oauth/callback")
            .expect("Correct base url");
        let redirect_uri = self
            .redirect_uri
            .as_ref()
            .unwrap_or(&default_redirect)
            .to_owned();

        self.redirect_uri = Some(redirect_uri.clone());

        let mut authorize_url = self
            .base_url
            .join("oauth/authorize")
            .expect("Correct base url");
        let authorize_url = authorize_url
            .query_pairs_mut()
            .append_pair("response_type", "code")
            .append_pair("client_id", &self.client_id)
            .append_pair("redirect_uri", &redirect_uri.to_string())
            .append_pair("scope", "all")
            .finish();

        authorize_url.to_string()
    }

    fn get_token_url(&self) -> Url {
        self.base_url
            .join(DRACOON_TOKEN_URL)
            .expect("Correct base url")
    }

    async fn connect_password_flow(
        &self,
        username: &str,
        password: &str,
    ) -> Result<Connection, DracoonClientError> {
        todo!()
    }

    async fn connect_authcode_flow(&self, code: &str) -> Result<Connection, DracoonClientError> {
        let token_url = self.get_token_url();

        let auth = OAuth2AuthCodeFlow {
            client_id: self.client_id.clone(),
            client_secret: self.client_secret.clone(),
            code: code.into(),
            grant_type: GRANT_TYPE_AUTH_CODE.into(),
            redirect_uri: self
                .redirect_uri
                .as_ref()
                .expect("Redirect URI must be available for authorization code")
                .to_string(),
        };

        let res = self.http.post(token_url).form(&auth).send().await?;
        Ok(OAuth2TokenResponse::from_response(res).await?.into())
    }

    async fn connect_refresh_token(
        &self,
        refresh_token: &str,
    ) -> Result<Connection, DracoonClientError> {
        todo!()
    }
}

impl DracoonClient<Connected> {
    pub async fn disconnect(self) -> Result<DracoonClient<Disconnected>, DracoonClientError> {
        todo!()
    }

    pub fn get_base_url(&self) -> &Url {
        &self.base_url
    }

    pub fn get_auth_header(&self) -> String {
        format!(
            "Bearer {}",
            self.connection
                .as_ref()
                .expect("Connected client has a connection")
                .access_token
        )
    }
}

#[cfg(test)]
mod tests {
    use tokio_test::assert_ok;

    use super::*;

    #[test]
    #[ignore]
    fn test_auth_code_authentication() {
        let mut mock_server = mockito::Server::new();
        let base_url = mock_server.url();

        let auth_res = include_str!("./tests/auth_ok.json");
        //let auth_res_json = serde_json::from_str(auth_res).expect("Valid JSON format");

        println!("{}", auth_res);

        let auth_res_2 = r#"{
        "access_token": "12345sdfjkdsfhk",
        "token_type": "bearer",
        "refresh_token": "4985985489fscjkfsjk",
        "expires_in_inactive": 28800,
        "expires_in": 28800,
        "scope": "all"
    }"#;

        let auth_mock = mock_server
            .mock("GET", "/oauth/token")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body_from_file("../tests/auth_ok.json")
            .create();

        let dracoon = DracoonClientBuilder::new()
            .with_base_url(base_url)
            .with_client_id("client_id")
            .with_client_secret("client_secret")
            .build()
            .expect("valid client config");

        let auth_code = OAuth2Flow::AuthCodeFlow("hello world".to_string());

        let res = tokio_test::block_on(dracoon.connect(auth_code));

        assert_ok!(res);

        //assert!(res.connection.is_some());
    }
}

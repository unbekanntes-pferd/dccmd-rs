use std::marker::PhantomData;
use chrono::{Utc, DateTime};
use crate::api::errors::DracoonClientError;

pub enum OAuth2Flow {
    PasswordFlow(String, String),
    RefreshToken(String)
}

// states of a client
pub struct Connected;
pub struct Disconnected;

struct Connection {
    access_token: String,
    refresh_token: String,
    expires_in: u32,
    connected_at: DateTime<Utc>
}

pub struct DracoonClient<State = Disconnected> {

    base_url: String,
    redirect_uri: Option<String>,
    client_id: String,
    client_secret: String,
    connection: Option<Connection>,
    connected: PhantomData<State>

}

pub struct DracoonClientBuilder {
    base_url: Option<String>,
    redirect_uri: Option<String>,
    client_id: Option<String>,
    client_secret: Option<String>
}

impl DracoonClientBuilder {
    pub fn new() -> Self {
        Self { base_url: None, redirect_uri: None, client_id: None, client_secret: None}
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

        let Some(base_url) = self.base_url else {
            return Err(DracoonClientError::MissingBaseUrl)
        };

        let Some(client_id) = self.client_id else {
            return Err(DracoonClientError::MissingClientId)
        };

        let Some(client_secret) = self.client_secret else {
            return Err(DracoonClientError::MissingClientSecret)
        };

        Ok(DracoonClient { base_url, redirect_uri: self.redirect_uri, client_id, client_secret, connection: None, connected: PhantomData })

    }
}


impl DracoonClient<Disconnected> {
    pub fn connect(self, oauth_flow: OAuth2Flow) -> Result<DracoonClient<Connected>, DracoonClientError> {
        
        let connection = match oauth_flow {
            OAuth2Flow::PasswordFlow(username, password) => self.connect_password_flow(&username, &password)?,
            OAuth2Flow::RefreshToken(token) => self.connect_refresh_token(&token)?
        };

        Ok(DracoonClient { client_id: self.client_id, client_secret: self.client_secret, connection: Some(connection), base_url: self.base_url, redirect_uri: self.redirect_uri, connected: PhantomData })

        

    }

    fn connect_password_flow(&self, username: &str, password: &str) -> Result<Connection, DracoonClientError> {

todo!()
    }

    fn connect_refresh_token(&self, refresh_token: &str) -> Result<Connection, DracoonClientError> {
todo!()
    }
}
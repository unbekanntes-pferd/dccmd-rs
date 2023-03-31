use std::marker::PhantomData;

use self::{
    auth::{errors::DracoonClientError, Connected, Disconnected, OAuth2Flow},
    auth::{DracoonClient, DracoonClientBuilder},
};

pub mod auth;
pub mod constants;
pub mod models;
pub mod nodes;

pub struct Dracoon<State = Disconnected> {
    client: DracoonClient<State>,
    state: PhantomData<State>,
}

pub struct DracoonBuilder {
    base_url: Option<String>,
    client_id: Option<String>,
    client_secret: Option<String>,
    redirect_uri: Option<String>,
    client_builder: DracoonClientBuilder,
}

impl DracoonBuilder {
    pub fn new() -> Self {
        let client_builder = DracoonClientBuilder::new();
        Self {
            base_url: None,
            client_id: None,
            client_secret: None,
            redirect_uri: None,
            client_builder,
        }
    }

    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.client_builder = self.client_builder.with_base_url(base_url);
        self
    }
    pub fn with_client_id(mut self, client_id: impl Into<String>) -> Self {
        self.client_builder = self.client_builder.with_client_id(client_id);
        self
    }
    pub fn with_client_secret(mut self, client_secret: impl Into<String>) -> Self {
        self.client_builder = self.client_builder.with_client_secret(client_secret);
        self
    }
    pub fn with_redirect_uri(mut self, redirect_uri: impl Into<String>) -> Self {
        self.client_builder = self.client_builder.with_redirect_uri(redirect_uri);
        self
    }

    pub fn build(self) -> Result<Dracoon<Disconnected>, DracoonClientError> {
        let dracoon = self.client_builder.build()?;

        Ok(Dracoon {
            client: dracoon,
            state: PhantomData,
        })
    }
}

impl Dracoon<Disconnected> {
    pub async fn connect(
        self,
        oauth_flow: OAuth2Flow,
    ) -> Result<Dracoon<Connected>, DracoonClientError> {
        let client = self.client.connect(oauth_flow).await?;

        Ok(Dracoon {
            client,
            state: PhantomData,
        })
    }

    pub fn get_authorize_url(&mut self) -> String {
        self.client.get_authorize_url()
    }
}


impl Dracoon<Connected> {
    pub fn build_api_url(&self, url_part: &str) -> String {
        format!("{}{}", self.client.base_url, url_part)
    }
}

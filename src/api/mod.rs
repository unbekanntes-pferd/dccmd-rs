use self::{auth::{DracoonClient, DracoonClientBuilder}, errors::DracoonClientError};

pub mod nodes;
pub mod auth;
pub mod errors;

pub struct Dracoon {
    client: DracoonClient
}

pub struct DracoonBuilder {
    base_url: Option<String>,
    client_id: Option<String>,
    client_secret: Option<String>,
    redirect_uri: Option<String>,
    client_builder: DracoonClientBuilder
}

impl DracoonBuilder {
    
    pub fn new() -> Self {
        let client_builder = DracoonClientBuilder::new();
        Self { base_url: None, client_id: None, client_secret: None, redirect_uri: None, client_builder }
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

    pub fn build(self) -> Result<Dracoon, DracoonClientError> {

        let dracoon = self.client_builder.build()?;

        Ok(Dracoon { client: dracoon })

    }
}


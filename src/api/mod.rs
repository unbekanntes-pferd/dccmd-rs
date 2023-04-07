use std::marker::PhantomData;

use dco3_crypto::PlainUserKeyPairContainer;
use reqwest::Url;

use self::{
    auth::{errors::DracoonClientError, Connected, Disconnected, OAuth2Flow},
    auth::{DracoonClient, DracoonClientBuilder},
    user::{models::UserAccount, User, UserAccountKeypairs},
};

pub mod auth;
pub mod constants;
pub mod models;
pub mod nodes;
pub mod user;
pub mod utils;

pub struct Dracoon<State = Disconnected> {
    client: DracoonClient<State>,
    state: PhantomData<State>,
    user_info: Option<UserAccount>,
    keypair: Option<PlainUserKeyPairContainer>,
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
            user_info: None,
            keypair: None,
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
            user_info: None,
            keypair: None,
        })
    }

    pub fn get_authorize_url(&mut self) -> String {
        self.client.get_authorize_url()
    }
}

impl Dracoon<Connected> {
    pub fn build_api_url(&self, url_part: &str) -> Url {
        self.client
            .get_base_url()
            .join(url_part)
            .expect("Correct base url")
    }

    pub async fn get_auth_header(&self) -> Result<String, DracoonClientError> {
        self.client.get_auth_header().await
    }

    pub fn get_base_url(&self) -> &Url {
        self.client.get_base_url()
    }

    pub fn get_refresh_token(&self) -> &str {
        self.client.get_refresh_token()
    }

    pub async fn get_user_info(&mut self) -> Result<&UserAccount, DracoonClientError> {
        match self.user_info {
            Some(ref user_info) => Ok(user_info),
            None => {
                let user_info = self.get_user_account().await?;
                self.user_info = Some(user_info);
                Ok(self.user_info.as_ref().expect("Just set user info"))
            }
        }
    }

    pub async fn get_keypair(
        &mut self,
        secret: Option<&str>,
    ) -> Result<&PlainUserKeyPairContainer, DracoonClientError> {
        match self.keypair {
            Some(ref keypair) => Ok(keypair),
            None => {
                let secret = secret.ok_or(DracoonClientError::MissingEncryptionSecret)?;
                let keypair = self.get_user_keypair(secret).await?;
                self.keypair = Some(keypair);
                Ok(self.keypair.as_ref().expect("Just set keypair"))
            }
        }
    }
}

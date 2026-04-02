use async_trait::async_trait;
use dco3::{
    auth::{Connected, Disconnected, OAuth2Flow},
    Dracoon, DracoonBuilder, DracoonClientError,
};
use keyring::Entry;
use secrecy::{ExposeSecret, SecretString};
use tracing::{debug, error, info, warn};

use crate::core::{
    constants::{CLIENT_ID, CLIENT_SECRET},
    models::{DcCmdError, PasswordAuth},
    session::{AuthMode, EncryptionState, Session},
};

const SERVICE_NAME: &str = env!("CARGO_PKG_NAME");

pub struct AuthService<B = DracoonAuthBackend, S = KeyringSecretStore, P = DialoguerPrompts> {
    backend: B,
    store: S,
    prompts: P,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ConnectMode {
    Password,
    RefreshToken,
    AuthCode,
}

pub enum RefreshTokenConnectError {
    InvalidToken(String),
    Other(DcCmdError),
}

#[async_trait]
pub trait AuthBackend: Send + Sync {
    type Client: Clone + Send + Sync + 'static;

    async fn connect_password(
        &self,
        base_url: &str,
        is_transfer: bool,
        password_auth: &PasswordAuth,
    ) -> Result<Self::Client, DcCmdError>;

    async fn connect_refresh_token(
        &self,
        base_url: &str,
        is_transfer: bool,
        refresh_token: String,
    ) -> Result<Self::Client, RefreshTokenConnectError>;

    async fn connect_auth_code(
        &self,
        base_url: &str,
        is_transfer: bool,
        auth_code: String,
    ) -> Result<Self::Client, DcCmdError>;

    async fn get_refresh_token(&self, client: &Self::Client) -> Result<String, DcCmdError>;

    async fn verify_encryption_secret(
        &self,
        client: &Self::Client,
        secret: &SecretString,
    ) -> Result<(), DcCmdError>;

    fn authorize_url(&self, base_url: &str, is_transfer: bool) -> Result<String, DcCmdError>;
}

pub trait SecretStore: Send + Sync {
    fn get_refresh_token(&self, base_url: &str) -> Result<String, DcCmdError>;
    fn set_refresh_token(&self, base_url: &str, refresh_token: &str) -> Result<(), DcCmdError>;
    fn delete_refresh_token(&self, base_url: &str) -> Result<(), DcCmdError>;

    fn get_encryption_secret(&self, account: &str) -> Result<String, DcCmdError>;
    fn set_encryption_secret(&self, account: &str, secret: &str) -> Result<(), DcCmdError>;
    fn delete_encryption_secret(&self, account: &str) -> Result<(), DcCmdError>;
}

pub trait Prompts: Send + Sync {
    fn ask_auth_code(&self, authorize_url: &str) -> Result<String, DcCmdError>;
    fn ask_encryption_secret(&self) -> Result<SecretString, DcCmdError>;
}

pub struct DracoonAuthBackend;
pub struct KeyringSecretStore;
pub struct DialoguerPrompts;

impl AuthService<DracoonAuthBackend, KeyringSecretStore, DialoguerPrompts> {
    pub fn new() -> Self {
        Self {
            backend: DracoonAuthBackend,
            store: KeyringSecretStore,
            prompts: DialoguerPrompts,
        }
    }

    pub async fn init_public(&self, url_path: &str) -> Result<Dracoon<Disconnected>, DcCmdError> {
        let base_url = Self::parse_base_url(url_path.to_string())?;

        let dccmd_user_agent = format!("{}|{}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));

        let dracoon = DracoonBuilder::new()
            .with_base_url(base_url)
            .with_client_id(CLIENT_ID)
            .with_client_secret(CLIENT_SECRET)
            .with_user_agent(dccmd_user_agent)
            .build()?;

        Ok(dracoon)
    }
}

impl<B, S, P> AuthService<B, S, P>
where
    B: AuthBackend,
    S: SecretStore,
    P: Prompts,
{
    #[cfg(test)]
    pub fn with_dependencies(backend: B, store: S, prompts: P) -> Self {
        Self {
            backend,
            store,
            prompts,
        }
    }

    pub async fn connect(
        &self,
        url_path: &str,
        password_auth: Option<PasswordAuth>,
        is_transfer: bool,
    ) -> Result<Session<B::Client>, DcCmdError> {
        let base_url = Self::parse_base_url(url_path.to_string())?;

        if let Some(auth) = password_auth {
            let client = self
                .backend
                .connect_password(&base_url, is_transfer, &auth)
                .await?;
            return Ok(Session::new(base_url, AuthMode::PasswordFlow, client));
        }

        let refresh_token = match self.store.get_refresh_token(&base_url) {
            Ok(token) => Some(token),
            Err(DcCmdError::InvalidAccount) => None,
            Err(e) => return Err(e),
        };

        match Self::resolve_connect_mode(false, refresh_token.is_some()) {
            ConnectMode::RefreshToken => {
                let refresh_token = refresh_token.expect("refresh token checked as present");
                if let Some(session) = self
                    .try_connect_with_refresh_token(&base_url, is_transfer, refresh_token)
                    .await?
                {
                    Ok(session)
                } else {
                    self.connect_with_auth_code_flow(&base_url, is_transfer)
                        .await
                }
            }
            ConnectMode::AuthCode => {
                info!("No stored refresh token for {base_url}; starting auth code flow.");
                self.connect_with_auth_code_flow(&base_url, is_transfer)
                    .await
            }
            ConnectMode::Password => unreachable!("password branch handled before keyring flow"),
        }
    }

    pub async fn connect_client(
        &self,
        url_path: &str,
        password_auth: Option<PasswordAuth>,
        is_transfer: bool,
    ) -> Result<B::Client, DcCmdError> {
        let session = self.connect(url_path, password_auth, is_transfer).await?;
        debug!("Connected with auth mode: {:?}", session.auth_mode());
        Ok(session.into_client())
    }

    pub async fn connect_with_refresh_token(
        &self,
        base_url: &str,
        refresh_token: String,
    ) -> Result<Session<B::Client>, DcCmdError> {
        let client = self
            .backend
            .connect_refresh_token(base_url, false, refresh_token)
            .await
            .map_err(|e| match e {
                RefreshTokenConnectError::InvalidToken(reason) => {
                    let _ = self.store.delete_refresh_token(base_url);
                    warn!("Stored refresh token rejected for {base_url}: {reason}");
                    DcCmdError::InvalidAccount
                }
                RefreshTokenConnectError::Other(err) => err,
            })?;

        self.persist_refreshed_token(base_url, &client).await;

        Ok(Session::new(
            base_url.to_string(),
            AuthMode::RefreshToken,
            client,
        ))
    }

    pub async fn ensure_encryption(
        &self,
        session: Session<B::Client>,
        encryption_password: Option<SecretString>,
    ) -> Result<Session<B::Client>, DcCmdError> {
        let account = format!("{}-crypto", session.base_url());

        let (secret, persist) = match encryption_password {
            Some(password) => (password, false),
            None => match self.store.get_encryption_secret(&account) {
                Ok(stored_secret) => (SecretString::new(stored_secret.into()), false),
                Err(DcCmdError::InvalidAccount) => (self.prompts.ask_encryption_secret()?, true),
                Err(DcCmdError::CredentialStorageFailed) => {
                    (self.prompts.ask_encryption_secret()?, false)
                }
                Err(e) => return Err(e),
            },
        };
        let client = session.client().clone();

        self.backend
            .verify_encryption_secret(&client, &secret)
            .await
            .map_err(|e| {
                error!("Error getting keypair: {}", e);
                debug!("Wrong credentials?");
                e
            })?;

        if persist {
            self.store
                .set_encryption_secret(&account, secret.expose_secret())?;
        }

        Ok(session.with_client(client, EncryptionState::Unlocked))
    }

    pub async fn ensure_encryption_client(
        &self,
        base_url: String,
        client: B::Client,
        encryption_password: Option<SecretString>,
    ) -> Result<B::Client, DcCmdError> {
        let session = Session::new(base_url, AuthMode::RefreshToken, client);
        let session = self.ensure_encryption(session, encryption_password).await?;
        debug!(
            "Session encryption state after initialization: {:?}",
            session.encryption_state()
        );
        Ok(session.into_client())
    }

    pub fn parse_base_url(url_str: String) -> Result<String, DcCmdError> {
        if url_str.starts_with("http://") {
            #[cfg(not(test))]
            {
                error!("HTTP is not supported.");
                return Err(DcCmdError::InvalidUrl(url_str));
            }
        };

        let (scheme, without_scheme) = if let Some(rest) = url_str.strip_prefix("https://") {
            ("https://", rest)
        } else if let Some(rest) = url_str.strip_prefix("http://") {
            ("http://", rest)
        } else {
            ("https://", url_str.as_str())
        };

        let uri_fragments: Vec<&str> = without_scheme.split('/').collect();

        match uri_fragments.len() {
            2.. => Ok(format!("{scheme}{}", uri_fragments[0])),
            _ => Err(DcCmdError::InvalidUrl(url_str)),
        }
    }

    fn resolve_connect_mode(has_password_auth: bool, has_refresh_token: bool) -> ConnectMode {
        if has_password_auth {
            ConnectMode::Password
        } else if has_refresh_token {
            ConnectMode::RefreshToken
        } else {
            ConnectMode::AuthCode
        }
    }
    async fn try_connect_with_refresh_token(
        &self,
        base_url: &str,
        is_transfer: bool,
        refresh_token: String,
    ) -> Result<Option<Session<B::Client>>, DcCmdError> {
        match self
            .backend
            .connect_refresh_token(base_url, is_transfer, refresh_token)
            .await
        {
            Ok(client) => {
                self.persist_refreshed_token(base_url, &client).await;

                Ok(Some(Session::new(
                    base_url.to_string(),
                    AuthMode::RefreshToken,
                    client,
                )))
            }
            Err(RefreshTokenConnectError::InvalidToken(reason)) => {
                let _ = self.store.delete_refresh_token(base_url);
                warn!("Stored refresh token rejected for {base_url}: {reason}");
                info!("Falling back to auth code flow for {base_url}.");
                Ok(None)
            }
            Err(RefreshTokenConnectError::Other(err)) => {
                error!("Error connecting with refresh token: {}", err);
                info!("Falling back to auth code flow for {base_url}.");
                Ok(None)
            }
        }
    }

    async fn persist_refreshed_token(&self, base_url: &str, client: &B::Client) {
        match self.backend.get_refresh_token(client).await {
            Ok(refreshed_token) => {
                if let Err(err) = self.store.set_refresh_token(base_url, &refreshed_token) {
                    debug!("Error storing refresh token: {}", err);
                    error!("Failed to store refresh token.");
                }
            }
            Err(err) => {
                debug!("Error retrieving refreshed token: {}", err);
                error!("Failed to retrieve refresh token.");
            }
        }
    }

    async fn connect_with_auth_code_flow(
        &self,
        base_url: &str,
        is_transfer: bool,
    ) -> Result<Session<B::Client>, DcCmdError> {
        let authorize_url = self.backend.authorize_url(base_url, is_transfer)?;
        let auth_code = self.prompts.ask_auth_code(&authorize_url)?;

        let client = self
            .backend
            .connect_auth_code(base_url, is_transfer, auth_code)
            .await?;

        let refresh_token = self.backend.get_refresh_token(&client).await?;
        self.store.set_refresh_token(base_url, &refresh_token)?;

        Ok(Session::new(
            base_url.to_string(),
            AuthMode::AuthCode,
            client,
        ))
    }
}

#[async_trait]
impl AuthBackend for DracoonAuthBackend {
    type Client = Dracoon<Connected>;

    async fn connect_password(
        &self,
        base_url: &str,
        is_transfer: bool,
        password_auth: &PasswordAuth,
    ) -> Result<Self::Client, DcCmdError> {
        let client = build_disconnected_client(base_url, is_transfer)?;

        client
            .connect(OAuth2Flow::password_flow(
                password_auth.username().to_string(),
                password_auth.password_str().to_string(),
            ))
            .await
            .map_err(Into::into)
    }

    async fn connect_refresh_token(
        &self,
        base_url: &str,
        is_transfer: bool,
        refresh_token: String,
    ) -> Result<Self::Client, RefreshTokenConnectError> {
        let client = build_disconnected_client(base_url, is_transfer)
            .map_err(RefreshTokenConnectError::Other)?;

        client
            .connect(OAuth2Flow::RefreshToken(refresh_token))
            .await
            .map_err(|e| {
                let reason = e.to_string();
                match e {
                    DracoonClientError::Http(res) if res.is_bad_request() => {
                        RefreshTokenConnectError::InvalidToken(reason)
                    }
                    DracoonClientError::Auth(_) => RefreshTokenConnectError::InvalidToken(reason),
                    _ => RefreshTokenConnectError::Other((&e).into()),
                }
            })
    }

    async fn connect_auth_code(
        &self,
        base_url: &str,
        is_transfer: bool,
        auth_code: String,
    ) -> Result<Self::Client, DcCmdError> {
        let client = build_disconnected_client(base_url, is_transfer)?;

        client
            .connect(OAuth2Flow::AuthCodeFlow(auth_code.trim_end().into()))
            .await
            .map_err(Into::into)
    }

    async fn get_refresh_token(&self, client: &Self::Client) -> Result<String, DcCmdError> {
        Ok(client.get_refresh_token().await)
    }

    async fn verify_encryption_secret(
        &self,
        client: &Self::Client,
        secret: &SecretString,
    ) -> Result<(), DcCmdError> {
        client
            .get_keypair(Some(secret.expose_secret().to_string()))
            .await
            .map_err(Into::into)
            .map(|_| ())
    }

    fn authorize_url(&self, base_url: &str, is_transfer: bool) -> Result<String, DcCmdError> {
        Ok(build_disconnected_client(base_url, is_transfer)?.get_authorize_url())
    }
}

impl SecretStore for KeyringSecretStore {
    fn get_refresh_token(&self, base_url: &str) -> Result<String, DcCmdError> {
        let entry = keyring_entry(base_url)?;
        entry.get_password().map_err(|_| DcCmdError::InvalidAccount)
    }

    fn set_refresh_token(&self, base_url: &str, refresh_token: &str) -> Result<(), DcCmdError> {
        let entry = keyring_entry(base_url)?;
        entry
            .set_password(refresh_token)
            .map_err(|_| DcCmdError::CredentialStorageFailed)
    }

    fn delete_refresh_token(&self, base_url: &str) -> Result<(), DcCmdError> {
        delete_keyring_secret(base_url)
    }

    fn get_encryption_secret(&self, account: &str) -> Result<String, DcCmdError> {
        let entry = keyring_entry(account)?;
        entry.get_password().map_err(|_| DcCmdError::InvalidAccount)
    }

    fn set_encryption_secret(&self, account: &str, secret: &str) -> Result<(), DcCmdError> {
        let entry = keyring_entry(account)?;
        entry
            .set_password(secret)
            .map_err(|_| DcCmdError::CredentialStorageFailed)
    }

    fn delete_encryption_secret(&self, account: &str) -> Result<(), DcCmdError> {
        delete_keyring_secret(account)
    }
}

impl Prompts for DialoguerPrompts {
    fn ask_auth_code(&self, authorize_url: &str) -> Result<String, DcCmdError> {
        println!("Please log in via browser (open url): ");
        println!("{}", authorize_url);

        dialoguer::Password::new()
            .with_prompt("Please enter authorization code")
            .interact()
            .or(Err(DcCmdError::IoError))
    }

    fn ask_encryption_secret(&self) -> Result<SecretString, DcCmdError> {
        dialoguer::Password::new()
            .with_prompt("Please enter your encryption secret")
            .interact()
            .map(|secret| SecretString::new(secret.into()))
            .or(Err(DcCmdError::IoError))
    }
}

fn build_disconnected_client(
    base_url: &str,
    is_transfer: bool,
) -> Result<Dracoon<Disconnected>, DcCmdError> {
    let token_rotation = if is_transfer { 5 } else { 1 };
    let dccmd_user_agent = format!("{}|{}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));

    DracoonBuilder::new()
        .with_base_url(base_url)
        .with_client_id(CLIENT_ID)
        .with_client_secret(CLIENT_SECRET)
        .with_token_rotation(token_rotation)
        .with_user_agent(dccmd_user_agent)
        .build()
        .map_err(Into::into)
}

fn keyring_entry(account: &str) -> Result<Entry, DcCmdError> {
    Entry::new(SERVICE_NAME, account).map_err(|_| DcCmdError::CredentialStorageFailed)
}

fn delete_keyring_secret(account: &str) -> Result<(), DcCmdError> {
    let entry = keyring_entry(account)?;
    if entry.get_password().is_err() {
        return Err(DcCmdError::InvalidAccount);
    }

    entry
        .delete_credential()
        .map_err(|_| DcCmdError::CredentialDeletionFailed)
}

#[cfg(test)]
mod tests;

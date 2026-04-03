use async_trait::async_trait;
use dco3::{
    auth::{Connected, Disconnected, OAuth2Flow},
    Dracoon, DracoonBuilder, DracoonClientError,
};
#[cfg(target_os = "linux")]
use keyring::{keyutils::KeyutilsCredential, secret_service::SsCredential};
use keyring::{Entry, Error as KeyringError};
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StoredSecretLookup {
    Found(String),
    Missing,
    Unavailable,
}

impl StoredSecretLookup {
    fn into_result(self) -> Result<String, DcCmdError> {
        match self {
            Self::Found(secret) => Ok(secret),
            Self::Missing => Err(DcCmdError::InvalidAccount),
            Self::Unavailable => Err(DcCmdError::CredentialStorageFailed),
        }
    }
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
    fn lookup_refresh_token(&self, base_url: &str) -> Result<StoredSecretLookup, DcCmdError>;
    fn get_refresh_token(&self, base_url: &str) -> Result<String, DcCmdError>;
    fn set_refresh_token(&self, base_url: &str, refresh_token: &str) -> Result<(), DcCmdError>;
    fn delete_refresh_token(&self, base_url: &str) -> Result<(), DcCmdError>;

    fn lookup_encryption_secret(&self, account: &str) -> Result<StoredSecretLookup, DcCmdError>;
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

        let refresh_lookup = match self.store.lookup_refresh_token(&base_url) {
            Ok(lookup) => lookup,
            Err(e) => return Err(e),
        };

        match Self::resolve_connect_mode(
            false,
            matches!(refresh_lookup, StoredSecretLookup::Found(_)),
        ) {
            ConnectMode::RefreshToken => {
                let StoredSecretLookup::Found(refresh_token) = refresh_lookup else {
                    unreachable!("refresh token checked as present");
                };
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
                match refresh_lookup {
                    StoredSecretLookup::Missing => {
                        info!("No stored refresh token for {base_url}; starting auth code flow.");
                    }
                    StoredSecretLookup::Unavailable => {
                        warn!(
                            "Stored refresh token for {base_url} is unavailable due to local credential storage access failures; starting auth code flow."
                        );
                    }
                    StoredSecretLookup::Found(_) => {
                        unreachable!("auth code flow selected without stored refresh token")
                    }
                }
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
            None => match self.store.lookup_encryption_secret(&account) {
                Ok(StoredSecretLookup::Found(stored_secret)) => {
                    (SecretString::new(stored_secret.into()), false)
                }
                Ok(StoredSecretLookup::Missing) => (self.prompts.ask_encryption_secret()?, true),
                Ok(StoredSecretLookup::Unavailable) => {
                    warn!(
                        "Stored encryption secret for {account} is unavailable due to local credential storage access failures; prompting again."
                    );
                    (self.prompts.ask_encryption_secret()?, true)
                }
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
    fn lookup_refresh_token(&self, base_url: &str) -> Result<StoredSecretLookup, DcCmdError> {
        lookup_secret(base_url, "refresh token")
    }

    fn get_refresh_token(&self, base_url: &str) -> Result<String, DcCmdError> {
        self.lookup_refresh_token(base_url)?.into_result()
    }

    fn set_refresh_token(&self, base_url: &str, refresh_token: &str) -> Result<(), DcCmdError> {
        store_secret(base_url, refresh_token, "refresh token")
    }

    fn delete_refresh_token(&self, base_url: &str) -> Result<(), DcCmdError> {
        delete_stored_secret(base_url, "refresh token")
    }

    fn lookup_encryption_secret(&self, account: &str) -> Result<StoredSecretLookup, DcCmdError> {
        lookup_secret(account, "encryption secret")
    }

    fn get_encryption_secret(&self, account: &str) -> Result<String, DcCmdError> {
        self.lookup_encryption_secret(account)?.into_result()
    }

    fn set_encryption_secret(&self, account: &str, secret: &str) -> Result<(), DcCmdError> {
        store_secret(account, secret, "encryption secret")
    }

    fn delete_encryption_secret(&self, account: &str) -> Result<(), DcCmdError> {
        delete_stored_secret(account, "encryption secret")
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

#[cfg(target_os = "linux")]
fn lookup_secret(account: &str, secret_label: &str) -> Result<StoredSecretLookup, DcCmdError> {
    let secret_service = match secret_service_entry(account) {
        Ok(entry) => Some(entry),
        Err(err) => {
            warn!(
                "Failed to create persistent Secret Service {secret_label} entry for {account}: {err}; trying keyutils fallback."
            );
            None
        }
    };

    if let Some(entry) = secret_service {
        match entry.get_password() {
            Ok(secret) => {
                warm_keyutils_secret(account, secret_label, &secret);
                return Ok(StoredSecretLookup::Found(secret));
            }
            Err(KeyringError::NoEntry) => {}
            Err(err) => {
                warn!(
                    "Failed to access persistent Secret Service {secret_label} for {account}: {err}; trying keyutils fallback."
                );
                return lookup_keyutils_fallback(account, secret_label, true);
            }
        }
    }

    lookup_keyutils_fallback(account, secret_label, false)
}

#[cfg(not(target_os = "linux"))]
fn lookup_secret(account: &str, _secret_label: &str) -> Result<StoredSecretLookup, DcCmdError> {
    match keyring_entry(account)
        .map_err(|_| DcCmdError::CredentialStorageFailed)?
        .get_password()
    {
        Ok(secret) => Ok(StoredSecretLookup::Found(secret)),
        Err(KeyringError::NoEntry) => Ok(StoredSecretLookup::Missing),
        Err(err) => {
            debug!("Error reading stored secret for {account}: {err}");
            Err(DcCmdError::CredentialStorageFailed)
        }
    }
}

#[cfg(target_os = "linux")]
fn store_secret(account: &str, secret: &str, secret_label: &str) -> Result<(), DcCmdError> {
    let secret_service_result = secret_service_entry(account)
        .and_then(|entry| entry.set_password(secret))
        .map_err(|err| {
            debug!("Failed to store {secret_label} for {account} in Secret Service: {err}");
            err
        });
    let keyutils_result = keyutils_entry(account)
        .and_then(|entry| entry.set_password(secret))
        .map_err(|err| {
            debug!("Failed to store {secret_label} for {account} in keyutils fallback: {err}");
            err
        });

    match (secret_service_result, keyutils_result) {
        (Ok(()), Ok(())) | (Ok(()), Err(_)) => Ok(()),
        (Err(secret_service_err), Ok(())) => {
            warn!(
                "Failed to persist {secret_label} for {account} in Secret Service: {secret_service_err}; stored only in keyutils fallback."
            );
            Ok(())
        }
        (Err(_), Err(_)) => Err(DcCmdError::CredentialStorageFailed),
    }
}

#[cfg(not(target_os = "linux"))]
fn store_secret(account: &str, secret: &str, _secret_label: &str) -> Result<(), DcCmdError> {
    keyring_entry(account)
        .map_err(|_| DcCmdError::CredentialStorageFailed)?
        .set_password(secret)
        .map_err(|_| DcCmdError::CredentialStorageFailed)
}

#[cfg(target_os = "linux")]
fn delete_stored_secret(account: &str, secret_label: &str) -> Result<(), DcCmdError> {
    let secret_service_result = delete_entry(
        secret_service_entry(account),
        "Secret Service",
        account,
        secret_label,
    );
    let keyutils_result = delete_entry(
        keyutils_entry(account),
        "keyutils fallback",
        account,
        secret_label,
    );

    if matches!(secret_service_result, DeleteOutcome::Missing)
        && matches!(keyutils_result, DeleteOutcome::Missing)
    {
        return Err(DcCmdError::InvalidAccount);
    }

    if matches!(secret_service_result, DeleteOutcome::Failed)
        || matches!(keyutils_result, DeleteOutcome::Failed)
    {
        return Err(DcCmdError::CredentialDeletionFailed);
    }

    Ok(())
}

#[cfg(not(target_os = "linux"))]
fn delete_stored_secret(account: &str, _secret_label: &str) -> Result<(), DcCmdError> {
    match keyring_entry(account)
        .map_err(|_| DcCmdError::CredentialStorageFailed)?
        .delete_credential()
    {
        Ok(()) => Ok(()),
        Err(KeyringError::NoEntry) => Err(DcCmdError::InvalidAccount),
        Err(_) => Err(DcCmdError::CredentialDeletionFailed),
    }
}

#[cfg(not(target_os = "linux"))]
fn keyring_entry(account: &str) -> Result<Entry, KeyringError> {
    Entry::new(SERVICE_NAME, account)
}

#[cfg(target_os = "linux")]
fn secret_service_entry(account: &str) -> Result<Entry, KeyringError> {
    Ok(Entry::new_with_credential(Box::new(
        SsCredential::new_with_target(None, SERVICE_NAME, account)?,
    )))
}

#[cfg(target_os = "linux")]
fn keyutils_entry(account: &str) -> Result<Entry, KeyringError> {
    Ok(Entry::new_with_credential(Box::new(
        KeyutilsCredential::new_with_target(None, SERVICE_NAME, account)?,
    )))
}

#[cfg(target_os = "linux")]
fn warm_keyutils_secret(account: &str, secret_label: &str, secret: &str) {
    match keyutils_entry(account) {
        Ok(entry) => {
            if let Err(err) = entry.set_password(secret) {
                debug!("Failed to warm keyutils {secret_label} for {account}: {err}");
            }
        }
        Err(err) => {
            debug!("Failed to create keyutils entry for {secret_label} {account}: {err}");
        }
    }
}

#[cfg(target_os = "linux")]
fn lookup_keyutils_fallback(
    account: &str,
    secret_label: &str,
    after_secret_service_failure: bool,
) -> Result<StoredSecretLookup, DcCmdError> {
    let entry = match keyutils_entry(account) {
        Ok(entry) => entry,
        Err(err) => {
            warn!("Failed to create keyutils entry for {secret_label} {account}: {err}");
            return Ok(StoredSecretLookup::Unavailable);
        }
    };

    match entry.get_password() {
        Ok(secret) => {
            if after_secret_service_failure {
                info!("Using keyutils fallback {secret_label} for {account}.");
            } else {
                info!(
                    "Using keyutils fallback {secret_label} for {account}; no persistent Secret Service entry was found."
                );
            }
            Ok(StoredSecretLookup::Found(secret))
        }
        Err(KeyringError::NoEntry) if after_secret_service_failure => {
            warn!(
                "No keyutils fallback {secret_label} was available for {account} after Secret Service access failed."
            );
            Ok(StoredSecretLookup::Unavailable)
        }
        Err(KeyringError::NoEntry) => Ok(StoredSecretLookup::Missing),
        Err(err) => {
            warn!("Failed to access keyutils fallback {secret_label} for {account}: {err}");
            Ok(StoredSecretLookup::Unavailable)
        }
    }
}

#[cfg(target_os = "linux")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DeleteOutcome {
    Deleted,
    Missing,
    Failed,
}

#[cfg(target_os = "linux")]
fn delete_entry(
    entry_result: Result<Entry, KeyringError>,
    backend_label: &str,
    account: &str,
    secret_label: &str,
) -> DeleteOutcome {
    match entry_result {
        Ok(entry) => match entry.delete_credential() {
            Ok(()) => DeleteOutcome::Deleted,
            Err(KeyringError::NoEntry) => DeleteOutcome::Missing,
            Err(err) => {
                warn!("Failed to delete {secret_label} for {account} from {backend_label}: {err}");
                DeleteOutcome::Failed
            }
        },
        Err(err) => {
            warn!("Failed to create {backend_label} entry for {secret_label} {account}: {err}");
            DeleteOutcome::Failed
        }
    }
}

#[cfg(test)]
mod tests;

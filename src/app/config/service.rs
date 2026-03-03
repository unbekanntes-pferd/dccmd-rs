use crate::{
    app::{
        auth::{
            AuthService, DialoguerPrompts, DracoonAuthBackend, KeyringSecretStore, SecretStore,
        },
        config::api::{RefreshTokenInfo, SystemApi, SystemInfo},
    },
    core::models::DcCmdError,
};

pub struct ConfigService<S = KeyringSecretStore> {
    auth_service: AuthService,
    store: S,
}

type DefaultAuthService = AuthService<DracoonAuthBackend, KeyringSecretStore, DialoguerPrompts>;

impl ConfigService<KeyringSecretStore> {
    pub fn new() -> Self {
        Self {
            auth_service: AuthService::new(),
            store: KeyringSecretStore,
        }
    }
}

impl<S> ConfigService<S>
where
    S: SecretStore,
{
    #[cfg(test)]
    pub fn with_store(store: S) -> Self {
        Self {
            auth_service: AuthService::new(),
            store,
        }
    }

    pub fn normalize_base_url(&self, target: &str) -> Result<String, DcCmdError> {
        let target = target.trim();
        let with_scheme = if target.starts_with("https://") || target.starts_with("http://") {
            target.to_string()
        } else {
            format!("https://{target}")
        };

        // AuthService parsing expects a URL path segment. Appending a dummy segment keeps
        // host-only inputs valid while still returning the normalized host URL.
        DefaultAuthService::parse_base_url(format!("{}/_", with_scheme.trim_end_matches('/')))
    }

    pub fn encryption_account(&self, target: &str) -> Result<String, DcCmdError> {
        Ok(format!("{}-crypto", self.normalize_base_url(target)?))
    }

    pub async fn get_refresh_token_info(
        &self,
        target: &str,
    ) -> Result<RefreshTokenInfo, DcCmdError> {
        let api_client = self.connect_with_refresh_token(target).await?;
        SystemApi::get_refresh_token_info(&api_client).await
    }

    pub async fn get_system_info(&self, target: &str) -> Result<SystemInfo, DcCmdError> {
        let api_client = self.connect_with_refresh_token(target).await?;
        SystemApi::get_system_info(&api_client).await
    }

    pub fn has_encryption_secret(&self, target: &str) -> Result<bool, DcCmdError> {
        let account = self.encryption_account(target)?;
        match self.store.get_encryption_secret(&account) {
            Ok(_) => Ok(true),
            Err(DcCmdError::InvalidAccount) => Ok(false),
            Err(e) => Err(e),
        }
    }

    pub fn remove_refresh_token(&self, target: &str) -> Result<(), DcCmdError> {
        let base_url = self.normalize_base_url(target)?;
        self.store.delete_refresh_token(&base_url)
    }

    pub fn remove_encryption_secret(&self, target: &str) -> Result<(), DcCmdError> {
        let account = self.encryption_account(target)?;
        self.store.delete_encryption_secret(&account)
    }

    async fn connect_with_refresh_token(
        &self,
        target: &str,
    ) -> Result<dco3::Dracoon<dco3::auth::Connected>, DcCmdError> {
        let base_url = self.normalize_base_url(target)?;
        let refresh_token = self.store.get_refresh_token(&base_url)?;

        let session = self
            .auth_service
            .connect_with_refresh_token(&base_url, refresh_token)
            .await?;

        Ok(session.into_client())
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashMap,
        sync::{Arc, Mutex},
    };

    use super::*;

    #[derive(Default)]
    struct StoreState {
        refresh_tokens: Mutex<HashMap<String, String>>,
        encryption_secrets: Mutex<HashMap<String, String>>,
        deleted_refresh_accounts: Mutex<Vec<String>>,
        deleted_encryption_accounts: Mutex<Vec<String>>,
    }

    #[derive(Clone, Default)]
    struct InMemoryStore {
        state: Arc<StoreState>,
    }

    impl InMemoryStore {
        fn with_encryption_secret(account: &str, secret: &str) -> Self {
            let mut encryption_secrets = HashMap::new();
            encryption_secrets.insert(account.to_string(), secret.to_string());
            Self {
                state: Arc::new(StoreState {
                    refresh_tokens: Mutex::new(HashMap::new()),
                    encryption_secrets: Mutex::new(encryption_secrets),
                    deleted_refresh_accounts: Mutex::new(Vec::new()),
                    deleted_encryption_accounts: Mutex::new(Vec::new()),
                }),
            }
        }

        fn deleted_refresh_accounts(&self) -> Vec<String> {
            self.state
                .deleted_refresh_accounts
                .lock()
                .expect("lock poisoned")
                .clone()
        }

        fn deleted_encryption_accounts(&self) -> Vec<String> {
            self.state
                .deleted_encryption_accounts
                .lock()
                .expect("lock poisoned")
                .clone()
        }
    }

    impl SecretStore for InMemoryStore {
        fn get_refresh_token(&self, base_url: &str) -> Result<String, DcCmdError> {
            self.state
                .refresh_tokens
                .lock()
                .expect("lock poisoned")
                .get(base_url)
                .cloned()
                .ok_or(DcCmdError::InvalidAccount)
        }

        fn set_refresh_token(
            &self,
            _base_url: &str,
            _refresh_token: &str,
        ) -> Result<(), DcCmdError> {
            Ok(())
        }

        fn delete_refresh_token(&self, base_url: &str) -> Result<(), DcCmdError> {
            self.state
                .deleted_refresh_accounts
                .lock()
                .expect("lock poisoned")
                .push(base_url.to_string());
            Ok(())
        }

        fn get_encryption_secret(&self, account: &str) -> Result<String, DcCmdError> {
            self.state
                .encryption_secrets
                .lock()
                .expect("lock poisoned")
                .get(account)
                .cloned()
                .ok_or(DcCmdError::InvalidAccount)
        }

        fn set_encryption_secret(&self, _account: &str, _secret: &str) -> Result<(), DcCmdError> {
            Ok(())
        }

        fn delete_encryption_secret(&self, account: &str) -> Result<(), DcCmdError> {
            self.state
                .deleted_encryption_accounts
                .lock()
                .expect("lock poisoned")
                .push(account.to_string());
            Ok(())
        }
    }

    #[test]
    fn test_normalize_base_url_strips_path_and_scheme() {
        let service = ConfigService::with_store(InMemoryStore::default());

        let normalized = service
            .normalize_base_url("dracoon.example.com/some/path")
            .unwrap();

        assert_eq!(normalized, "https://dracoon.example.com");
    }

    #[test]
    fn test_has_encryption_secret_returns_false_when_missing() {
        let service = ConfigService::with_store(InMemoryStore::default());

        let has_secret = service
            .has_encryption_secret("dracoon.example.com")
            .unwrap();

        assert!(!has_secret);
    }

    #[test]
    fn test_has_encryption_secret_returns_true_when_present() {
        let store =
            InMemoryStore::with_encryption_secret("https://dracoon.example.com-crypto", "secret");
        let service = ConfigService::with_store(store);

        let has_secret = service
            .has_encryption_secret("dracoon.example.com")
            .unwrap();

        assert!(has_secret);
    }

    #[test]
    fn test_remove_refresh_token_uses_normalized_base_url() {
        let store = InMemoryStore::default();
        let view = store.clone();
        let service = ConfigService::with_store(store);

        service
            .remove_refresh_token("dracoon.example.com/some/path")
            .unwrap();

        assert_eq!(
            view.deleted_refresh_accounts(),
            vec!["https://dracoon.example.com".to_string()]
        );
    }

    #[test]
    fn test_remove_encryption_secret_uses_crypto_account() {
        let store = InMemoryStore::default();
        let view = store.clone();
        let service = ConfigService::with_store(store);

        service
            .remove_encryption_secret("dracoon.example.com")
            .unwrap();

        assert_eq!(
            view.deleted_encryption_accounts(),
            vec!["https://dracoon.example.com-crypto".to_string()]
        );
    }
}

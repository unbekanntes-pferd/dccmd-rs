use crate::{
    app::auth::{
        DialoguerPrompts, DracoonAuthBackend, KeyringSecretStore, SecretStore, StoredSecretLookup,
    },
    core::models::DcCmdError,
};

pub struct ConfigService<S = KeyringSecretStore> {
    store: S,
}

type DefaultAuthService =
    crate::app::auth::AuthService<DracoonAuthBackend, KeyringSecretStore, DialoguerPrompts>;

impl ConfigService<KeyringSecretStore> {
    pub fn new() -> Self {
        Self {
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
        Self { store }
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

    pub fn has_encryption_secret(&self, target: &str) -> Result<bool, DcCmdError> {
        let account = self.encryption_account(target)?;
        match self.store.lookup_encryption_secret(&account)? {
            StoredSecretLookup::Found(_) => Ok(true),
            StoredSecretLookup::Missing => Ok(false),
            StoredSecretLookup::Unavailable => Err(DcCmdError::CredentialStorageFailed),
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

    pub fn get_refresh_token(&self, target: &str) -> Result<String, DcCmdError> {
        let base_url = self.normalize_base_url(target)?;
        match self.store.lookup_refresh_token(&base_url)? {
            StoredSecretLookup::Found(refresh_token) => Ok(refresh_token),
            StoredSecretLookup::Missing => Err(DcCmdError::InvalidAccount),
            StoredSecretLookup::Unavailable => Err(DcCmdError::CredentialStorageFailed),
        }
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
        unavailable_refresh_accounts: Mutex<Vec<String>>,
        unavailable_encryption_accounts: Mutex<Vec<String>>,
        deleted_refresh_accounts: Mutex<Vec<String>>,
        deleted_encryption_accounts: Mutex<Vec<String>>,
    }

    #[derive(Clone, Default)]
    struct InMemoryStore {
        state: Arc<StoreState>,
    }

    impl InMemoryStore {
        fn with_refresh_token(base_url: &str, refresh_token: &str) -> Self {
            let mut refresh_tokens = HashMap::new();
            refresh_tokens.insert(base_url.to_string(), refresh_token.to_string());
            Self {
                state: Arc::new(StoreState {
                    refresh_tokens: Mutex::new(refresh_tokens),
                    encryption_secrets: Mutex::new(HashMap::new()),
                    unavailable_refresh_accounts: Mutex::new(Vec::new()),
                    unavailable_encryption_accounts: Mutex::new(Vec::new()),
                    deleted_refresh_accounts: Mutex::new(Vec::new()),
                    deleted_encryption_accounts: Mutex::new(Vec::new()),
                }),
            }
        }

        fn with_encryption_secret(account: &str, secret: &str) -> Self {
            let mut encryption_secrets = HashMap::new();
            encryption_secrets.insert(account.to_string(), secret.to_string());
            Self {
                state: Arc::new(StoreState {
                    refresh_tokens: Mutex::new(HashMap::new()),
                    encryption_secrets: Mutex::new(encryption_secrets),
                    unavailable_refresh_accounts: Mutex::new(Vec::new()),
                    unavailable_encryption_accounts: Mutex::new(Vec::new()),
                    deleted_refresh_accounts: Mutex::new(Vec::new()),
                    deleted_encryption_accounts: Mutex::new(Vec::new()),
                }),
            }
        }

        fn with_unavailable_refresh_token(base_url: &str) -> Self {
            Self {
                state: Arc::new(StoreState {
                    refresh_tokens: Mutex::new(HashMap::new()),
                    encryption_secrets: Mutex::new(HashMap::new()),
                    unavailable_refresh_accounts: Mutex::new(vec![base_url.to_string()]),
                    unavailable_encryption_accounts: Mutex::new(Vec::new()),
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
        fn lookup_refresh_token(&self, base_url: &str) -> Result<StoredSecretLookup, DcCmdError> {
            if self
                .state
                .unavailable_refresh_accounts
                .lock()
                .expect("lock poisoned")
                .iter()
                .any(|account| account == base_url)
            {
                return Ok(StoredSecretLookup::Unavailable);
            }

            Ok(self
                .state
                .refresh_tokens
                .lock()
                .expect("lock poisoned")
                .get(base_url)
                .cloned()
                .map(StoredSecretLookup::Found)
                .unwrap_or(StoredSecretLookup::Missing))
        }

        fn get_refresh_token(&self, base_url: &str) -> Result<String, DcCmdError> {
            match self.lookup_refresh_token(base_url)? {
                StoredSecretLookup::Found(refresh_token) => Ok(refresh_token),
                StoredSecretLookup::Missing => Err(DcCmdError::InvalidAccount),
                StoredSecretLookup::Unavailable => Err(DcCmdError::CredentialStorageFailed),
            }
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

        fn lookup_encryption_secret(
            &self,
            account: &str,
        ) -> Result<StoredSecretLookup, DcCmdError> {
            if self
                .state
                .unavailable_encryption_accounts
                .lock()
                .expect("lock poisoned")
                .iter()
                .any(|stored_account| stored_account == account)
            {
                return Ok(StoredSecretLookup::Unavailable);
            }

            Ok(self
                .state
                .encryption_secrets
                .lock()
                .expect("lock poisoned")
                .get(account)
                .cloned()
                .map(StoredSecretLookup::Found)
                .unwrap_or(StoredSecretLookup::Missing))
        }

        fn get_encryption_secret(&self, account: &str) -> Result<String, DcCmdError> {
            match self.lookup_encryption_secret(account)? {
                StoredSecretLookup::Found(secret) => Ok(secret),
                StoredSecretLookup::Missing => Err(DcCmdError::InvalidAccount),
                StoredSecretLookup::Unavailable => Err(DcCmdError::CredentialStorageFailed),
            }
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
    fn test_get_refresh_token_uses_normalized_base_url() {
        let store = InMemoryStore::with_refresh_token(
            "https://dracoon.example.com",
            "stored-refresh-token",
        );
        let service = ConfigService::with_store(store);

        let refresh_token = service
            .get_refresh_token("dracoon.example.com/some/path")
            .unwrap();

        assert_eq!(refresh_token, "stored-refresh-token");
    }

    #[test]
    fn test_get_refresh_token_errors_when_storage_is_unavailable() {
        let store = InMemoryStore::with_unavailable_refresh_token("https://dracoon.example.com");
        let service = ConfigService::with_store(store);

        let err = service
            .get_refresh_token("dracoon.example.com/some/path")
            .unwrap_err();

        assert!(matches!(err, DcCmdError::CredentialStorageFailed));
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

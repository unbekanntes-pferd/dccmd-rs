use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use super::*;
use mockall::{mock, predicate::eq, Sequence};

mock! {
    pub Backend {}

    #[async_trait]
    impl AuthBackend for Backend {
        type Client = String;

        async fn connect_password(
            &self,
            base_url: &str,
            is_transfer: bool,
            password_auth: &PasswordAuth,
        ) -> Result<String, DcCmdError>;

        async fn connect_refresh_token(
            &self,
            base_url: &str,
            is_transfer: bool,
            refresh_token: String,
        ) -> Result<String, RefreshTokenConnectError>;

        async fn connect_auth_code(
            &self,
            base_url: &str,
            is_transfer: bool,
            auth_code: String,
        ) -> Result<String, DcCmdError>;

        async fn get_refresh_token(&self, client: &String) -> Result<String, DcCmdError>;

        async fn verify_encryption_secret(
            &self,
            client: &String,
            secret: &SecretString,
        ) -> Result<(), DcCmdError>;

        fn authorize_url(&self, base_url: &str, is_transfer: bool) -> Result<String, DcCmdError>;
    }
}

mock! {
    pub Prompt {}

    impl Prompts for Prompt {
        fn ask_auth_code(&self, authorize_url: &str) -> Result<String, DcCmdError>;
        fn ask_encryption_secret(&self) -> Result<SecretString, DcCmdError>;
    }
}

#[derive(Default)]
struct InMemorySecretStoreState {
    refresh_tokens: Mutex<HashMap<String, String>>,
    encryption_secrets: Mutex<HashMap<String, String>>,
    unavailable_refresh_accounts: Mutex<Vec<String>>,
    unavailable_encryption_accounts: Mutex<Vec<String>>,
    set_refresh_calls: Mutex<u32>,
    delete_refresh_calls: Mutex<u32>,
    set_encryption_calls: Mutex<u32>,
}

#[derive(Clone, Default)]
struct InMemorySecretStore {
    state: Arc<InMemorySecretStoreState>,
}

impl InMemorySecretStore {
    fn with_refresh_token(base_url: &str, token: &str) -> Self {
        let mut refresh_tokens = HashMap::new();
        refresh_tokens.insert(base_url.to_string(), token.to_string());
        let state = InMemorySecretStoreState {
            refresh_tokens: Mutex::new(refresh_tokens),
            encryption_secrets: Mutex::new(HashMap::new()),
            unavailable_refresh_accounts: Mutex::new(Vec::new()),
            unavailable_encryption_accounts: Mutex::new(Vec::new()),
            set_refresh_calls: Mutex::new(0),
            delete_refresh_calls: Mutex::new(0),
            set_encryption_calls: Mutex::new(0),
        };
        Self {
            state: Arc::new(state),
        }
    }

    fn with_encryption_secret(account: &str, secret: &str) -> Self {
        let mut encryption_secrets = HashMap::new();
        encryption_secrets.insert(account.to_string(), secret.to_string());
        let state = InMemorySecretStoreState {
            refresh_tokens: Mutex::new(HashMap::new()),
            encryption_secrets: Mutex::new(encryption_secrets),
            unavailable_refresh_accounts: Mutex::new(Vec::new()),
            unavailable_encryption_accounts: Mutex::new(Vec::new()),
            set_refresh_calls: Mutex::new(0),
            delete_refresh_calls: Mutex::new(0),
            set_encryption_calls: Mutex::new(0),
        };
        Self {
            state: Arc::new(state),
        }
    }

    fn with_unavailable_refresh_token(base_url: &str) -> Self {
        let state = InMemorySecretStoreState {
            refresh_tokens: Mutex::new(HashMap::new()),
            encryption_secrets: Mutex::new(HashMap::new()),
            unavailable_refresh_accounts: Mutex::new(vec![base_url.to_string()]),
            unavailable_encryption_accounts: Mutex::new(Vec::new()),
            set_refresh_calls: Mutex::new(0),
            delete_refresh_calls: Mutex::new(0),
            set_encryption_calls: Mutex::new(0),
        };
        Self {
            state: Arc::new(state),
        }
    }

    fn with_unavailable_encryption_secret(account: &str) -> Self {
        let state = InMemorySecretStoreState {
            refresh_tokens: Mutex::new(HashMap::new()),
            encryption_secrets: Mutex::new(HashMap::new()),
            unavailable_refresh_accounts: Mutex::new(Vec::new()),
            unavailable_encryption_accounts: Mutex::new(vec![account.to_string()]),
            set_refresh_calls: Mutex::new(0),
            delete_refresh_calls: Mutex::new(0),
            set_encryption_calls: Mutex::new(0),
        };
        Self {
            state: Arc::new(state),
        }
    }

    fn set_refresh_calls(&self) -> u32 {
        *self.state.set_refresh_calls.lock().expect("lock poisoned")
    }

    fn delete_refresh_calls(&self) -> u32 {
        *self
            .state
            .delete_refresh_calls
            .lock()
            .expect("lock poisoned")
    }

    fn set_encryption_calls(&self) -> u32 {
        *self
            .state
            .set_encryption_calls
            .lock()
            .expect("lock poisoned")
    }

    fn refresh_token_for(&self, base_url: &str) -> Option<String> {
        self.state
            .refresh_tokens
            .lock()
            .expect("lock poisoned")
            .get(base_url)
            .cloned()
    }
}

impl SecretStore for InMemorySecretStore {
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

    fn set_refresh_token(&self, base_url: &str, refresh_token: &str) -> Result<(), DcCmdError> {
        self.state
            .refresh_tokens
            .lock()
            .expect("lock poisoned")
            .insert(base_url.to_string(), refresh_token.to_string());
        *self.state.set_refresh_calls.lock().expect("lock poisoned") += 1;
        Ok(())
    }

    fn delete_refresh_token(&self, base_url: &str) -> Result<(), DcCmdError> {
        self.state
            .refresh_tokens
            .lock()
            .expect("lock poisoned")
            .remove(base_url);
        *self
            .state
            .delete_refresh_calls
            .lock()
            .expect("lock poisoned") += 1;
        Ok(())
    }

    fn lookup_encryption_secret(&self, account: &str) -> Result<StoredSecretLookup, DcCmdError> {
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

    fn set_encryption_secret(&self, account: &str, secret: &str) -> Result<(), DcCmdError> {
        self.state
            .encryption_secrets
            .lock()
            .expect("lock poisoned")
            .insert(account.to_string(), secret.to_string());
        *self
            .state
            .set_encryption_calls
            .lock()
            .expect("lock poisoned") += 1;
        Ok(())
    }

    fn delete_encryption_secret(&self, account: &str) -> Result<(), DcCmdError> {
        let removed = self
            .state
            .encryption_secrets
            .lock()
            .expect("lock poisoned")
            .remove(account);
        if removed.is_some() {
            Ok(())
        } else {
            Err(DcCmdError::InvalidAccount)
        }
    }
}

#[tokio::test]
async fn test_connect_uses_password_flow_when_password_auth_is_set() {
    let mut backend = MockBackend::new();
    backend
        .expect_connect_password()
        .times(1)
        .returning(|_, _, _| Ok("password-client".to_string()));
    let store = InMemorySecretStore::with_refresh_token("https://example.com", "stored-token");
    let store_view = store.clone();
    let prompts = MockPrompt::new();
    let service = AuthService::with_dependencies(backend, store, prompts);

    let password_auth = PasswordAuth::new(
        "alice".to_string(),
        SecretString::new("secret".to_string().into()),
    );

    let session = service
        .connect("example.com/path", Some(password_auth), false)
        .await
        .unwrap();

    assert_eq!(session.auth_mode(), AuthMode::PasswordFlow);
    assert_eq!(store_view.set_refresh_calls(), 0);
    assert_eq!(store_view.delete_refresh_calls(), 0);
}

#[tokio::test]
async fn test_connect_client_returns_connected_client() {
    let mut backend = MockBackend::new();
    backend
        .expect_connect_password()
        .times(1)
        .returning(|_, _, _| Ok("password-client".to_string()));
    let store = InMemorySecretStore::with_refresh_token("https://example.com", "stored-token");
    let prompts = MockPrompt::new();
    let service = AuthService::with_dependencies(backend, store, prompts);

    let password_auth = PasswordAuth::new(
        "alice".to_string(),
        SecretString::new("secret".to_string().into()),
    );

    let client = service
        .connect_client("example.com/path", Some(password_auth), false)
        .await
        .unwrap();

    assert_eq!(client, "password-client");
}

#[tokio::test]
async fn test_connect_uses_refresh_token_and_rotates_token() {
    let mut backend = MockBackend::new();
    backend
        .expect_connect_refresh_token()
        .with(
            eq("https://example.com"),
            eq(false),
            eq("stored-token".to_string()),
        )
        .times(1)
        .returning(|_, _, _| Ok("refresh-client".to_string()));
    backend
        .expect_get_refresh_token()
        .with(eq("refresh-client".to_string()))
        .times(1)
        .returning(|_| Ok("new-token".to_string()));
    let store = InMemorySecretStore::with_refresh_token("https://example.com", "stored-token");
    let store_view = store.clone();
    let prompts = MockPrompt::new();
    let service = AuthService::with_dependencies(backend, store, prompts);

    let session = service
        .connect("example.com/path", None, false)
        .await
        .unwrap();

    assert_eq!(session.auth_mode(), AuthMode::RefreshToken);
    assert_eq!(store_view.set_refresh_calls(), 1);
    assert_eq!(store_view.delete_refresh_calls(), 0);
    assert_eq!(
        store_view.refresh_token_for("https://example.com"),
        Some("new-token".to_string())
    );
}

#[tokio::test]
async fn test_connect_with_refresh_token_rotates_token() {
    let mut backend = MockBackend::new();
    backend
        .expect_connect_refresh_token()
        .with(
            eq("https://example.com"),
            eq(false),
            eq("stored-token".to_string()),
        )
        .times(1)
        .returning(|_, _, _| Ok("refresh-client".to_string()));
    backend
        .expect_get_refresh_token()
        .with(eq("refresh-client".to_string()))
        .times(1)
        .returning(|_| Ok("rotated-token".to_string()));
    let store = InMemorySecretStore::with_refresh_token("https://example.com", "stored-token");
    let store_view = store.clone();
    let prompts = MockPrompt::new();
    let service = AuthService::with_dependencies(backend, store, prompts);

    let session = service
        .connect_with_refresh_token("https://example.com", "stored-token".to_string())
        .await
        .unwrap();

    assert_eq!(session.auth_mode(), AuthMode::RefreshToken);
    assert_eq!(store_view.set_refresh_calls(), 1);
    assert_eq!(store_view.delete_refresh_calls(), 0);
    assert_eq!(
        store_view.refresh_token_for("https://example.com"),
        Some("rotated-token".to_string())
    );
}

#[tokio::test]
async fn test_connect_invalid_refresh_falls_back_to_auth_code() {
    let mut sequence = Sequence::new();
    let mut backend = MockBackend::new();
    backend
        .expect_connect_refresh_token()
        .with(
            eq("https://example.com"),
            eq(false),
            eq("bad-token".to_string()),
        )
        .times(1)
        .in_sequence(&mut sequence)
        .returning(|_, _, _| {
            Err(RefreshTokenConnectError::InvalidToken(
                "refresh token rejected".to_string(),
            ))
        });
    backend
        .expect_authorize_url()
        .with(eq("https://example.com"), eq(false))
        .times(1)
        .in_sequence(&mut sequence)
        .returning(|_, _| Ok("https://authorize.local".to_string()));
    backend
        .expect_connect_auth_code()
        .with(
            eq("https://example.com"),
            eq(false),
            eq("code-from-prompt".to_string()),
        )
        .times(1)
        .in_sequence(&mut sequence)
        .returning(|_, _, _| Ok("auth-code-client".to_string()));
    backend
        .expect_get_refresh_token()
        .with(eq("auth-code-client".to_string()))
        .times(1)
        .in_sequence(&mut sequence)
        .returning(|_| Ok("refreshed-token".to_string()));

    let store = InMemorySecretStore::with_refresh_token("https://example.com", "bad-token");
    let store_view = store.clone();
    let mut prompts = MockPrompt::new();
    prompts
        .expect_ask_auth_code()
        .with(eq("https://authorize.local"))
        .times(1)
        .returning(|_| Ok("code-from-prompt".to_string()));
    let service = AuthService::with_dependencies(backend, store, prompts);

    let session = service
        .connect("example.com/path", None, false)
        .await
        .unwrap();

    assert_eq!(session.auth_mode(), AuthMode::AuthCode);
    assert_eq!(store_view.delete_refresh_calls(), 1);
    assert_eq!(store_view.set_refresh_calls(), 1);
    assert_eq!(
        store_view.refresh_token_for("https://example.com"),
        Some("refreshed-token".to_string())
    );
}

#[tokio::test]
async fn test_connect_other_refresh_error_falls_back_to_auth_code() {
    let mut sequence = Sequence::new();
    let mut backend = MockBackend::new();
    backend
        .expect_connect_refresh_token()
        .with(
            eq("https://example.com"),
            eq(false),
            eq("transient-error-token".to_string()),
        )
        .times(1)
        .in_sequence(&mut sequence)
        .returning(|_, _, _| {
            Err(RefreshTokenConnectError::Other(
                DcCmdError::ConnectionFailed,
            ))
        });
    backend
        .expect_authorize_url()
        .with(eq("https://example.com"), eq(false))
        .times(1)
        .in_sequence(&mut sequence)
        .returning(|_, _| Ok("https://authorize.local".to_string()));
    backend
        .expect_connect_auth_code()
        .with(
            eq("https://example.com"),
            eq(false),
            eq("code-from-prompt".to_string()),
        )
        .times(1)
        .in_sequence(&mut sequence)
        .returning(|_, _, _| Ok("auth-code-client".to_string()));
    backend
        .expect_get_refresh_token()
        .with(eq("auth-code-client".to_string()))
        .times(1)
        .in_sequence(&mut sequence)
        .returning(|_| Ok("token-after-fallback".to_string()));
    let store =
        InMemorySecretStore::with_refresh_token("https://example.com", "transient-error-token");
    let store_view = store.clone();
    let mut prompts = MockPrompt::new();
    prompts
        .expect_ask_auth_code()
        .with(eq("https://authorize.local"))
        .times(1)
        .returning(|_| Ok("code-from-prompt".to_string()));
    let service = AuthService::with_dependencies(backend, store, prompts);

    let session = service
        .connect("example.com/path", None, false)
        .await
        .unwrap();

    assert_eq!(session.auth_mode(), AuthMode::AuthCode);
    assert_eq!(store_view.delete_refresh_calls(), 0);
    assert_eq!(store_view.set_refresh_calls(), 1);
    assert_eq!(
        store_view.refresh_token_for("https://example.com"),
        Some("token-after-fallback".to_string())
    );
}

#[tokio::test]
async fn test_connect_unavailable_refresh_storage_falls_back_to_auth_code() {
    let mut sequence = Sequence::new();
    let mut backend = MockBackend::new();
    backend
        .expect_authorize_url()
        .with(eq("https://example.com"), eq(false))
        .times(1)
        .in_sequence(&mut sequence)
        .returning(|_, _| Ok("https://authorize.local".to_string()));
    backend
        .expect_connect_auth_code()
        .with(
            eq("https://example.com"),
            eq(false),
            eq("code-from-prompt".to_string()),
        )
        .times(1)
        .in_sequence(&mut sequence)
        .returning(|_, _, _| Ok("auth-code-client".to_string()));
    backend
        .expect_get_refresh_token()
        .with(eq("auth-code-client".to_string()))
        .times(1)
        .in_sequence(&mut sequence)
        .returning(|_| Ok("token-after-recovery".to_string()));

    let store = InMemorySecretStore::with_unavailable_refresh_token("https://example.com");
    let store_view = store.clone();
    let mut prompts = MockPrompt::new();
    prompts
        .expect_ask_auth_code()
        .with(eq("https://authorize.local"))
        .times(1)
        .returning(|_| Ok("code-from-prompt".to_string()));
    let service = AuthService::with_dependencies(backend, store, prompts);

    let session = service
        .connect("example.com/path", None, false)
        .await
        .unwrap();

    assert_eq!(session.auth_mode(), AuthMode::AuthCode);
    assert_eq!(store_view.delete_refresh_calls(), 0);
    assert_eq!(store_view.set_refresh_calls(), 1);
    assert_eq!(
        store_view.refresh_token_for("https://example.com"),
        Some("token-after-recovery".to_string())
    );
}

#[tokio::test]
async fn test_ensure_encryption_uses_stored_secret() {
    let mut backend = MockBackend::new();
    backend
        .expect_verify_encryption_secret()
        .times(1)
        .returning(|_, _| Ok(()));
    let store =
        InMemorySecretStore::with_encryption_secret("https://example.com-crypto", "stored-secret");
    let store_view = store.clone();
    let mut prompts = MockPrompt::new();
    prompts.expect_ask_encryption_secret().times(0);
    let service = AuthService::with_dependencies(backend, store, prompts);
    let session = Session::new(
        "https://example.com".to_string(),
        AuthMode::RefreshToken,
        "client".to_string(),
    );

    let session = service.ensure_encryption(session, None).await.unwrap();

    assert_eq!(session.encryption_state(), EncryptionState::Unlocked);
    assert_eq!(store_view.set_encryption_calls(), 0);
}

#[tokio::test]
async fn test_ensure_encryption_prompts_and_persists_on_missing_secret() {
    let mut backend = MockBackend::new();
    backend
        .expect_verify_encryption_secret()
        .times(1)
        .returning(|_, _| Ok(()));

    let store = InMemorySecretStore::default();
    let store_view = store.clone();
    let mut prompts = MockPrompt::new();
    prompts
        .expect_ask_encryption_secret()
        .times(1)
        .returning(|| Ok(SecretString::new("prompt-secret".to_string().into())));

    let service = AuthService::with_dependencies(backend, store, prompts);
    let session = Session::new(
        "https://example.com".to_string(),
        AuthMode::RefreshToken,
        "client".to_string(),
    );

    let session = service.ensure_encryption(session, None).await.unwrap();

    assert_eq!(session.encryption_state(), EncryptionState::Unlocked);
    assert_eq!(store_view.set_encryption_calls(), 1);
}

#[tokio::test]
async fn test_ensure_encryption_prompts_and_persists_when_secret_storage_is_unavailable() {
    let mut backend = MockBackend::new();
    backend
        .expect_verify_encryption_secret()
        .times(1)
        .returning(|_, _| Ok(()));

    let store =
        InMemorySecretStore::with_unavailable_encryption_secret("https://example.com-crypto");
    let store_view = store.clone();
    let mut prompts = MockPrompt::new();
    prompts
        .expect_ask_encryption_secret()
        .times(1)
        .returning(|| Ok(SecretString::new("prompt-secret".to_string().into())));

    let service = AuthService::with_dependencies(backend, store, prompts);
    let session = Session::new(
        "https://example.com".to_string(),
        AuthMode::RefreshToken,
        "client".to_string(),
    );

    let session = service.ensure_encryption(session, None).await.unwrap();

    assert_eq!(session.encryption_state(), EncryptionState::Unlocked);
    assert_eq!(store_view.set_encryption_calls(), 1);
}

#[tokio::test]
async fn test_ensure_encryption_client_returns_unlocked_client() {
    let mut backend = MockBackend::new();
    backend
        .expect_verify_encryption_secret()
        .times(1)
        .returning(|_, _| Ok(()));
    let store =
        InMemorySecretStore::with_encryption_secret("https://example.com-crypto", "stored-secret");
    let prompts = MockPrompt::new();
    let service = AuthService::with_dependencies(backend, store, prompts);

    let client = service
        .ensure_encryption_client(
            "https://example.com".to_string(),
            "client".to_string(),
            None,
        )
        .await
        .unwrap();

    assert_eq!(client, "client".to_string());
}

#[test]
fn test_resolve_connect_mode_password() {
    let mode = AuthService::<MockBackend, InMemorySecretStore, MockPrompt>::resolve_connect_mode(
        true, true,
    );
    assert_eq!(mode, ConnectMode::Password);
}

#[test]
fn test_resolve_connect_mode_refresh_token() {
    let mode = AuthService::<MockBackend, InMemorySecretStore, MockPrompt>::resolve_connect_mode(
        false, true,
    );
    assert_eq!(mode, ConnectMode::RefreshToken);
}

#[test]
fn test_resolve_connect_mode_auth_code() {
    let mode = AuthService::<MockBackend, InMemorySecretStore, MockPrompt>::resolve_connect_mode(
        false, false,
    );
    assert_eq!(mode, ConnectMode::AuthCode);
}

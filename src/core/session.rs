#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuthMode {
    RefreshToken,
    PasswordFlow,
    AuthCode,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EncryptionState {
    Locked,
    Unlocked,
}

#[derive(Clone)]
pub struct Session<C> {
    base_url: String,
    auth_mode: AuthMode,
    encryption_state: EncryptionState,
    client: C,
}

impl<C> Session<C> {
    pub fn new(base_url: String, auth_mode: AuthMode, client: C) -> Self {
        Self {
            base_url,
            auth_mode,
            encryption_state: EncryptionState::Locked,
            client,
        }
    }

    pub fn base_url(&self) -> &str {
        self.base_url.as_str()
    }

    pub fn auth_mode(&self) -> AuthMode {
        self.auth_mode
    }

    pub fn encryption_state(&self) -> EncryptionState {
        self.encryption_state
    }

    pub fn client(&self) -> &C {
        &self.client
    }

    pub fn into_client(self) -> C {
        self.client
    }

    pub fn with_client(self, client: C, encryption_state: EncryptionState) -> Self {
        Self {
            base_url: self.base_url,
            auth_mode: self.auth_mode,
            encryption_state,
            client,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{AuthMode, EncryptionState, Session};

    #[test]
    fn test_session_new_defaults_to_locked() {
        let session = Session::new(
            "https://example.com".to_string(),
            AuthMode::RefreshToken,
            "client".to_string(),
        );

        assert_eq!(session.base_url(), "https://example.com");
        assert_eq!(session.auth_mode(), AuthMode::RefreshToken);
        assert_eq!(session.encryption_state(), EncryptionState::Locked);
        assert_eq!(session.client(), "client");
    }

    #[test]
    fn test_with_client_updates_client_and_encryption_state() {
        let session = Session::new(
            "https://example.com".to_string(),
            AuthMode::AuthCode,
            "old-client".to_string(),
        );

        let updated = session.with_client("new-client".to_string(), EncryptionState::Unlocked);

        assert_eq!(updated.base_url(), "https://example.com");
        assert_eq!(updated.auth_mode(), AuthMode::AuthCode);
        assert_eq!(updated.encryption_state(), EncryptionState::Unlocked);
        assert_eq!(updated.client(), "new-client");
    }
}

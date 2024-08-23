use keyring::Entry;

use crate::cmd::models::DcCmdError;

pub trait HandleCredentials {
    fn set_dracoon_env(&self, secret: &str) -> Result<(), DcCmdError>;
    fn get_dracoon_env(&self) -> Result<String, DcCmdError>;
    fn delete_dracoon_env(&self) -> Result<(), DcCmdError>;
}

impl HandleCredentials for Entry {
    fn set_dracoon_env(&self, secret: &str) -> Result<(), DcCmdError> {
        match self.set_password(secret) {
            Ok(()) => Ok(()),
            Err(_) => Err(DcCmdError::CredentialStorageFailed),
        }
    }
    fn get_dracoon_env(&self) -> Result<String, DcCmdError> {
        match self.get_password() {
            Ok(pwd) => Ok(pwd),
            Err(_) => Err(DcCmdError::InvalidAccount),
        }
    }
    fn delete_dracoon_env(&self) -> Result<(), DcCmdError> {
        if self.get_password().is_err() {
            return Err(DcCmdError::InvalidAccount);
        }

        match self.delete_password() {
            Ok(()) => Ok(()),
            Err(_) => Err(DcCmdError::CredentialDeletionFailed),
        }
    }
}

#[allow(clippy::module_name_repetitions)]
pub fn get_client_credentials() -> (String, String) {
    let env_content = std::fs::read_to_string("../../../.env").unwrap_or_default();

    let client_id = env_content
        .lines()
        .find(|line| line.starts_with("CLIENT_ID="))
        .and_then(|line| line.split("CLIENT_ID=").nth(1))
        .unwrap_or("dccmd_rs_unbekanntes-pferd")
        .to_string();

    let client_secret = env_content
        .lines()
        .find(|line| line.starts_with("CLIENT_SECRET="))
        .and_then(|line| line.split("CLIENT_SECRET=").nth(1))
        .unwrap_or("LspjGm1S3EGgyC4NhtQcvGHzjzMOAv5b")
        .to_string();

    (client_id, client_secret)
}

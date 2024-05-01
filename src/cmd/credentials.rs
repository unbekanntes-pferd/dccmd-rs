use keyring::Entry;

use crate::cmd::models::DcCmdError;

// service name to store
const SERVICE_NAME: &str = env!("CARGO_PKG_NAME");

pub trait HandleCredentials {
    fn set_dracoon_env(&self, secret: &str) -> Result<(), DcCmdError>;
    fn get_dracoon_env(&self) -> Result<String, DcCmdError>;
    fn delete_dracoon_env(&self, dracoon_url: &str) -> Result<(), DcCmdError>;
}

impl HandleCredentials for Entry {
    fn set_dracoon_env(&self, secret: &str) -> Result<(), DcCmdError> {
        match self.set_password(secret) {
            Ok(_) => Ok(()),
            Err(_) => Err(DcCmdError::CredentialStorageFailed),
        }
    }
    fn get_dracoon_env(&self) -> Result<String, DcCmdError> {
        match self.get_password() {
            Ok(pwd) => Ok(pwd),
            Err(_) => Err(DcCmdError::InvalidAccount),
        }
    }
    fn delete_dracoon_env(&self, dracoon_url: &str) -> Result<(), DcCmdError> {
        if self.get_password().is_err() {
            return Err(DcCmdError::InvalidAccount);
        }

        match self.delete_password() {
            Ok(_) => Ok(()),
            Err(_) => Err(DcCmdError::CredentialDeletionFailed),
        }
    }
}

#[allow(clippy::module_name_repetitions)]
pub fn get_client_credentials() -> (String, String) {
    let client_id = include_str!("../../.env")
        .split('\n')
        .next()
        .expect("env file has more than one line")
        .split("CLIENT_ID=")
        .nth(1)
        .expect("CLIENT_ID MUST be provided");
    let client_secret = include_str!("../../.env")
        .split('\n')
        .nth(1)
        .expect("env file has more than one line")
        .split("CLIENT_SECRET=")
        .nth(1)
        .expect("CLIENT_SECRET MUST be provided");

    (client_id.into(), client_secret.into())
}

use keyring::Entry;

use crate::cmd::models::DcCmdError;

// service name to store
const SERVICE_NAME: &str = env!("CARGO_PKG_NAME");

pub fn set_dracoon_env(entry: &Entry, secret: &str) -> Result<(), DcCmdError> {
    match entry.set_password(secret) {
        Ok(_) => Ok(()),
        Err(_) => Err(DcCmdError::CredentialStorageFailed),
    }
}

pub fn get_dracoon_env(entry: &Entry) -> Result<String, DcCmdError> {
    match entry.get_password() {
        Ok(pwd) => Ok(pwd),
        Err(_) => Err(DcCmdError::InvalidAccount),
    }
}

pub fn delete_dracoon_env(entry: &Entry, dracoon_url: &str) -> Result<(), DcCmdError> {
    if entry.get_password().is_err() {
        return Err(DcCmdError::InvalidAccount);
    }

    match entry.delete_password() {
        Ok(_) => Ok(()),
        Err(_) => Err(DcCmdError::CredentialDeletionFailed),
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

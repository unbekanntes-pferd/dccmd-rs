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
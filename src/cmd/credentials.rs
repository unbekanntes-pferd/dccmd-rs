use keytar::{delete_password, get_password, set_password};

use crate::cmd::models::DcCmdError;

// service name to store
const SERVICE_NAME: &str = env!("CARGO_PKG_NAME");

pub fn set_dracoon_env(dracoon_url: &str, refresh_token: &str) -> Result<(), DcCmdError> {
    match set_password(SERVICE_NAME, dracoon_url, refresh_token) {
        Ok(_) => Ok(()),
        Err(_) => Err(DcCmdError::CredentialStorageFailed),
    }
}

pub fn get_dracoon_env(dracoon_url: &str) -> Result<String, DcCmdError> {
    match get_password(SERVICE_NAME, dracoon_url) {
        Ok(pwd) => 
        if pwd.success {
            Ok(pwd.password)
        } else {
            Err(DcCmdError::InvalidAccount)
        },
        Err(_) => Err(DcCmdError::InvalidAccount),
    }
}

pub fn delete_dracoon_env(dracoon_url: &str) -> Result<(), DcCmdError> {
    if get_dracoon_env(dracoon_url).is_err() {
        return Err(DcCmdError::InvalidAccount);
    }

    match delete_password(SERVICE_NAME, dracoon_url) {
        Ok(_) => Ok(()),
        Err(_) => Err(DcCmdError::CredentialDeletionFailed),
    }
}

pub fn set_dracoon_crypto_env(dracoon_url: &str, crypto_env: &str) -> Result<(), DcCmdError> {
    match set_password(SERVICE_NAME, &format!("{dracoon_url}-crypto"), crypto_env) {
        Ok(_) => Ok(()),
        Err(_) => Err(DcCmdError::CredentialStorageFailed),
    }
}

pub fn get_dracoon_crypto_env(dracoon_url: &str) -> Result<String, DcCmdError> {
    match get_password(SERVICE_NAME, &format!("{dracoon_url}-crypto")) {
        Ok(pwd) => {
            if pwd.success {
                Ok(pwd.password)
            } else {
                Err(DcCmdError::InvalidAccount)
            }
        }
        Err(_) => Err(DcCmdError::InvalidAccount),
    }
}

pub fn delete_dracoon_crypto_env(dracoon_url: &str) -> Result<(), DcCmdError> {
    if get_dracoon_crypto_env(dracoon_url).is_err() {
        return Err(DcCmdError::InvalidAccount);
    }

    match delete_password(SERVICE_NAME, &format!("{dracoon_url}-crypto")) {
        Ok(_) => Ok(()),
        Err(_) => Err(DcCmdError::CredentialDeletionFailed),
    }
}

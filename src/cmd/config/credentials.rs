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

use console::Term;
use dco3::{Dracoon, OAuth2Flow};
use dialoguer::Confirm;
use keyring::Entry;

use self::{
    credentials::{get_client_credentials, HandleCredentials},
    models::{ConfigAuthCommand, ConfigCryptoCommand},
};

use super::{
    models::{ConfigCommand, DcCmdError},
    utils::strings::format_error_message,
    SERVICE_NAME,
};

pub mod credentials;
pub mod models;

pub struct ConfigCommandHandler {
    entry: Entry,
    term: Term,
}

impl ConfigCommandHandler {
    pub fn new(entry: Entry, term: Term) -> Self {
        Self { entry, term }
    }

    pub async fn get_refresh_token_info(&self, target: String) -> Result<(), DcCmdError> {
        let (client_id, client_secret) = get_client_credentials();
        let Ok(refresh_token) = self.entry.get_dracoon_env() else {
            let msg = format_error_message(
                format!("No token found for this DRACOON url: {}.", target).as_str(),
            );
            self.term
                .write_line(&msg)
                .map_err(|_| DcCmdError::IoError)?;
            return Err(DcCmdError::InvalidAccount);
        };

        let dracoon = Dracoon::builder()
            .with_base_url(&target)
            .with_client_id(client_id)
            .with_client_secret(client_secret)
            .build()?
            .connect(OAuth2Flow::refresh_token(refresh_token))
            .await?;

        let user_info = dracoon.get_user_info().await?;

        self.term
            .write_line(&format!("► Token stored for: {}", target))
            .map_err(|_| DcCmdError::IoError)?;
        self.term
            .write_line(&format!(
                "► User: {} {}",
                user_info.first_name, user_info.last_name
            ))
            .map_err(|_| DcCmdError::IoError)?;
        self.term
            .write_line(&format!(
                "► Email: {}",
                user_info.email.unwrap_or_else(|| "N/A".to_string())
            ))
            .map_err(|_| DcCmdError::IoError)?;
        self.term
            .write_line(&format!("► Username: {}", user_info.user_name))
            .map_err(|_| DcCmdError::IoError)?;

        Ok(())
    }

    pub async fn remove_refresh_token(&self, target: String) -> Result<(), DcCmdError> {

        let confirmed = Confirm::new()
            .with_prompt("Are you sure you want to remove the token?")
            .interact_opt();

        self.entry.delete_dracoon_env()?;
        self.term
            .write_line(&format!("► Token removed for {}", target))
            .map_err(|_| DcCmdError::IoError)?;

        Ok(())
    }

    pub fn get_encryption_secret_info(&self, target: String) -> Result<(), DcCmdError> {
        let Ok(encryption_secret) = self.entry.get_dracoon_env() else {
            let msg = format_error_message("No encryption secret found.");
            self.term
                .write_line(&msg)
                .map_err(|_| DcCmdError::IoError)?;
            return Err(DcCmdError::InvalidAccount);
        };

        self.term
            .write_line(format!("► Encryption secret securely stored for {}.", target).as_str())
            .map_err(|_| DcCmdError::IoError)?;

        Ok(())
    }

    pub fn remove_encryption_secret(&self, target: String) -> Result<(), DcCmdError> {
        self.entry.delete_dracoon_env()?;
        self.term
            .write_line(format!("► Encryption secret removed for {}.", target).as_str())
            .map_err(|_| DcCmdError::IoError)?;

        Ok(())
    }
}

pub async fn handle_config_cmd(cmd: ConfigCommand, term: Term) -> Result<(), DcCmdError> {
    match cmd {
        ConfigCommand::Auth { cmd } => match cmd {
            ConfigAuthCommand::Ls { target } => {
                let (target, entry) = prepare_config_cmd(target, &term, false)?;

                let handler = ConfigCommandHandler::new(entry, term);
                handler.get_refresh_token_info(target).await?;
                Ok(())
            }
            ConfigAuthCommand::Rm { target } => {
                let (target, entry) = prepare_config_cmd(target, &term, false)?;

                let handler = ConfigCommandHandler::new(entry, term);
                handler.remove_refresh_token(target).await?;
                Ok(())
            }
        },
        ConfigCommand::Crypto { cmd } => match cmd {
            ConfigCryptoCommand::Ls { target } => {
                let (target, entry) = prepare_config_cmd(target, &term, true)?;

                let handler = ConfigCommandHandler::new(entry, term);
                handler.get_encryption_secret_info(target)?;
                Ok(())
            }
            ConfigCryptoCommand::Rm { target } => {
                let (target, entry) = prepare_config_cmd(target, &term, true)?;

                let handler = ConfigCommandHandler::new(entry, term);
                handler.remove_encryption_secret(target)?;
                Ok(())
            }
        },
    }
}

fn prepare_config_cmd(
    target: String,
    term: &Term,
    is_crypto: bool,
) -> Result<(String, Entry), DcCmdError> {
    let base_url = format!(
        "https://{}",
        target
            .strip_prefix("https://")
            .unwrap_or(&target)
            .trim_end_matches('/')
    );

    let base_url = if is_crypto {
        format!("{}/-crypto", base_url)
    } else {
        base_url
    };

    let Ok(entry) = Entry::new(SERVICE_NAME, &base_url) else {
        let msg =
            format_error_message("Secure storage for credentials not available on this platform.");
        term.write_line(&msg).map_err(|_| DcCmdError::IoError)?;
        return Err(DcCmdError::CredentialStorageFailed);
    };

    Ok((base_url, entry))
}

use console::Term;
use dco3::{Dracoon, OAuth2Flow};
use keyring::Entry;
use tracing::debug;

use self::{
    credentials::{get_client_credentials, HandleCredentials},
    models::ConfigAuthCommand,
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

    pub async fn add_refresh_token(
        &self,
        target: String,
        refresh_token: String,
    ) -> Result<(), DcCmdError> {
        let (client_id, client_secret) = get_client_credentials();

        let dracoon = Dracoon::builder()
            .with_base_url(&target)
            .with_client_id(client_id)
            .with_client_secret(client_secret)
            .build()?
            .connect(OAuth2Flow::refresh_token(&refresh_token))
            .await
            .map_err(|e| {
                if e.is_auth_error() {
                    debug!("Invalid refresh token provided.");
                    if let Err(_) = self.term.write_line("► Invalid refresh token provided.") {
                        return DcCmdError::IoError;
                    }
                }
                e.into()
            })?;

        self.entry.set_dracoon_env(&refresh_token)?;
        self.term
            .write_line(&format!("► Token stored for {}", target))
            .map_err(|_| DcCmdError::IoError)?;

        Ok(())
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
        self.entry.delete_dracoon_env()?;
        self.term
            .write_line(&format!("► Token removed for {}", target))
            .map_err(|_| DcCmdError::IoError)?;

        Ok(())
    }
}

pub async fn handle_config_cmd(cmd: ConfigCommand, term: Term) -> Result<(), DcCmdError> {
    match cmd {
        ConfigCommand::Auth { cmd } => match cmd {
            ConfigAuthCommand::Add {
                target,
                refresh_token,
            } => {
                let (target, entry) = prepare_config_cmd(target, &term)?;

                let handler = ConfigCommandHandler::new(entry, term);
                handler.add_refresh_token(target, refresh_token).await?;
                Ok(())
            }
            ConfigAuthCommand::Ls { target } => {
                let (target, entry) = prepare_config_cmd(target, &term)?;

                let handler = ConfigCommandHandler::new(entry, term);
                handler.get_refresh_token_info(target).await?;
                Ok(())
            }
            ConfigAuthCommand::Rm { target } => {
                let (target, entry) = prepare_config_cmd(target, &term)?;

                let handler = ConfigCommandHandler::new(entry, term);
                handler.remove_refresh_token(target).await?;
                Ok(())
            }
        },
        _ => unimplemented!(),
    }
}

fn prepare_config_cmd(target: String, term: &Term) -> Result<(String, Entry), DcCmdError> {
    let base_url = format!(
        "https://{}",
        target
            .strip_prefix("https://")
            .unwrap_or(&target)
            .trim_end_matches('/')
    );

    let Ok(entry) = Entry::new(SERVICE_NAME, &base_url) else {
        let msg =
            format_error_message("Secure storage for credentials not available on this platform.");
        term.write_line(&msg).map_err(|_| DcCmdError::IoError)?;
        return Err(DcCmdError::CredentialStorageFailed);
    };

    Ok((base_url, entry))
}

use console::Term;
use dco3::{auth::Connected, AuthenticationMethods, Dracoon, OAuth2Flow, User};
use dialoguer::Confirm;
use keyring::Entry;

use self::{
    credentials::{get_client_credentials, HandleCredentials},
    models::{ConfigAuthCommand, ConfigCryptoCommand},
};

use super::{
    models::{ConfigCommand, DcCmdError},
    utils::strings::{format_error_message, to_readable_size},
    SERVICE_NAME,
};

pub mod credentials;
pub mod logs;
pub mod models;

pub struct ConfigCommandHandler {
    entry: Box<dyn HandleCredentials>,
    term: Term,
}

impl ConfigCommandHandler {
    pub fn new(entry: impl HandleCredentials + 'static, term: Term) -> Self {
        Self {
            entry: Box::new(entry),
            term,
        }
    }

    async fn get_dracoon_client(&self, target: &str) -> Result<Dracoon<Connected>, DcCmdError> {
        let (client_id, client_secret) = get_client_credentials();
        let Ok(refresh_token) = self.entry.get_dracoon_env() else {
            let msg = format_error_message(
                format!("No token found for this DRACOON url: {target}.").as_str(),
            );
            self.term
                .write_line(&msg)
                .map_err(|_| DcCmdError::IoError)?;
            return Err(DcCmdError::InvalidAccount);
        };

        let dracoon = Dracoon::builder()
            .with_base_url(target)
            .with_client_id(client_id)
            .with_client_secret(client_secret)
            .build()?
            .connect(OAuth2Flow::refresh_token(refresh_token))
            .await?;

        Ok(dracoon)
    }

    pub async fn get_refresh_token_info(&self, target: String) -> Result<(), DcCmdError> {
        let dracoon = self.get_dracoon_client(&target).await?;

        let user_info = dracoon.get_user_info().await?;

        self.term
            .write_line(&format!("► Token stored for: {target}"))
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

    pub async fn get_system_info(&self, target: &str) -> Result<(), DcCmdError> {
        let dracoon = self.get_dracoon_client(target).await?;

        let oidc_info = dracoon
            .system()
            .auth
            .get_openid_idp_configurations()
            .await?;
        let ad_info = dracoon
            .system()
            .auth
            .get_active_directory_configurations()
            .await?;
        let customer_info = dracoon.user().get_customer_info().await?;

        self.term
            .write_line(&format!("► System info for: {target}"))
            .map_err(|_| DcCmdError::IoError)?;

        // Customer info
        self.term
            .write_line(&format!("► Customer: {}", customer_info.name))
            .map_err(|_| DcCmdError::IoError)?;

        let percent_space_used =
            (customer_info.space_used as f64 / customer_info.space_limit as f64) * 100.0;
        let percent_users_used =
            (customer_info.accounts_used as f64 / customer_info.accounts_limit as f64) * 100.0;
        let space_used = to_readable_size(customer_info.space_used);
        let space_limit = to_readable_size(customer_info.space_limit);

        self.term
            .write_line(&format!(
                "► Space used: {space_used} / {space_limit} ({percent_space_used:.2}%)"
            ))
            .map_err(|_| DcCmdError::IoError)?;

        self.term
            .write_line(&format!(
                "► Users used: {} / {} ({percent_users_used:.2}%)",
                customer_info.accounts_used, customer_info.accounts_limit
            ))
            .map_err(|_| DcCmdError::IoError)?;

        // Authentication methods
        if !oidc_info.is_empty() {
            self.term
                .write_line("\n► OpenID Connect IDP configurations:")
                .map_err(|_| DcCmdError::IoError)?;

            for info in oidc_info {
                self.term
                    .write_line(&format!("► {} ({})", info.name, info.id))
                    .map_err(|_| DcCmdError::IoError)?;
            }
        } else {
            self.term
                .write_line("\n► No OpenID Connect IDP configurations found.")
                .map_err(|_| DcCmdError::IoError)?;
        }

        if !ad_info.items.is_empty() {
            self.term
                .write_line("\n► Active Directory configurations:")
                .map_err(|_| DcCmdError::IoError)?;

            for info in ad_info.items {
                self.term
                    .write_line(&format!("► {} ({})", info.alias, info.id))
                    .map_err(|_| DcCmdError::IoError)?;
            }
        } else {
            self.term
                .write_line("\n► No Active Directory configurations found.")
                .map_err(|_| DcCmdError::IoError)?;
        }
        Ok(())
    }

    pub fn remove_refresh_token(&self, target: &str) -> Result<(), DcCmdError> {
        let confirmed = Confirm::new()
            .with_prompt("Are you sure you want to remove the token?")
            .interact_opt();

        if let Ok(Some(true)) = confirmed {
            self.entry.delete_dracoon_env()?;
            self.term
                .write_line(&format!("► Token removed for {target}"))
                .map_err(|_| DcCmdError::IoError)?;
        }

        Ok(())
    }

    pub fn get_encryption_secret_info(&self, target: &str) -> Result<(), DcCmdError> {
        let Ok(_) = self.entry.get_dracoon_env() else {
            let msg = format_error_message("No encryption secret found.");
            self.term
                .write_line(&msg)
                .map_err(|_| DcCmdError::IoError)?;
            return Err(DcCmdError::InvalidAccount);
        };

        self.term
            .write_line(
                format!(
                    "► Encryption secret securely stored for {}.",
                    target.trim_end_matches("-crypto")
                )
                .as_str(),
            )
            .map_err(|_| DcCmdError::IoError)?;

        Ok(())
    }

    pub fn remove_encryption_secret(&self, target: &str) -> Result<(), DcCmdError> {
        self.entry.delete_dracoon_env()?;
        self.term
            .write_line(format!("► Encryption secret removed for {target}.").as_str())
            .map_err(|_| DcCmdError::IoError)?;

        Ok(())
    }
}

pub async fn handle_config_cmd(cmd: ConfigCommand, term: Term) -> Result<(), DcCmdError> {
    match cmd {
        ConfigCommand::Auth { cmd } => match cmd {
            ConfigAuthCommand::Ls { target } => {
                let (target, entry) = prepare_config_cmd(&target, &term, false)?;

                let handler = ConfigCommandHandler::new(entry, term);
                handler.get_refresh_token_info(target).await?;
                Ok(())
            }
            ConfigAuthCommand::Rm { target } => {
                let (target, entry) = prepare_config_cmd(&target, &term, false)?;

                let handler = ConfigCommandHandler::new(entry, term);
                handler.remove_refresh_token(&target)?;
                Ok(())
            }
        },
        ConfigCommand::Crypto { cmd } => match cmd {
            ConfigCryptoCommand::Ls { target } => {
                let (target, entry) = prepare_config_cmd(&target, &term, true)?;

                let handler = ConfigCommandHandler::new(entry, term);
                handler.get_encryption_secret_info(&target)?;
                Ok(())
            }
            ConfigCryptoCommand::Rm { target } => {
                let (target, entry) = prepare_config_cmd(&target, &term, true)?;

                let handler = ConfigCommandHandler::new(entry, term);
                handler.remove_encryption_secret(&target)?;
                Ok(())
            }
        },
        ConfigCommand::SystemInfo { target } => {
            let (target, entry) = prepare_config_cmd(&target, &term, false)?;

            let handler = ConfigCommandHandler::new(entry, term);

            handler.get_system_info(&target).await?;

            Ok(())
        }
    }
}

fn prepare_config_cmd(
    target: &str,
    term: &Term,
    is_crypto: bool,
) -> Result<(String, impl HandleCredentials), DcCmdError> {
    let base_url = format!(
        "https://{}",
        target
            .strip_prefix("https://")
            .unwrap_or(target)
            .trim_end_matches('/')
    );

    let base_url = if is_crypto {
        format!("{base_url}/-crypto")
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

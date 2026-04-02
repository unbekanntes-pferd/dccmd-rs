use crate::{
    app::requests::{ConfigAuthRequest, ConfigCryptoRequest, ConfigRequest},
    app::{config::service::ConfigService, results::ConfigPlatformResult},
    core::models::DcCmdError,
};

use super::CliPlatform;

impl CliPlatform {
    pub(super) async fn execute_config_cmd(
        &self,
        cmd: ConfigRequest,
    ) -> Result<ConfigPlatformResult, DcCmdError> {
        let service = ConfigService::new();

        match cmd {
            ConfigRequest::Auth { cmd } => match cmd {
                ConfigAuthRequest::Ls { target } => {
                    let base_url = service.normalize_base_url(&target)?;
                    match service.get_refresh_token_info(&target).await {
                        Ok(user_info) => Ok(ConfigPlatformResult::AuthTokenInfo {
                            base_url,
                            user_info,
                        }),
                        Err(DcCmdError::InvalidAccount) => {
                            Ok(ConfigPlatformResult::MissingToken { base_url })
                        }
                        Err(e) => Err(e),
                    }
                }
                ConfigAuthRequest::Rm { target } => {
                    let base_url = service.normalize_base_url(&target)?;
                    service.remove_refresh_token(&target)?;
                    Ok(ConfigPlatformResult::AuthTokenRemoved { base_url })
                }
            },
            ConfigRequest::Crypto { cmd } => match cmd {
                ConfigCryptoRequest::Ls { target } => {
                    let base_url = service.normalize_base_url(&target)?;
                    if !service.has_encryption_secret(&target)? {
                        return Ok(ConfigPlatformResult::MissingCryptoSecret);
                    }
                    Ok(ConfigPlatformResult::CryptoSecretStored { base_url })
                }
                ConfigCryptoRequest::Rm { target } => {
                    let base_url = service.normalize_base_url(&target)?;
                    service.remove_encryption_secret(&target)?;
                    Ok(ConfigPlatformResult::CryptoSecretRemoved { base_url })
                }
            },
            ConfigRequest::SystemInfo { target } => {
                let base_url = service.normalize_base_url(&target)?;
                match service.get_system_info(&target).await {
                    Ok(system_info) => Ok(ConfigPlatformResult::SystemInfo {
                        base_url,
                        system_info,
                    }),
                    Err(DcCmdError::InvalidAccount) => {
                        Ok(ConfigPlatformResult::MissingToken { base_url })
                    }
                    Err(e) => Err(e),
                }
            }
        }
    }
}

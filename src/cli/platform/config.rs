use dco3::{auth::Connected, Dracoon};

use crate::{
    app::requests::{ConfigAuthRequest, ConfigCryptoRequest, ConfigRequest},
    app::{
        auth::AuthService,
        config::{api::SystemApi, service::ConfigService},
        results::ConfigPlatformResult,
    },
    core::models::DcCmdError,
};

use super::CliPlatform;

enum StoredRefreshClient {
    MissingToken {
        base_url: String,
    },
    Connected {
        base_url: String,
        client: Dracoon<Connected>,
    },
}

impl CliPlatform {
    async fn connect_config_refresh_client(
        &self,
        service: &ConfigService,
        target: &str,
    ) -> Result<StoredRefreshClient, DcCmdError> {
        let base_url = service.normalize_base_url(target)?;
        let refresh_token = match service.get_refresh_token(target) {
            Ok(refresh_token) => refresh_token,
            Err(DcCmdError::InvalidAccount) => {
                return Ok(StoredRefreshClient::MissingToken { base_url });
            }
            Err(e) => return Err(e),
        };

        match AuthService::new()
            .connect_with_refresh_token(&base_url, refresh_token)
            .await
        {
            Ok(session) => Ok(StoredRefreshClient::Connected {
                base_url,
                client: session.into_client(),
            }),
            Err(DcCmdError::InvalidAccount) => Ok(StoredRefreshClient::MissingToken { base_url }),
            Err(e) => Err(e),
        }
    }

    pub(super) async fn execute_config_cmd(
        &self,
        cmd: ConfigRequest,
    ) -> Result<ConfigPlatformResult, DcCmdError> {
        let service = ConfigService::new();

        match cmd {
            ConfigRequest::Auth { cmd } => match cmd {
                ConfigAuthRequest::Ls { target } => match self
                    .connect_config_refresh_client(&service, &target)
                    .await?
                {
                    StoredRefreshClient::MissingToken { base_url } => {
                        Ok(ConfigPlatformResult::MissingToken { base_url })
                    }
                    StoredRefreshClient::Connected { base_url, client } => {
                        let user_info = SystemApi::get_refresh_token_info(&client).await?;
                        Ok(ConfigPlatformResult::AuthTokenInfo {
                            base_url,
                            user_info,
                        })
                    }
                },
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
            ConfigRequest::SystemInfo { target } => match self
                .connect_config_refresh_client(&service, &target)
                .await?
            {
                StoredRefreshClient::MissingToken { base_url } => {
                    Ok(ConfigPlatformResult::MissingToken { base_url })
                }
                StoredRefreshClient::Connected { base_url, client } => {
                    let system_info = SystemApi::get_system_info(&client).await?;
                    Ok(ConfigPlatformResult::SystemInfo {
                        base_url,
                        system_info,
                    })
                }
            },
        }
    }
}

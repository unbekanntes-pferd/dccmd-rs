use async_trait::async_trait;
use dco3::{auth::Connected, AuthenticationMethods, Dracoon, User};

use crate::core::models::DcCmdError;

#[derive(Clone)]
pub struct RefreshTokenInfo {
    pub first_name: String,
    pub last_name: String,
    pub email: Option<String>,
    pub user_name: String,
}

#[derive(Clone)]
pub struct OpenIdConfigInfo {
    pub id: u64,
    pub name: String,
}

#[derive(Clone)]
pub struct ActiveDirectoryConfigInfo {
    pub id: u64,
    pub alias: String,
}

#[derive(Clone)]
pub struct CustomerInfo {
    pub name: String,
    pub space_used: u64,
    pub space_limit: u64,
    pub accounts_used: u64,
    pub accounts_limit: u64,
}

#[derive(Clone)]
pub struct SystemInfo {
    pub customer: CustomerInfo,
    pub oidc_configs: Vec<OpenIdConfigInfo>,
    pub ad_configs: Vec<ActiveDirectoryConfigInfo>,
}

#[async_trait]
pub trait SystemApi: Send + Sync {
    async fn get_refresh_token_info(&self) -> Result<RefreshTokenInfo, DcCmdError>;
    async fn get_system_info(&self) -> Result<SystemInfo, DcCmdError>;
}

#[async_trait]
impl SystemApi for Dracoon<Connected> {
    async fn get_refresh_token_info(&self) -> Result<RefreshTokenInfo, DcCmdError> {
        let user_info = self.get_user_info().await?;

        Ok(RefreshTokenInfo {
            first_name: user_info.first_name,
            last_name: user_info.last_name,
            email: user_info.email,
            user_name: user_info.user_name,
        })
    }

    async fn get_system_info(&self) -> Result<SystemInfo, DcCmdError> {
        let oidc_info = self.system().auth.get_openid_idp_configurations().await?;
        let ad_info = self
            .system()
            .auth
            .get_active_directory_configurations()
            .await?;
        let customer_info = self.user().get_customer_info().await?;

        let oidc_configs = oidc_info
            .into_iter()
            .map(|cfg| OpenIdConfigInfo {
                id: cfg.id,
                name: cfg.name.unwrap_or_default(),
            })
            .collect::<Vec<_>>();

        let ad_configs = ad_info
            .items
            .into_iter()
            .map(|cfg| ActiveDirectoryConfigInfo {
                id: cfg.id,
                alias: cfg.alias,
            })
            .collect::<Vec<_>>();

        Ok(SystemInfo {
            customer: CustomerInfo {
                name: customer_info.name,
                space_used: customer_info.space_used,
                space_limit: customer_info.space_limit,
                accounts_used: customer_info.accounts_used,
                accounts_limit: customer_info.accounts_limit,
            },
            oidc_configs,
            ad_configs,
        })
    }
}

use crate::{
    app::{
        outcome::CommandOutcome,
        requests::{ConfigAuthRequest, ConfigRequest},
        results::{AppResult, AppStatus, ConfigPlatformResult},
        App, Platform, Ui,
    },
    core::models::DcCmdError,
};

impl<P: Platform, U: Ui> App<P, U> {
    pub(in crate::app) async fn handle_config(
        &self,
        cmd: ConfigRequest,
    ) -> Result<AppResult, DcCmdError> {
        let mut outcome = CommandOutcome::default();
        let mut status = AppStatus::Success;
        if matches!(
            &cmd,
            ConfigRequest::Auth {
                cmd: ConfigAuthRequest::Rm { .. }
            }
        ) && !self
            .ui
            .confirm("Are you sure you want to remove the token?")?
        {
            return Ok(AppResult::success(outcome));
        }

        match self.platform.config(cmd).await? {
            ConfigPlatformResult::AuthTokenInfo {
                base_url,
                user_info,
            } => {
                self.write_info(&mut outcome, &format!("► Token stored for: {base_url}"))?;
                self.write_info(
                    &mut outcome,
                    &format!("► User: {} {}", user_info.first_name, user_info.last_name),
                )?;
                self.write_info(
                    &mut outcome,
                    &format!(
                        "► Email: {}",
                        user_info.email.unwrap_or_else(|| "N/A".to_string())
                    ),
                )?;
                self.write_info(
                    &mut outcome,
                    &format!("► Username: {}", user_info.user_name),
                )?;
            }
            ConfigPlatformResult::AuthTokenRemoved { base_url } => {
                self.write_success(&mut outcome, &format!("Token removed for {base_url}"))?;
            }
            ConfigPlatformResult::CryptoSecretStored { base_url } => {
                self.write_info(
                    &mut outcome,
                    &format!("► Encryption secret securely stored for {base_url}."),
                )?;
            }
            ConfigPlatformResult::CryptoSecretRemoved { base_url } => {
                self.write_success(
                    &mut outcome,
                    &format!("Encryption secret removed for {base_url}."),
                )?;
            }
            ConfigPlatformResult::SystemInfo {
                base_url,
                system_info,
            } => {
                let customer_info = system_info.customer;
                self.write_info(&mut outcome, &format!("► System info for: {base_url}"))?;
                self.write_info(&mut outcome, &format!("► Customer: {}", customer_info.name))?;

                let percent_space_used =
                    (customer_info.space_used as f64 / customer_info.space_limit as f64) * 100.0;
                let percent_users_used = (customer_info.accounts_used as f64
                    / customer_info.accounts_limit as f64)
                    * 100.0;

                self.write_info(
                    &mut outcome,
                    &format!(
                        "► Space used: {} / {} ({percent_space_used:.2}%)",
                        crate::core::utils::strings::to_readable_size(customer_info.space_used),
                        crate::core::utils::strings::to_readable_size(customer_info.space_limit)
                    ),
                )?;
                self.write_info(
                    &mut outcome,
                    &format!(
                        "► Users used: {} / {} ({percent_users_used:.2}%)",
                        customer_info.accounts_used, customer_info.accounts_limit
                    ),
                )?;

                if !system_info.oidc_configs.is_empty() {
                    self.write_info(&mut outcome, "\n► OpenID Connect IDP configurations:")?;
                    for info in system_info.oidc_configs {
                        self.write_info(&mut outcome, &format!("► {} ({})", info.name, info.id))?;
                    }
                } else {
                    self.write_info(
                        &mut outcome,
                        "\n► No OpenID Connect IDP configurations found.",
                    )?;
                }

                if !system_info.ad_configs.is_empty() {
                    self.write_info(&mut outcome, "\n► Active Directory configurations:")?;
                    for info in system_info.ad_configs {
                        self.write_info(&mut outcome, &format!("► {} ({})", info.alias, info.id))?;
                    }
                } else {
                    self.write_info(
                        &mut outcome,
                        "\n► No Active Directory configurations found.",
                    )?;
                }
            }
            ConfigPlatformResult::MissingToken { base_url } => {
                self.write_error(
                    &mut outcome,
                    &format!("No token found for this DRACOON url: {base_url}."),
                )?;
                status = AppStatus::Failure;
            }
            ConfigPlatformResult::MissingCryptoSecret => {
                self.write_error(&mut outcome, "No encryption secret found.")?;
                status = AppStatus::Failure;
            }
        }

        Ok(super::super::app_result_from_status(status, outcome))
    }
}

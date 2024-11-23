use std::fmt::Display;

use chrono::{Date, DateTime, Utc};
use dco3::users::{UserData, UserItem};
use serde::Deserialize;
use tabled::Tabled;
use tracing::debug;

use crate::cmd::models::DcCmdError;

use super::auth::AuthMethod;

pub struct CreateUserOptions<'o> {
    pub first_name: &'o str,
    pub last_name: &'o str,
    pub email: &'o str,
    pub login: Option<&'o str>,
    pub oidc_id: Option<u32>,
    pub mfa_enforced: bool,
    pub is_import: bool,
    pub first_group_id: Option<u64>,
}

impl<'o> CreateUserOptions<'o> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        first_name: &'o str,
        last_name: &'o str,
        email: &'o str,
        login: Option<&'o str>,
        oidc_id: Option<u32>,
        mfa_enforced: bool,
        is_import: bool,
        first_group_id: Option<u64>,
    ) -> CreateUserOptions<'o> {
        CreateUserOptions {
            first_name,
            last_name,
            email,
            login,
            oidc_id,
            mfa_enforced,
            is_import,
            first_group_id,
        }
    }
}

#[derive(Tabled)]
pub struct UserInfo {
    pub id: u64,
    pub first_name: String,
    pub last_name: String,
    pub username: String,
    #[tabled(display_with = "display_option")]
    pub email: Option<String>,
    #[tabled(display_with = "display_option")]
    pub expire_at: Option<DateTime<Utc>>,
    pub is_locked: bool,
    #[tabled(display_with = "display_option")]
    pub last_login_at: Option<DateTime<Utc>>,
}

pub fn display_option<T: Display>(o: &Option<T>) -> String {
    match o {
        Some(v) => v.to_string(),
        None => "N/A".to_string(),
    }
}

impl TryFrom<UserItem> for UserInfo {
    type Error = DcCmdError;
    fn try_from(user: UserItem) -> Result<Self, Self::Error> {

        let last_login: Option<DateTime<Utc>> = if let Some(last_login) = user.last_login_success_at {
            Some(
                DateTime::parse_from_rfc3339(&last_login)
                    .or(Err(DcCmdError::InvalidArgument("Failed to parse last login date".to_string())))?
                    .into(),
            )
        } else {
            None
        };

        Ok(Self {
            id: user.id,
            first_name: user.first_name,
            last_name: user.last_name,
            username: user.user_name,
            email: user.email,
            expire_at: user.expire_at,
            is_locked: user.is_locked,
            last_login_at: last_login,
        })
    }
}

impl TryFrom<UserData> for UserInfo {
    type Error = DcCmdError;
    fn try_from(user: UserData) -> Result<Self, Self::Error> {
        let last_login: Option<DateTime<Utc>> = if let Some(last_login) = user.last_login_success_at {
            Some(
                DateTime::parse_from_rfc3339(&last_login)
                    .or(Err(DcCmdError::InvalidArgument("Failed to parse last login date".to_string())))?
                    .into(),
            )
        } else {
            None
        };

        let expire_at: Option<DateTime<Utc>> = if let Some(expire_at) = user.expire_at {
            Some(
                DateTime::parse_from_rfc3339(&expire_at)
                    .or(Err(DcCmdError::InvalidArgument("Failed to parse expire date".to_string())))?
                    .into(),
            )
        } else {
            None
        };

        Ok(Self {
            id: user.id,
            first_name: user.first_name,
            last_name: user.last_name,
            username: user.user_name,
            email: user.email,
            expire_at,
            is_locked: user.is_locked,
            last_login_at: last_login,
        })
    }
}

#[derive(Deserialize)]
pub struct UserImport {
    pub first_name: String,
    pub last_name: String,
    pub email: String,
    pub login: Option<String>,
    pub mfa_enabled: Option<bool>,
}

pub struct UsersSwitchAuthOptions {
    curr_method: String,
    new_method: String,
    curr_oidc_id: Option<u64>,
    new_oidc_id: Option<u64>,
    curr_ad_id: Option<u64>,
    new_ad_id: Option<u64>,
    filter: Option<String>,
    login: Box<dyn Fn(&UserData) -> String>,
}

impl UsersSwitchAuthOptions {
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        curr_method: String,
        new_method: String,
        curr_oidc_id: Option<u64>,
        new_oidc_id: Option<u64>,
        curr_ad_id: Option<u64>,
        new_ad_id: Option<u64>,
        filter: Option<String>,
        login: Option<String>,
    ) -> Result<Self, DcCmdError> {
        let is_same_method = curr_method == new_method;
        let is_same_oidc_id = curr_oidc_id == new_oidc_id;
        let is_same_ad_id = curr_ad_id == new_ad_id;

        debug!(
            "Switching auth method from {} to {}",
            curr_method, new_method
        );
        debug!("Login transformation: {:?}", login);

        // local to local not allowed
        if curr_method == AuthMethod::Local.to_string() && is_same_method {
            return Err(DcCmdError::InvalidArgument(
                "Cannot switch from local to local (same method).".to_string(),
            ));
        }

        // oidc to oidc (same id) not allowed
        if curr_method == AuthMethod::Oidc.to_string() && is_same_oidc_id {
            return Err(DcCmdError::InvalidArgument(
                "Cannot switch from OIDC to OIDC (same OIDC ID).".to_string(),
            ));
        }

        // ad to ad (same id) not allowed
        if curr_method == AuthMethod::Ad.to_string() && is_same_ad_id {
            return Err(DcCmdError::InvalidArgument(
                "Cannot switch from AD to AD (same AD ID).".to_string(),
            ));
        }

        // build transform login function
        let login_fn = match login {
            Some(l) => UsersSwitchAuthOptions::build_login_fn(l),
            None => Box::new(|user: &UserData| {
                user.email
                    .as_deref()
                    .unwrap_or(user.user_name.as_str())
                    .to_string()
            }),
        };

        Ok(Self {
            curr_method,
            new_method,
            curr_oidc_id,
            new_oidc_id,
            curr_ad_id,
            new_ad_id,
            filter,
            login: login_fn,
        })
    }

    fn build_login_fn(login: String) -> Box<dyn Fn(&UserData) -> String> {
        let first_name_str = "firstname";
        let last_name_str = "lastname";
        let user_name_str = "username";

        Box::new(move |user| match login.as_str().to_lowercase().trim() {
            l if l.contains(first_name_str) || l.contains(last_name_str) => {
                debug!("Transforming login using: {}", l);
                let first_name = user.first_name.to_lowercase();
                let last_name = user.last_name.to_lowercase();
                l.replace(first_name_str, &first_name)
                    .replace(last_name_str, &last_name)
            }
            l if l.contains(user_name_str) => {
                let user_name = &user.user_name;
                l.replace(user_name_str, user_name)
            }
            "email" => user
                .email
                .as_deref()
                .unwrap_or(format!("{}@example.com", user.user_name.as_str()).as_str())
                .to_string(),
            "username" => user.user_name.clone(),
            _ => user
                .email
                .as_deref()
                .unwrap_or(user.user_name.as_str())
                .to_string(),
        })
    }

    pub fn curr_method(&self) -> &str {
        &self.curr_method
    }

    pub fn new_method(&self) -> &str {
        &self.new_method
    }

    pub fn curr_oidc_id(&self) -> Option<u64> {
        self.curr_oidc_id
    }

    pub fn new_oidc_id(&self) -> Option<u64> {
        self.new_oidc_id
    }

    pub fn curr_ad_id(&self) -> Option<u64> {
        self.curr_ad_id
    }

    pub fn new_ad_id(&self) -> Option<u64> {
        self.new_ad_id
    }

    pub fn filter(&self) -> Option<String> {
        self.filter.clone()
    }

    pub fn transform_login(&self) -> &dyn Fn(&UserData) -> String {
        &self.login
    }
}

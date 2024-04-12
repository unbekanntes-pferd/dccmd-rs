use std::fmt::Display;

use chrono::{DateTime, Utc};
use dco3::users::{UserData, UserItem};
use serde::Deserialize;
use tabled::Tabled;

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

fn display_option<T: Display>(o: &Option<T>) -> String {
    match o {
        Some(v) => v.to_string(),
        None => "N/A".to_string(),
    }
}

impl From<UserItem> for UserInfo {
    fn from(user: UserItem) -> Self {
        let last_login = if user.last_login_success_at.is_none() {
            None
        } else {
            Some(
                DateTime::parse_from_rfc3339(&user.last_login_success_at.unwrap())
                    .expect("Failed to parse last login date")
                    .into(),
            )
        };

        Self {
            id: user.id,
            first_name: user.first_name,
            last_name: user.last_name,
            username: user.user_name,
            email: user.email,
            expire_at: user.expire_at,
            is_locked: user.is_locked,
            last_login_at: last_login,
        }
    }
}

impl From<UserData> for UserInfo {
    fn from(user: UserData) -> Self {
        let last_login = if user.last_login_success_at.is_none() {
            None
        } else {
            Some(
                DateTime::parse_from_rfc3339(&user.last_login_success_at.unwrap())
                    .expect("Failed to parse last login date")
                    .into(),
            )
        };

        let expire_at = if user.expire_at.is_none() {
            None
        } else {
            Some(
                DateTime::parse_from_rfc3339(&user.expire_at.unwrap())
                    .expect("Failed to parse expire date")
                    .into(),
            )
        };

        Self {
            id: user.id,
            first_name: user.first_name,
            last_name: user.last_name,
            username: user.user_name,
            email: user.email,
            expire_at,
            is_locked: user.is_locked,
            last_login_at: last_login,
        }
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

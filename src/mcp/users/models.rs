use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::app::users::models::UserInfo;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct McpUser {
    pub id: u64,
    pub username: String,
    pub first_name: String,
    pub last_name: String,
    pub email: Option<String>,
    pub expire_at: Option<String>,
    pub is_locked: bool,
    pub last_login_at: Option<String>,
}

impl From<dco3::users::UserItem> for McpUser {
    fn from(value: dco3::users::UserItem) -> Self {
        Self {
            id: value.id,
            username: value.user_name,
            first_name: value.first_name,
            last_name: value.last_name,
            email: value.email,
            expire_at: value.expire_at.map(|value| value.to_rfc3339()),
            is_locked: value.is_locked,
            last_login_at: value.last_login_success_at,
        }
    }
}

impl From<dco3::users::UserData> for McpUser {
    fn from(value: dco3::users::UserData) -> Self {
        Self {
            id: value.id,
            username: value.user_name,
            first_name: value.first_name,
            last_name: value.last_name,
            email: value.email,
            expire_at: value.expire_at,
            is_locked: value.is_locked,
            last_login_at: value.last_login_success_at,
        }
    }
}

impl From<UserInfo> for McpUser {
    fn from(value: UserInfo) -> Self {
        Self {
            id: value.id,
            username: value.username,
            first_name: value.first_name,
            last_name: value.last_name,
            email: value.email,
            expire_at: value.expire_at.map(|value| value.to_rfc3339()),
            is_locked: value.is_locked,
            last_login_at: value.last_login_at.map(|value| value.to_rfc3339()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct McpOidcIdp {
    pub id: u64,
    pub name: String,
}

impl From<crate::app::config::api::OpenIdConfigInfo> for McpOidcIdp {
    fn from(value: crate::app::config::api::OpenIdConfigInfo) -> Self {
        Self {
            id: value.id,
            name: value.name,
        }
    }
}

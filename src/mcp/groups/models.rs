use dco3::groups::{Group, GroupUser};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct McpGroup {
    pub id: u64,
    pub name: String,
    pub created_at: String,
    pub updated_at: Option<String>,
    pub expire_at: Option<String>,
    pub cnt_users: Option<u64>,
}

impl From<Group> for McpGroup {
    fn from(value: Group) -> Self {
        Self {
            id: value.id,
            name: value.name,
            created_at: value.created_at.to_rfc3339(),
            updated_at: value.updated_at.map(|value| value.to_rfc3339()),
            expire_at: value.expire_at.map(|value| value.to_rfc3339()),
            cnt_users: value.cnt_users,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct McpGroupUser {
    pub id: i64,
    pub username: Option<String>,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub email: Option<String>,
    pub is_member: bool,
}

impl From<GroupUser> for McpGroupUser {
    fn from(value: GroupUser) -> Self {
        Self {
            id: value.user_info.id,
            username: value.user_info.user_name,
            first_name: value.user_info.first_name,
            last_name: value.user_info.last_name,
            email: value.user_info.email,
            is_member: value.is_member,
        }
    }
}

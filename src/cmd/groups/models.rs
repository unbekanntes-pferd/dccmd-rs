use clap::Parser;
use dco3::groups::{Group, GroupUser};
use tabled::Tabled;

#[derive(Tabled)]
pub struct GroupInfo {
    pub id: String,
    pub name: String,
    pub created_at: String,
    pub cnt_users: u64,
    #[tabled(display_with = "crate::cmd::users::display_option")]
    pub updated_at: Option<String>,
}

impl From<Group> for GroupInfo {
    fn from(group: Group) -> Self {
        Self {
            id: group.id.to_string(),
            name: group.name,
            cnt_users: group.cnt_users.unwrap_or(0),
            created_at: group.created_at.to_string(),
            updated_at: group.updated_at.map(|dt| dt.to_rfc3339()),
        }
    }
}

#[derive(Tabled)]
pub struct GroupUserInfo {
    pub id: i64,
    pub email: String,
    pub first_name: String,
    pub last_name: String,
    pub user_name: String,
    pub group_id: u64,
    pub group_name: String,
}

impl GroupUserInfo {
    pub fn new(user: GroupUser, group: Group) -> Self {
        Self {
            id: user.user_info.id,
            email: user.user_info.email.unwrap_or("N/A".to_string()),
            first_name: user.user_info.first_name.unwrap_or("N/A".to_string()),
            last_name: user.user_info.last_name.unwrap_or("N/A".to_string()),
            user_name: user.user_info.user_name.unwrap_or("N/A".to_string()),
            group_id: group.id,
            group_name: group.name,
        }
    }
}

#[derive(Parser)]
pub enum GroupsUsersCommand {
    Ls {
        target: String,

        /// filter (group e.g. user name)
        #[clap(long)]
        filter: Option<String>,

        /// skip n users (default offset: 0)
        #[clap(short, long)]
        offset: Option<u32>,

        /// limit n users (default limit: 500)
        #[clap(long)]
        limit: Option<u32>,

        /// fetch all group users (default: 500)
        #[clap(long)]
        all: bool,

        /// print user information in CSV format
        #[clap(long)]
        csv: bool,
    },
}

pub struct GroupUsersOptions {
    pub filter: Option<String>,
    pub offset: Option<u32>,
    pub limit: Option<u32>,
    pub all: bool,
    pub csv: bool,
}

impl GroupUsersOptions {
    pub fn new(
        filter: Option<String>,
        offset: Option<u32>,
        limit: Option<u32>,
        all: bool,
        csv: bool,
    ) -> Self {
        Self {
            filter,
            offset,
            limit,
            all,
            csv,
        }
    }
}

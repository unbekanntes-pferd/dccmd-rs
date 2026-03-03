use dco3::{
    eventlog::{AuditNodeList, LogEventList, LogOperationList},
    groups::Group,
    users::UserItem,
    RangedItems,
};
use std::ops::Deref;

use crate::app::{
    config::api::{RefreshTokenInfo, SystemInfo},
    groups::GroupUsersPage,
    outcome::CommandOutcome,
    users::{models::UserInfo, EnforceMfaResult, ImportUsersResult, SwitchAuthResult},
};

pub struct AppResult {
    outcome: CommandOutcome,
}

impl AppResult {
    pub fn messages(outcome: CommandOutcome) -> Self {
        Self { outcome }
    }

    pub fn into_outcome(self) -> CommandOutcome {
        self.outcome
    }
}

impl Deref for AppResult {
    type Target = CommandOutcome;

    fn deref(&self) -> &Self::Target {
        &self.outcome
    }
}

pub enum UsersPlatformResult {
    Created {
        user_name: String,
        user_id: u64,
        auth_method: String,
    },
    Invited {
        first_name: String,
        last_name: String,
    },
    Listed {
        users: RangedItems<UserItem>,
        csv: bool,
    },
    Removed {
        message: String,
    },
    Imported(ImportUsersResult),
    Info {
        user: UserInfo,
    },
    SwitchedAuth(SwitchAuthResult),
    EnforcedMfa(EnforceMfaResult),
}

pub enum GroupsPlatformResult {
    Created {
        group_name: String,
        group_id: u64,
    },
    Listed {
        groups: RangedItems<Group>,
        csv: bool,
    },
    Removed {
        group_id: u64,
    },
    UsersListed {
        pages: Vec<GroupUsersPage>,
        csv: bool,
    },
    UserAdded {
        group_id: u64,
        user_id: u64,
    },
}

pub enum ReportsPlatformResult {
    Events {
        events: LogEventList,
        csv: bool,
    },
    Permissions {
        permissions: AuditNodeList,
        csv: bool,
    },
    OperationTypes {
        operations: LogOperationList,
    },
}

pub enum ConfigPlatformResult {
    AuthTokenInfo {
        base_url: String,
        user_info: RefreshTokenInfo,
    },
    AuthTokenRemoved {
        base_url: String,
    },
    CryptoSecretStored {
        base_url: String,
    },
    CryptoSecretRemoved {
        base_url: String,
    },
    SystemInfo {
        base_url: String,
        system_info: SystemInfo,
    },
    MissingToken {
        base_url: String,
    },
    MissingCryptoSecret,
}

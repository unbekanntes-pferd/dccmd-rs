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
    nodes::{download::DownloadOutcome, transfer::TransferOutcome, upload::UploadOutcome},
    outcome::CommandOutcome,
    users::{models::UserInfo, EnforceMfaResult, ImportUsersResult, SwitchAuthResult},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppStatus {
    Success,
    PartialFailure,
    Failure,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppPayload {
    Download(DownloadOutcome),
    Upload(UploadOutcome),
    Transfer(TransferOutcome),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppResult {
    status: AppStatus,
    outcome: CommandOutcome,
    payload: Option<AppPayload>,
}

impl AppResult {
    pub fn success(outcome: CommandOutcome) -> Self {
        Self {
            status: AppStatus::Success,
            outcome,
            payload: None,
        }
    }

    pub fn partial_failure(outcome: CommandOutcome) -> Self {
        Self {
            status: AppStatus::PartialFailure,
            outcome,
            payload: None,
        }
    }

    pub fn failure(outcome: CommandOutcome) -> Self {
        Self {
            status: AppStatus::Failure,
            outcome,
            payload: None,
        }
    }

    pub fn with_payload(mut self, payload: AppPayload) -> Self {
        self.payload = Some(payload);
        self
    }

    pub fn status(&self) -> AppStatus {
        self.status
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn payload(&self) -> Option<&AppPayload> {
        self.payload.as_ref()
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

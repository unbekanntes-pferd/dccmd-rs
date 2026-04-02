use crate::{
    app::requests::{
        ConfigAuthRequest, ConfigCryptoRequest, ConfigRequest, GroupsRequest, GroupsUsersRequest,
        ReportsRequest, UsersRequest,
    },
    command::{
        config::{ConfigAuthCommand, ConfigCommand, ConfigCryptoCommand},
        GroupsCommand, GroupsUsersCommand, ReportsCommand, UsersCommand,
    },
};

pub(super) fn map_config_request(cmd: ConfigCommand) -> ConfigRequest {
    match cmd {
        ConfigCommand::Auth { cmd } => ConfigRequest::Auth {
            cmd: match cmd {
                ConfigAuthCommand::Ls { target } => ConfigAuthRequest::Ls { target },
                ConfigAuthCommand::Rm { target } => ConfigAuthRequest::Rm { target },
            },
        },
        ConfigCommand::Crypto { cmd } => ConfigRequest::Crypto {
            cmd: match cmd {
                ConfigCryptoCommand::Ls { target } => ConfigCryptoRequest::Ls { target },
                ConfigCryptoCommand::Rm { target } => ConfigCryptoRequest::Rm { target },
            },
        },
        ConfigCommand::SystemInfo { target } => ConfigRequest::SystemInfo { target },
    }
}

pub(super) fn map_users_request(cmd: UsersCommand) -> UsersRequest {
    match cmd {
        UsersCommand::Ls {
            target,
            filter,
            offset,
            limit,
            all,
            csv,
        } => UsersRequest::Ls {
            target,
            filter,
            offset,
            limit,
            all,
            csv,
        },
        UsersCommand::Create {
            target,
            first_name,
            last_name,
            email,
            login,
            oidc_id,
            mfa_enforced,
            group_id,
        } => UsersRequest::Create {
            target,
            first_name,
            last_name,
            email,
            login,
            oidc_id,
            mfa_enforced,
            group_id,
        },
        UsersCommand::Invite {
            target,
            first_name,
            last_name,
            email,
        } => UsersRequest::Invite {
            target,
            first_name,
            last_name,
            email,
        },
        UsersCommand::Rm {
            target,
            user_name,
            user_id,
        } => UsersRequest::Rm {
            target,
            user_name,
            user_id,
        },
        UsersCommand::Import {
            target,
            source,
            oidc_id,
        } => UsersRequest::Import {
            target,
            source,
            oidc_id,
        },
        UsersCommand::Info {
            target,
            user_name,
            user_id,
        } => UsersRequest::Info {
            target,
            user_name,
            user_id,
        },
        UsersCommand::SwitchAuth {
            target,
            current_method,
            new_method,
            current_oidc_id,
            new_oidc_id,
            current_ad_id,
            new_ad_id,
            filter,
            login,
        } => UsersRequest::SwitchAuth {
            target,
            current_method,
            new_method,
            current_oidc_id,
            new_oidc_id,
            current_ad_id,
            new_ad_id,
            filter,
            login,
        },
        UsersCommand::EnforceMfa {
            target,
            auth_method,
            filter,
            auth_method_id,
            group_id,
        } => UsersRequest::EnforceMfa {
            target,
            auth_method,
            filter,
            auth_method_id,
            group_id,
        },
    }
}

pub(super) fn map_groups_request(cmd: GroupsCommand) -> GroupsRequest {
    match cmd {
        GroupsCommand::Ls {
            target,
            filter,
            offset,
            limit,
            all,
            csv,
        } => GroupsRequest::Ls {
            target,
            filter,
            offset,
            limit,
            all,
            csv,
        },
        GroupsCommand::Create { target, name } => GroupsRequest::Create { target, name },
        GroupsCommand::Rm {
            target,
            group_name,
            group_id,
        } => GroupsRequest::Rm {
            target,
            group_name,
            group_id,
        },
        GroupsCommand::Users { cmd } => GroupsRequest::Users {
            cmd: match cmd {
                GroupsUsersCommand::Ls {
                    target,
                    filter,
                    offset,
                    limit,
                    all,
                    csv,
                } => GroupsUsersRequest::Ls {
                    target,
                    filter,
                    offset,
                    limit,
                    all,
                    csv,
                },
                GroupsUsersCommand::Add {
                    target,
                    group_name,
                    group_id,
                    user_name,
                    user_id,
                } => GroupsUsersRequest::Add {
                    target,
                    group_name,
                    group_id,
                    user_name,
                    user_id,
                },
            },
        },
    }
}

pub(super) fn map_reports_request(cmd: ReportsCommand) -> ReportsRequest {
    match cmd {
        ReportsCommand::Events {
            target,
            filter,
            offset,
            limit,
            all,
            csv,
            operation_type,
            user_id,
            status,
            start_date,
            end_date,
        } => ReportsRequest::Events {
            target,
            filter,
            offset,
            limit,
            all,
            csv,
            operation_type,
            user_id,
            status,
            start_date,
            end_date,
        },
        ReportsCommand::OperationTypes { target } => ReportsRequest::OperationTypes { target },
        ReportsCommand::Permissions {
            target,
            filter,
            offset,
            limit,
            all,
            csv,
        } => ReportsRequest::Permissions {
            target,
            filter,
            offset,
            limit,
            all,
            csv,
        },
    }
}

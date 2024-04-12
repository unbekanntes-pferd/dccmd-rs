use std::sync::Arc;

use console::Term;
use dco3::{
    auth::Connected,
    user::UserAuthData,
    users::{CreateUserRequest, UserItem, UsersFilter},
    Dracoon, ListAllParams, Users,
};
use futures_util::{future::join_all, stream, StreamExt};
use tokio::sync::Mutex;
use tracing::{error, info};

mod models;
mod print;

use super::{
    init_dracoon,
    models::{DcCmdError, UserCommand},
    utils::strings::format_success_message,
};

use crate::cmd::users::models::UserImport;

use self::models::UserInfo;

pub struct UserCommandHandler {
    client: Dracoon<Connected>,
    term: Term,
}

impl UserCommandHandler {
    pub async fn try_new(target_domain: &str, term: Term) -> Result<Self, DcCmdError> {
        let client = init_dracoon(target_domain, None, false).await?;
        Ok(Self { client, term })
    }

    pub async fn new_from_client(client: Dracoon<Connected>, term: Term) -> Self {
        Self { client, term }
    }

    async fn import_users(&self, source: String, oidc_id: Option<u32>) -> Result<(), DcCmdError> {
        let mut rdr = csv::ReaderBuilder::new()
            .flexible(true)
            .trim(csv::Trim::All)
            .from_path(&source)
            .map_err(|e| {
                error!("Error reading file: {}", e);
                DcCmdError::InvalidArgument(format!("File not found: {}", source))
            })?;

        let imports = rdr
            .deserialize::<UserImport>()
            .collect::<Result<Vec<_>, csv::Error>>()
            .map_err(|e| {
                error!("Error reading record: {}", e);
                DcCmdError::InvalidArgument(format!("Invalid CSV format. Expected fields: first_name, last_name, email, login (optional), mfa_enabled (optional).\n{})", e))
            })?;

        // build requests per import
        let reqs = imports
            .iter()
            .map(|import| {
                self.create_user(
                    &import.first_name,
                    &import.last_name,
                    &import.email,
                    import.login.as_deref(),
                    oidc_id,
                    true,
                    import.mfa_enabled.unwrap_or(false),
                )
            })
            .collect::<Vec<_>>();

        stream::iter(reqs)
            .chunks(5)
            .for_each_concurrent(None, |f| async move {
                join_all(f).await;
            })
            .await;

        let msg = format!("{} users imported", imports.len());

        self.term
            .write_line(format_success_message(&msg).as_str())
            .map_err(|_| DcCmdError::IoError)?;

        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    async fn create_user(
        &self,
        first_name: &str,
        last_name: &str,
        email: &str,
        login: Option<&str>,
        oidc_id: Option<u32>,
        mfa_enforced: bool,
        is_import: bool,
    ) -> Result<(), DcCmdError> {
        let payload = if let (Some(login), Some(oidc_id)) = (login, oidc_id) {
            let user_auth_data = UserAuthData::new_oidc(login, oidc_id.into());
            CreateUserRequest::builder(first_name, last_name)
                .with_auth_data(user_auth_data)
                .with_email(email)
                .with_mfa_enforced(mfa_enforced)
                .build()
        } else if let (None, Some(oidc_id)) = (login, oidc_id) {
            let user_auth_data = UserAuthData::new_oidc(email, oidc_id.into());
            CreateUserRequest::builder(first_name, last_name)
                .with_auth_data(user_auth_data)
                .with_email(email)
                .with_mfa_enforced(mfa_enforced)
                .build()
        } else {
            let user_auth_data = UserAuthData::builder(dco3::users::AuthMethod::Basic)
                .with_must_change_password(true)
                .build();
            CreateUserRequest::builder(first_name, last_name)
                .with_auth_data(user_auth_data)
                .with_user_name(login.unwrap_or(email))
                .with_email(email)
                .with_notify_user(true)
                .with_mfa_enforced(mfa_enforced)
                .build()
        };

        let user = self.client.users.create_user(payload).await?;

        info!(
            "User {} created (id: {} | auth: {})",
            user.user_name, user.id, user.auth_data.method
        );

        if !is_import {
            self.term
                .write_line(
                    format_success_message(format!("User {} created", user.user_name).as_str())
                        .as_str(),
                )
                .map_err(|_| DcCmdError::IoError)?;

            self.term
                .write_line(&format!("► user id: {}", user.id))
                .map_err(|_| DcCmdError::IoError)?;

            self.term
                .write_line(&format!("► user login: {}", user.user_name))
                .map_err(|_| DcCmdError::IoError)?;

            self.term
                .write_line(&format!("► auth method: {}", user.auth_data.method))
                .map_err(|_| DcCmdError::IoError)?;
        }

        Ok(())
    }

    async fn list_users(
        &self,
        search: Option<String>,
        offset: Option<u32>,
        limit: Option<u32>,
        all: bool,
        csv: bool,
    ) -> Result<(), DcCmdError> {
        let params = if let Some(search) = search {
            let filter = UsersFilter::username_contains(search);
            ListAllParams::builder()
                .with_filter(filter)
                .with_offset(offset.unwrap_or(0) as u64)
                .with_limit(limit.unwrap_or(500) as u64)
                .build()
        } else {
            ListAllParams::builder()
                .with_offset(offset.unwrap_or(0) as u64)
                .with_limit(limit.unwrap_or(500) as u64)
                .build()
        };

        let results = self
            .client
            .users
            .get_users(Some(params), None, None)
            .await?;

        if all {
            let total = results.range.total;
            let shared_results = Arc::new(Mutex::new(results.clone()));

            let reqs = (500..=total)
                .step_by(500)
                .map(|offset| {
                    let params = ListAllParams::builder()
                        .with_offset(offset)
                        .with_limit(500)
                        .build();
                    self.client.users.get_users(Some(params), None, None)
                })
                .collect::<Vec<_>>();

            stream::iter(reqs)
                .for_each_concurrent(5, |f| {
                    let shared_results_clone = Arc::clone(&shared_results);
                    async move {
                        match f.await {
                            Ok(mut users) => {
                                let mut shared_results = shared_results_clone.lock().await;
                                shared_results.items.append(&mut users.items);
                            }
                            Err(e) => {
                                error!("Failed to fetch users: {}", e);
                            }
                        }
                    }
                })
                .await;

            let results = shared_results.lock().await.clone();

            self.print_users(results, csv)?;
        } else {
            self.print_users(results, csv)?;
        }

        Ok(())
    }

    async fn delete_user(
        &self,
        user_name: Option<String>,
        user_id: Option<u64>,
    ) -> Result<(), DcCmdError> {
        let confirm_msg = if let Some(user_name) = user_name {
            let user = self.find_user_by_username(&user_name).await?;
            self.client.users.delete_user(user.id).await?;
            format!("User {} deleted", user_name)
        } else if let Some(user_id) = user_id {
            self.client.users.delete_user(user_id).await?;
            format!("User {} (id) deleted", user_id)
        } else {
            error!("User name or user id must be provided");
            return Err(DcCmdError::InvalidArgument(
                "User name or user id must be provided".to_string(),
            ));
        };

        self.term
            .write_line(format_success_message(confirm_msg.as_str()).as_str())
            .map_err(|_| DcCmdError::IoError)?;

        Ok(())
    }

    async fn get_user_info(
        &self,
        user_name: Option<String>,
        user_id: Option<u64>,
    ) -> Result<(), DcCmdError> {
        let user: UserInfo = if let Some(user_name) = user_name {
            self.find_user_by_username(&user_name).await?.into()
        } else if let Some(user_id) = user_id {
            self.client.users.get_user(user_id, None).await?.into()
        } else {
            error!("User name or user id must be provided");
            return Err(DcCmdError::InvalidArgument(
                "User name or user id must be provided".to_string(),
            ));
        };

        self.print_user_info(user)?;

        Ok(())
    }

    pub async fn find_user_by_username(&self, user_name: &str) -> Result<UserItem, DcCmdError> {
        let user_filter = UsersFilter::username_contains(user_name);
        let params = ListAllParams::builder().with_filter(user_filter).build();

        let results = self
            .client
            .users
            .get_users(Some(params), None, None)
            .await?;

        if results.items.is_empty() {
            error!("No user found with username: {}", user_name);
            let msg = format!("No user found with username: {}", user_name);
            return Err(DcCmdError::InvalidArgument(msg));
        }

        Ok(results.items.first().expect("No user found").clone())
    }
}

pub async fn handle_users_cmd(
    cmd: UserCommand,
    term: Term,
    target_domain: String,
) -> Result<(), DcCmdError> {
    let handler = UserCommandHandler::try_new(&target_domain, term).await?;

    let client = init_dracoon(&target_domain, None, false).await?;

    match cmd {
        UserCommand::Create {
            first_name,
            last_name,
            email,
            login,
            oidc_id,
            mfa_enforced,
        } => {
            handler
                .create_user(
                    &first_name,
                    &last_name,
                    &email,
                    login.as_deref(),
                    oidc_id,
                    mfa_enforced,
                    false,
                )
                .await?;
        }
        UserCommand::Ls {
            search,
            offset,
            limit,
            all,
            csv,
        } => {
            handler.list_users(search, offset, limit, all, csv).await?;
        }
        UserCommand::Rm { user_name, user_id } => {
            handler.delete_user(user_name, user_id).await?;
        }
        UserCommand::Import { source, oidc_id } => {
            handler.import_users(source, oidc_id).await?;
        }
        UserCommand::Info { user_name, user_id } => {
            handler.get_user_info(user_name, user_id).await?;
        }
    }
    Ok(())
}

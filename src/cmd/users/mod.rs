use std::sync::{atomic::AtomicU32, Arc};

use console::Term;
use dco3::{
    auth::Connected,
    user::UserAuthData,
    users::{CreateUserRequest, UserItem, UsersFilter},
    Dracoon, ListAllParams, RangedItems, Users,
};
use futures_util::{future::join_all, stream, StreamExt};
use indicatif::{ProgressBar, ProgressStyle};
use models::UsersSwitchAuthOptions;
use tokio::sync::Mutex;
use tracing::{error, info};

mod auth;
mod mfa;
mod models;
mod print;

use super::{
    init_dracoon,
    models::{build_params, DcCmdError, ListOptions, UsersCommand},
    utils::strings::format_success_message,
};

pub use models::display_option;

use crate::cmd::users::models::UserImport;

use self::models::UserInfo;

pub struct UserCommandHandler {
    client: Dracoon<Connected>,
    term: Term,
}

impl UserCommandHandler {
    pub async fn try_new(
        target_domain: &str,
        term: Term,
        is_import: bool,
    ) -> Result<Self, DcCmdError> {
        let client = if is_import {
            init_dracoon(target_domain, None, true).await?
        } else {
            init_dracoon(target_domain, None, false).await?
        };

        Ok(Self { client, term })
    }

    pub fn new_from_client(client: Dracoon<Connected>, term: Term) -> Self {
        Self { client, term }
    }

    async fn import_users(&self, source: String, oidc_id: Option<u32>) -> Result<(), DcCmdError> {
        let mut rdr = csv::ReaderBuilder::new()
            .flexible(true)
            .trim(csv::Trim::All)
            .from_path(&source)
            .map_err(|e| {
                error!("Error reading file: {}", e);
                DcCmdError::InvalidArgument(format!("File not found: {source}"))
            })?;

        let imports = rdr
            .deserialize::<UserImport>()
            .collect::<Result<Vec<_>, csv::Error>>()
            .map_err(|e| {
                error!("Error reading record: {e}");
                DcCmdError::InvalidArgument(format!("Invalid CSV format. Expected fields: first_name, last_name, email, login (optional), mfa_enabled (optional).\n{e})"))
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
                    import.mfa_enabled.unwrap_or(false),
                    true,
                )
            })
            .collect::<Vec<_>>();

        let progress_bar = ProgressBar::new(reqs.len() as u64);
        progress_bar.set_style(
            ProgressStyle::default_bar()
                .template(
                    "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {percent}% {msg}",
                )
                .unwrap()
                .progress_chars("=>-"),
        );

        let errors = Arc::new(AtomicU32::new(0));

        stream::iter(reqs)
            .chunks(5)
            .for_each_concurrent(None, |f| {
                let errors = Arc::clone(&errors);
                let progress_bar = progress_bar.clone();
                async move {
                    let results = join_all(f).await;
                    results.iter().filter(|r| r.is_err()).for_each(|r| {
                        error!("Failed to import user: {:?}", r);
                    });
                    #[allow(clippy::cast_possible_truncation)]
                    let err_count = results.iter().filter(|r| r.is_err()).count() as u32;
                    let prev_err_count =
                        errors.fetch_add(err_count, std::sync::atomic::Ordering::Relaxed);
                    progress_bar.inc(results.len() as u64);
                    if prev_err_count > 0 {
                        error!("Current error count: {}", prev_err_count);
                    }
                }
            })
            .await;

        #[allow(clippy::arithmetic_side_effects, clippy::cast_possible_truncation)]
        let imported = imports.len() as u32 - errors.load(std::sync::atomic::Ordering::Relaxed);

        let msg = format!("{imported} users imported");

        progress_bar.finish_with_message(msg.clone());

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
        } else if let (None, Some(oidc_id)) = (login, oidc_id) {
            let user_auth_data = UserAuthData::new_oidc(email, oidc_id.into());
            CreateUserRequest::builder(first_name, last_name)
                .with_auth_data(user_auth_data)
                .with_email(email)
        } else {
            let user_auth_data = UserAuthData::builder(dco3::users::AuthMethod::Basic)
                .with_must_change_password(true)
                .build();

            CreateUserRequest::builder(first_name, last_name)
                .with_auth_data(user_auth_data)
                .with_user_name(login.unwrap_or(email))
                .with_email(email)
                .with_notify_user(true)
        };

        let payload = if mfa_enforced {
            payload.with_mfa_enforced(true).build()
        } else {
            payload.build()
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
        opts: ListOptions,
        print: bool,
    ) -> Result<RangedItems<UserItem>, DcCmdError> {
        let params = build_params(
            opts.filter(),
            opts.offset().unwrap_or(0),
            opts.limit().unwrap_or(500).into(),
        )?;

        let results = self
            .client
            .users
            .get_users(Some(params), None, None)
            .await?;

        if opts.all() {
            let total = results.range.total;
            let shared_results = Arc::new(Mutex::new(results.clone()));

            let reqs = (500..=total)
                .step_by(500)
                .map(|offset| {
                    let params = build_params(opts.filter(), offset, opts.limit())
                        .expect("failed to build params");
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

            if print {
                self.print_users(&results, opts.csv())?;
            }
        } else if print {
            self.print_users(&results, opts.csv())?;
        }

        Ok(results)
    }

    async fn delete_user(
        &self,
        user_name: Option<String>,
        user_id: Option<u64>,
    ) -> Result<(), DcCmdError> {
        let confirm_msg = if let Some(user_name) = user_name {
            let user = self.find_user_by_username(&user_name).await?;
            self.client.users.delete_user(user.id).await?;
            format!("User {user_name} deleted",)
        } else if let Some(user_id) = user_id {
            self.client.users.delete_user(user_id).await?;
            format!("User {user_id} (id) deleted",)
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

        let Some(user) = results.items.into_iter().find(|u| u.user_name == user_name) else {
            error!("No user found with username: {user_name}");
            let msg = format!("No user found with username: {user_name}");
            return Err(DcCmdError::InvalidArgument(msg));
        };

        Ok(user)
    }
}

pub async fn handle_users_cmd(cmd: UsersCommand, term: Term) -> Result<(), DcCmdError> {
    let target = match &cmd {
        UsersCommand::Create { target, .. }
        | UsersCommand::Ls { target, .. }
        | UsersCommand::Rm { target, .. }
        | UsersCommand::Import { target, .. }
        | UsersCommand::Info { target, .. }
        | UsersCommand::SwitchAuth { target, .. }
        | UsersCommand::EnforceMfa { target, .. } => target,
    };

    let handler = match &cmd {
        UsersCommand::Import { .. } => UserCommandHandler::try_new(target, term, true).await?,
        _ => UserCommandHandler::try_new(target, term, false).await?,
    };

    match cmd {
        UsersCommand::Create {
            target: _,
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
        UsersCommand::Ls {
            target: _,
            filter,
            offset,
            limit,
            all,
            csv,
        } => {
            handler
                .list_users(ListOptions::new(filter, offset, limit, all, csv), true)
                .await?;
        }
        UsersCommand::Rm {
            target: _,
            user_name,
            user_id,
        } => {
            handler.delete_user(user_name, user_id).await?;
        }
        UsersCommand::Import {
            target: _,
            source,
            oidc_id,
        } => {
            handler.import_users(source, oidc_id).await?;
        }
        UsersCommand::Info {
            target: _,
            user_name,
            user_id,
        } => {
            handler.get_user_info(user_name, user_id).await?;
        }
        UsersCommand::SwitchAuth {
            target: _,
            current_method,
            new_method,
            current_oidc_id,
            new_oidc_id,
            current_ad_id,
            new_ad_id,
            filter,
            login,
        } => {
            let opts = UsersSwitchAuthOptions::try_new(
                current_method,
                new_method,
                current_oidc_id,
                new_oidc_id,
                current_ad_id,
                new_ad_id,
                filter,
                login,
            )?;
            handler.switch_auth(opts).await?;
        }
        UsersCommand::EnforceMfa {
            target: _,
            auth_method,
            filter,
            auth_method_id,
            group_id,
        } => {
            handler
                .enforce_mfa(auth_method, filter, auth_method_id, group_id)
                .await?;
        }
    }
    Ok(())
}

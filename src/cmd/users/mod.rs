use std::sync::{atomic::AtomicU32, Arc};

use console::Term;
use dco3::{
    auth::Connected,
    nodes::{NodeType, RoomGuestUserInvitation},
    user::UserAuthData,
    users::{CreateUserRequest, UserItem, UsersFilter},
    Dracoon, Groups, ListAllParams, Nodes, RangedItems, Rooms, Users,
};
use futures_util::{stream, StreamExt};
use indicatif::{ProgressBar, ProgressStyle};
use models::{CreateUserOptions, UsersSwitchAuthOptions};
use tokio::sync::Mutex;
use tracing::{error, info};

mod auth;
mod mfa;
mod models;
mod print;

use super::{
    config::MAX_CONCURRENT_REQUESTS,
    init_dracoon,
    models::{build_params, DcCmdError, ListOptions, UsersCommand},
    utils::strings::{build_node_path, format_success_message, parse_path},
};

pub use models::display_option;

use crate::cmd::users::models::UserImport;

use self::models::UserInfo;

#[derive(Clone)]
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

        let user_count = imports.len();

        let progress_bar = ProgressBar::new(user_count as u64);
        progress_bar.set_style(
            ProgressStyle::default_bar()
                .template(
                    "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {percent}% {msg}",
                )
                .unwrap()
                .progress_chars("=>-"),
        );

        let errors = Arc::new(AtomicU32::new(0));
        let semaphore = Arc::new(tokio::sync::Semaphore::new(MAX_CONCURRENT_REQUESTS));
        let mut handles = Vec::new();

        for import in imports {
            let handler = self.clone();
            let semaphore = semaphore.clone();
            let errors = errors.clone();
            let progress_bar = progress_bar.clone();

            let handle = tokio::spawn(async move {
                let _permit = semaphore.acquire().await.map_err(|e| {
                    error!("Failed to acquire semaphore: {}", e);
                    DcCmdError::IoError
                })?;

                match handler
                    .create_user(CreateUserOptions::new(
                        &import.first_name,
                        &import.last_name,
                        &import.email,
                        import.login.as_deref(),
                        oidc_id,
                        import.mfa_enabled.unwrap_or(false),
                        true,
                        None,
                    ))
                    .await
                {
                    Ok(_) => {
                        progress_bar.inc(1);
                    }
                    Err(e) => {
                        error!("Failed to import user: {e}");
                        let prev_err_count =
                            errors.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        if prev_err_count > 0 {
                            error!("Current error count: {prev_err_count}");
                        }
                        progress_bar.inc(1);
                    }
                }

                Ok::<(), DcCmdError>(())
            });

            handles.push(handle);
        }

        for handle in handles {
            if let Err(e) = handle.await {
                error!("Error uploading file: {}", e);
                return Err(DcCmdError::IoError);
            }
        }

        let imported = user_count - errors.load(std::sync::atomic::Ordering::Relaxed) as usize;

        let msg = format!("{imported} users imported");

        progress_bar.finish_with_message(msg.clone());

        self.term
            .write_line(format_success_message(&msg).as_str())
            .map_err(|_| DcCmdError::IoError)?;

        Ok(())
    }

    async fn create_user<'o>(&self, opts: CreateUserOptions<'o>) -> Result<(), DcCmdError> {
        let payload = if let (Some(login), Some(oidc_id)) = (opts.login, opts.oidc_id) {
            let user_auth_data = UserAuthData::new_oidc(login, oidc_id.into());
            CreateUserRequest::builder(opts.first_name, opts.last_name)
                .with_auth_data(user_auth_data)
                .with_email(opts.email)
        } else if let (None, Some(oidc_id)) = (opts.login, opts.oidc_id) {
            let user_auth_data = UserAuthData::new_oidc(opts.email, oidc_id.into());
            CreateUserRequest::builder(opts.first_name, opts.last_name)
                .with_auth_data(user_auth_data)
                .with_email(opts.email)
        } else {
            let user_auth_data = UserAuthData::builder(dco3::users::AuthMethod::Basic)
                .with_must_change_password(true)
                .build();

            CreateUserRequest::builder(opts.first_name, opts.last_name)
                .with_auth_data(user_auth_data)
                .with_user_name(opts.login.unwrap_or(opts.email))
                .with_email(opts.email)
                .with_notify_user(true)
        };

        let payload = if opts.mfa_enforced {
            payload.with_mfa_enforced(true).build()
        } else {
            payload.build()
        };

        let user = self.client.users().create_user(payload).await?;

        if let Some(group_id) = opts.first_group_id {
            let result = self
                .client
                .groups()
                .add_group_users(group_id, vec![user.id].into())
                .await;

            if let Err(e) = result {
                error!("Failed to add user to group: {}", e);
            }
        }

        info!(
            "User {} created (id: {} | auth: {})",
            user.user_name, user.id, user.auth_data.method
        );

        if !opts.is_import {
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

    async fn invite_user(
        &self,
        room_id: u64,
        first_name: &str,
        last_name: &str,
        email: &str,
    ) -> Result<(), DcCmdError> {
        let payload = RoomGuestUserInvitation::new(email, first_name, last_name);

        self.client
            .nodes()
            .invite_guest_users(room_id, vec![payload].into())
            .await?;

        self.term
            .write_line(
                format_success_message(format!("User {first_name} {last_name} invited").as_str())
                    .as_str(),
            )
            .map_err(|_| DcCmdError::IoError)?;

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
            .users()
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
                    self.client.users().get_users(Some(params), None, None)
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
            self.client.users().delete_user(user.id).await?;
            format!("User {user_name} deleted",)
        } else if let Some(user_id) = user_id {
            self.client.users().delete_user(user_id).await?;
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
            self.client.users().get_user(user_id, None).await?.into()
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
            .users()
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
        | UsersCommand::EnforceMfa { target, .. }
        | UsersCommand::Invite { target, .. } => target,
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
            group_id,
        } => {
            handler
                .create_user(CreateUserOptions::new(
                    &first_name,
                    &last_name,
                    &email,
                    login.as_deref(),
                    oidc_id,
                    mfa_enforced,
                    false,
                    group_id,
                ))
                .await?;
        }
        UsersCommand::Invite {
            target: _,
            ref first_name,
            ref last_name,
            ref email,
        } => {
            let (parent_path, node_name, depth) =
                parse_path(target, handler.client.get_base_url().as_ref())?;
            let node_path = build_node_path((parent_path.clone(), node_name.clone(), depth));

            let node = handler
                .client
                .nodes()
                .get_node_from_path(&node_path)
                .await?
                .ok_or(DcCmdError::InvalidPath(target.to_string()))?;

            let room_id = match node.node_type {
                NodeType::Room => node.id,
                NodeType::Folder => node.auth_parent_id.expect("Folder must have parent room"),
                _ => {
                    error!("Target must be a room or a folder: {target}");
                    return Err(DcCmdError::InvalidPath(target.to_string()));
                }
            };

            handler
                .invite_user(room_id, first_name, last_name, email)
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

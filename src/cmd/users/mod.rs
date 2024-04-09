use console::Term;
use dco3::{
    auth::Connected,
    user::UserAuthData,
    users::{CreateUserRequest, UserItem, UsersFilter},
    Dracoon, ListAllParams, Users,
};
use futures_util::{future::join_all, stream, StreamExt};
use tracing::error;

mod models;
mod print;

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

    async fn import_users(&self, source: String) -> Result<(), DcCmdError> {

        // read file from CSV and serialize to UserImport struct
        //let mut rdr = csv::Reader::from_reader(data);

        let mut rdr = csv::ReaderBuilder::new()
        .flexible(true)
        .trim(csv::Trim::All)
        .from_path(source).map_err(
            |e| {
                error!("Error reading file: {}", e);
                DcCmdError::IoError
            }
        )?;
        

        let imports: Vec<_> = rdr
            .records()
            .flat_map(|r| {
                
                if let Err(ref e) = r {
                    error!("Error reading record: {}", e);
                }

            let record = r?;
            record.deserialize::<UserImport>(None)
            })
            .collect();

        eprintln!("{}", imports.len());

        // build requests per import
        let reqs = imports
            .iter()
            .map(|import| {
                self.create_user(
                    &import.first_name,
                    &import.last_name,
                    &import.email,
                    import.login.as_deref(),
                    import.oidc_id,
                    true,
                )
            })
            .collect::<Vec<_>>();

        stream::iter(reqs)
           .chunks(5)
           .for_each_concurrent(None, |f| async move {
               join_all(f).await;
           }).await;

        let msg = format!("{} users imported", imports.len());

        self.term
            .write_line(format_success_message(&msg).as_str())
            .map_err(|_| DcCmdError::IoError)?;

        Ok(())

    }

    async fn create_user(
        &self,
        first_name: &str,
        last_name: &str,
        email: &str,
        login: Option<&str>,
        oidc_id: Option<u32>,
        is_import: bool,
    ) -> Result<(), DcCmdError> {
        let payload = if let (Some(login), Some(oidc_id)) = (login, oidc_id) {
            let user_auth_data = UserAuthData::new_oidc(login, oidc_id.into());
            CreateUserRequest::builder(first_name, last_name)
                .with_auth_data(user_auth_data)
                .with_email(email)
                .build()
        } else if let (None, Some(oidc_id)) = (login, oidc_id) {
            let user_auth_data = UserAuthData::new_oidc(email, oidc_id.into());
            CreateUserRequest::builder(first_name, last_name)
                .with_auth_data(user_auth_data)
                .with_email(email)
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
                .build()
        };

        let user = self.client.create_user(payload).await?;

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

        let results = self.client.get_users(Some(params), None, None).await?;

        self.print_users(results, csv)?;

        Ok(())
    }

    async fn delete_user(
        &self,
        user_name: Option<String>,
        user_id: Option<u64>,
    ) -> Result<(), DcCmdError> {
        let confirm_msg = if let Some(user_name) = user_name {
            let user = self.find_user_by_username(&user_name).await?;
            self.client.delete_user(user.id).await?;
            format!("User {} deleted", user_name)
        } else if let Some(user_id) = user_id {
            self.client.delete_user(user_id).await?;
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
            let user_id = user_id;
            self.client.get_user(user_id, None).await?.into()
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

        let results = self.client.get_users(Some(params), None, None).await?;

        if results.items.is_empty() {
            error!("No user found with username: {}", user_name);
            let msg = format!("No user found with username: {}", user_name);
            return Err(DcCmdError::InvalidArgument(msg));
        }

        Ok(results.items.first().expect("No user found").clone())
    }
}

use super::{
    init_dracoon,
    models::{DcCmdError, UserCommand},
    utils::strings::format_success_message,
};

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
        } => {
            handler
                .create_user(
                    &first_name,
                    &last_name,
                    &email,
                    login.as_deref(),
                    oidc_id,
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
            handler.import_users(source).await?;
        }
        UserCommand::Info { user_name, user_id } => {
            handler.get_user_info(user_name, user_id).await?;
        }
    }
    Ok(())
}

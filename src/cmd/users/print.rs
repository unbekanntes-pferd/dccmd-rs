use dco3::{users::UserItem, RangedItems};
use tabled::{
    settings::{object::Segment, Modify, Panel, Style, Width},
    Table,
};

use crate::cmd::models::{DcCmdError, PrintFormat};

use super::{models::UserInfo, UserCommandHandler};

impl UserCommandHandler {
    pub fn print_user_info(&self, user_info: UserInfo) -> Result<(), DcCmdError> {
        let last_login = if let Some(last_login) = user_info.last_login_at {
            last_login.to_string()
        } else {
            "N/A".to_string()
        };

        let expire_at = if let Some(expire_at) = user_info.expire_at {
            expire_at.to_string()
        } else {
            "N/A".to_string()
        };

        self.term
            .write_line(&format!("► user id: {}", user_info.id))
            .map_err(|_| DcCmdError::IoError)?;
        self.term
            .write_line(&format!(
                "► name: {} {}",
                user_info.first_name, user_info.last_name
            ))
            .map_err(|_| DcCmdError::IoError)?;
        self.term
            .write_line(&format!("► username: {}", user_info.username))
            .map_err(|_| DcCmdError::IoError)?;
        self.term
            .write_line(&format!(
                "► email: {}",
                user_info.email.unwrap_or_else(|| "N/A".to_string())
            ))
            .map_err(|_| DcCmdError::IoError)?;
        self.term
            .write_line(&format!("► last login at: {}", last_login))
            .map_err(|_| DcCmdError::IoError)?;
        self.term
            .write_line(&format!("► expire at: {}", expire_at))
            .map_err(|_| DcCmdError::IoError)?;
        self.term
            .write_line(&format!("► locked: {}", user_info.is_locked))
            .map_err(|_| DcCmdError::IoError)?;

        Ok(())
    }

    pub fn print_users(&self, users: RangedItems<UserItem>, csv: bool) -> Result<(), DcCmdError> {
        let print_mode = if csv {
            PrintFormat::Csv
        } else {
            PrintFormat::Pretty
        };

        match print_mode {
            PrintFormat::Csv => {
                let header = "id,first_name,last_name,user_name,email,expire_at,is_locked,is_encryption_enabled,has_manageable_rooms,last_login_success_at";
                self.term
                    .write_line(header)
                    .map_err(|_| DcCmdError::IoError)?;

                for user_item in users.items {
                    let expire_at = if let Some(expire_at) = user_item.expire_at {
                        expire_at.to_string()
                    } else {
                        "N/A".to_string()
                    };

                    self.term
                        .write_line(&format!(
                            "{},{},{},{},{},{},{},{},{},{}",
                            user_item.id,
                            user_item.first_name,
                            user_item.last_name,
                            user_item.user_name,
                            user_item.email.unwrap_or_else(|| "N/A".to_string()),
                            expire_at,
                            user_item.is_locked,
                            user_item.is_encryption_enabled.unwrap_or(false),
                            user_item.has_manageable_rooms.unwrap_or(false),
                            user_item
                                .last_login_success_at
                                .unwrap_or_else(|| "N/A".to_string())
                        ))
                        .map_err(|_| DcCmdError::IoError)?;
                }
            }
            PrintFormat::Pretty => {
                let total = users.range.total;
                let users: Vec<_> = users.items.into_iter().map(UserInfo::from).collect();
                let displayed = users.len();
                let mut user_table = Table::new(users);
                user_table
                    .with(Panel::footer(format!(
                        "{} users ({} total)",
                        displayed, total
                    )))
                    .with(Style::modern())
                    .with(Modify::new(Segment::all()).with(Width::wrap(16)));

                println!("{}", user_table);
            }
        }

        Ok(())
    }
}

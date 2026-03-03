use console::Term;
use dco3::{
    groups::{Group, GroupUser},
    RangedItems,
};
use tabled::{
    derive::display,
    settings::{object::Segment, Modify, Panel, Style, Width},
    Table, Tabled,
};

use crate::core::models::{DcCmdError, PrintFormat};

#[derive(Tabled)]
#[tabled(display(Option, "display::option", "N/A"))]
struct GroupInfo {
    id: String,
    name: String,
    created_at: String,
    cnt_users: u64,
    updated_at: Option<String>,
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
struct GroupUserInfo {
    id: i64,
    email: String,
    first_name: String,
    last_name: String,
    user_name: String,
    group_id: u64,
    group_name: String,
}

impl GroupUserInfo {
    fn new(user: GroupUser, group: Group) -> Self {
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

pub fn print_groups(term: &Term, groups: RangedItems<Group>, csv: bool) -> Result<(), DcCmdError> {
    let print_mode = if csv {
        PrintFormat::Csv
    } else {
        PrintFormat::Pretty
    };

    match print_mode {
        PrintFormat::Csv => {
            let header = "id,name,cnt_users,created_at,updated_at";
            term.write_line(header).map_err(|_| DcCmdError::IoError)?;

            for group in groups.items {
                let updated_at = match group.updated_at {
                    Some(updated_at) => updated_at.to_rfc3339(),
                    None => "N/A".to_string(),
                };
                term.write_line(&format!(
                    "{},{},{},{},{}",
                    group.id,
                    group.name,
                    group.cnt_users.unwrap_or(0),
                    group.created_at,
                    updated_at
                ))
                .map_err(|_| DcCmdError::IoError)?;
            }

            Ok(())
        }
        PrintFormat::Pretty => {
            let total = groups.range.total;
            let groups: Vec<_> = groups.items.into_iter().map(GroupInfo::from).collect();
            let displayed = groups.len();
            let mut user_table = Table::new(groups);
            user_table
                .with(Panel::footer(
                    format!("{displayed} groups ({total} total)",),
                ))
                .with(Style::modern())
                .with(Modify::new(Segment::all()).with(Width::wrap(16)));

            term.write_line(&format!("{user_table}"))
                .map_err(|_| DcCmdError::IoError)?;

            Ok(())
        }
    }
}

pub fn print_group_users(
    term: &Term,
    users: RangedItems<GroupUser>,
    group: &Group,
    csv: bool,
    is_first: bool,
) -> Result<(), DcCmdError> {
    let print_mode = if csv {
        PrintFormat::Csv
    } else {
        PrintFormat::Pretty
    };

    match print_mode {
        PrintFormat::Csv => {
            if is_first {
                let header = "id,user_name,first_name,last_name,email,group_id,group_name";
                term.write_line(header).map_err(|_| DcCmdError::IoError)?;
            }

            for user in users.items {
                term.write_line(&format!(
                    "{},{},{},{},{},{},{}",
                    user.user_info.id,
                    user.user_info.user_name.unwrap_or("N/A".to_string()),
                    user.user_info.first_name.unwrap_or("N/A".to_string()),
                    user.user_info.last_name.unwrap_or("N/A".to_string()),
                    user.user_info.email.unwrap_or("N/A".to_string()),
                    group.id,
                    group.name,
                ))
                .map_err(|_| DcCmdError::IoError)?;
            }

            Ok(())
        }
        PrintFormat::Pretty => {
            let total = users.range.total;
            let users: Vec<_> = users
                .items
                .into_iter()
                .map(|user| GroupUserInfo::new(user, group.clone()))
                .collect();
            if !users.is_empty() {
                let displayed = users.len();
                let mut user_table = Table::new(users);
                user_table
                    .with(Panel::footer(format!("{displayed} users ({total} total)",)))
                    .with(Style::modern())
                    .with(Modify::new(Segment::all()).with(Width::wrap(16)));

                println!("{user_table}");
            }

            Ok(())
        }
    }
}

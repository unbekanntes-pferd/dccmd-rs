use dco3::{groups::Group, RangedItems};
use tabled::{
    settings::{object::Segment, Modify, Panel, Style, Width},
    Table,
};

use crate::cmd::{
    groups::models::GroupInfo,
    models::{DcCmdError, PrintFormat},
};

use super::GroupCommandHandler;

impl GroupCommandHandler {
    pub fn print_groups(&self, groups: RangedItems<Group>, csv: bool) -> Result<(), DcCmdError> {
        let print_mode = if csv {
            PrintFormat::Csv
        } else {
            PrintFormat::Pretty
        };

        match print_mode {
            PrintFormat::Csv => {
                let header = "id,name,cnt_users,created_at,updated_at";
                self.term
                    .write_line(header)
                    .map_err(|_| DcCmdError::IoError)?;

                for group in groups.items {
                    let updated_at = match group.updated_at {
                        Some(updated_at) => updated_at.to_rfc3339(),
                        None => "N/A".to_string(),
                    };
                    self.term
                        .write_line(&format!(
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

                println!("{user_table}");

                Ok(())
            }
        }
    }
}

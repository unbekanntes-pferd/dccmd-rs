use chrono::SecondsFormat;
use dco3::eventlog::LogEventList;
use tabled::{
    settings::{Panel, Style},
    Tabled,
};

use crate::cmd::models::DcCmdError;

use super::ReportsCommandHandler;

#[derive(Tabled)]
struct LogEventInfo {
    id: i64,
    time: String,
    user_id: i64,
    message: String,
}

impl LogEventInfo {
    fn new(id: i64, time: String, user_id: i64, message: String) -> Self {
        Self {
            id,
            time,
            user_id,
            message,
        }
    }
}

impl ReportsCommandHandler {
    pub fn print_events(&self, events: LogEventList, csv: bool) -> Result<(), DcCmdError> {
        if csv {
            self.print_events_csv(events)
        } else {
            self.print_events_table(events)
        }
    }

    fn print_events_csv(&self, events: LogEventList) -> Result<(), DcCmdError> {
        const NOT_AVAILABLE: &str = "N/A";
        let header = "id,time,user_id,message,operation_id,operation_name,status,user_client,user_name,customer_id,auth_parent_source,auth_parent_target,object_id1,object_id2,object_type1,object_type2,object_name1,object_name2,attribute1,attribute2,attribute3";

        self.term
            .write_line(header)
            .map_err(|_| DcCmdError::IoError)?;

        for event in events.items {
            let id = event.id.to_string();
            let time = event.time.to_rfc3339_opts(SecondsFormat::Secs, true);
            let user_id = event.user_id.to_string();
            let message = event.message;
            let operation_id = event
                .operation_id
                .map(|id| id.to_string())
                .unwrap_or_else(|| NOT_AVAILABLE.to_string());
            let operation_name = event
                .operation_name
                .unwrap_or_else(|| NOT_AVAILABLE.to_string());
            let status = event
                .status
                .map(|s| i64::from(s).to_string())
                .unwrap_or_else(|| NOT_AVAILABLE.to_string());
            let user_client = event
                .user_client
                .unwrap_or_else(|| NOT_AVAILABLE.to_string());
            let user_name = event.user_name.unwrap_or_else(|| NOT_AVAILABLE.to_string());
            let customer_id = event
                .customer_id
                .map(|id| id.to_string())
                .unwrap_or_else(|| NOT_AVAILABLE.to_string());
            let auth_parent_source = event
                .auth_parent_source
                .unwrap_or_else(|| NOT_AVAILABLE.to_string());
            let auth_parent_target = event
                .auth_parent_target
                .unwrap_or_else(|| NOT_AVAILABLE.to_string());
            let object_id1 = event
                .object_id1
                .map(|id| id.to_string())
                .unwrap_or_else(|| NOT_AVAILABLE.to_string());
            let object_id2 = event
                .object_id2
                .map(|id| id.to_string())
                .unwrap_or_else(|| NOT_AVAILABLE.to_string());
            let object_type1 = event
                .object_type1
                .map(|id| id.to_string())
                .unwrap_or_else(|| NOT_AVAILABLE.to_string());
            let object_type2 = event
                .object_type2
                .map(|id| id.to_string())
                .unwrap_or_else(|| NOT_AVAILABLE.to_string());
            let object_name1 = event
                .object_name1
                .unwrap_or_else(|| NOT_AVAILABLE.to_string());
            let object_name2 = event
                .object_name2
                .unwrap_or_else(|| NOT_AVAILABLE.to_string());
            let attribute1 = event
                .attribute1
                .unwrap_or_else(|| NOT_AVAILABLE.to_string());
            let attribute2 = event
                .attribute2
                .unwrap_or_else(|| NOT_AVAILABLE.to_string());
            let attribute3 = event
                .attribute3
                .unwrap_or_else(|| NOT_AVAILABLE.to_string());

            let line = format!(
            "{id},{time},{user_id},{message},{operation_id},{operation_name},{status},{user_client},{user_name},{customer_id},{auth_parent_source},{auth_parent_target},{object_id1},{object_id2},{object_type1},{object_type2},{object_name1},{object_name2},{attribute1},{attribute2},{attribute3}",
        );

            self.term
                .write_line(&line)
                .map_err(|_| DcCmdError::IoError)?;
        }

        Ok(())
    }

    fn print_events_table(&self, events: LogEventList) -> Result<(), DcCmdError> {
        let event_count = events.items.len();
        let total_events = events.range.total;
        let events = events
            .items
            .iter()
            .map(|event| {
                LogEventInfo::new(
                    event.id,
                    event.time.to_rfc3339_opts(SecondsFormat::Secs, true),
                    event.user_id,
                    event.message.clone(),
                )
            })
            .collect::<Vec<_>>();

        let mut table = tabled::Table::new(events);
        table.with(Style::modern()).with(Panel::footer(format!(
            "{} events ({} total)",
            event_count, total_events
        )));

        self.term
            .write_line(&table.to_string())
            .map_err(|_| DcCmdError::IoError)?;
        Ok(())
    }
}

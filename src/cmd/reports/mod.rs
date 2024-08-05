use std::time::Duration;

use console::Term;
use dco3::{auth::Connected, Dracoon, Eventlog, Public};
use indicatif::ProgressBar;
use models::EventOptions;
use tracing::{error, warn};

use super::{
    init_dracoon,
    models::{DcCmdError, ListOptions, ReportsCommand},
};

mod events;
mod models;
mod permissions;
mod print;

pub struct ReportsCommandHandler {
    term: Term,
    client: Dracoon<Connected>,
}

impl ReportsCommandHandler {
    pub fn new(term: Term, client: Dracoon<Connected>) -> Self {
        Self { term, client }
    }

    pub async fn check_dracoon_api_version(&self) -> Result<(), DcCmdError> {
        if let Some(major_version) = self
            .client
            .public
            .get_software_version()
            .await?
            .rest_api_version
            .split('.')
            .next()
        {
            if let Ok(num) = major_version.parse::<u8>() {
                if num > 4 {
                    error!("Permissions report is only available for API version 4.x (DRACOON Server) - used version: {}", self.client.public.get_software_version().await?.rest_api_version);
                    return Err(DcCmdError::InvalidArgument(
                        "Permissions report is only available for API version 4.x (DRACOON Server)"
                            .to_string(),
                    ));
                }
            }
        } else {
            warn!("Failed to parse API version");
        }

        Ok(())
    }
}

pub async fn handle_reports_cmd(cmd: ReportsCommand, term: Term) -> Result<(), DcCmdError> {
    let target = match &cmd {
        ReportsCommand::Events { target, .. }
        | ReportsCommand::Permissions { target, .. }
        | ReportsCommand::OperationTypes { target } => target,
    };

    let client = init_dracoon(target, None, false).await?;
    let handler = ReportsCommandHandler::new(term, client);

    match cmd {
        ReportsCommand::Events {
            target: _,
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
        } => {
            handler.check_dracoon_api_version().await?;

            let list_opts = ListOptions::new(filter, offset, limit, all, csv);

            let opts = EventOptions::new(
                list_opts,
                start_date,
                end_date,
                user_id,
                operation_type,
                status,
            )?;

            let spinner = ProgressBar::new_spinner().with_message("Loading events...");
            spinner.enable_steady_tick(Duration::from_millis(100));
            let events = handler.get_events(opts).await?;
            spinner.finish_and_clear();

            handler.print_events(events, csv)?;

            Ok(())
        }
        ReportsCommand::OperationTypes { target: _ } => {
            let event_types = handler.client.eventlog.get_event_operations().await?;

            handler.print_event_types(event_types)?;

            Ok(())
        }
        ReportsCommand::Permissions {
            target: _,
            filter,
            offset,
            limit,
            all,
            csv,
        } => {
            let list_opts = ListOptions::new(filter, offset, limit, all, csv);

            handler.check_dracoon_api_version().await?;

            let spinner = ProgressBar::new_spinner().with_message("Loading permissions...");
            spinner.enable_steady_tick(Duration::from_millis(100));
            let permissions = handler.get_permissions(list_opts).await?;

            spinner.finish_and_clear();

            handler.print_permissions(permissions, csv)?;

            Ok(())
        }
    }
}

use std::time::Duration;

use console::Term;
use dco3::{auth::Connected, Dracoon};
use indicatif::ProgressBar;
use models::EventOptions;

use super::{
    init_dracoon,
    models::{DcCmdError, ListOptions, ReportsCommand},
};

mod events;
mod models;
mod print;

pub struct ReportsCommandHandler {
    term: Term,
    client: Dracoon<Connected>,
}

impl ReportsCommandHandler {
    pub fn new(term: Term, client: Dracoon<Connected>) -> Self {
        Self { term, client }
    }
}

pub async fn handle_reports_cmd(cmd: ReportsCommand, term: Term) -> Result<(), DcCmdError> {
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
        } => {
            let client = init_dracoon(&target, None, false).await?;

            let handler = ReportsCommandHandler::new(term, client);

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
    }
}

use console::Term;
use dco3::{auth::Connected, Dracoon};

use super::models::{DcCmdError, ReportsCommand};

mod events;

pub struct ReportsCommandHandler {
    term: Term,
    client: Dracoon<Connected>,
}

impl ReportsCommandHandler {
    
}

pub async fn handle_reports_cmd(cmd: ReportsCommand, term: Term) -> Result<(), DcCmdError> {
    todo!()
}
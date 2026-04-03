#![allow(clippy::module_name_repetitions)]
#![allow(clippy::struct_excessive_bools)]

use clap::Parser;
use cli::{runner::CliRunner, term_ui::TermUi};
use command::DcCmd;
use console::Term;
use core::{
    logs::init_logging,
    models::{DcCmdError, PasswordAuth},
};
use secrecy::SecretString;

pub mod app;
pub mod cli;
pub mod command;
pub mod core;
pub mod mcp;

pub async fn run() -> i32 {
    let opt = DcCmd::parse();

    let term = Term::stdout();
    let err_term = Term::stderr();

    if let Err(e) = init_logging(opt.debug) {
        let _ = TermUi::write_cli_error(&err_term, &e);
        return 1;
    }

    let password_auth = match (opt.username, opt.password) {
        (Some(username), Some(password)) => Some(PasswordAuth::new(
            username,
            SecretString::new(password.into()),
        )),
        _ => None,
    };

    let encryption_password = opt
        .encryption_password
        .map(|secret| SecretString::new(secret.into()));

    let runner = CliRunner::new(term, err_term.clone(), password_auth, encryption_password);
    let res = runner.execute(opt.cmd).await;

    if let Err(e) = res {
        if !matches!(e, DcCmdError::CommandFailed) {
            let _ = TermUi::write_cli_error(&err_term, &e);
        }
        return 1;
    }

    0
}

#![allow(dead_code)]
#![allow(unused_variables)]

use clap::Parser;
use cmd::{
    handle_error,
    models::{DcCmd, DcCmdCommand, PasswordAuth},
    nodes::{
        create_folder, create_room, delete_node, download::download, list_nodes, upload::upload,
    },
};
use console::Term;
use std::fs::OpenOptions;
use tracing::{error, metadata::LevelFilter};
use tracing_subscriber::filter::EnvFilter;

use crate::cmd::models::DcCmdError;

mod cmd;

#[tokio::main]
async fn main() {
    let opt = DcCmd::parse();

    let term = Term::stdout();
    let err_term = Term::stderr();

    let env_filter = if opt.debug {
        EnvFilter::from_default_env().add_directive(LevelFilter::DEBUG.into())
    } else {
        EnvFilter::from_default_env().add_directive(LevelFilter::INFO.into())
    };

    // set up logging file
    let log_file_path = opt.log_file_path.unwrap_or("dccmd-rs.log".to_string());

    let log_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_file_path)
        .map_err(|e| {
            error!("Failed to create or open log file: {}", e);
            DcCmdError::LogFileCreationFailed
        });

    if let Err(e) = &log_file {
        handle_error(&err_term, e);
    }

    let log_file = log_file.unwrap();

    // set up logging format
    let log_format = tracing_subscriber::fmt::format()
        .with_level(true)
        .with_thread_names(false)
        .with_target(true)
        .with_ansi(false)
        .compact();

    // initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .event_format(log_format)
        .with_writer(std::sync::Mutex::new(log_file))
        .init();


    let password_auth = match (opt.username, opt.password) {
        (Some(username), Some(password)) => Some(PasswordAuth(username, password)),
        _ => None,
    };

    let res = match opt.cmd {
        DcCmdCommand::Download {
            source,
            target,
            velocity,
            recursive,
        } => {
            download(
                source,
                target,
                velocity,
                recursive,
                password_auth,
                opt.encryption_password,
            )
            .await
        }
        DcCmdCommand::Upload {
            source,
            target,
            overwrite,
            classification,
            velocity,
            recursive,
            skip_root,
        } => {
            upload(
                source.into(),
                target,
                overwrite,
                classification,
                velocity,
                recursive,
                skip_root,
                password_auth,
                opt.encryption_password,
            )
            .await
        }
        DcCmdCommand::Ls {
            source,
            long,
            human_readable,
            managed,
            all,
            offset,
            limit,
        } => {
            list_nodes(
                term,
                source,
                Some(long),
                Some(human_readable),
                Some(managed),
                Some(all),
                offset,
                limit,
                password_auth,
            )
            .await
        }
        DcCmdCommand::Mkdir {
            source,
            classification,
            notes,
        } => create_folder(term, source, classification, notes, password_auth).await,
        DcCmdCommand::Mkroom {
            source,
            classification,
        } => create_room(term, source, classification, password_auth).await,
        DcCmdCommand::Rm { source, recursive } => {
            delete_node(term, source, Some(recursive), password_auth).await
        },
        DcCmdCommand::Version => {
            println!("dccmd-rs {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
    };

    if let Err(e) = res {
        handle_error(&err_term, &e);
    }
}

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
use tracing::metadata::LevelFilter;
use tracing_subscriber::{filter::EnvFilter, fmt, prelude::*};

mod cmd;

#[tokio::main]
async fn main() {
    let opt = DcCmd::parse();

    let env_filter = if opt.debug {
        EnvFilter::from_default_env().add_directive(LevelFilter::DEBUG.into())
    } else {
        EnvFilter::from_default_env()
    };

    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(env_filter)
        .init();

    let term = Term::stdout();
    let err_term = Term::stderr();

    let password_auth = match (opt.username, opt.password) {
        (Some(username), Some(password)) => Some(PasswordAuth(username, password)),
        _ => None
    };

    let res = match opt.cmd {
        DcCmdCommand::Download {
            source,
            target,
            velocity,
            recursive,
        } => download(source, target, velocity, recursive, password_auth, opt.encryption_password).await,
        DcCmdCommand::Upload {
            source,
            target,
            overwrite,
            classification,
            velocity,
            recursive,
        } => {
            upload(
                source.try_into().expect("Invalid path"),
                target,
                overwrite,
                classification,
                velocity,
                recursive,
                password_auth,
                opt.encryption_password
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
        DcCmdCommand::Rm { source, recursive } => delete_node(term, source, Some(recursive), password_auth).await,
    };

    if let Err(e) = res {
        handle_error(&err_term, &e);
    }
}

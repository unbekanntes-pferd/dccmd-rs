#![allow(dead_code)]
#![allow(unused_variables)]

use clap::Parser;
use cmd::{
    handle_error,
    models::{DcCmd, DcCmdCommand}, nodes::{download::download, upload::upload, list_nodes, create_folder, create_room, delete_node},
};
use console::Term;
use tracing::metadata::LevelFilter;
use tracing_subscriber::{filter::EnvFilter, prelude::*, fmt};

mod api;
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

    let res = match opt.cmd {
        DcCmdCommand::Download { source, target, velocity, recursive } => download(source, target, velocity, recursive).await,
        DcCmdCommand::Upload { source, target, overwrite, classification } => upload(source.try_into().expect("Invalid path"), target, overwrite, classification).await,
        DcCmdCommand::Ls {
            source,
            long,
            human_readable,
            managed,
            all,
            offset,
            limit
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
            )
            .await
        }
        DcCmdCommand::Mkdir {
            source,
            classification,
            notes,
        } => create_folder(term, source, classification, notes).await,
        DcCmdCommand::Mkroom { source, classification } => create_room(term, source, classification).await,
        DcCmdCommand::Rm { source, recursive } => delete_node(term, source, Some(recursive)).await,
    };

    if let Err(e) = res {
        handle_error(&err_term, &e);
    }
}
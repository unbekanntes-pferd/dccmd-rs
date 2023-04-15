#![allow(dead_code)]
#![allow(unused_variables)]

use clap::Parser;
use cmd::{
    create_folder, delete_node, download, get_nodes, handle_error,
    models::{DcCmd, DcCmdCommand},
};
use console::Term;

mod api;
mod cmd;

#[tokio::main]
async fn main() -> () {
    tracing_subscriber::fmt::init();
    let opt = DcCmd::parse();

    let term = Term::stdout();
    let err_term = Term::stderr();

    let res = match opt.cmd {
        DcCmdCommand::Download { source, target } => download(source, target).await,
        DcCmdCommand::Upload { source, target } => {
            Ok(println!("Uploading {} to {}", source, target))
        }
        DcCmdCommand::Ls {
            source,
            long,
            human_readable,
            managed,
            all,
        } => {
            get_nodes(
                term,
                source,
                Some(long),
                Some(human_readable),
                Some(managed),
                Some(all),
            )
            .await
        }
        DcCmdCommand::Mkdir {
            source,
            classification,
            notes,
        } => create_folder(term, source, classification, notes).await,
        DcCmdCommand::Mkroom { source } => Ok(println!("Creating room {}", source)),
        DcCmdCommand::Rm { source } => delete_node(term, source).await,
    };

    if let Err(e) = res {
        handle_error(err_term, e);
    }
}

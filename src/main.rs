#![allow(dead_code)]
#![allow(unused_variables)]

use clap::Parser;
use cmd::{
    download, get_nodes, handle_error,
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
            all
        } => get_nodes(term, source, Some(long), Some(human_readable), Some(managed), Some(all)).await,
    };

    if let Err(e) = res {
        handle_error(err_term, e);
    }
}

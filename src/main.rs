#![allow(dead_code)]
#![allow(unused_variables)]

use clap::Parser;
use cmd::{models::{DcCmd, DcCmdError}, get_nodes};

mod api;
mod cmd;



#[tokio::main]
async fn main() -> Result<(), DcCmdError>{
    
    tracing_subscriber::fmt::init();
    
    let opt = DcCmd::parse();

    match opt {
        DcCmd::Download { source, target } => println!("Downloading {} to {}", source, target),
        DcCmd::Upload { source, target } => println!("Uploading {} to {}", source, target),
        DcCmd::Ls { source } => get_nodes(source).await?
    };

    Ok(())

}


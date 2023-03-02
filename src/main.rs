use clap::Parser;
use cmd::models::DcCmd;

mod api;
mod cmd;

fn main() {
    let opt = DcCmd::parse();

    match opt {
        DcCmd::Download { source, target } => println!("Downloading {} to {}", source, target),
        DcCmd::Upload { source, target } => println!("Uploading {} to {}", source, target)
    }

}


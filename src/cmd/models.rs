use clap::Parser;

#[derive(Parser)]
#[clap(rename_all = "kebab-case", about="DRACOON Commander (dccmd-rs)")]
pub enum DcCmd {
    Upload {
        source: String,
        target: String
    },
    Download {
        source: String,
        target: String
    },
}
use clap::Parser;

#[derive(Parser)]
pub enum McpCommand {
    /// Start the STDIO MCP server for a fixed DRACOON target
    Start {
        /// Fixed DRACOON target for this server instance (https://your.dracoon.domain)
        target: String,

        /// Expose destructive node tools such as update or delete
        #[clap(long)]
        allow_destructive: bool,
    },
}

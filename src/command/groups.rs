use clap::Parser;

#[derive(Parser)]
pub enum GroupsCommand {
    /// List groups in DRACOON
    Ls {
        /// DRACOON url
        target: String,

        /// search filter (e.g. group name)
        #[clap(long)]
        filter: Option<String>,

        /// skip n groups (default offset: 0)
        #[clap(short, long)]
        offset: Option<u64>,

        /// limit n groups (default limit: 500)
        #[clap(long)]
        limit: Option<u32>,

        /// fetch all groups (default: 500)
        #[clap(long)]
        all: bool,

        /// print user information in CSV format
        #[clap(long)]
        csv: bool,
    },

    /// Create a group in DRACOON
    Create {
        /// DRACOON url
        target: String,

        /// Group name
        #[clap(long, short)]
        name: String,
    },

    /// delete a group in DRACOON
    Rm {
        /// DRACOON url
        target: String,

        ///  Group name
        #[clap(long, short)]
        group_name: Option<String>,

        /// Group id
        #[clap(long)]
        group_id: Option<u64>,
    },

    Users {
        #[clap(subcommand)]
        cmd: GroupsUsersCommand,
    },
}

#[derive(Parser)]
pub enum GroupsUsersCommand {
    Ls {
        target: String,

        /// filter (group e.g. user name)
        #[clap(long)]
        filter: Option<String>,

        /// skip n users (default offset: 0)
        #[clap(short, long)]
        offset: Option<u32>,

        /// limit n users (default limit: 500)
        #[clap(long)]
        limit: Option<u32>,

        /// fetch all group users (default: 500)
        #[clap(long)]
        all: bool,

        /// print user information in CSV format
        #[clap(long)]
        csv: bool,
    },
    Add {
        target: String,

        #[clap(long)]
        group_name: Option<String>,

        #[clap(long)]
        group_id: Option<u64>,

        #[clap(long)]
        user_name: Option<String>,

        #[clap(long)]
        user_id: Option<u64>,
    },
}

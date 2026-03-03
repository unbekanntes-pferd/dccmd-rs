use clap::Parser;

#[derive(Parser)]
pub enum ReportsCommand {
    Events {
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

        /// operation id (see DRACOON API documentation)
        #[clap(long)]
        operation_type: Option<u64>,

        /// user id for filtering events
        #[clap(long)]
        user_id: Option<u64>,

        /// status (0 for success, 2 for failure)
        #[clap(long)]
        status: Option<u8>,

        /// start date (format: yyyy-mm-dd)
        #[clap(long)]
        start_date: Option<String>,

        /// end date (format: yyyy-mm-dd)
        #[clap(long)]
        end_date: Option<String>,
    },
    OperationTypes {
        /// DRACOON url
        target: String,
    },
    Permissions {
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
}

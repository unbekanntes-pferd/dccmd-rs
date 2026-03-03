use clap::Parser;

#[derive(Parser)]
pub enum UsersCommand {
    /// List users in DRACOON
    Ls {
        /// DRACOON url
        target: String,

        /// search filter (e.g. username, first name, last name)
        #[clap(long)]
        filter: Option<String>,

        /// skip n users (default offset: 0)
        #[clap(short, long)]
        offset: Option<u64>,

        /// limit n users (default limit: 500)
        #[clap(long)]
        limit: Option<u32>,

        /// fetch all users (default: 500)
        #[clap(long)]
        all: bool,

        /// print user information in CSV format
        #[clap(long)]
        csv: bool,
    },

    /// Create a user in DRACOON
    Create {
        /// DRACOON url
        target: String,

        /// User first name
        #[clap(long, short)]
        first_name: String,

        /// User last name
        #[clap(long, short)]
        last_name: String,

        /// User email
        #[clap(long, short)]
        email: String,

        /// Login (for OIDC)
        #[clap(long)]
        login: Option<String>,

        /// OIDC config id
        #[clap(long)]
        oidc_id: Option<u32>,

        /// OIDC config id
        #[clap(long)]
        mfa_enforced: bool,

        /// group id for first group assignment
        #[clap(long)]
        group_id: Option<u64>,
    },

    /// Invite a guest user into DRACOON
    Invite {
        /// DRACOON url and path
        target: String,

        /// User first name
        #[clap(long, short)]
        first_name: String,

        /// User last name
        #[clap(long, short)]
        last_name: String,

        /// User email
        #[clap(long, short)]
        email: String,
    },

    /// delete a user in DRACOON
    Rm {
        /// DRACOON url
        target: String,

        /// User login
        #[clap(long, short)]
        user_name: Option<String>,

        #[clap(long)]
        user_id: Option<u64>,
    },

    /// import users from CSV file into DRACOON
    Import {
        /// DRACOON url
        target: String,

        /// Source file path
        source: String,

        /// OIDC config id
        #[clap(long)]
        oidc_id: Option<u32>,
    },

    /// print user information in DRACOON
    Info {
        /// DRACOON url
        target: String,

        /// User login
        #[clap(long, short)]
        user_name: Option<String>,

        #[clap(long)]
        user_id: Option<u64>,
    },

    /// swith auth method for users in DRACOON
    SwitchAuth {
        /// DRACOON url
        target: String,

        /// current auth method in DRACOON
        #[clap(long)]
        current_method: String,

        /// new auth method in DRACOON
        #[clap(long)]
        new_method: String,

        /// optional current OIDC config id
        #[clap(long)]
        current_oidc_id: Option<u64>,

        /// optional new OIDC config id
        #[clap(long)]
        new_oidc_id: Option<u64>,

        /// optional current AD config id
        #[clap(long)]
        current_ad_id: Option<u64>,

        /// optional new AD config id
        #[clap(long)]
        new_ad_id: Option<u64>,

        /// optional user filter
        #[clap(long)]
        filter: Option<String>,

        /// optional login transformation
        /// (e.g. email, username, firstname.lastname)
        /// default: email
        #[clap(long)]
        login: Option<String>,
    },

    EnforceMfa {
        /// DRACOON url
        target: String,

        /// optional auth method
        #[clap(long)]
        auth_method: Option<String>,

        /// optional user filter
        #[clap(long)]
        filter: Option<String>,

        /// optional auth method id (required for oidc / ad)
        #[clap(long)]
        auth_method_id: Option<u64>,

        /// optional group id
        #[clap(long)]
        group_id: Option<u64>,
    },
}

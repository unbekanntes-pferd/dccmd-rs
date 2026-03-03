use clap::Parser;

use super::{
    config::ConfigCommand, CreateContainerType, GroupsCommand, ReportsCommand, UsersCommand,
};

#[derive(Parser)]
#[clap(rename_all = "kebab-case", about = "DRACOON Commander (dccmd-rs)")]
pub struct DcCmd {
    #[clap(subcommand)]
    pub cmd: DcCmdCommand,

    /// Enable debug logging
    #[clap(long)]
    pub debug: bool,

    /// optional username
    #[clap(long)]
    pub username: Option<String>,

    /// optional password
    #[clap(long)]
    pub password: Option<String>,

    /// optional encryption password
    #[clap(long)]
    pub encryption_password: Option<String>,
}

#[derive(Parser)]
pub enum DcCmdCommand {
    /// Upload a file or folder to DRACOON
    Upload {
        /// Source file path
        source: String,

        /// Target file path in DRACOON
        target: String,

        /// Overwrite existing file in DRACOON
        #[clap(long)]
        overwrite: bool,

        /// Preserve Download Share Links and point them to the new node in DRACOON
        #[clap(long)]
        keep_share_links: bool,

        /// classification of the node (1-4)
        #[clap(long)]
        classification: Option<u8>,

        #[clap(long, short)]
        velocity: Option<u8>,

        /// recursive upload
        #[clap(short, long)]
        recursive: bool,

        /// skip root
        #[clap(long)]
        skip_root: bool,

        /// share upload
        #[clap(long)]
        share: bool,

        #[clap(long)]
        share_password: Option<String>,
    },
    /// Download a file or container from DRACOON to target
    Download {
        /// Source file path in DRACOON
        source: String,
        /// Target file path
        target: String,

        #[clap(long, short)]
        velocity: Option<u8>,

        /// recursive download
        #[clap(short, long)]
        recursive: bool,

        #[clap(long)]
        share_password: Option<String>,

        #[clap(long)]
        include_rooms: bool,
    },
    /// Transfer files across DRACOON instances
    Transfer {
        /// Source file path in DRACOON
        source: String,

        /// Target file path in DRACOON
        target: String,

        /// Overwrite existing file in DRACOON
        #[clap(long)]
        overwrite: bool,

        /// Preserve Download Share Links and point them to the new node in DRACOON
        #[clap(long)]
        keep_share_links: bool,

        /// classification of the node (1-4)
        #[clap(long)]
        classification: Option<u8>,

        /// share upload
        #[clap(long)]
        share: bool,

        #[clap(long)]
        share_password: Option<String>,
    },
    /// List nodes in DRACOON
    Ls {
        /// Source file path in DRACOON
        source: String,

        /// Filter nodes (e.g. by name)
        #[clap(long)]
        filter: Option<String>,

        /// Print node information (details)
        #[clap(short, long)]
        long: bool,

        /// human readable node size
        #[clap(short = 'r', long)]
        human_readable: bool,

        /// skip n nodes (default offset: 0)
        #[clap(short, long)]
        offset: Option<u64>,

        /// limit n nodes (default limit: 500)
        #[clap(long)]
        limit: Option<u32>,

        /// Display nodes as room manager / room admin
        #[clap(long)]
        managed: bool,

        /// fetch all nodes (default: 500)
        #[clap(long)]
        all: bool,
    },

    Cp {
        /// Source file path in DRACOON
        source: String,

        /// Target file path in DRACOON
        target: String,
    },

    /// Create a container in DRACOON (defaults to folder)
    Mkdir {
        /// Source file path in DRACOON
        source: String,

        /// container type (default: folder)
        #[clap(long = "type", value_enum, default_value_t = CreateContainerType::Folder)]
        r#type: CreateContainerType,

        /// classification of the node (1-4)
        #[clap(long)]
        classification: Option<u8>,

        /// Notes
        #[clap(long)]
        notes: Option<String>,

        /// admin usernames (room only)
        #[clap(long, short = 'a')]
        admin_users: Option<Vec<String>>,

        /// inherit permissions from parent room (room only)
        #[clap(long)]
        inherit_permissions: bool,
    },

    /// Create a room in DRACOON (deprecated; use `mkdir --type room`)
    #[clap(hide = true)]
    Mkroom {
        /// Source file path in DRACOON
        source: String,

        /// admin usernames
        #[clap(long, short)]
        admin_users: Option<Vec<String>>,

        /// classification of the node (1-4)
        #[clap(long)]
        classification: Option<u8>,

        /// inherit permissions from parent room
        #[clap(long)]
        inherit_permissions: bool,
    },

    /// Delete a node in DRACOON
    Rm {
        /// Source file path in DRACOON
        source: String,

        /// recursive delete (mandatory for rooms / folders)
        #[clap(short, long)]
        recursive: bool,
    },

    /// Manage users in DRACOON
    Users {
        #[clap(subcommand)]
        cmd: UsersCommand,
    },

    /// Manage groups in DRACOON
    Groups {
        #[clap(subcommand)]
        cmd: GroupsCommand,
    },

    /// Configure DRACOON Commander
    Config {
        #[clap(subcommand)]
        cmd: ConfigCommand,
    },

    /// Generate reports from DRACOON
    Reports {
        #[clap(subcommand)]
        cmd: ReportsCommand,
    },

    /// Print current dccmd-rs version
    Version,
}
#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::{CreateContainerType, DcCmd, DcCmdCommand};

    #[test]
    fn test_mkdir_defaults_to_folder_type() {
        let parsed =
            DcCmd::try_parse_from(["dccmd-rs", "mkdir", "example.com/room/folder"]).unwrap();

        match parsed.cmd {
            DcCmdCommand::Mkdir { r#type, .. } => {
                assert_eq!(r#type, CreateContainerType::Folder);
            }
            _ => panic!("Expected mkdir command"),
        }
    }

    #[test]
    fn test_mkdir_can_parse_room_type_and_room_options() {
        let parsed = DcCmd::try_parse_from([
            "dccmd-rs",
            "mkdir",
            "example.com/room/new-room",
            "--type",
            "room",
            "-a",
            "foo1",
            "-a",
            "foo2",
            "--inherit-permissions",
        ])
        .unwrap();

        match parsed.cmd {
            DcCmdCommand::Mkdir {
                r#type,
                admin_users,
                inherit_permissions,
                ..
            } => {
                assert_eq!(r#type, CreateContainerType::Room);
                assert_eq!(
                    admin_users,
                    Some(vec!["foo1".to_string(), "foo2".to_string()])
                );
                assert!(inherit_permissions);
            }
            _ => panic!("Expected mkdir command"),
        }
    }
}

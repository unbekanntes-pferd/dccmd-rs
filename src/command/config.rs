use clap::Parser;

#[derive(Parser)]
pub enum ConfigAuthCommand {
    /// List DRACOON refresh token
    Ls {
        /// DRACOON url
        target: String,
    },

    /// Remove a DRACOON refresh token
    Rm {
        /// DRACOON url
        target: String,
    },
}

#[derive(Parser)]
pub enum ConfigCryptoCommand {
    /// List DRACOON encryption secret
    Ls {
        /// DRACOON url
        target: String,
    },

    /// Remove a DRACOON encryption secret
    Rm {
        /// DRACOON url
        target: String,
    },
}

#[derive(Parser)]
pub enum ConfigCommand {
    /// Manage DRACOON Commander auth credentials (refresh token)
    Auth {
        #[clap(subcommand)]
        cmd: ConfigAuthCommand,
    },

    /// Manage DRACOON Commander encryption credentials (encryption secret)
    Crypto {
        #[clap(subcommand)]
        cmd: ConfigCryptoCommand,
    },

    SystemInfo {
        /// DRACOON url
        target: String,
    },
}

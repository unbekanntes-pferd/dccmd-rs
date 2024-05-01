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

    /// Add a DRACOON refresh token manually to store securely
    Add {
        /// DRACOON url
        target: String,

        /// Refresh token
        refresh_token: String,
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

    /// Add a DRACOON encryption secret manually to store securely
    Add {
        /// DRACOON url
        target: String,

        /// Encryption secret
        secret: String,
    },
}

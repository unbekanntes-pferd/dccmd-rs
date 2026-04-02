#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigAuthRequest {
    Ls { target: String },
    Rm { target: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigCryptoRequest {
    Ls { target: String },
    Rm { target: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigRequest {
    Auth { cmd: ConfigAuthRequest },
    Crypto { cmd: ConfigCryptoRequest },
    SystemInfo { target: String },
}

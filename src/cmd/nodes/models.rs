use console::Term;
use dco3::{auth::Connected, Dracoon};

use crate::cmd::{
    init_dracoon,
    models::{DcCmdError, PasswordAuth},
};

pub struct CmdMkRoomOptions {
    pub inherit_permissions: bool,
    pub classification: Option<u8>,
    pub auth: Option<PasswordAuth>,
    pub admin_users: Option<Vec<String>>,
}

impl CmdMkRoomOptions {
    pub fn new(
        inherit_permissions: bool,
        classification: Option<u8>,
        auth: Option<PasswordAuth>,
        admin_users: Option<Vec<String>>,
    ) -> Self {
        Self {
            inherit_permissions,
            classification,
            auth,
            admin_users,
        }
    }
}

pub struct CmdDownloadOptions {
    pub recursive: bool,
    pub velocity: Option<u8>,
    pub auth: Option<PasswordAuth>,
    pub encryption_password: Option<String>,
    pub share_password: Option<String>,
}

impl CmdDownloadOptions {
    pub fn new(
        recursive: bool,
        velocity: Option<u8>,
        auth: Option<PasswordAuth>,
        encryption_password: Option<String>,
        share_password: Option<String>,
    ) -> Self {
        Self {
            recursive,
            velocity,
            auth,
            encryption_password,
            share_password,
        }
    }
}

pub struct CmdUploadOptions {
    pub overwrite: bool,
    pub recursive: bool,
    pub skip_root: bool,
    pub share: bool,
    pub classification: Option<u8>,
    pub velocity: Option<u8>,
    pub auth: Option<PasswordAuth>,
    pub encryption_password: Option<String>,
    pub share_password: Option<String>,
}

#[allow(clippy::too_many_arguments)]
impl CmdUploadOptions {
    pub fn new(
        overwrite: bool,
        recursive: bool,
        skip_root: bool,
        share: bool,
        classification: Option<u8>,
        velocity: Option<u8>,
        auth: Option<PasswordAuth>,
        encryption_password: Option<String>,
        share_password: Option<String>,
    ) -> Self {
        Self {
            overwrite,
            classification,
            velocity,
            recursive,
            skip_root,
            share,
            auth,
            encryption_password,
            share_password,
        }
    }
}

pub struct UploadCommandHandler {
    client: Dracoon<Connected>,
    term: Term,
}

impl UploadCommandHandler {
    pub async fn try_new(target_domain: &str, term: Term) -> Result<Self, DcCmdError> {
        let client = init_dracoon(target_domain, None, false).await?;
        Ok(Self { client, term })
    }

    pub fn client(&self) -> &Dracoon<Connected> {
        &self.client
    }
}

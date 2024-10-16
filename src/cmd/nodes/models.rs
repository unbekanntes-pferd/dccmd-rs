#![allow(clippy::struct_excessive_bools)]

use crate::cmd::models::{ListOptions, PasswordAuth};

pub struct CmdCopyOptions {
    pub auth: Option<PasswordAuth>,
}

impl CmdCopyOptions {
    pub fn new(auth: Option<PasswordAuth>) -> Self {
        Self { auth }
    }
}

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

#[allow(clippy::struct_excessive_bools)]
#[derive(Clone)]
pub struct CmdUploadOptions {
    pub overwrite: bool,
    pub keep_share_links: bool,
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
#[allow(clippy::fn_params_excessive_bools)]
impl CmdUploadOptions {
    pub fn new(
        overwrite: bool,
        keep_share_links: bool,
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
            keep_share_links,
            recursive,
            skip_root,
            share,
            classification,
            velocity,
            auth,
            encryption_password,
            share_password,
        }
    }
}

pub struct CmdListNodesOptions {
    list_opts: ListOptions,
    human_readable: bool,
    long: bool,
    managed: bool,
    auth: Option<PasswordAuth>,
}

impl CmdListNodesOptions {
    pub fn new(
        list_opts: ListOptions,
        human_readable: bool,
        long: bool,
        managed: bool,
        auth: Option<PasswordAuth>,
    ) -> Self {
        Self {
            list_opts,
            human_readable,
            long,
            managed,
            auth,
        }
    }

    pub fn list_opts(&self) -> &ListOptions {
        &self.list_opts
    }

    pub fn human_readable(&self) -> bool {
        self.human_readable
    }

    pub fn long(&self) -> bool {
        self.long
    }

    pub fn managed(&self) -> bool {
        self.managed
    }

    pub fn auth(&self) -> Option<PasswordAuth> {
        self.auth.clone()
    }
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Clone)]
pub struct CmdTransferOptions {
    pub overwrite: bool,
    pub keep_share_links: bool,
    pub share: bool,
    pub classification: Option<u8>,
    pub share_password: Option<String>,
}

impl CmdTransferOptions {
    pub fn new(
        overwrite: bool,
        keep_share_links: bool,
        share: bool,
        classification: Option<u8>,
        share_password: Option<String>,
    ) -> Self {
        Self {
            overwrite,
            keep_share_links,
            share,
            classification,
            share_password,
        }
    }
}

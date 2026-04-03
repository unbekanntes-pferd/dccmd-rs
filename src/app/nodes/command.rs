#![allow(clippy::struct_excessive_bools)]

use crate::{command::CreateContainerType, core::models::ListOptions};

pub struct CmdCopyOptions;

impl CmdCopyOptions {
    pub fn new() -> Self {
        Self
    }
}

#[derive(Clone)]
pub struct CmdDeleteOptions {
    recursive: bool,
}

impl CmdDeleteOptions {
    pub fn new(recursive: bool) -> Self {
        Self { recursive }
    }

    pub fn recursive(&self) -> bool {
        self.recursive
    }
}

#[derive(Clone)]
pub struct CmdCreateContainerOptions {
    pub container_type: CreateContainerType,
    pub classification: Option<u8>,
    pub notes: Option<String>,
    pub admin_users: Option<Vec<String>>,
    pub inherit_permissions: bool,
}

impl CmdCreateContainerOptions {
    pub fn new(
        container_type: CreateContainerType,
        classification: Option<u8>,
        notes: Option<String>,
        admin_users: Option<Vec<String>>,
        inherit_permissions: bool,
    ) -> Self {
        Self {
            container_type,
            classification,
            notes,
            admin_users,
            inherit_permissions,
        }
    }
}

pub struct CmdDownloadOptions {
    pub recursive: bool,
    pub velocity: Option<u8>,
    pub share_password: Option<String>,
    pub include_rooms: bool,
}

impl CmdDownloadOptions {
    pub fn new(
        recursive: bool,
        velocity: Option<u8>,
        share_password: Option<String>,
        include_rooms: bool,
    ) -> Self {
        Self {
            recursive,
            velocity,
            share_password,
            include_rooms,
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
            share_password,
        }
    }
}

pub struct CmdListNodesOptions {
    list_opts: ListOptions,
    human_readable: bool,
    long: bool,
    managed: bool,
}

impl CmdListNodesOptions {
    pub fn new(list_opts: ListOptions, human_readable: bool, long: bool, managed: bool) -> Self {
        Self {
            list_opts,
            human_readable,
            long,
            managed,
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

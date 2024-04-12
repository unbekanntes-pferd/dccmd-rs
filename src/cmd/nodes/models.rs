use console::Term;
use dco3::{auth::Connected, Dracoon};

use crate::cmd::{init_dracoon, models::DcCmdError};

pub struct UploadOptions {
    pub overwrite: bool,
    pub classification: Option<u8>,
    pub velocity: Option<u8>,
    pub recursive: bool,
    pub skip_root: bool,
    pub share: bool,
}

impl UploadOptions {
    pub fn new(
        overwrite: bool,
        classification: Option<u8>,
        velocity: Option<u8>,
        recursive: bool,
        skip_root: bool,
        share: bool,
    ) -> Self {
        Self {
            overwrite,
            classification,
            velocity,
            recursive,
            skip_root,
            share,
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

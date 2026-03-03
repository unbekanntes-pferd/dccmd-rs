use async_trait::async_trait;
use dco3::{
    auth::Connected,
    nodes::Node,
    shares::{CreateDownloadShareRequest, DownloadShares},
    Dracoon,
};

use crate::core::models::DcCmdError;

const PUBLIC_DOWNLOAD_SHARE_URL: &str = "public/download-shares/";

#[async_trait]
pub trait DownloadShareLinkCreator: Send + Sync {
    async fn create_download_share_link(
        &self,
        node: &Node,
        share_password: Option<String>,
    ) -> Result<String, DcCmdError>;
}

#[derive(Clone)]
pub struct DracoonDownloadShareLinkCreator {
    client: Dracoon<Connected>,
}

impl DracoonDownloadShareLinkCreator {
    pub fn new(client: Dracoon<Connected>) -> Self {
        Self { client }
    }
}

#[async_trait]
impl DownloadShareLinkCreator for DracoonDownloadShareLinkCreator {
    async fn create_download_share_link(
        &self,
        node: &Node,
        share_password: Option<String>,
    ) -> Result<String, DcCmdError> {
        let share_request = if let Some(password) = share_password {
            CreateDownloadShareRequest::builder(node.id)
                .with_password(password)
                .build()
        } else {
            CreateDownloadShareRequest::builder(node.id).build()
        };

        let share = self
            .client
            .shares()
            .create_download_share(share_request)
            .await?;

        Ok(format!(
            "{}{}{}",
            self.client.get_base_url(),
            PUBLIC_DOWNLOAD_SHARE_URL,
            share.access_key
        ))
    }
}

use async_trait::async_trait;
use dco3::{
    auth::Connected,
    nodes::{models::DownloadProgressCallback, Node},
    Download, Dracoon,
};
use tokio::io::AsyncWrite;

use crate::{app::nodes::api::NodesApi, core::models::DcCmdError};

#[async_trait]
pub trait DownloadApi: NodesApi + Send + Sync {
    async fn download_node<'w>(
        &'w self,
        node: &Node,
        writer: &'w mut (dyn AsyncWrite + Send + Unpin),
        callback: Option<DownloadProgressCallback>,
    ) -> Result<(), DcCmdError>;
}

#[async_trait]
impl DownloadApi for Dracoon<Connected> {
    async fn download_node<'w>(
        &'w self,
        node: &Node,
        writer: &'w mut (dyn AsyncWrite + Send + Unpin),
        callback: Option<DownloadProgressCallback>,
    ) -> Result<(), DcCmdError> {
        self.download(node, writer, callback, None)
            .await
            .map_err(Into::into)
    }
}

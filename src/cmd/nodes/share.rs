use dco3::{
    auth::Connected, nodes::Node, shares::CreateDownloadShareRequest, DownloadShares, Dracoon,
};

use crate::cmd::models::DcCmdError;

const SHARE_URL: &str = "public/download-shares/";

pub async fn share_node(client: &Dracoon<Connected>, node: &Node) -> Result<String, DcCmdError> {
    let share_request = CreateDownloadShareRequest::builder(node.id).build();

    let share = client.shares.create_download_share(share_request).await?;

    let share_link = format!("{}{}{}", client.get_base_url(), SHARE_URL, share.access_key);

    Ok(share_link)
}

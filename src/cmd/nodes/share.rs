use dco3::{
    auth::Connected, nodes::Node, shares::CreateDownloadShareRequest, DownloadShares, Dracoon,
};

use crate::cmd::models::DcCmdError;

const SHARE_URL: &str = "public/download-shares/";

pub async fn share_node(
    client: &Dracoon<Connected>,
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

    let share = client.shares().create_download_share(share_request).await?;

    let share_link = format!("{}{}{}", client.get_base_url(), SHARE_URL, share.access_key);

    Ok(share_link)
}

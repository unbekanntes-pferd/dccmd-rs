use tracing::debug;

use crate::api::{DracoonBuilder, nodes::Nodes, constants::get_client_credentials, Dracoon, auth::{Connected, OAuth2Flow}};
use self::{models::DcCmdError, credentials::get_dracoon_env};

pub mod models;
pub mod credentials;
pub mod utils;



pub async fn download(source: String, target: String) -> Result<(), DcCmdError>{
todo!()

}

pub fn upload(source: String, target: String) {

    todo!()

}

pub async fn get_nodes(source: String) -> Result<(), DcCmdError> {
    debug!("Fetching node list from {}", source);
    let dracoon = init_dracoon(source).await?;
    let node_list = dracoon.get_nodes(None).await?;


    node_list.items
    .iter()
    .for_each(|node| println!("{}", node.name));

    Ok(())

}

async fn init_dracoon(url_path: String) -> Result<Dracoon<Connected>, DcCmdError> {

    let (client_id, client_secret) = get_client_credentials();
    let base_url = parse_base_url(url_path)?;

    let mut dracoon = DracoonBuilder::new()
                                 .with_base_url(base_url.clone())
                                 .with_client_id(client_id)
                                 .with_client_secret(client_secret)
                                 .build()?;

    let dracoon = match get_dracoon_env(&base_url) {
        Ok(refresh_token) => dracoon.connect(OAuth2Flow::RefreshToken(refresh_token)).await?,
        Err(_) => {
            debug!("No refresh token stored for {}", base_url);
            println!("Please log in via browser (open url): ");
            println!("{}", dracoon.get_authorize_url());
            println!("Please enter authorization code: ");
            let mut auth_code = String::new();
            std::io::stdin()
                .read_line(&mut auth_code)
                .expect("Error parsing user input (auth code).");

            dracoon.connect(OAuth2Flow::AuthCodeFlow(auth_code.trim_end().into())).await?

//TODO: store credentials securely
        }
    };

    debug!("Successfully authenticated to {}", base_url);

    Ok(dracoon)

}


fn parse_base_url(url_str: String) -> Result<String, DcCmdError> {

if url_str.starts_with("http://") {
    return Err(DcCmdError::InvalidUrl)
};

let url_str = match url_str.starts_with("https://") {
    true => url_str,
    false => format!("https://{url_str}")
};

let uri_fragments: Vec<&str> = url_str[8..].split('/').collect();

match uri_fragments.len() {
    2.. => Ok(format!("https://{}", uri_fragments[0])),
    _ => Err(DcCmdError::InvalidUrl)
}
}

#[cfg(test)]
mod tests {
    use super::*;


    #[test]
    fn test_base_url_parse_https() {
        let base_url = parse_base_url("https://bla.dracoon.com/bla/somefile.pdf".into()).unwrap();
        assert_eq!(base_url, "https://bla.dracoon.com");
    }

    #[test]
    fn test_base_url_parse_no_https() {
        let base_url = parse_base_url("bla.dracoon.com/bla/somefile.pdf".into()).unwrap();
        assert_eq!(base_url, "https://bla.dracoon.com");
    }

    #[test]
    fn test_base_url_parse_invalid_path() {
        let base_url = parse_base_url("bla.dracoon.com".into());
        assert_eq!(base_url, Err(DcCmdError::InvalidUrl));
    }

}
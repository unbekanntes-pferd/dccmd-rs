use console::Term;
use keyring::Entry;
use tracing::{debug, error};


use self::{
    credentials::{get_dracoon_env, set_dracoon_env},
    models::DcCmdError, utils::{ strings::{format_error_message}},
   
};
use crate::{
    api::{
        auth::{Connected, OAuth2Flow},
        constants::get_client_credentials,
        Dracoon, DracoonBuilder,
    },
};

pub mod credentials;
pub mod models;
pub mod utils;
pub mod nodes;


// service name to store
const SERVICE_NAME: &str = env!("CARGO_PKG_NAME");

/// initializes a dracoon client with encryption enabled (plain keypair ready to use)
async fn init_encryption(
    mut dracoon: Dracoon<Connected>,
) -> Result<Dracoon<Connected>, DcCmdError> {

    let account = format!("{}-crypto", dracoon.get_base_url());

    let entry = Entry::new(SERVICE_NAME, &account).map_err(|_|
    DcCmdError::CredentialStorageFailed)?;


    let (secret, store) =
        if let Ok(secret) = get_dracoon_env(&entry) {
            (secret, false)
        } else {
            let secret = dialoguer::Password::new()
                .with_prompt("Please enter your encryption secret")
                .interact()
                .or(Err(DcCmdError::IoError))?;
            (secret, true)
        };

    let keypair = dracoon.get_keypair(Some(&secret)).await?;

    if store {
        set_dracoon_env(&entry, &secret)?;
    }

    Ok(dracoon)
}



async fn init_dracoon(url_path: &str) -> Result<Dracoon<Connected>, DcCmdError> {
    let (client_id, client_secret) = get_client_credentials();
    let base_url = parse_base_url(url_path.to_string())?;

    let mut dracoon = DracoonBuilder::new()
        .with_base_url(base_url.clone())
        .with_client_id(client_id)
        .with_client_secret(client_secret)
        .build()?;

    let entry = Entry::new(SERVICE_NAME, base_url.as_str()).map_err(|_| {
        error!("Failed to open keyring entry for {}", base_url);
        DcCmdError::CredentialStorageFailed
    })?;

    let dracoon = if let Ok(refresh_token) = get_dracoon_env(&entry) {
         
            dracoon
                .connect(OAuth2Flow::RefreshToken(refresh_token))
                .await?
        } else {
            debug!("No refresh token stored for {}", base_url);
            println!("Please log in via browser (open url): ");
            println!("{}", dracoon.get_authorize_url());
            println!("Please enter authorization code: ");
            let mut auth_code = String::new();
            std::io::stdin()
                .read_line(&mut auth_code)
                .expect("Error parsing user input (auth code).");

            let dracoon = dracoon
                .connect(OAuth2Flow::AuthCodeFlow(auth_code.trim_end().into()))
                .await?;

            set_dracoon_env(&entry, dracoon.get_refresh_token())?;

            dracoon
        };

    debug!("Successfully authenticated to {}", base_url);

    Ok(dracoon)
}

fn parse_base_url(url_str: String) -> Result<String, DcCmdError> {
    if url_str.starts_with("http://") {
        error!("HTTP is not supported.");
        return Err(DcCmdError::InvalidUrl(url_str));
    };

    let url_str = if url_str.starts_with("https://") {
        url_str
    } else {
        format!("https://{url_str}")
    };

    let uri_fragments: Vec<&str> = url_str[8..].split('/').collect();

    match uri_fragments.len() {
        2.. => Ok(format!("https://{}", uri_fragments[0])),
        _ => Err(DcCmdError::InvalidUrl(url_str)),
    }
}

pub fn handle_error(term: &Term, err: &DcCmdError) {
    let err_msg = get_error_message(err);
    let err_msg = format_error_message(&err_msg);

    term.write_line(&err_msg)
        .expect("Error writing error message to terminal.");
}

fn get_error_message(err: &DcCmdError) -> String {
    match err {
        DcCmdError::InvalidUrl(url) => format!("Invalid URL: {url}"),
        DcCmdError::InvalidPath(path) => format!("Invalid path: {path}"),
        DcCmdError::IoError => "Error reading / writing content.".into(),
        DcCmdError::DracoonError(e) => format!("{e}"),
        DcCmdError::ConnectionFailed => "Connection failed.".into(),
        DcCmdError::CredentialDeletionFailed => "Credential deletion failed.".into(),
        DcCmdError::CredentialStorageFailed => "Credential store failed.".into(),
        DcCmdError::InvalidAccount => "Invalid account.".into(),
        DcCmdError::Unknown => "Unknown error.".into(),
        DcCmdError::DracoonS3Error(e) => format!("{e}"),
        DcCmdError::DracoonAuthError(e) => format!("{e}"),
        DcCmdError::InvalidArgument(msg) => msg.to_string()
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
        assert_eq!(
            base_url,
            Err(DcCmdError::InvalidUrl("https://bla.dracoon.com".into()))
        );
    }
}

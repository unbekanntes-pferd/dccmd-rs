use console::Term;
use keyring::Entry;
use tracing::error;

use crate::cmd::credentials::{delete_dracoon_env, get_client_credentials};

use self::{
    credentials::{get_dracoon_env, set_dracoon_env},
    models::{DcCmdError, PasswordAuth},
    utils::strings::format_error_message,
};
use dco3::{
    auth::{Connected, Disconnected, OAuth2Flow},
    Dracoon, DracoonBuilder,
};

pub mod credentials;
pub mod models;
pub mod nodes;
pub mod users;
pub mod utils;

// service name to store
const SERVICE_NAME: &str = env!("CARGO_PKG_NAME");

/// initializes a dracoon client with encryption enabled (plain keypair ready to use)
async fn init_encryption(
    dracoon: Dracoon<Connected>,
    encryption_password: Option<String>,
) -> Result<Dracoon<Connected>, DcCmdError> {
    let account = format!("{}-crypto", dracoon.get_base_url());

    let entry = Entry::new(SERVICE_NAME, &account).map_err(|_| DcCmdError::CredentialStorageFailed);

    // Helper to get password from user
    let ask_for_secret = || {
        dialoguer::Password::new()
            .with_prompt("Please enter your encryption secret")
            .interact()
            .or(Err(DcCmdError::IoError))
    };

    let (secret, store) = match encryption_password {
        // Provided password, don't store
        Some(password) => (password, false),
        None => {
            // Entry present and holds a secret
            if let Ok(entry) = &entry {
                if let Ok(stored_secret) = get_dracoon_env(entry) {
                    (stored_secret, false)
                } else {
                    // Entry present but no secret, ask and store
                    (ask_for_secret()?, true)
                }
            } else {
                // No entry, ask but don't store
                (ask_for_secret()?, false)
            }
        }
    };

    let _ = dracoon.get_keypair(Some(secret.clone())).await?;

    // If necessary, create a new entry to store the secret
    if store {
        let entry =
            Entry::new(SERVICE_NAME, &account).map_err(|_| DcCmdError::CredentialStorageFailed)?;
        set_dracoon_env(&entry, &secret)?;
    }

    Ok(dracoon)
}

async fn init_dracoon(
    url_path: &str,
    password_auth: Option<PasswordAuth>,
    is_transfer: bool,
) -> Result<Dracoon<Connected>, DcCmdError> {
    let (client_id, client_secret) = get_client_credentials();
    let base_url = parse_base_url(url_path.to_string())?;

    // use multiple access tokens for transfers
    let token_rotation = if is_transfer { 5 } else { 1 };

    let dccmd_user_agent = format!("{}|{}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));

    let dracoon = DracoonBuilder::new()
        .with_base_url(base_url.clone())
        .with_client_id(client_id)
        .with_client_secret(client_secret)
        .with_token_rotation(token_rotation)
        .with_user_agent(dccmd_user_agent)
        .build()?;

    let entry =
        Entry::new(SERVICE_NAME, &base_url).map_err(|_| DcCmdError::CredentialStorageFailed);

    // Always use password auth first if present
    if let Some(password_auth) = password_auth {
        return authenticate_password_flow(dracoon, password_auth).await;
    }

    // Entry not present & no password auth? Game over.
    let entry = match entry {
        Ok(entry) => entry,
        Err(_) => {
            error!("Can't open keyring entry for {}", base_url);
            return Err(DcCmdError::CredentialStorageFailed);
        }
    };

    // Attempt to use refresh token if exists
    if let Ok(refresh_token) = get_dracoon_env(&entry) {
        if let Ok(dracoon) = dracoon
            .clone()
            .connect(OAuth2Flow::RefreshToken(refresh_token))
            .await
        {
            return Ok(dracoon);
        }
        // Refresh token didn't work, delete it
        let _ = delete_dracoon_env(&entry, &base_url);
    }

    // Final resort: auth code flow
    authenticate_auth_code_flow(dracoon, entry).await
}

async fn authenticate_auth_code_flow(
    dracoon: Dracoon<Disconnected>,
    entry: Entry,
) -> Result<Dracoon<Connected>, DcCmdError> {
    println!("Please log in via browser (open url): ");
    println!("{}", dracoon.get_authorize_url());

    let auth_code = dialoguer::Password::new()
        .with_prompt("Please enter authorization code")
        .interact()
        .or(Err(DcCmdError::IoError))?;

    let dracoon = dracoon
        .connect(OAuth2Flow::AuthCodeFlow(auth_code.trim_end().into()))
        .await?;

    // TODO: if this fails, offer to store in plain
    set_dracoon_env(&entry, &dracoon.get_refresh_token().await)?;

    Ok(dracoon)
}

async fn authenticate_password_flow(
    dracoon: Dracoon<Disconnected>,
    password_auth: PasswordAuth,
) -> Result<Dracoon<Connected>, DcCmdError> {
    let dracoon = dracoon
        .connect(OAuth2Flow::password_flow(password_auth.0, password_auth.1))
        .await?;

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

    error!("{}", err_msg);

    term.write_line(&err_msg)
        .expect("Error writing error message to terminal.");

    // exit with error code
    std::process::exit(1);
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
        DcCmdError::InvalidArgument(msg) => msg.to_string(),
        DcCmdError::LogFileCreationFailed => "Log file creation failed.".into(),
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

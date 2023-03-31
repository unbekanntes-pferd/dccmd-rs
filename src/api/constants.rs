/// constants for grant_type
pub const GRANT_TYPE_PASSWORD: &str = "password";
pub const GRANT_TYPE_AUTH_CODE: &str = "authorization_code";
pub const GRANT_TYPE_REFRESH_TOKEN: &str = "refresh_token";
pub const TOKEN_TYPE_HINT_ACCESS: &str = "access_token";

/// constants for API urls
/// AUTH
pub const DRACOON_TOKEN_URL: &str = "oauth/token";
pub const DRACOON_REDIRECT_URL: &str = "oauth/callback";
pub const DRACOON_TOKEN_REVOKE_URL: &str = "oauth/revoke";

/// API
pub const DRACOON_API_PREFIX: &str = "api/v4";

/// NODES
pub const GET_NODES: &str = "nodes";

/// user agent header
pub const APP_USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"));

pub fn get_client_credentials() -> (String, String) {
    let client_id = include_str!("../../.env")
        .split('\n')
        .next()
        .expect("env file has more than one line")
        .split("CLIENT_ID=")
        .nth(1)
        .expect("CLIENT_ID MUST be provided");
    let client_secret = include_str!("../../.env")
        .split('\n')
        .nth(1)
        .expect("env file has more than one line")
        .split("CLIENT_SECRET=")
        .nth(1)
        .expect("CLIENT_SECRET MUST be provided");

    (client_id.into(), client_secret.into())
}

mod groups;
mod nodes;
mod paths;
mod platform;
mod progress;
mod server;
mod users;

use secrecy::SecretString;

use crate::core::models::DcCmdError;

pub async fn start(
    target: String,
    allow_destructive: bool,
    encryption_password: Option<SecretString>,
) -> Result<(), DcCmdError> {
    let paths = paths::WorkspacePathGuard::from_current_dir()?;
    let platform = platform::McpPlatform::new(target, encryption_password, paths)?;
    platform.preflight().await?;

    server::DccmdMcpServer::new(platform, allow_destructive)
        .serve_stdio()
        .await
}

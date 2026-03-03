use std::path::PathBuf;

pub mod constants;
pub mod logs;
pub mod models;
pub mod session;
pub mod utils;

use crate::core::models::DcCmdError;

pub(crate) fn get_or_create_config_dir() -> Result<PathBuf, DcCmdError> {
    let config_dir = dirs::config_dir().ok_or(DcCmdError::ConfigDirUnavailable)?;
    let config_dir = config_dir.join(constants::APPLICATION_NAME);

    if !config_dir.exists() {
        std::fs::create_dir_all(&config_dir).map_err(|err| {
            DcCmdError::ConfigDirCreationFailed(format!("{} ({err})", config_dir.display()))
        })?;
    }

    Ok(config_dir)
}

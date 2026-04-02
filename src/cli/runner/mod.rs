mod mapping;

use async_trait::async_trait;
use console::Term;
use secrecy::SecretString;

use self::mapping::{
    map_config_request, map_groups_request, map_reports_request, map_users_request,
};
use crate::cli::{platform::CliPlatform, term_ui::TermUi};
use crate::{
    app::{
        nodes::command::{
            CmdCopyOptions, CmdCreateContainerOptions, CmdDeleteOptions, CmdDownloadOptions,
            CmdListNodesOptions, CmdTransferOptions, CmdUploadOptions,
        },
        results::{AppResult, AppStatus},
        App,
    },
    command::{AppCommand, CreateContainerType, DcCmdCommand},
    core::models::{DcCmdError, ListOptions, PasswordAuth},
};

#[async_trait]
pub(crate) trait AppExecutor: Send + Sync {
    async fn execute_app_command(&self, command: AppCommand) -> Result<AppResult, DcCmdError>;
}

#[async_trait]
impl AppExecutor for App<CliPlatform, TermUi> {
    async fn execute_app_command(&self, command: AppCommand) -> Result<AppResult, DcCmdError> {
        self.execute(command).await
    }
}

pub struct CliRunner<E: AppExecutor = App<CliPlatform, TermUi>> {
    app: E,
    term: Term,
    password_auth: Option<PasswordAuth>,
    encryption_password: Option<SecretString>,
}

impl CliRunner<App<CliPlatform, TermUi>> {
    pub fn new(
        term: Term,
        err_term: Term,
        password_auth: Option<PasswordAuth>,
        encryption_password: Option<SecretString>,
    ) -> Self {
        let app = App::new(CliPlatform::new(), TermUi::new(term.clone(), err_term));

        CliRunner {
            app,
            term,
            password_auth,
            encryption_password,
        }
    }
}

impl<E: AppExecutor> CliRunner<E> {
    #[cfg(test)]
    fn new_with_executor(
        app: E,
        term: Term,
        password_auth: Option<PasswordAuth>,
        encryption_password: Option<SecretString>,
    ) -> Self {
        Self {
            app,
            term,
            password_auth,
            encryption_password,
        }
    }

    pub async fn execute(&self, cmd: DcCmdCommand) -> Result<(), DcCmdError> {
        match cmd {
            DcCmdCommand::Download {
                source,
                target,
                velocity,
                recursive,
                share_password,
                include_rooms,
            } => {
                self.run_app_command(AppCommand::Download {
                    source,
                    target,
                    opts: CmdDownloadOptions::new(
                        recursive,
                        velocity,
                        self.password_auth.clone(),
                        self.encryption_password.clone(),
                        share_password,
                        include_rooms,
                    ),
                })
                .await
            }
            DcCmdCommand::Upload {
                source,
                target,
                overwrite,
                keep_share_links,
                classification,
                velocity,
                recursive,
                skip_root,
                share,
                share_password,
            } => {
                self.run_app_command(AppCommand::Upload {
                    source,
                    target,
                    opts: CmdUploadOptions::new(
                        overwrite,
                        keep_share_links,
                        recursive,
                        skip_root,
                        share,
                        classification,
                        velocity,
                        self.password_auth.clone(),
                        self.encryption_password.clone(),
                        share_password,
                    ),
                })
                .await
            }
            DcCmdCommand::Transfer {
                source,
                target,
                overwrite,
                keep_share_links,
                classification,
                share,
                share_password,
            } => {
                self.run_app_command(AppCommand::Transfer {
                    source,
                    target,
                    opts: CmdTransferOptions::new(
                        overwrite,
                        keep_share_links,
                        share,
                        classification,
                        share_password,
                    ),
                })
                .await
            }
            DcCmdCommand::Ls {
                source,
                filter,
                long,
                human_readable,
                managed,
                all,
                offset,
                limit,
            } => {
                let list_opts = ListOptions::new(filter, offset, limit, all, false);
                let opts = CmdListNodesOptions::new(
                    list_opts,
                    human_readable,
                    long,
                    managed,
                    self.password_auth.clone(),
                );

                self.run_app_command(AppCommand::Ls { source, opts }).await
            }
            DcCmdCommand::Cp { source, target } => {
                self.run_app_command(AppCommand::Cp {
                    source,
                    target,
                    opts: CmdCopyOptions::new(self.password_auth.clone()),
                })
                .await
            }
            DcCmdCommand::Mkdir {
                source,
                r#type,
                classification,
                notes,
                admin_users,
                inherit_permissions,
            } => {
                self.run_app_command(AppCommand::Mkdir {
                    source,
                    opts: CmdCreateContainerOptions::new(
                        r#type,
                        classification,
                        notes,
                        self.password_auth.clone(),
                        admin_users,
                        inherit_permissions,
                    ),
                    deprecated_alias: false,
                })
                .await
            }
            DcCmdCommand::Mkroom {
                inherit_permissions,
                source,
                classification,
                admin_users,
            } => {
                self.run_app_command(AppCommand::Mkdir {
                    source,
                    opts: CmdCreateContainerOptions::new(
                        CreateContainerType::Room,
                        classification,
                        None,
                        self.password_auth.clone(),
                        admin_users,
                        inherit_permissions,
                    ),
                    deprecated_alias: true,
                })
                .await
            }
            DcCmdCommand::Rm { source, recursive } => {
                self.run_app_command(AppCommand::Rm {
                    source,
                    opts: CmdDeleteOptions::new(recursive, self.password_auth.clone()),
                })
                .await
            }
            DcCmdCommand::Users { cmd } => {
                self.run_app_command(AppCommand::Users {
                    cmd: map_users_request(cmd),
                    auth: self.password_auth.clone(),
                })
                .await
            }
            DcCmdCommand::Groups { cmd } => {
                self.run_app_command(AppCommand::Groups {
                    cmd: map_groups_request(cmd),
                    auth: self.password_auth.clone(),
                })
                .await
            }
            DcCmdCommand::Reports { cmd } => {
                self.run_app_command(AppCommand::Reports {
                    cmd: map_reports_request(cmd),
                    auth: self.password_auth.clone(),
                })
                .await
            }
            DcCmdCommand::Version => self.execute_version(),
            DcCmdCommand::Config { cmd } => {
                self.run_app_command(AppCommand::Config {
                    cmd: map_config_request(cmd),
                })
                .await
            }
        }
    }

    pub(super) fn execute_version(&self) -> Result<(), DcCmdError> {
        TermUi::write_version(&self.term)
    }

    async fn run_app_command(&self, command: AppCommand) -> Result<(), DcCmdError> {
        let result = self.app.execute_app_command(command).await?;
        match result.status() {
            AppStatus::Success => Ok(()),
            AppStatus::PartialFailure | AppStatus::Failure => Err(DcCmdError::CommandFailed),
        }
    }
}

#[cfg(test)]
mod tests;

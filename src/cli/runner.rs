use async_trait::async_trait;
use console::Term;
use secrecy::SecretString;

use crate::cli::{platform::CliPlatform, term_ui::TermUi};
use crate::{
    app::{
        nodes::command::{
            CmdCopyOptions, CmdCreateContainerOptions, CmdDeleteOptions, CmdDownloadOptions,
            CmdListNodesOptions, CmdTransferOptions, CmdUploadOptions,
        },
        outcome::OutcomeMessageKind,
        results::AppResult,
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
                    cmd,
                    auth: self.password_auth.clone(),
                })
                .await
            }
            DcCmdCommand::Groups { cmd } => {
                self.run_app_command(AppCommand::Groups {
                    cmd,
                    auth: self.password_auth.clone(),
                })
                .await
            }
            DcCmdCommand::Reports { cmd } => {
                self.run_app_command(AppCommand::Reports {
                    cmd,
                    auth: self.password_auth.clone(),
                })
                .await
            }
            DcCmdCommand::Version => self.execute_version(),
            DcCmdCommand::Config { cmd } => self.run_app_command(AppCommand::Config { cmd }).await,
        }
    }

    pub(super) fn execute_version(&self) -> Result<(), DcCmdError> {
        TermUi::write_version(&self.term)
    }

    async fn run_app_command(&self, command: AppCommand) -> Result<(), DcCmdError> {
        let outcome = self.app.execute_app_command(command).await?.into_outcome();
        if outcome
            .messages
            .iter()
            .any(|message| message.kind == OutcomeMessageKind::Error)
        {
            return Err(DcCmdError::CommandFailed);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;
    use crate::{
        app::{
            outcome::{CommandOutcome, OutcomeMessageKind},
            results::AppResult,
        },
        command::{
            config::{ConfigAuthCommand, ConfigCommand},
            AppCommand, CreateContainerType, GroupsCommand, GroupsUsersCommand, UsersCommand,
        },
    };

    type AppCommandChecker = Box<dyn FnOnce(AppCommand) + Send>;

    struct MockAppExecutor {
        checker: Mutex<Option<AppCommandChecker>>,
        fail: bool,
        outcome_error: bool,
    }

    impl MockAppExecutor {
        fn with_checker(checker: impl FnOnce(AppCommand) + Send + 'static) -> Self {
            Self {
                checker: Mutex::new(Some(Box::new(checker))),
                fail: false,
                outcome_error: false,
            }
        }

        fn with_error() -> Self {
            Self {
                checker: Mutex::new(None),
                fail: true,
                outcome_error: false,
            }
        }

        fn with_error_outcome() -> Self {
            Self {
                checker: Mutex::new(None),
                fail: false,
                outcome_error: true,
            }
        }
    }

    #[async_trait]
    impl AppExecutor for MockAppExecutor {
        async fn execute_app_command(&self, command: AppCommand) -> Result<AppResult, DcCmdError> {
            if self.fail {
                return Err(DcCmdError::InvalidArgument("app failed".to_string()));
            }

            if let Some(checker) = self.checker.lock().expect("lock poisoned").take() {
                checker(command);
            }

            let mut outcome = CommandOutcome::default();
            if self.outcome_error {
                outcome.push(OutcomeMessageKind::Error, "app reported failure");
            }
            Ok(AppResult::messages(outcome))
        }
    }

    #[tokio::test]
    async fn test_execute_dispatches_ls_to_app_command() {
        let app = MockAppExecutor::with_checker(|command| match command {
            AppCommand::Ls { source, opts } => {
                assert_eq!(source, "example.com/rooms");
                assert!(opts.long());
                assert!(opts.human_readable());
                assert!(opts.managed());
                assert!(opts.list_opts().all());
            }
            _ => panic!("expected AppCommand::Ls"),
        });

        let runner = CliRunner::new_with_executor(
            app,
            Term::buffered_stdout(),
            None,
            Some(SecretString::new("enc".into())),
        );

        runner
            .execute(DcCmdCommand::Ls {
                source: "example.com/rooms".to_string(),
                filter: None,
                long: true,
                human_readable: true,
                managed: true,
                all: true,
                offset: None,
                limit: None,
            })
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_execute_dispatches_mkroom_alias_to_mkdir_app_command() {
        let app = MockAppExecutor::with_checker(|command| match command {
            AppCommand::Mkdir {
                source,
                opts,
                deprecated_alias,
            } => {
                assert_eq!(source, "example.com/room/new-room");
                assert!(deprecated_alias);
                assert_eq!(opts.container_type, CreateContainerType::Room);
                assert_eq!(opts.classification, Some(3));
                assert_eq!(opts.admin_users, Some(vec!["ops-admin".to_string()]));
                assert!(!opts.inherit_permissions);
            }
            _ => panic!("expected AppCommand::Mkdir"),
        });

        let runner = CliRunner::new_with_executor(app, Term::buffered_stdout(), None, None);

        runner
            .execute(DcCmdCommand::Mkroom {
                source: "example.com/room/new-room".to_string(),
                admin_users: Some(vec!["ops-admin".to_string()]),
                classification: Some(3),
                inherit_permissions: false,
            })
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_execute_dispatches_users_groups_and_reports_to_app() {
        let app = MockAppExecutor::with_checker(|command| match command {
            AppCommand::Users { cmd, auth } => {
                assert!(auth.is_none());
                match cmd {
                    UsersCommand::Ls { target, .. } => assert_eq!(target, "example.com"),
                    _ => panic!("expected users ls"),
                }
            }
            _ => panic!("expected AppCommand::Users"),
        });
        let runner = CliRunner::new_with_executor(app, Term::buffered_stdout(), None, None);
        runner
            .execute(DcCmdCommand::Users {
                cmd: UsersCommand::Ls {
                    target: "example.com".to_string(),
                    filter: None,
                    offset: None,
                    limit: None,
                    all: false,
                    csv: false,
                },
            })
            .await
            .unwrap();

        let app = MockAppExecutor::with_checker(|command| match command {
            AppCommand::Groups { cmd, auth } => {
                assert!(auth.is_none());
                match cmd {
                    GroupsCommand::Users { cmd } => match cmd {
                        GroupsUsersCommand::Add {
                            target, group_id, ..
                        } => {
                            assert_eq!(target, "example.com");
                            assert_eq!(group_id, Some(7));
                        }
                        _ => panic!("expected group user add"),
                    },
                    _ => panic!("expected groups users"),
                }
            }
            _ => panic!("expected AppCommand::Groups"),
        });
        let runner = CliRunner::new_with_executor(app, Term::buffered_stdout(), None, None);
        runner
            .execute(DcCmdCommand::Groups {
                cmd: GroupsCommand::Users {
                    cmd: GroupsUsersCommand::Add {
                        target: "example.com".to_string(),
                        group_name: None,
                        group_id: Some(7),
                        user_name: None,
                        user_id: Some(9),
                    },
                },
            })
            .await
            .unwrap();

        let app = MockAppExecutor::with_checker(|command| match command {
            AppCommand::Reports { cmd, auth } => {
                assert!(auth.is_none());
                match cmd {
                    crate::command::ReportsCommand::OperationTypes { target } => {
                        assert_eq!(target, "example.com")
                    }
                    _ => panic!("expected operation types"),
                }
            }
            _ => panic!("expected AppCommand::Reports"),
        });
        let runner = CliRunner::new_with_executor(app, Term::buffered_stdout(), None, None);
        runner
            .execute(DcCmdCommand::Reports {
                cmd: crate::command::ReportsCommand::OperationTypes {
                    target: "example.com".to_string(),
                },
            })
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_execute_dispatches_global_password_auth_to_users_groups_and_reports() {
        let password_auth = Some(PasswordAuth::new(
            "alice".to_string(),
            SecretString::new("secret".into()),
        ));

        let app = MockAppExecutor::with_checker(|command| match command {
            AppCommand::Users { auth, .. } => {
                let auth = auth.expect("users auth should be set");
                assert_eq!(auth.username(), "alice");
                assert_eq!(auth.password_str(), "secret");
            }
            _ => panic!("expected users command"),
        });
        let runner =
            CliRunner::new_with_executor(app, Term::buffered_stdout(), password_auth.clone(), None);
        runner
            .execute(DcCmdCommand::Users {
                cmd: UsersCommand::Ls {
                    target: "example.com".to_string(),
                    filter: None,
                    offset: None,
                    limit: None,
                    all: false,
                    csv: false,
                },
            })
            .await
            .unwrap();

        let app = MockAppExecutor::with_checker(|command| match command {
            AppCommand::Groups { auth, .. } => {
                let auth = auth.expect("groups auth should be set");
                assert_eq!(auth.username(), "alice");
                assert_eq!(auth.password_str(), "secret");
            }
            _ => panic!("expected groups command"),
        });
        let runner =
            CliRunner::new_with_executor(app, Term::buffered_stdout(), password_auth.clone(), None);
        runner
            .execute(DcCmdCommand::Groups {
                cmd: GroupsCommand::Ls {
                    target: "example.com".to_string(),
                    filter: None,
                    offset: None,
                    limit: None,
                    all: false,
                    csv: false,
                },
            })
            .await
            .unwrap();

        let app = MockAppExecutor::with_checker(|command| match command {
            AppCommand::Reports { auth, .. } => {
                let auth = auth.expect("reports auth should be set");
                assert_eq!(auth.username(), "alice");
                assert_eq!(auth.password_str(), "secret");
            }
            _ => panic!("expected reports command"),
        });
        let runner =
            CliRunner::new_with_executor(app, Term::buffered_stdout(), password_auth, None);
        runner
            .execute(DcCmdCommand::Reports {
                cmd: crate::command::ReportsCommand::OperationTypes {
                    target: "example.com".to_string(),
                },
            })
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_execute_dispatches_config_to_app_command() {
        let app = MockAppExecutor::with_checker(|command| match command {
            AppCommand::Config { cmd } => match cmd {
                ConfigCommand::Auth { cmd } => match cmd {
                    ConfigAuthCommand::Ls { target } => assert_eq!(target, "example.com"),
                    _ => panic!("expected config auth ls"),
                },
                _ => panic!("expected config auth command"),
            },
            _ => panic!("expected AppCommand::Config"),
        });
        let runner = CliRunner::new_with_executor(app, Term::buffered_stdout(), None, None);

        runner
            .execute(DcCmdCommand::Config {
                cmd: ConfigCommand::Auth {
                    cmd: ConfigAuthCommand::Ls {
                        target: "example.com".to_string(),
                    },
                },
            })
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_execute_dispatches_transfer_to_app_command() {
        let app = MockAppExecutor::with_checker(|command| match command {
            AppCommand::Transfer {
                source,
                target,
                opts,
            } => {
                assert_eq!(source, "src.example.com/room/file.txt");
                assert_eq!(target, "dst.example.com/room/");
                assert!(opts.overwrite);
                assert!(opts.keep_share_links);
                assert!(opts.share);
                assert_eq!(opts.classification, Some(2));
                assert_eq!(opts.share_password.as_deref(), Some("pw"));
            }
            _ => panic!("expected AppCommand::Transfer"),
        });
        let runner = CliRunner::new_with_executor(app, Term::buffered_stdout(), None, None);

        runner
            .execute(DcCmdCommand::Transfer {
                source: "src.example.com/room/file.txt".to_string(),
                target: "dst.example.com/room/".to_string(),
                overwrite: true,
                keep_share_links: true,
                classification: Some(2),
                share: true,
                share_password: Some("pw".to_string()),
            })
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_execute_propagates_app_executor_error() {
        let app = MockAppExecutor::with_error();
        let runner = CliRunner::new_with_executor(app, Term::buffered_stdout(), None, None);

        let result = runner
            .execute(DcCmdCommand::Cp {
                source: "example.com/a.txt".to_string(),
                target: "example.com/b".to_string(),
            })
            .await;

        assert!(matches!(
            result,
            Err(DcCmdError::InvalidArgument(msg)) if msg == "app failed"
        ));
    }

    #[tokio::test]
    async fn test_execute_converts_error_outcome_to_command_failed() {
        let app = MockAppExecutor::with_error_outcome();
        let runner = CliRunner::new_with_executor(app, Term::buffered_stdout(), None, None);

        let result = runner
            .execute(DcCmdCommand::Cp {
                source: "example.com/a.txt".to_string(),
                target: "example.com/b".to_string(),
            })
            .await;

        assert!(matches!(result, Err(DcCmdError::CommandFailed)));
    }
}

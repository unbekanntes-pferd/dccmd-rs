use std::sync::Mutex;

use async_trait::async_trait;
use console::Term;
use secrecy::SecretString;

use super::{AppExecutor, CliRunner};
use crate::{
    app::{
        outcome::{CommandOutcome, OutcomeMessageKind},
        requests::{
            ConfigAuthRequest, ConfigRequest, GroupsRequest, GroupsUsersRequest, ReportsRequest,
            UsersRequest,
        },
        results::{AppResult, AppStatus},
    },
    command::{
        config::{ConfigAuthCommand, ConfigCommand},
        AppCommand, CreateContainerType, DcCmdCommand, GroupsCommand, GroupsUsersCommand,
        UsersCommand,
    },
    core::models::{DcCmdError, PasswordAuth},
};

type AppCommandChecker = Box<dyn FnOnce(AppCommand) + Send>;

struct MockAppExecutor {
    checker: Mutex<Option<AppCommandChecker>>,
    fail: bool,
    status: AppStatus,
}

impl MockAppExecutor {
    fn with_checker(checker: impl FnOnce(AppCommand) + Send + 'static) -> Self {
        Self {
            checker: Mutex::new(Some(Box::new(checker))),
            fail: false,
            status: AppStatus::Success,
        }
    }

    fn with_error() -> Self {
        Self {
            checker: Mutex::new(None),
            fail: true,
            status: AppStatus::Success,
        }
    }

    fn with_failure_result() -> Self {
        Self {
            checker: Mutex::new(None),
            fail: false,
            status: AppStatus::Failure,
        }
    }

    fn with_partial_result() -> Self {
        Self {
            checker: Mutex::new(None),
            fail: false,
            status: AppStatus::PartialFailure,
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
        if self.status == AppStatus::Failure {
            outcome.push(OutcomeMessageKind::Error, "app reported failure");
        }
        let result = match self.status {
            AppStatus::Success => AppResult::success(outcome),
            AppStatus::PartialFailure => AppResult::partial_failure(outcome),
            AppStatus::Failure => AppResult::failure(outcome),
        };
        Ok(result)
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
                UsersRequest::Ls { target, .. } => assert_eq!(target, "example.com"),
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
                GroupsRequest::Users { cmd } => match cmd {
                    GroupsUsersRequest::Add {
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
                ReportsRequest::OperationTypes { target } => {
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
    let runner = CliRunner::new_with_executor(app, Term::buffered_stdout(), password_auth, None);
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
            ConfigRequest::Auth { cmd } => match cmd {
                ConfigAuthRequest::Ls { target } => assert_eq!(target, "example.com"),
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
async fn test_execute_converts_failure_status_to_command_failed() {
    let app = MockAppExecutor::with_failure_result();
    let runner = CliRunner::new_with_executor(app, Term::buffered_stdout(), None, None);

    let result = runner
        .execute(DcCmdCommand::Cp {
            source: "example.com/a.txt".to_string(),
            target: "example.com/b".to_string(),
        })
        .await;

    assert!(matches!(result, Err(DcCmdError::CommandFailed)));
}

#[tokio::test]
async fn test_execute_converts_partial_status_to_command_failed() {
    let app = MockAppExecutor::with_partial_result();
    let runner = CliRunner::new_with_executor(app, Term::buffered_stdout(), None, None);

    let result = runner
        .execute(DcCmdCommand::Cp {
            source: "example.com/a.txt".to_string(),
            target: "example.com/b".to_string(),
        })
        .await;

    assert!(matches!(result, Err(DcCmdError::CommandFailed)));
}

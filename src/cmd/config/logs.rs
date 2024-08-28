use std::fs::OpenOptions;

use console::Term;
use tracing::{error, level_filters::LevelFilter};
use tracing_subscriber::EnvFilter;

use crate::cmd::{handle_error, models::DcCmdError};

pub fn init_logging(err_term: &Term, debug: bool, log_file: Option<String>) {
    let log_format = tracing_subscriber::fmt::format()
        .with_level(true)
        .with_thread_names(false)
        .with_target(true)
        .with_ansi(false)
        .compact();

    let env_filter = if debug {
        EnvFilter::from_default_env().add_directive(LevelFilter::DEBUG.into()).add_directive("hyper_utils=warn".parse().expect("invalid crate setup"))
    } else {
        EnvFilter::from_default_env().add_directive(LevelFilter::INFO.into())
    };

    // set up logging file
    let log_file_path = log_file.unwrap_or("dccmd-rs.log".to_string());

    let log_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_file_path)
        .map_err(|e| {
            error!("Failed to create or open log file: {}", e);
            DcCmdError::LogFileCreationFailed
        });

    if let Err(e) = &log_file {
        handle_error(err_term, e);
    }

    let log_file = log_file.unwrap();

    // initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .event_format(log_format)
        .with_writer(std::sync::Mutex::new(log_file))
        .init();
}

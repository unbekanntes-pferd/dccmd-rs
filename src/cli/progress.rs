use std::{
    io::{self, IsTerminal},
    sync::Arc,
    time::Duration,
};

use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};

use crate::app::nodes::progress::{ProgressReporter, ProgressTask};

pub struct IndicatifProgressReporter;

impl IndicatifProgressReporter {
    pub fn new() -> Self {
        Self
    }
}

struct IndicatifProgressTask {
    inner: ProgressBar,
}

impl ProgressTask for IndicatifProgressTask {
    fn set_length(&self, length: u64) {
        self.inner.set_length(length);
    }

    fn set_message(&self, message: &str) {
        self.inner.set_message(message.to_string());
    }

    fn inc(&self, delta: u64) {
        self.inner.inc(delta);
    }

    fn finish_with_message(&self, message: &str) {
        self.inner.finish_with_message(message.to_string());
    }

    fn finish_and_clear(&self) {
        self.inner.finish_and_clear();
    }

    fn enable_steady_tick(&self, interval: Duration) {
        self.inner.enable_steady_tick(interval);
    }
}

impl ProgressReporter for IndicatifProgressReporter {
    fn bar(&self, length: u64) -> Arc<dyn ProgressTask> {
        let progress_bar = ProgressBar::new(length);
        if !io::stderr().is_terminal() {
            progress_bar.set_draw_target(ProgressDrawTarget::hidden());
        }
        progress_bar.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}) {msg}")
                .expect("valid progress template")
                .progress_chars("=>-"),
        );

        Arc::new(IndicatifProgressTask {
            inner: progress_bar,
        })
    }

    fn spinner(&self, message: &str) -> Arc<dyn ProgressTask> {
        let progress_spinner = ProgressBar::new_spinner();
        if !io::stderr().is_terminal() {
            progress_spinner.set_draw_target(ProgressDrawTarget::hidden());
        }
        progress_spinner.set_message(message.to_string());
        Arc::new(IndicatifProgressTask {
            inner: progress_spinner,
        })
    }
}

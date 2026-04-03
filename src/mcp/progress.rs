use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use rmcp::{
    model::{Meta, ProgressNotificationParam},
    Peer, RoleServer,
};

use crate::app::nodes::progress::{NoopProgressReporter, ProgressReporter, ProgressTask};

const DEFAULT_NOTIFY_INTERVAL: Duration = Duration::from_millis(100);

#[derive(Clone)]
pub struct McpProgressReporter {
    inner: Option<Arc<ReporterState>>,
}

struct ReporterState {
    peer: Peer<RoleServer>,
    progress_token: rmcp::model::ProgressToken,
}

pub struct McpProgressTask {
    inner: Option<Arc<TaskState>>,
}

struct TaskState {
    peer: Peer<RoleServer>,
    progress_token: rmcp::model::ProgressToken,
    state: Mutex<TaskSnapshot>,
}

#[derive(Default)]
struct TaskSnapshot {
    progress: f64,
    total: Option<f64>,
    message: Option<String>,
    last_sent_at: Option<Instant>,
}

impl McpProgressReporter {
    pub fn from_meta(meta: &Meta, peer: Peer<RoleServer>) -> Arc<dyn ProgressReporter> {
        match meta.get_progress_token() {
            Some(progress_token) => Arc::new(Self {
                inner: Some(Arc::new(ReporterState {
                    peer,
                    progress_token,
                })),
            }),
            None => Arc::new(NoopProgressReporter),
        }
    }

    fn task(&self, total: Option<u64>, message: Option<&str>) -> Arc<dyn ProgressTask> {
        let Some(inner) = &self.inner else {
            return Arc::new(McpProgressTask { inner: None });
        };

        let task = Arc::new(McpProgressTask {
            inner: Some(Arc::new(TaskState {
                peer: inner.peer.clone(),
                progress_token: inner.progress_token.clone(),
                state: Mutex::new(TaskSnapshot {
                    progress: 0.0,
                    total: total.map(|value| value as f64),
                    message: message.map(ToOwned::to_owned),
                    last_sent_at: None,
                }),
            })),
        });

        if total.is_some() || message.is_some() {
            task.emit(true);
        }

        task
    }
}

impl ProgressReporter for McpProgressReporter {
    fn bar(&self, length: u64) -> Arc<dyn ProgressTask> {
        self.task(Some(length), None)
    }

    fn item_bar(&self, length: u64) -> Arc<dyn ProgressTask> {
        self.task(Some(length), None)
    }

    fn spinner(&self, message: &str) -> Arc<dyn ProgressTask> {
        self.task(None, Some(message))
    }
}

impl McpProgressTask {
    fn emit(&self, force: bool) {
        let Some(inner) = &self.inner else {
            return;
        };

        let mut snapshot = inner.state.lock().expect("progress lock poisoned");
        let now = Instant::now();
        if !force {
            if let Some(last_sent_at) = snapshot.last_sent_at {
                if now.duration_since(last_sent_at) < DEFAULT_NOTIFY_INTERVAL
                    && snapshot.total.is_none_or(|total| snapshot.progress < total)
                {
                    return;
                }
            }
        }

        snapshot.last_sent_at = Some(now);
        let progress = snapshot.progress;
        let total = snapshot.total;
        let message = snapshot.message.clone();
        drop(snapshot);

        let peer = inner.peer.clone();
        let progress_token = inner.progress_token.clone();
        tokio::spawn(async move {
            let mut params = ProgressNotificationParam::new(progress_token, progress);
            if let Some(total) = total {
                params = params.with_total(total);
            }
            if let Some(message) = message {
                params = params.with_message(message);
            }
            let _ = peer.notify_progress(params).await;
        });
    }
}

impl ProgressTask for McpProgressTask {
    fn set_length(&self, length: u64) {
        if let Some(inner) = &self.inner {
            inner.state.lock().expect("progress lock poisoned").total = Some(length as f64);
        }
        self.emit(false);
    }

    fn set_message(&self, message: &str) {
        if let Some(inner) = &self.inner {
            inner.state.lock().expect("progress lock poisoned").message = Some(message.to_string());
        }
        self.emit(false);
    }

    fn inc(&self, delta: u64) {
        if let Some(inner) = &self.inner {
            inner.state.lock().expect("progress lock poisoned").progress += delta as f64;
        }
        self.emit(false);
    }

    fn finish_with_message(&self, message: &str) {
        if let Some(inner) = &self.inner {
            let mut state = inner.state.lock().expect("progress lock poisoned");
            state.message = Some(message.to_string());
            if let Some(total) = state.total {
                state.progress = total;
            }
        }
        self.emit(true);
    }

    fn finish_and_clear(&self) {
        if let Some(inner) = &self.inner {
            let mut state = inner.state.lock().expect("progress lock poisoned");
            state.message = None;
            if let Some(total) = state.total {
                state.progress = total;
            }
        }
        self.emit(true);
    }

    fn enable_steady_tick(&self, _interval: Duration) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_token_progress_reporter_is_noop_compatible() {
        let reporter = McpProgressReporter { inner: None };
        let task = reporter.spinner("Listing");
        task.set_message("Still listing");
        task.inc(5);
        task.finish_with_message("Done");
    }
}

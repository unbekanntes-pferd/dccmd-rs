use std::{sync::Arc, time::Duration};

pub trait ProgressTask: Send + Sync {
    fn set_length(&self, length: u64);
    fn set_message(&self, message: &str);
    fn inc(&self, delta: u64);
    fn finish_with_message(&self, message: &str);
    fn finish_and_clear(&self);
    fn enable_steady_tick(&self, interval: Duration);
}

pub trait ProgressReporter: Send + Sync {
    fn bar(&self, length: u64) -> Arc<dyn ProgressTask>;
    fn item_bar(&self, length: u64) -> Arc<dyn ProgressTask> {
        self.bar(length)
    }
    fn spinner(&self, message: &str) -> Arc<dyn ProgressTask>;
}

const DEFAULT_SPINNER_TICK_INTERVAL: Duration = Duration::from_millis(100);

pub fn start_progress_bar(
    progress: &dyn ProgressReporter,
    length: u64,
    message: Option<&str>,
) -> Arc<dyn ProgressTask> {
    let bar = progress.bar(length);
    bar.set_length(length);

    if let Some(message) = message {
        bar.set_message(message);
    }

    bar
}

pub fn start_item_progress_bar(
    progress: &dyn ProgressReporter,
    length: u64,
    message: Option<&str>,
) -> Arc<dyn ProgressTask> {
    let bar = progress.item_bar(length);
    bar.set_length(length);

    if let Some(message) = message {
        bar.set_message(message);
    }

    bar
}

pub fn start_spinner(progress: &dyn ProgressReporter, message: &str) -> Arc<dyn ProgressTask> {
    let spinner = progress.spinner(message);
    spinner.enable_steady_tick(DEFAULT_SPINNER_TICK_INTERVAL);
    spinner
}

pub fn update_remaining_files_message(task: &dyn ProgressTask, action: &str, remaining: u64) {
    task.set_message(&format!("{action} {remaining} files"));
}

#[derive(Default)]
pub struct NoopProgressReporter;

#[derive(Default)]
struct NoopProgressTask;

impl ProgressTask for NoopProgressTask {
    fn set_length(&self, _length: u64) {}

    fn set_message(&self, _message: &str) {}

    fn inc(&self, _delta: u64) {}

    fn finish_with_message(&self, _message: &str) {}

    fn finish_and_clear(&self) {}

    fn enable_steady_tick(&self, _interval: Duration) {}
}

impl ProgressReporter for NoopProgressReporter {
    fn bar(&self, _length: u64) -> Arc<dyn ProgressTask> {
        Arc::new(NoopProgressTask)
    }

    fn spinner(&self, _message: &str) -> Arc<dyn ProgressTask> {
        Arc::new(NoopProgressTask)
    }
}

#[cfg(test)]
mod tests {
    use std::{
        sync::{Arc, Mutex},
        time::Duration,
    };

    use super::{
        start_item_progress_bar, start_progress_bar, start_spinner, update_remaining_files_message,
        ProgressReporter, ProgressTask,
    };

    #[derive(Default)]
    struct RecordingProgressReporter {
        events: Arc<Mutex<Vec<String>>>,
    }

    impl RecordingProgressReporter {
        fn events(&self) -> Vec<String> {
            self.events.lock().expect("lock poisoned").clone()
        }
    }

    struct RecordingProgressTask {
        events: Arc<Mutex<Vec<String>>>,
    }

    impl ProgressTask for RecordingProgressTask {
        fn set_length(&self, length: u64) {
            self.events
                .lock()
                .expect("lock poisoned")
                .push(format!("set_length:{length}"));
        }

        fn set_message(&self, message: &str) {
            self.events
                .lock()
                .expect("lock poisoned")
                .push(format!("set_message:{message}"));
        }

        fn inc(&self, _delta: u64) {}

        fn finish_with_message(&self, _message: &str) {}

        fn finish_and_clear(&self) {}

        fn enable_steady_tick(&self, interval: Duration) {
            self.events
                .lock()
                .expect("lock poisoned")
                .push(format!("enable_tick:{}ms", interval.as_millis()));
        }
    }

    impl ProgressReporter for RecordingProgressReporter {
        fn bar(&self, length: u64) -> Arc<dyn ProgressTask> {
            self.events
                .lock()
                .expect("lock poisoned")
                .push(format!("bar:{length}"));
            Arc::new(RecordingProgressTask {
                events: self.events.clone(),
            })
        }

        fn item_bar(&self, length: u64) -> Arc<dyn ProgressTask> {
            self.events
                .lock()
                .expect("lock poisoned")
                .push(format!("item_bar:{length}"));
            Arc::new(RecordingProgressTask {
                events: self.events.clone(),
            })
        }

        fn spinner(&self, message: &str) -> Arc<dyn ProgressTask> {
            self.events
                .lock()
                .expect("lock poisoned")
                .push(format!("spinner:{message}"));
            Arc::new(RecordingProgressTask {
                events: self.events.clone(),
            })
        }
    }

    #[test]
    fn start_progress_bar_applies_common_setup() {
        let reporter = RecordingProgressReporter::default();

        let _ = start_progress_bar(&reporter, 123, Some("Uploading"));

        assert_eq!(
            reporter.events(),
            vec![
                "bar:123".to_string(),
                "set_length:123".to_string(),
                "set_message:Uploading".to_string(),
            ]
        );
    }

    #[test]
    fn start_spinner_enables_default_tick() {
        let reporter = RecordingProgressReporter::default();

        let _ = start_spinner(&reporter, "Listing files and folders...");

        assert_eq!(
            reporter.events(),
            vec![
                "spinner:Listing files and folders...".to_string(),
                "enable_tick:100ms".to_string(),
            ]
        );
    }

    #[test]
    fn start_item_progress_bar_applies_common_setup() {
        let reporter = RecordingProgressReporter::default();

        let _ = start_item_progress_bar(&reporter, 9, Some("Creating folders"));

        assert_eq!(
            reporter.events(),
            vec![
                "item_bar:9".to_string(),
                "set_length:9".to_string(),
                "set_message:Creating folders".to_string(),
            ]
        );
    }

    #[test]
    fn update_remaining_files_message_formats_consistently() {
        let reporter = RecordingProgressReporter::default();
        let bar = start_progress_bar(&reporter, 42, Some("Downloading 2 files"));

        update_remaining_files_message(bar.as_ref(), "Downloading", 1);

        let events = reporter.events();
        assert!(events.contains(&"set_message:Downloading 1 files".to_string()));
    }
}

use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

pub struct Logger {
    multi_progress: MultiProgress,
    active_spinner: Arc<Mutex<Option<ProgressBar>>>,
    // we need to keep the guard alive
    _guard: WorkerGuard,
}

impl Logger {
    pub fn init() -> Self {
        // 1. Configure File Appender (Warn/Error only)
        let file_appender = tracing_appender::rolling::never(".", "log.txt");
        let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

        // Filter for file logging: WARN or ERROR only
        let file_filter = EnvFilter::new("warn");

        // Use a custom format that doesn't use colors for the file
        let file_layer = fmt::layer()
            .with_ansi(false)
            .with_writer(non_blocking)
            .with_filter(file_filter);

        // 2. Configure Stdout Layer (Info and above, Docker styling)
        // We use a separate filter for stdout to allow info logs to stream
        let stdout_filter = EnvFilter::new("info");
        let stdout_layer = fmt::layer()
            .with_writer(std::io::stdout)
            .with_filter(stdout_filter);

        // Register the subscriber with both layers
        tracing_subscriber::registry()
            .with(file_layer)
            .with(stdout_layer)
            .init();

        Self {
            multi_progress: MultiProgress::new(),
            active_spinner: Arc::new(Mutex::new(None)),
            _guard: guard,
        }
    }

    /// Starts a spinner that updates in-place.
    /// If a spinner is already running, it updates the message.
    pub fn start_spinner(&self, msg: &str) {
        let mut active = self.active_spinner.lock().unwrap();

        if let Some(pb) = active.as_ref() {
            pb.set_message(msg.to_string());
        } else {
            let pb = self.multi_progress.add(ProgressBar::new_spinner());
            pb.set_style(
                ProgressStyle::default_spinner()
                    .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏ ")
                    .template("{spinner:.green} {msg}")
                    .unwrap(),
            );
            pb.set_message(msg.to_string());
            pb.enable_steady_tick(Duration::from_millis(100));
            *active = Some(pb);
        }
    }

    /// Completes the current spinner with a success message.
    pub fn success(&self, msg: &str) {
        let mut active = self.active_spinner.lock().unwrap();
        if let Some(pb) = active.take() {
            pb.set_style(
                ProgressStyle::default_spinner()
                    .template("{msg}") // Just the message, no spinner
                    .unwrap(),
            );
            // Prefix with checkmark manually
            pb.finish_with_message(format!("{} {}", console::style("✔").green(), msg));
        } else {
            let _ = self
                .multi_progress
                .println(format!("{} {}", console::style("✔").green(), msg));
        }
    }

    // Helper to print normal lines without breaking the spinner
    pub fn info(&self, msg: &str) {
        let _ = self.multi_progress.println(msg);
    }
}

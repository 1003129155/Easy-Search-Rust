// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Shared process-wide logging for every EasySearch crate.

use std::fmt::Write as _;
use std::fs::{File, OpenOptions};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Mutex, OnceLock};

use chrono::{Local, NaiveDate};
use tracing::field::{Field, Visit};
use tracing::span::{Attributes, Id};
use tracing::{Event, Subscriber};
use tracing_subscriber::layer::Context;
use tracing_subscriber::prelude::*;
use tracing_subscriber::registry::LookupSpan;

const LOG_RETENTION_DAYS: i64 = 3;
const LOG_FILE_PREFIX: &str = "easysearch-";
const LOG_FILE_SUFFIX: &str = ".log";

static LOG_STATE: OnceLock<Mutex<LogState>> = OnceLock::new();
static LOG_LEVEL: AtomicU8 = AtomicU8::new(LogLevel::Warn as u8);
static TRACING_INITIALIZED: OnceLock<()> = OnceLock::new();

/// Supported application log levels, ordered from most to least verbose.
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum LogLevel {
    Debug = 0,
    Info = 1,
    #[default]
    Warn = 2,
    Error = 3,
}

impl LogLevel {
    /// Parse a case-insensitive level name. Invalid values use `WARN`.
    #[must_use]
    pub fn parse(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "debug" => Self::Debug,
            "info" => Self::Info,
            "error" => Self::Error,
            _ => Self::Warn,
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Debug => "DEBUG",
            Self::Info => "INFO",
            Self::Warn => "WARN",
            Self::Error => "ERROR",
        }
    }
}

struct LogState {
    directory: PathBuf,
    date: NaiveDate,
    file: File,
}

/// Initialize the shared daily log and the process-wide `tracing` subscriber.
pub fn init() {
    let directory = crate::paths::app_root_dir();
    let _ = std::fs::create_dir_all(&directory);
    remove_legacy_logs(&directory);
    let today = Local::now().date_naive();
    cleanup_old_logs(&directory, today);

    if LOG_STATE.get().is_none()
        && let Some(state) = open_log_state(directory, today)
    {
        let _ = LOG_STATE.set(Mutex::new(state));
    }

    TRACING_INITIALIZED.get_or_init(|| {
        let _ = tracing_subscriber::registry()
            .with(EasySearchLayer)
            .try_init();
    });
}

/// Change the process-wide minimum level. This takes effect immediately.
pub fn set_level(level: LogLevel) {
    LOG_LEVEL.store(level as u8, Ordering::Release);
}

/// Change the process-wide minimum level from a settings string.
pub fn set_level_from_str(level: &str) {
    set_level(LogLevel::parse(level));
}

/// Return whether a message at `level` would currently be written.
#[must_use]
pub fn enabled(level: LogLevel) -> bool {
    level as u8 >= LOG_LEVEL.load(Ordering::Acquire)
}

/// Write a message through the shared logger.
pub fn write(level: LogLevel, target: &str, message: &str) {
    if !enabled(level) {
        return;
    }

    let now = Local::now();
    let date = now.date_naive();
    if let Some(lock) = LOG_STATE.get()
        && let Ok(mut state) = lock.lock()
    {
        if state.date != date {
            let Some(next) = open_log_state(state.directory.clone(), date) else {
                return;
            };
            *state = next;
            cleanup_old_logs(&state.directory, date);
        }
        let _ = writeln!(
            state.file,
            "[{}] [{}] [{}] {}",
            now.format("%Y-%m-%d %H:%M:%S%.3f"),
            level.as_str(),
            target,
            message
        );
        let _ = state.file.flush();
    }

    if cfg!(debug_assertions) {
        eprintln!("[{}] [{}] {message}", level.as_str(), target);
    }
}

fn open_log_state(directory: PathBuf, date: NaiveDate) -> Option<LogState> {
    let path = directory.join(format!(
        "{LOG_FILE_PREFIX}{}{LOG_FILE_SUFFIX}",
        date.format("%Y-%m-%d")
    ));
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .ok()?;
    Some(LogState {
        directory,
        date,
        file,
    })
}

fn remove_legacy_logs(directory: &Path) {
    for name in ["easysearch.log", "easysearch.log.old"] {
        let _ = std::fs::remove_file(directory.join(name));
    }
}

fn cleanup_old_logs(directory: &Path, today: NaiveDate) {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let Some(date) = name
            .strip_prefix(LOG_FILE_PREFIX)
            .and_then(|name| name.strip_suffix(LOG_FILE_SUFFIX))
            .and_then(|date| NaiveDate::parse_from_str(date, "%Y-%m-%d").ok())
        else {
            continue;
        };
        if today.signed_duration_since(date).num_days() >= LOG_RETENTION_DAYS {
            let _ = std::fs::remove_file(path);
        }
    }
}

struct EasySearchLayer;

impl<S> tracing_subscriber::Layer<S> for EasySearchLayer
where
    S: Subscriber + for<'lookup> LookupSpan<'lookup>,
{
    fn on_new_span(&self, attributes: &Attributes<'_>, id: &Id, context: Context<'_, S>) {
        let Some(span) = context.span(id) else {
            return;
        };
        let mut visitor = MessageVisitor::default();
        attributes.record(&mut visitor);
        span.extensions_mut().insert(SpanFields(visitor.finish()));
    }

    fn on_event(&self, event: &Event<'_>, context: Context<'_, S>) {
        let level = match *event.metadata().level() {
            tracing::Level::ERROR => LogLevel::Error,
            tracing::Level::WARN => LogLevel::Warn,
            tracing::Level::INFO => LogLevel::Info,
            tracing::Level::DEBUG | tracing::Level::TRACE => LogLevel::Debug,
        };
        if !enabled(level) {
            return;
        }
        let mut visitor = MessageVisitor::default();
        event.record(&mut visitor);
        let mut message = visitor.finish();
        if let Some(scope) = context.event_scope(event) {
            for span in scope.from_root() {
                if let Some(fields) = span.extensions().get::<SpanFields>()
                    && !fields.0.is_empty()
                {
                    message.push_str(" span.");
                    message.push_str(span.name());
                    message.push('{');
                    message.push_str(&fields.0);
                    message.push('}');
                }
            }
        }
        write(level, event.metadata().target(), &message);
    }
}

struct SpanFields(String);

#[derive(Default)]
struct MessageVisitor {
    message: Option<String>,
    fields: String,
}

impl MessageVisitor {
    fn push(&mut self, field: &Field, value: impl std::fmt::Display) {
        if field.name() == "message" {
            self.message = Some(value.to_string());
        } else {
            if !self.fields.is_empty() {
                self.fields.push(' ');
            }
            let _ = write!(self.fields, "{}={value}", field.name());
        }
    }

    fn finish(self) -> String {
        match (self.message, self.fields.is_empty()) {
            (Some(message), false) => format!("{message} {}", self.fields),
            (Some(message), true) => message,
            (None, _) => self.fields,
        }
    }
}

impl Visit for MessageVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        self.push(field, format_args!("{value:?}"));
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        self.push(field, value);
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        self.push(field, value);
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.push(field, value);
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        self.push(field, value);
    }
}

#[macro_export]
macro_rules! log_debug {
    ($($arg:tt)*) => {{
        if $crate::logging::enabled($crate::logging::LogLevel::Debug) {
            $crate::logging::write($crate::logging::LogLevel::Debug, module_path!(), &format!($($arg)*));
        }
    }};
}

#[macro_export]
macro_rules! log_info {
    ($($arg:tt)*) => {{
        if $crate::logging::enabled($crate::logging::LogLevel::Info) {
            $crate::logging::write($crate::logging::LogLevel::Info, module_path!(), &format!($($arg)*));
        }
    }};
}

#[macro_export]
macro_rules! log_warn {
    ($($arg:tt)*) => {{
        if $crate::logging::enabled($crate::logging::LogLevel::Warn) {
            $crate::logging::write($crate::logging::LogLevel::Warn, module_path!(), &format!($($arg)*));
        }
    }};
}

#[macro_export]
macro_rules! log_error {
    ($($arg:tt)*) => {{
        if $crate::logging::enabled($crate::logging::LogLevel::Error) {
            $crate::logging::write($crate::logging::LogLevel::Error, module_path!(), &format!($($arg)*));
        }
    }};
}

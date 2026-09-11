//! Host-owned, bounded diagnostics for plugin lifecycle support.
//!
//! The diagnostic model deliberately stores metadata rather than request
//! payloads. This keeps support information useful without turning the
//! diagnostics path into a second persistence channel for user content.

use std::collections::VecDeque;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

pub const DEFAULT_DIAGNOSTIC_CAPACITY: usize = 256;
pub const MAX_DIAGNOSTIC_EXPORT_EVENTS: usize = DEFAULT_DIAGNOSTIC_CAPACITY;
const MAX_TEXT_CHARS: usize = 128;
const MAX_ERROR_CLASS_CHARS: usize = 64;

/// User-selected reduction applied before previewing or exporting a bundle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DiagnosticExportOptions {
    pub max_events: usize,
    pub failures_only: bool,
}

impl Default for DiagnosticExportOptions {
    fn default() -> Self {
        Self {
            max_events: MAX_DIAGNOSTIC_EXPORT_EVENTS,
            failures_only: false,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiagnosticPhase {
    Load,
    Open,
    Update,
    Run,
    Close,
    Restart,
    Reclaimed,
}

impl DiagnosticPhase {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Load => "load",
            Self::Open => "open",
            Self::Update => "update",
            Self::Run => "run",
            Self::Close => "close",
            Self::Restart => "restart",
            Self::Reclaimed => "reclaimed",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiagnosticStatus {
    Success,
    Failure,
}

impl DiagnosticStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Failure => "failure",
        }
    }
}

/// One sanitized lifecycle event. All fields are bounded and optional fields
/// describe package metadata, never plugin input or filesystem contents.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiagnosticEvent {
    recorded_at_ms: u64,
    request_id: String,
    phase: DiagnosticPhase,
    status: DiagnosticStatus,
    plugin_id: Option<String>,
    host_pid: Option<u32>,
    duration_ms: Option<u64>,
    error_class: Option<String>,
    capability: Option<String>,
    scope: Option<String>,
    pointer_state: Option<String>,
    version: Option<String>,
    package_hash: Option<String>,
    signed: Option<bool>,
    pending_delete: bool,
}

impl DiagnosticEvent {
    #[must_use]
    pub fn success(request_id: impl AsRef<str>, phase: DiagnosticPhase) -> Self {
        Self::new(request_id, phase, DiagnosticStatus::Success)
    }

    #[must_use]
    pub fn failure(
        request_id: impl AsRef<str>,
        phase: DiagnosticPhase,
        error: impl AsRef<str>,
    ) -> Self {
        let mut event = Self::new(request_id, phase, DiagnosticStatus::Failure);
        event.error_class = Some(sanitize_error_class(error.as_ref()));
        event
    }

    fn new(request_id: impl AsRef<str>, phase: DiagnosticPhase, status: DiagnosticStatus) -> Self {
        Self {
            recorded_at_ms: now_millis(),
            request_id: sanitize_text(request_id.as_ref(), MAX_TEXT_CHARS),
            phase,
            status,
            plugin_id: None,
            host_pid: None,
            duration_ms: None,
            error_class: None,
            capability: None,
            scope: None,
            pointer_state: None,
            version: None,
            package_hash: None,
            signed: None,
            pending_delete: false,
        }
    }

    #[must_use]
    pub fn with_plugin(mut self, plugin_id: impl AsRef<str>) -> Self {
        self.plugin_id = Some(sanitize_text(plugin_id.as_ref(), MAX_TEXT_CHARS));
        self
    }

    #[must_use]
    pub const fn with_host_pid(mut self, host_pid: u32) -> Self {
        self.host_pid = Some(host_pid);
        self
    }

    #[must_use]
    pub const fn with_duration_ms(mut self, duration_ms: u64) -> Self {
        self.duration_ms = Some(duration_ms);
        self
    }

    #[must_use]
    pub fn with_error_class(mut self, error: impl AsRef<str>) -> Self {
        self.error_class = Some(sanitize_error_class(error.as_ref()));
        self
    }

    #[must_use]
    pub fn with_capability(mut self, capability: impl AsRef<str>) -> Self {
        self.capability = Some(sanitize_text(capability.as_ref(), MAX_TEXT_CHARS));
        self
    }

    #[must_use]
    pub fn with_scope(mut self, scope: impl AsRef<str>) -> Self {
        self.scope = Some(sanitize_text(scope.as_ref(), MAX_TEXT_CHARS));
        self
    }

    #[must_use]
    pub fn with_pointer_state(mut self, state: impl AsRef<str>) -> Self {
        let state = match state.as_ref() {
            "active" | "previous" | "pending-delete" => state.as_ref(),
            _ => "unknown",
        };
        self.pointer_state = Some(state.to_owned());
        self.pending_delete = state == "pending-delete";
        self
    }

    #[must_use]
    pub fn with_version(mut self, version: impl AsRef<str>) -> Self {
        self.version = Some(sanitize_text(version.as_ref(), MAX_TEXT_CHARS));
        self
    }

    #[must_use]
    pub fn with_package_hash(mut self, package_hash: impl AsRef<str>) -> Self {
        let value = sanitize_text(package_hash.as_ref(), MAX_TEXT_CHARS);
        self.package_hash = value.starts_with("sha256:").then_some(value);
        self
    }

    #[must_use]
    pub const fn with_signed(mut self, signed: bool) -> Self {
        self.signed = Some(signed);
        self
    }

    #[must_use]
    pub fn request_id(&self) -> &str {
        &self.request_id
    }

    #[must_use]
    pub const fn status(&self) -> DiagnosticStatus {
        self.status
    }

    #[must_use]
    pub fn error_class(&self) -> Option<&str> {
        self.error_class.as_deref()
    }

    #[must_use]
    pub fn plugin_id(&self) -> Option<&str> {
        self.plugin_id.as_deref()
    }

    fn as_json(&self) -> Value {
        json!({
            "recorded_at_ms": self.recorded_at_ms,
            "request_id": self.request_id,
            "phase": self.phase.as_str(),
            "status": self.status.as_str(),
            "plugin_id": self.plugin_id,
            "host_pid": self.host_pid,
            "duration_ms": self.duration_ms,
            "error_class": self.error_class,
            "capability": self.capability,
            "scope": self.scope,
            "pointer_state": self.pointer_state,
            "version": self.version,
            "package_hash": self.package_hash,
            "signed": self.signed,
            "pending_delete": self.pending_delete,
        })
    }
}

/// Fixed-capacity event storage owned by the main `NovaHub` process.
#[derive(Clone, Debug)]
pub struct DiagnosticEventBuffer {
    capacity: usize,
    events: VecDeque<DiagnosticEvent>,
}

impl Default for DiagnosticEventBuffer {
    fn default() -> Self {
        Self::with_capacity(DEFAULT_DIAGNOSTIC_CAPACITY)
    }
}

impl DiagnosticEventBuffer {
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            events: VecDeque::with_capacity(capacity.max(1)),
        }
    }

    pub fn push(&mut self, event: DiagnosticEvent) {
        if self.events.len() >= self.capacity {
            let _ = self.events.pop_front();
        }
        self.events.push_back(event);
    }

    #[must_use]
    pub fn snapshot(&self) -> Vec<DiagnosticEvent> {
        self.events.iter().cloned().collect()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.events.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// Produces the user-previewable local bundle. It contains only the
    /// already-sanitized event metadata and never serializes request payloads.
    ///
    /// # Errors
    ///
    /// Returns an error if JSON serialization of the bounded preview fails.
    pub fn preview_json(&self) -> Result<String, serde_json::Error> {
        self.preview_json_with_options(DiagnosticExportOptions::default())
    }

    /// Produces a reduced preview while preserving chronological order.
    ///
    /// # Errors
    ///
    /// Returns an error if JSON serialization fails.
    pub fn preview_json_with_options(
        &self,
        options: DiagnosticExportOptions,
    ) -> Result<String, serde_json::Error> {
        let max_events = options.max_events.clamp(1, MAX_DIAGNOSTIC_EXPORT_EVENTS);
        let mut events = self
            .events
            .iter()
            .rev()
            .filter(|event| !options.failures_only || event.status == DiagnosticStatus::Failure)
            .take(max_events)
            .map(DiagnosticEvent::as_json)
            .collect::<Vec<_>>();
        events.reverse();
        serde_json::to_string_pretty(&events)
    }

    #[must_use]
    pub fn summary(&self) -> String {
        if self.events.is_empty() {
            return "No plugin diagnostics".to_owned();
        }
        let failures = self
            .events
            .iter()
            .filter(|event| event.status == DiagnosticStatus::Failure)
            .count();
        format!(
            "Plugin diagnostics: {} events, {failures} failures",
            self.events.len()
        )
    }
}

fn sanitize_text(value: &str, max_chars: usize) -> String {
    value
        .chars()
        .filter(|character| !character.is_control())
        .collect::<String>()
        .trim()
        .chars()
        .take(max_chars)
        .collect()
}

fn sanitize_error_class(value: &str) -> String {
    let class = value.split(':').next().unwrap_or(value);
    sanitize_text(class, MAX_ERROR_CLASS_CHARS)
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| {
            u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
        })
}

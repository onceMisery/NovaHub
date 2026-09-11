#![forbid(unsafe_code)]

use std::io::{self, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::{
    Arc,
    mpsc::{self, Receiver, RecvTimeoutError},
};
use std::time::{Duration, Instant};

use novahub_ipc::{Envelope, IPC_PROTOCOL_MAJOR, MAX_ENVELOPE_BYTES, decode, encode};
use novahub_plugin_manager::{AuthorizedCapability, EffectiveGrant};
use serde_json::{Value, json};

/// Errors returned by the host-side Plugin Host process client.
#[derive(Debug)]
pub enum PluginHostError {
    Io(io::Error),
    Ipc(String),
    Json(serde_json::Error),
    Host(String),
}

impl std::fmt::Display for PluginHostError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(f, "plugin host I/O failed: {error}"),
            Self::Ipc(error) => write!(f, "plugin host IPC failed: {error}"),
            Self::Json(error) => write!(f, "plugin host payload failed: {error}"),
            Self::Host(error) => write!(f, "plugin host returned an error: {error}"),
        }
    }
}

impl std::error::Error for PluginHostError {}

impl From<io::Error> for PluginHostError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<serde_json::Error> for PluginHostError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}

/// Result of one bounded Component session driven by the host application.
#[derive(Debug)]
pub struct PluginFixtureRun {
    pub host_pid: u32,
    pub loaded: Value,
    pub opened: Value,
    pub updated: Value,
    pub closed: Value,
}

/// Maximum number of interactive rows copied into the native Shell.
pub const PLUGIN_VIEW_SLOT_COUNT: usize = 6;
/// Maximum number of list/grid items retained in the host snapshot. The Shell
/// binds only `PLUGIN_VIEW_SLOT_COUNT` rows at a time.
pub const MAX_PLUGIN_VIEW_ITEMS: usize = 100;

/// A capability request emitted by the sibling Plugin Host while it is
/// servicing the current application request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PluginCapabilityRequest {
    pub request_id: String,
    pub plugin_id: String,
    pub session_id: String,
    pub capability: AuthorizedCapability,
    pub payload: Option<String>,
}

/// Bounded result returned to the waiting Plugin Host capability import.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PluginCapabilityResponse {
    Unit,
    Text(String),
    Bytes(Vec<u8>),
    Error(String),
}

/// Main-process owner of platform capability execution.
pub trait PluginCapabilityHandler: Send + Sync {
    fn handle(&self, request: PluginCapabilityRequest) -> PluginCapabilityResponse;

    /// Releases capability state scoped to one completed or failed session.
    fn end_session(&self, _plugin_id: &str, _session_id: &str) {}
}

#[derive(Default)]
struct DenyCapabilityHandler;

impl PluginCapabilityHandler for DenyCapabilityHandler {
    fn handle(&self, _request: PluginCapabilityRequest) -> PluginCapabilityResponse {
        PluginCapabilityResponse::Error("capability_not_configured".into())
    }
}

/// A bounded list/grid item owned by the host renderer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PluginViewItem {
    pub id: String,
    pub title: String,
    pub subtitle: String,
    pub accessory: Option<PluginViewAccessory>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PluginViewAccessory {
    pub status: String,
    pub badge: String,
    pub shortcut: String,
    pub icon_id: Option<String>,
}

/// A bounded form field owned by the host renderer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PluginViewField {
    pub id: String,
    pub label: String,
    pub control: String,
    pub required: bool,
    pub initial_value: String,
    pub helper: String,
    pub error: String,
    pub options: Vec<PluginViewOption>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PluginViewOption {
    pub id: String,
    pub label: String,
}

/// One typed Detail metadata entry after host validation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PluginViewMetadata {
    pub key: String,
    pub kind: String,
    pub value: String,
}

/// A bounded action-panel item owned by the host renderer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PluginViewAction {
    pub id: String,
    pub title: String,
    pub is_default: bool,
    pub destructive: bool,
}

/// Renderer-neutral, bounded representation of the latest official View.
/// The Shell can present this surface without understanding plugin-specific
/// JSON or allowing unbounded Markdown to enter the native window.
#[derive(Clone, Debug, PartialEq)]
pub struct PluginViewSurface {
    pub kind: String,
    pub title: String,
    pub body: String,
    pub progress: Option<f32>,
    pub items: Vec<PluginViewItem>,
    pub next_cursor: Option<String>,
    pub metadata: Vec<PluginViewMetadata>,
    pub fields: Vec<PluginViewField>,
    pub actions: Vec<PluginViewAction>,
}

impl PluginFixtureRun {
    #[must_use]
    pub fn updated_revision(&self) -> Option<u64> {
        self.updated.get("revision").and_then(Value::as_u64)
    }

    /// Produces a bounded, renderer-neutral summary of the host-owned View.
    /// The full payload remains available to a future Shell view model, while
    /// the MVP command path avoids dumping arbitrary plugin text into logs.
    #[must_use]
    pub fn updated_view_summary(&self) -> String {
        summarize_view(&self.updated)
    }

    /// Converts the latest View into a bounded host-owned display surface.
    #[must_use]
    pub fn updated_view_surface(&self) -> PluginViewSurface {
        surface_from_view(&self.updated)
    }
}

fn summarize_view(response: &Value) -> String {
    let Some(view) = response.get("view") else {
        return "Plugin returned no view".to_owned();
    };
    let title = view
        .get("title")
        .and_then(Value::as_str)
        .unwrap_or("Plugin view");
    let kind = view
        .get("kind")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    match kind {
        "empty" => view.get("description").and_then(Value::as_str).map_or_else(
            || title.to_owned(),
            |description| format!("{title}: {description}"),
        ),
        "loading" => format!(
            "{title}: {}",
            view.get("message")
                .and_then(Value::as_str)
                .unwrap_or("Loading")
        ),
        "progress" => format!(
            "{title}: {}/{}",
            view.get("completed").and_then(Value::as_u64).unwrap_or(0),
            view.get("total").and_then(Value::as_u64).unwrap_or(0)
        ),
        "error" => format!(
            "{title}: {}",
            view.get("message")
                .and_then(Value::as_str)
                .unwrap_or("Plugin error")
        ),
        "list" | "grid" => format!(
            "{title} ({})",
            view.get("items")
                .and_then(Value::as_array)
                .map_or(0, Vec::len)
        ),
        "detail" => {
            let markdown = view
                .get("markdown")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .chars()
                .take(160)
                .collect::<String>();
            if markdown.is_empty() {
                title.to_owned()
            } else {
                format!("{title}: {markdown}")
            }
        }
        "form" => format!(
            "{title} ({} fields)",
            view.get("fields")
                .and_then(Value::as_array)
                .map_or(0, Vec::len)
        ),
        "action_panel" => format!(
            "{title} ({} actions)",
            view.get("actions")
                .and_then(Value::as_array)
                .map_or(0, Vec::len)
        ),
        _ => title.to_owned(),
    }
}

const MAX_SURFACE_TEXT: usize = 4 * 1024;

fn surface_from_view(response: &Value) -> PluginViewSurface {
    let Some(view) = response.get("view") else {
        return PluginViewSurface {
            kind: "empty".into(),
            title: "Plugin view".into(),
            body: "Plugin returned no view".into(),
            progress: None,
            items: Vec::new(),
            next_cursor: None,
            metadata: Vec::new(),
            fields: Vec::new(),
            actions: Vec::new(),
        };
    };
    let kind = view
        .get("kind")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let title = bounded_text(
        view.get("title")
            .and_then(Value::as_str)
            .unwrap_or("Plugin view"),
        256,
    );
    let metadata = view_metadata(view.get("metadata"), kind);
    let actions = view_actions(view.get("actions"), kind);
    let body = match kind {
        "empty" => bounded_text(
            view.get("description")
                .and_then(Value::as_str)
                .unwrap_or("No content"),
            MAX_SURFACE_TEXT,
        ),
        "loading" => bounded_text(
            view.get("message")
                .and_then(Value::as_str)
                .unwrap_or("Loading"),
            MAX_SURFACE_TEXT,
        ),
        "progress" => format!(
            "Progress: {}/{}",
            view.get("completed").and_then(Value::as_u64).unwrap_or(0),
            view.get("total").and_then(Value::as_u64).unwrap_or(0)
        ),
        "error" => bounded_text(
            view.get("message")
                .and_then(Value::as_str)
                .unwrap_or("Plugin error"),
            MAX_SURFACE_TEXT,
        ),
        "list" | "grid" => list_surface_body(view.get("items")),
        "detail" => detail_surface_body(view, &metadata),
        "form" => fields_surface_body(view.get("fields")),
        "action_panel" => actions_surface_body(&actions),
        _ => "Unsupported plugin view".into(),
    };
    PluginViewSurface {
        kind: bounded_text(kind, 32),
        title,
        body,
        progress: (kind == "progress").then(|| {
            let completed = view.get("completed").and_then(Value::as_u64).unwrap_or(0);
            let total = view.get("total").and_then(Value::as_u64).unwrap_or(1);
            progress_fraction(completed, total)
        }),
        items: view_items(view.get("items"), kind),
        next_cursor: view_next_cursor(view, kind),
        metadata,
        fields: view_fields(view.get("fields"), kind),
        actions,
    }
}

fn progress_fraction(completed: u64, total: u64) -> f32 {
    if total == 0 {
        return 0.0;
    }

    // Slint renders progress as f32. Absolute counter precision is irrelevant
    // after the bounded ratio has been computed for display.
    #[allow(clippy::cast_precision_loss)]
    let ratio = completed.min(total) as f32 / total as f32;
    ratio
}

fn list_surface_body(items: Option<&Value>) -> String {
    items
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .take(MAX_PLUGIN_VIEW_ITEMS)
                .filter_map(parse_view_item)
                .map(format_view_item)
                .collect::<Vec<_>>()
                .join("\n")
        })
        .filter(|body| !body.is_empty())
        .unwrap_or_else(|| "No items".into())
}

fn format_view_item(item: PluginViewItem) -> String {
    let mut line = format!("{} - {}", item.title, item.subtitle);
    if let Some(accessory) = item.accessory {
        if !accessory.status.is_empty() {
            line.push_str(" · ");
            line.push_str(&accessory.status);
        }
        if !accessory.badge.is_empty() {
            line.push_str(" [");
            line.push_str(&accessory.badge);
            line.push(']');
        }
        if !accessory.shortcut.is_empty() {
            line.push_str(" (");
            line.push_str(&accessory.shortcut);
            line.push(')');
        }
    }
    bounded_text(&line, 512)
}

fn parse_view_item(item: &Value) -> Option<PluginViewItem> {
    Some(PluginViewItem {
        id: bounded_text(item.get("id")?.as_str()?, 128),
        title: bounded_text(item.get("title")?.as_str()?, 160),
        subtitle: bounded_text(
            item.get("subtitle")
                .and_then(Value::as_str)
                .unwrap_or_default(),
            240,
        ),
        accessory: parse_view_accessory(item.get("accessory")),
    })
}

fn parse_view_accessory(value: Option<&Value>) -> Option<PluginViewAccessory> {
    let value = value?.as_object()?;
    let status = bounded_text(
        value
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or_default(),
        96,
    );
    let badge = bounded_text(
        value
            .get("badge")
            .and_then(Value::as_str)
            .unwrap_or_default(),
        64,
    );
    let shortcut = bounded_text(
        value
            .get("shortcut")
            .and_then(Value::as_str)
            .unwrap_or_default(),
        32,
    );
    let icon_id = value
        .get("icon")
        .and_then(Value::as_object)
        .and_then(|icon| icon.get("id"))
        .and_then(Value::as_str)
        .map(|id| bounded_text(id, 128))
        .filter(|id| !id.trim().is_empty() && !id.chars().any(char::is_control));
    if status.chars().any(char::is_control)
        || badge.chars().any(char::is_control)
        || shortcut.chars().any(char::is_control)
    {
        return None;
    }
    if status.trim().is_empty()
        && badge.trim().is_empty()
        && shortcut.trim().is_empty()
        && icon_id.is_none()
    {
        return None;
    }
    Some(PluginViewAccessory {
        status,
        badge,
        shortcut,
        icon_id,
    })
}

fn detail_surface_body(view: &Value, metadata: &[PluginViewMetadata]) -> String {
    let mut lines = Vec::new();
    if let Some(markdown) = view.get("markdown").and_then(Value::as_str) {
        lines.push(bounded_text(markdown, MAX_SURFACE_TEXT));
    }
    lines.extend(metadata.iter().map(|item| match item.kind.as_str() {
        "link" => format!("{}: link {}", item.key, item.value),
        "tag" => format!("{}: #{}", item.key, item.value),
        _ => format!("{}: {}", item.key, item.value),
    }));
    let body = lines.join("\n");
    if body.is_empty() {
        "No detail content".into()
    } else {
        bounded_text(&body, MAX_SURFACE_TEXT)
    }
}

fn view_metadata(metadata: Option<&Value>, kind: &str) -> Vec<PluginViewMetadata> {
    if kind != "detail" {
        return Vec::new();
    }
    metadata
        .and_then(Value::as_array)
        .map(|metadata| {
            let mut keys = std::collections::BTreeSet::new();
            metadata
                .iter()
                .take(PLUGIN_VIEW_SLOT_COUNT)
                .filter_map(parse_view_metadata)
                .filter(|item| keys.insert(item.key.clone()))
                .collect()
        })
        .unwrap_or_default()
}

fn parse_view_metadata(item: &Value) -> Option<PluginViewMetadata> {
    let key = bounded_text(item.get("key")?.as_str()?, 96);
    let value = item.get("value")?;
    let kind = value.get("kind")?.as_str()?;
    let limit = match kind {
        "text" => 512,
        "link" => 2_048,
        "tag" => 96,
        _ => return None,
    };
    let value = bounded_text(value.get("value")?.as_str()?, limit);
    if key.trim().is_empty()
        || value.trim().is_empty()
        || key.chars().any(char::is_control)
        || value.chars().any(char::is_control)
        || (kind == "link" && !value.starts_with("https://"))
    {
        return None;
    }
    Some(PluginViewMetadata {
        key,
        kind: kind.to_owned(),
        value,
    })
}

fn fields_surface_body(fields: Option<&Value>) -> String {
    fields
        .and_then(Value::as_array)
        .map(|fields| {
            fields
                .iter()
                .take(PLUGIN_VIEW_SLOT_COUNT)
                .filter_map(|field| {
                    let label = field.get("label").and_then(Value::as_str)?;
                    let required = field
                        .get("required")
                        .and_then(Value::as_bool)
                        .unwrap_or(false);
                    Some(format!(
                        "{}{}",
                        bounded_text(label, 160),
                        if required { " *" } else { "" }
                    ))
                })
                .collect::<Vec<_>>()
                .join("\n")
        })
        .filter(|body| !body.is_empty())
        .unwrap_or_else(|| "No fields".into())
}

fn actions_surface_body(actions: &[PluginViewAction]) -> String {
    let body = actions
        .iter()
        .map(|action| {
            let prefix = match (action.is_default, action.destructive) {
                (true, true) => "Enter + Confirm: ",
                (true, false) => "Enter: ",
                (false, true) => "Confirm: ",
                (false, false) => "",
            };
            format!("{prefix}{}", action.title)
        })
        .collect::<Vec<_>>()
        .join("\n");
    if body.is_empty() {
        "No actions".into()
    } else {
        body
    }
}

fn view_items(items: Option<&Value>, kind: &str) -> Vec<PluginViewItem> {
    if !matches!(kind, "list" | "grid") {
        return Vec::new();
    }
    items
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .take(MAX_PLUGIN_VIEW_ITEMS)
                .filter_map(parse_view_item)
                .collect()
        })
        .unwrap_or_default()
}

fn view_fields(fields: Option<&Value>, kind: &str) -> Vec<PluginViewField> {
    if kind != "form" {
        return Vec::new();
    }
    fields
        .and_then(Value::as_array)
        .map(|fields| {
            let mut field_ids = std::collections::BTreeSet::new();
            fields
                .iter()
                .take(PLUGIN_VIEW_SLOT_COUNT)
                .filter_map(parse_view_field)
                .filter(|field| field_ids.insert(field.id.clone()))
                .collect()
        })
        .unwrap_or_default()
}

fn parse_view_field(field: &Value) -> Option<PluginViewField> {
    let control = field
        .get("control")
        .and_then(Value::as_str)
        .unwrap_or("text");
    if !matches!(
        control,
        "text" | "password" | "select" | "checkbox" | "switch"
    ) {
        return None;
    }
    let mut options = parse_view_options(field.get("options"));
    if control == "select" && options.is_empty() {
        return None;
    }
    if control != "select" {
        options.clear();
    }
    let initial_value = bounded_text(
        field
            .get("initial_value")
            .and_then(Value::as_str)
            .unwrap_or_default(),
        4_096,
    );
    let initial_value = match control {
        "select" if !options.iter().any(|option| option.id == initial_value) => options
            .first()
            .map_or_else(String::new, |option| option.id.clone()),
        "checkbox" | "switch" if !matches!(initial_value.as_str(), "true" | "false") => {
            "false".into()
        }
        _ => initial_value,
    };
    Some(PluginViewField {
        id: bounded_text(field.get("id")?.as_str()?, 128),
        label: bounded_text(field.get("label")?.as_str()?, 160),
        control: control.to_owned(),
        required: field
            .get("required")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        initial_value,
        helper: bounded_text(
            field
                .get("helper")
                .and_then(Value::as_str)
                .unwrap_or_default(),
            512,
        ),
        error: bounded_text(
            field
                .get("error")
                .and_then(Value::as_str)
                .unwrap_or_default(),
            512,
        ),
        options,
    })
}

fn parse_view_options(options: Option<&Value>) -> Vec<PluginViewOption> {
    let mut ids = std::collections::BTreeSet::new();
    let mut labels = std::collections::BTreeSet::new();
    options
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .take(32)
        .filter_map(|option| {
            let parsed = PluginViewOption {
                id: bounded_text(option.get("id")?.as_str()?, 128),
                label: bounded_text(option.get("label")?.as_str()?, 160),
            };
            (!parsed.id.trim().is_empty()
                && !parsed.label.trim().is_empty()
                && ids.insert(parsed.id.clone())
                && labels.insert(parsed.label.clone()))
            .then_some(parsed)
        })
        .collect()
}

fn view_actions(actions: Option<&Value>, kind: &str) -> Vec<PluginViewAction> {
    if kind != "action_panel" {
        return Vec::new();
    }
    let Some(actions) = actions.and_then(Value::as_array) else {
        return Vec::new();
    };
    if actions.is_empty() || actions.len() > PLUGIN_VIEW_SLOT_COUNT {
        return Vec::new();
    }
    let mut ids = std::collections::BTreeSet::new();
    let Some(actions) = actions
        .iter()
        .map(|action| parse_view_action(action, &mut ids))
        .collect::<Option<Vec<_>>>()
    else {
        return Vec::new();
    };
    if actions.iter().filter(|action| action.is_default).count() == 1 {
        actions
    } else {
        Vec::new()
    }
}

fn parse_view_action(
    action: &Value,
    ids: &mut std::collections::BTreeSet<String>,
) -> Option<PluginViewAction> {
    let id = action.get("id")?.as_str()?;
    let title = action.get("title")?.as_str()?;
    if id.trim().is_empty()
        || id.chars().count() > 128
        || id.chars().any(char::is_control)
        || title.trim().is_empty()
        || title.chars().count() > 160
        || title.chars().any(char::is_control)
        || !ids.insert(id.to_owned())
    {
        return None;
    }
    let is_default = match action.get("role")?.as_str()? {
        "default" => true,
        "secondary" => false,
        _ => return None,
    };
    Some(PluginViewAction {
        id: id.to_owned(),
        title: title.to_owned(),
        is_default,
        destructive: action
            .get("destructive")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    })
}

fn view_next_cursor(view: &Value, kind: &str) -> Option<String> {
    if !matches!(kind, "list" | "grid") {
        return None;
    }
    let cursor = view.get("next_cursor")?.as_str()?;
    (!cursor.trim().is_empty()
        && cursor.chars().count() <= 256
        && !cursor.chars().any(char::is_control))
    .then(|| cursor.to_owned())
}

fn bounded_text(value: &str, max_chars: usize) -> String {
    value.chars().take(max_chars).collect()
}

/// Small synchronous client used only around a user-triggered plugin action.
/// The UI thread must call it from a worker in production; keeping the client
/// synchronous makes the IPC framing and shutdown behavior deterministic.
pub struct PluginHostClient {
    child: Child,
    input: ChildStdin,
    responses: Receiver<Result<novahub_ipc::Envelope, String>>,
    next_request_id: u64,
    auth_token: String,
    capability_handler: Arc<dyn PluginCapabilityHandler>,
}

const RESPONSE_TIMEOUT: Duration = Duration::from_secs(2);
// Component compilation is paid once per short-lived Host. Keep it bounded,
// but allow unoptimized development builds enough time to compile a valid
// Component; plugin calls remain on the stricter response budget above.
const COMPONENT_LOAD_TIMEOUT: Duration = Duration::from_secs(30);
const AUTH_TOKEN_BYTES: usize = 32;

fn debug_host_enabled() -> bool {
    std::env::var_os("NOVAHUB_PLUGIN_HOST_DEBUG").is_some()
}

fn new_auth_token() -> Result<String, PluginHostError> {
    let mut bytes = [0_u8; AUTH_TOKEN_BYTES];
    getrandom::fill(&mut bytes)
        .map_err(|error| PluginHostError::Io(io::Error::other(error.to_string())))?;
    let mut token = String::with_capacity(AUTH_TOKEN_BYTES * 2);
    for byte in bytes {
        use std::fmt::Write as _;

        let _ = write!(token, "{byte:02x}");
    }
    Ok(token)
}

/// Bounds Plugin Host recovery to one replay of the current user-triggered
/// action. A failed Host never becomes a background restart loop.
pub struct PluginHostSupervisor {
    executable: PathBuf,
    arguments: Vec<String>,
    max_restarts: u8,
    capability_handler: Arc<dyn PluginCapabilityHandler>,
}

impl PluginHostSupervisor {
    /// Creates a supervisor with one bounded recovery attempt.
    #[must_use]
    pub fn new(executable: impl Into<PathBuf>) -> Self {
        Self {
            executable: executable.into(),
            arguments: Vec::new(),
            max_restarts: 1,
            capability_handler: Arc::new(DenyCapabilityHandler),
        }
    }

    #[must_use]
    pub fn with_capability_handler(
        mut self,
        capability_handler: Arc<dyn PluginCapabilityHandler>,
    ) -> Self {
        self.capability_handler = capability_handler;
        self
    }

    /// Resolves the sibling Host executable for production or development.
    ///
    /// # Errors
    ///
    /// Returns an I/O error when the current executable path cannot be resolved.
    pub fn from_default_path() -> Result<Self, PluginHostError> {
        Ok(Self::new(default_host_path()?))
    }

    /// Runs one complete Component session, replaying it once after a process
    /// or IPC failure. Host-returned plugin errors are not retried.
    ///
    /// # Errors
    ///
    /// Returns the last Host, IPC, serialization, or I/O error.
    pub fn run_component_session(
        &self,
        session_id: &str,
        component_path: impl AsRef<Path>,
        input: &str,
    ) -> Result<PluginFixtureRun, PluginHostError> {
        self.run_component_session_with_context(
            session_id,
            component_path,
            "unbound",
            &EffectiveGrant::default(),
            input,
        )
    }

    /// Runs one complete Component session with the plugin identity and
    /// effective grant used by WIT capability imports.
    ///
    /// # Errors
    ///
    /// Returns the last Host, IPC, serialization, or I/O error.
    pub fn run_component_session_with_context(
        &self,
        session_id: &str,
        component_path: impl AsRef<Path>,
        plugin_id: &str,
        grant: &EffectiveGrant,
        input: &str,
    ) -> Result<PluginFixtureRun, PluginHostError> {
        self.run_with_recovery(|| {
            let result =
                self.run_once(session_id, component_path.as_ref(), plugin_id, grant, input);
            self.capability_handler.end_session(plugin_id, session_id);
            result
        })
    }

    /// Runs a one-shot command in a sibling Host and reclaims that Host after
    /// the result arrives. Recoverable process failures are replayed once.
    ///
    /// # Errors
    ///
    /// Returns the last process, IPC, serialization, or Host execution error.
    pub fn run_component_one_shot(
        &self,
        session_id: &str,
        component_path: impl AsRef<Path>,
        command_id: &str,
        input: &str,
    ) -> Result<Value, PluginHostError> {
        self.run_component_one_shot_with_context(
            session_id,
            component_path,
            "unbound",
            &EffectiveGrant::default(),
            command_id,
            input,
        )
    }

    /// Runs one command with the plugin identity and effective grant used by
    /// WIT capability imports.
    ///
    /// # Errors
    ///
    /// Returns the last Host, IPC, serialization, or I/O error.
    pub fn run_component_one_shot_with_context(
        &self,
        session_id: &str,
        component_path: impl AsRef<Path>,
        plugin_id: &str,
        grant: &EffectiveGrant,
        command_id: &str,
        input: &str,
    ) -> Result<Value, PluginHostError> {
        self.run_with_recovery(|| {
            let result = self.run_one_shot_once(
                session_id,
                component_path.as_ref(),
                plugin_id,
                grant,
                command_id,
                input,
            );
            self.capability_handler.end_session(plugin_id, session_id);
            result
        })
    }

    fn run_with_recovery<T, F>(&self, mut attempt: F) -> Result<T, PluginHostError>
    where
        F: FnMut() -> Result<T, PluginHostError>,
    {
        let mut restarts = 0_u8;
        loop {
            match attempt() {
                Ok(value) => return Ok(value),
                Err(error) if is_recoverable_host_error(&error) && restarts < self.max_restarts => {
                    restarts = restarts.saturating_add(1);
                }
                Err(error) => return Err(error),
            }
        }
    }

    fn run_once(
        &self,
        session_id: &str,
        component_path: &Path,
        plugin_id: &str,
        grant: &EffectiveGrant,
        input: &str,
    ) -> Result<PluginFixtureRun, PluginHostError> {
        let mut client = PluginHostClient::spawn_with_args_and_handler(
            &self.executable,
            &self.arguments,
            Arc::clone(&self.capability_handler),
        )?;
        let host_pid = client.process_id();
        let loaded = client.load_with_context(session_id, component_path, plugin_id, grant)?;
        let opened = client.open(session_id)?;
        let revision = opened
            .get("revision")
            .and_then(Value::as_u64)
            .ok_or_else(|| PluginHostError::Ipc("open response has no revision".into()))?;
        let updated = if input.is_empty() {
            opened.clone()
        } else {
            client.update(session_id, revision, input)?
        };
        let closed = client.close(session_id)?;
        Ok(PluginFixtureRun {
            host_pid,
            loaded,
            opened,
            updated,
            closed,
        })
    }

    fn run_one_shot_once(
        &self,
        session_id: &str,
        component_path: &Path,
        plugin_id: &str,
        grant: &EffectiveGrant,
        command_id: &str,
        input: &str,
    ) -> Result<Value, PluginHostError> {
        let mut client = PluginHostClient::spawn_with_args_and_handler(
            &self.executable,
            &self.arguments,
            Arc::clone(&self.capability_handler),
        )?;
        client.load_with_context(session_id, component_path, plugin_id, grant)?;
        client.run(session_id, command_id, input)
    }
}

fn is_recoverable_host_error(error: &PluginHostError) -> bool {
    matches!(error, PluginHostError::Io(_) | PluginHostError::Ipc(_))
}

impl PluginHostClient {
    /// Starts a Plugin Host executable with piped stdin/stdout.
    ///
    /// # Errors
    ///
    /// Returns an I/O error when the executable cannot be spawned or its pipes
    /// cannot be created.
    pub fn spawn(executable: impl AsRef<Path>) -> Result<Self, PluginHostError> {
        Self::spawn_with_args(executable, &[])
    }

    /// Starts a Plugin Host with an explicit main-process capability owner.
    ///
    /// # Errors
    ///
    /// Returns an I/O error when the executable cannot be spawned or its pipes
    /// cannot be created.
    pub fn spawn_with_capability_handler(
        executable: impl AsRef<Path>,
        capability_handler: Arc<dyn PluginCapabilityHandler>,
    ) -> Result<Self, PluginHostError> {
        Self::spawn_with_args_and_handler(executable, &[], capability_handler)
    }

    fn spawn_with_args(
        executable: impl AsRef<Path>,
        arguments: &[String],
    ) -> Result<Self, PluginHostError> {
        Self::spawn_with_args_and_handler(executable, arguments, Arc::new(DenyCapabilityHandler))
    }

    fn spawn_with_args_and_handler(
        executable: impl AsRef<Path>,
        arguments: &[String],
        capability_handler: Arc<dyn PluginCapabilityHandler>,
    ) -> Result<Self, PluginHostError> {
        let auth_token = new_auth_token()?;
        let mut command = Command::new(executable.as_ref());
        command
            .args(arguments)
            .env("NOVAHUB_PLUGIN_HOST_TOKEN", &auth_token)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(if debug_host_enabled() {
                Stdio::inherit()
            } else {
                Stdio::null()
            });
        configure_plugin_cache(&mut command);
        let mut child = command.spawn()?;
        let input = child
            .stdin
            .take()
            .ok_or_else(|| PluginHostError::Ipc("host stdin was not piped".into()))?;
        let output = child
            .stdout
            .take()
            .ok_or_else(|| PluginHostError::Ipc("host stdout was not piped".into()))?;
        let (sender, responses) = mpsc::sync_channel(1);
        std::thread::Builder::new()
            .name("novahub-plugin-host-reader".into())
            .spawn(move || read_host_responses(output, sender))
            .map_err(PluginHostError::Io)?;
        Ok(Self {
            child,
            input,
            responses,
            next_request_id: 0,
            auth_token,
            capability_handler,
        })
    }

    /// Resolves the sibling Host binary or an explicit development override.
    ///
    /// # Errors
    ///
    /// Returns an error when the current executable has no usable parent path
    /// or when the Host cannot be spawned.
    pub fn spawn_default() -> Result<Self, PluginHostError> {
        if let Some(path) = std::env::var_os("NOVAHUB_PLUGIN_HOST") {
            return Self::spawn(path);
        }
        let current = std::env::current_exe()?;
        let parent = current
            .parent()
            .ok_or_else(|| PluginHostError::Ipc("NovaHub executable has no parent".into()))?;
        let name = if cfg!(windows) {
            "novahub-plugin-host.exe"
        } else {
            "novahub-plugin-host"
        };
        Self::spawn(parent.join(name))
    }

    /// Loads a bounded Component into a pending session.
    ///
    /// # Errors
    ///
    /// Returns an IPC, serialization, or Host validation error.
    pub fn load(
        &mut self,
        session_id: &str,
        component_path: impl AsRef<Path>,
    ) -> Result<Value, PluginHostError> {
        self.load_with_context(
            session_id,
            component_path,
            "unbound",
            &EffectiveGrant::default(),
        )
    }

    /// Loads a Component with the identity and effective grant used for WIT
    /// capability imports.
    ///
    /// # Errors
    ///
    /// Returns an IPC, serialization, or Host validation error.
    pub fn load_with_context(
        &mut self,
        session_id: &str,
        component_path: impl AsRef<Path>,
        plugin_id: &str,
        grant: &EffectiveGrant,
    ) -> Result<Value, PluginHostError> {
        self.request_with_timeout(
            &json!({
                "type": "load",
                "session_id": session_id,
                "component_path": component_path.as_ref(),
                "plugin_id": plugin_id,
                "grant": grant,
            }),
            COMPONENT_LOAD_TIMEOUT,
        )
    }

    /// Instantiates a loaded Component and returns its initial View payload.
    ///
    /// # Errors
    ///
    /// Returns an IPC, serialization, or Host execution error.
    pub fn open(&mut self, session_id: &str) -> Result<Value, PluginHostError> {
        self.request(&json!({ "type": "open", "session_id": session_id }))
    }

    /// Sends one bounded input update and returns the next View payload.
    ///
    /// # Errors
    ///
    /// Returns an IPC, serialization, stale-session, or Host execution error.
    pub fn update(
        &mut self,
        session_id: &str,
        revision: u64,
        input: &str,
    ) -> Result<Value, PluginHostError> {
        self.request(&json!({
            "type": "update",
            "session_id": session_id,
            "revision": revision,
            "input": input,
        }))
    }

    /// Runs one command and reclaims the short-lived Host immediately after
    /// receiving its bounded result. No persistent View session is created.
    ///
    /// # Errors
    ///
    /// Returns an IPC, serialization, timeout, or Host execution error. The
    /// Host is still shut down after a successful or failed request.
    pub fn run(
        &mut self,
        session_id: &str,
        command_id: &str,
        input: &str,
    ) -> Result<Value, PluginHostError> {
        let result = self.request(&json!({
            "type": "run",
            "session_id": session_id,
            "command_id": command_id,
            "input": input,
        }));
        let shutdown = self.shutdown();
        match (result, shutdown) {
            (Ok(value), Ok(())) => Ok(value),
            (Err(error), _) | (Ok(_), Err(error)) => Err(error),
        }
    }

    /// Closes a session and terminates the short-lived Host process.
    ///
    /// # Errors
    ///
    /// Returns an IPC, serialization, or Host execution error.
    pub fn close(&mut self, session_id: &str) -> Result<Value, PluginHostError> {
        let response = self.request(&json!({ "type": "close", "session_id": session_id }))?;
        self.shutdown()?;
        Ok(response)
    }

    #[cfg(test)]
    fn close_session_without_shutdown(
        &mut self,
        session_id: &str,
    ) -> Result<Value, PluginHostError> {
        self.request(&json!({ "type": "close", "session_id": session_id }))
    }

    /// Returns whether the child process is still alive.
    ///
    /// # Errors
    ///
    /// Returns an I/O error when the child status cannot be queried.
    pub fn is_running(&mut self) -> Result<bool, PluginHostError> {
        Ok(self.child.try_wait()?.is_none())
    }

    fn request(&mut self, payload: &Value) -> Result<Value, PluginHostError> {
        self.request_with_timeout(payload, RESPONSE_TIMEOUT)
    }

    fn request_with_timeout(
        &mut self,
        payload: &Value,
        timeout: Duration,
    ) -> Result<Value, PluginHostError> {
        self.next_request_id = self.next_request_id.saturating_add(1);
        let request_id = format!("app-{}", self.next_request_id);
        let payload = serde_json::to_vec(payload)?;
        let payload_length = payload.len();
        let envelope = encode(&Envelope {
            protocol_major: IPC_PROTOCOL_MAJOR,
            request_id: request_id.clone(),
            payload,
            auth_token: self.auth_token.clone(),
        })
        .map_err(|error| PluginHostError::Ipc(format!("request encoding failed: {error:?}")))?;
        let length = u32::try_from(envelope.len())
            .map_err(|_| PluginHostError::Ipc("request frame exceeds u32".into()))?;
        if debug_host_enabled() {
            eprintln!(
                "NovaHub Plugin Host request: id={request_id}, payload_bytes={}, envelope_bytes={}, token_bytes={}",
                payload_length,
                envelope.len(),
                self.auth_token.len()
            );
        }
        self.input.write_all(&length.to_le_bytes())?;
        self.input.write_all(&envelope)?;
        self.input.flush()?;

        let deadline = Instant::now() + timeout;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return self.response_timeout(&request_id);
            }
            let response = match self.responses.recv_timeout(remaining) {
                Ok(Ok(response)) => response,
                Ok(Err(error)) => return Err(PluginHostError::Ipc(error)),
                Err(RecvTimeoutError::Timeout) => return self.response_timeout(&request_id),
                Err(RecvTimeoutError::Disconnected) => {
                    if debug_host_enabled() {
                        eprintln!(
                            "NovaHub Plugin Host response channel disconnected: id={request_id}"
                        );
                    }
                    return Err(PluginHostError::Ipc(
                        "plugin host response stream closed".into(),
                    ));
                }
            };
            if response.protocol_major != IPC_PROTOCOL_MAJOR
                || response.auth_token != self.auth_token
            {
                return Err(PluginHostError::Ipc(
                    "host response authentication failed".into(),
                ));
            }
            let value: Value = serde_json::from_slice(&response.payload)?;
            if value.get("type").and_then(Value::as_str) == Some("capability_request") {
                self.handle_capability_request(&response.request_id, &value)?;
                continue;
            }
            if response.request_id != request_id {
                return Err(PluginHostError::Ipc("response request ID mismatch".into()));
            }
            if value.get("type").and_then(Value::as_str) == Some("error") {
                let code = value
                    .get("code")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown_host_error");
                return Err(PluginHostError::Host(code.to_owned()));
            }
            return Ok(value);
        }
    }

    fn handle_capability_request(
        &mut self,
        envelope_request_id: &str,
        value: &Value,
    ) -> Result<(), PluginHostError> {
        let capability_request_id = required_string(value, "capability_request_id")?;
        if capability_request_id != envelope_request_id {
            return Err(PluginHostError::Ipc(
                "capability request ID mismatch".into(),
            ));
        }
        let call = value
            .get("call")
            .ok_or_else(|| PluginHostError::Ipc("capability request has no call".into()))?;
        let capability = serde_json::from_value::<AuthorizedCapability>(
            call.get("capability").cloned().ok_or_else(|| {
                PluginHostError::Ipc("capability request has no operation".into())
            })?,
        )?;
        let request = PluginCapabilityRequest {
            request_id: capability_request_id.to_owned(),
            plugin_id: required_string(value, "plugin_id")?.to_owned(),
            session_id: required_string(value, "session_id")?.to_owned(),
            capability,
            payload: call
                .get("payload")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned),
        };
        let result = self.capability_handler.handle(request);
        let result = match result {
            PluginCapabilityResponse::Unit => json!({ "kind": "unit" }),
            PluginCapabilityResponse::Text(value) => {
                json!({ "kind": "text", "value": value })
            }
            PluginCapabilityResponse::Bytes(value) => {
                json!({ "kind": "bytes", "value": value })
            }
            PluginCapabilityResponse::Error(code) => {
                json!({ "kind": "error", "code": bounded_error_code(&code) })
            }
        };
        self.send_envelope(
            capability_request_id,
            &json!({
                "type": "capability_response",
                "capability_request_id": capability_request_id,
                "result": result,
            }),
        )
    }

    fn send_envelope(&mut self, request_id: &str, payload: &Value) -> Result<(), PluginHostError> {
        let payload = serde_json::to_vec(payload)?;
        let envelope = encode(&Envelope {
            protocol_major: IPC_PROTOCOL_MAJOR,
            request_id: request_id.to_owned(),
            payload,
            auth_token: self.auth_token.clone(),
        })
        .map_err(|error| PluginHostError::Ipc(format!("request encoding failed: {error:?}")))?;
        let length = u32::try_from(envelope.len())
            .map_err(|_| PluginHostError::Ipc("request frame exceeds u32".into()))?;
        self.input.write_all(&length.to_le_bytes())?;
        self.input.write_all(&envelope)?;
        self.input.flush()?;
        Ok(())
    }

    fn response_timeout<T>(&mut self, request_id: &str) -> Result<T, PluginHostError> {
        if debug_host_enabled() {
            let status = self.child.try_wait().ok().flatten();
            eprintln!("NovaHub Plugin Host timeout: id={request_id}, child_status={status:?}");
        }
        Err(PluginHostError::Ipc(
            "plugin host response deadline exceeded".into(),
        ))
    }

    fn shutdown(&mut self) -> Result<(), PluginHostError> {
        self.input.flush()?;
        if self.child.try_wait()?.is_none() {
            let _ = self.child.kill();
            self.child.wait()?;
        }
        Ok(())
    }

    #[must_use]
    pub fn process_id(&self) -> u32 {
        self.child.id()
    }
}

fn configure_plugin_cache(command: &mut Command) {
    match std::env::var_os("NOVAHUB_PLUGIN_CACHE_DIR") {
        Some(path) if path.is_empty() => {
            command.env_remove("NOVAHUB_PLUGIN_CACHE_DIR");
        }
        Some(path) => {
            command.env("NOVAHUB_PLUGIN_CACHE_DIR", path);
        }
        None => {
            if let Ok(data_dir) = crate::default_data_dir() {
                command.env(
                    "NOVAHUB_PLUGIN_CACHE_DIR",
                    data_dir.join("cache").join("wasmtime"),
                );
            }
        }
    }
}

fn required_string<'a>(value: &'a Value, field: &str) -> Result<&'a str, PluginHostError> {
    value
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| PluginHostError::Ipc(format!("host response has no {field}")))
}

fn bounded_error_code(value: &str) -> String {
    value
        .chars()
        .filter(|character| !character.is_control())
        .take(128)
        .collect()
}

#[allow(clippy::needless_pass_by_value)]
fn read_host_responses(
    output: ChildStdout,
    sender: mpsc::SyncSender<Result<novahub_ipc::Envelope, String>>,
) {
    let mut output = BufReader::new(output);
    loop {
        let mut length = [0; 4];
        if let Err(error) = output.read_exact(&mut length) {
            let _ = sender.send(Err(format!("host response read failed: {error}")));
            return;
        }
        let length = u32::from_le_bytes(length) as usize;
        if length > MAX_ENVELOPE_BYTES {
            let _ = sender.send(Err("response frame exceeds host limit".into()));
            return;
        }
        let mut frame = vec![0; length];
        if let Err(error) = output.read_exact(&mut frame) {
            let _ = sender.send(Err(format!("host response frame read failed: {error}")));
            return;
        }
        let response =
            decode(&frame).map_err(|error| format!("response decoding failed: {error:?}"));
        if sender.send(response).is_err() {
            return;
        }
    }
}

impl Drop for PluginHostClient {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}

/// Resolves the default Host executable path without spawning it.
///
/// # Errors
///
/// Returns an I/O error when the current executable path cannot be resolved.
pub fn default_host_path() -> Result<PathBuf, PluginHostError> {
    if let Some(path) = std::env::var_os("NOVAHUB_PLUGIN_HOST") {
        return Ok(PathBuf::from(path));
    }
    let current = std::env::current_exe()?;
    let parent = current
        .parent()
        .ok_or_else(|| PluginHostError::Ipc("NovaHub executable has no parent".into()))?;
    Ok(parent.join(if cfg!(windows) {
        "novahub-plugin-host.exe"
    } else {
        "novahub-plugin-host"
    }))
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use serde_json::json;

    use super::{
        PLUGIN_VIEW_SLOT_COUNT, PluginFixtureRun, PluginHostClient, PluginHostError,
        PluginHostSupervisor, default_host_path,
    };

    #[test]
    fn default_host_path_is_sibling_or_explicit_override() {
        let path = default_host_path().expect("current executable has a parent");
        assert!(!path.as_os_str().is_empty());
    }

    #[test]
    fn capability_component_closed_session_rejects_followup_when_configured() {
        let (Ok(host), Ok(fixture)) = (
            std::env::var("NOVAHUB_PLUGIN_HOST"),
            std::env::var("NOVAHUB_COMPONENT_FIXTURE"),
        ) else {
            return;
        };
        let mut client = PluginHostClient::spawn(host).expect("spawn configured Plugin Host");
        client
            .load("closed-capability", fixture)
            .expect("load Component fixture");
        let opened = client
            .open("closed-capability")
            .expect("open Component session");
        let revision = opened["revision"].as_u64().expect("open revision");
        client
            .close_session_without_shutdown("closed-capability")
            .expect("close Component session");

        let error = client
            .update("closed-capability", revision, "capability:storage:1")
            .expect_err("closed session cannot issue a capability follow-up");
        assert!(
            matches!(
                &error,
                PluginHostError::Host(code) if code == "session_not_found"
            ),
            "unexpected closed-session error: {error:?}"
        );
    }

    #[test]
    fn supervisor_retries_a_recoverable_io_error_once() {
        let supervisor = PluginHostSupervisor::new("unused-host");
        let attempts = Cell::new(0_u8);
        let result = supervisor.run_with_recovery(|| {
            let attempt = attempts.get().saturating_add(1);
            attempts.set(attempt);
            if attempt == 1 {
                Err(PluginHostError::Ipc("unexpected EOF".into()))
            } else {
                Ok("recovered")
            }
        });

        assert_eq!(result.expect("second attempt succeeds"), "recovered");
        assert_eq!(attempts.get(), 2);
    }

    #[test]
    fn supervisor_does_not_retry_host_business_errors() {
        let supervisor = PluginHostSupervisor::new("unused-host");
        let attempts = Cell::new(0_u8);
        let result = supervisor.run_with_recovery(|| {
            attempts.set(attempts.get().saturating_add(1));
            Err::<(), _>(PluginHostError::Host("component_timeout".into()))
        });

        assert!(matches!(result, Err(PluginHostError::Host(_))));
        assert_eq!(attempts.get(), 1);
    }

    #[test]
    fn supervisor_bounds_recoverable_failures_to_two_attempts() {
        let supervisor = PluginHostSupervisor::new("unused-host");
        let attempts = Cell::new(0_u8);
        let result = supervisor.run_with_recovery(|| {
            attempts.set(attempts.get().saturating_add(1));
            Err::<(), _>(PluginHostError::Io(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "host exited",
            )))
        });

        assert!(matches!(result, Err(PluginHostError::Io(_))));
        assert_eq!(attempts.get(), 2);
    }

    #[test]
    fn view_summary_preserves_kind_specific_bounded_information() {
        let run = PluginFixtureRun {
            host_pid: 0,
            loaded: json!({"type":"loaded"}),
            opened: json!({"type":"view"}),
            updated: json!({
                "type":"view",
                "view":{"kind":"action_panel","title":"Actions","actions":[{"id":"copy"}]}
            }),
            closed: json!({"type":"ack"}),
        };
        assert_eq!(run.updated_view_summary(), "Actions (1 actions)");
    }

    #[test]
    fn view_surface_preserves_kind_and_bounds_plugin_content() {
        let run = PluginFixtureRun {
            host_pid: 0,
            loaded: json!({"type":"loaded"}),
            opened: json!({"type":"view"}),
            updated: json!({
                "type":"view",
                "view": {
                    "kind":"form",
                    "title":"Translate",
                    "fields":[{"id":"text","label":"Text","required":true}]
                }
            }),
            closed: json!({"type":"ack"}),
        };
        assert_eq!(
            run.updated_view_surface(),
            super::PluginViewSurface {
                kind: "form".into(),
                title: "Translate".into(),
                body: "Text *".into(),
                progress: None,
                items: Vec::new(),
                next_cursor: None,
                metadata: Vec::new(),
                fields: vec![super::PluginViewField {
                    id: "text".into(),
                    label: "Text".into(),
                    control: "text".into(),
                    required: true,
                    initial_value: String::new(),
                    helper: String::new(),
                    error: String::new(),
                    options: Vec::new(),
                }],
                actions: Vec::new(),
            }
        );
    }

    #[test]
    fn form_surface_preserves_controls_and_normalizes_untrusted_values() {
        let surface = super::surface_from_view(&json!({
            "view": {
                "kind": "form",
                "title": "Account",
                "fields": [
                    {
                        "id": "username",
                        "label": "Username",
                        "control": "text",
                        "initial_value": "nova",
                        "helper": "Public name"
                    },
                    {
                        "id": "password",
                        "label": "Password",
                        "control": "password",
                        "error": "Too short"
                    },
                    {
                        "id": "region",
                        "label": "Region",
                        "control": "select",
                        "initial_value": "missing",
                        "options": [
                            {"id": "cn", "label": "China"},
                            {"id": "us", "label": "United States"},
                            {"id": "cn", "label": "Duplicate"}
                        ]
                    },
                    {
                        "id": "terms",
                        "label": "Accept terms",
                        "control": "checkbox",
                        "initial_value": "invalid",
                        "required": true
                    },
                    {
                        "id": "updates",
                        "label": "Automatic updates",
                        "control": "switch",
                        "initial_value": "true"
                    },
                    {"id": "custom", "label": "Custom", "control": "unknown"}
                ]
            }
        }));

        assert_eq!(surface.fields.len(), 5);
        assert_eq!(surface.fields[0].control, "text");
        assert_eq!(surface.fields[0].helper, "Public name");
        assert_eq!(surface.fields[1].control, "password");
        assert_eq!(surface.fields[1].error, "Too short");
        assert_eq!(surface.fields[2].control, "select");
        assert_eq!(surface.fields[2].initial_value, "cn");
        assert_eq!(surface.fields[2].options.len(), 2);
        assert_eq!(surface.fields[2].options[1].id, "us");
        assert_eq!(surface.fields[3].initial_value, "false");
        assert!(surface.fields[3].required);
        assert_eq!(surface.fields[4].initial_value, "true");
    }

    #[test]
    fn progress_surface_produces_a_finite_bounded_fraction() {
        for (completed, total, expected) in [(5, 10, 0.5), (20, 10, 1.0), (0, 0, 0.0)] {
            let surface = super::surface_from_view(&json!({
                "view": {
                    "kind": "progress",
                    "completed": completed,
                    "total": total
                }
            }));
            let progress = surface.progress.expect("progress view has a fraction");
            assert!(progress.is_finite());
            assert!((progress - expected).abs() < f32::EPSILON);
        }
    }

    #[test]
    fn detail_surface_preserves_only_safe_typed_metadata() {
        let surface = super::surface_from_view(&json!({
            "view": {
                "kind": "detail",
                "title": "Package",
                "markdown": "Verified details",
                "metadata": [
                    {"key": "publisher", "value": {"kind": "text", "value": "NovaHub"}},
                    {"key": "docs", "value": {"kind": "link", "value": "https://example.com/docs"}},
                    {"key": "format", "value": {"kind": "tag", "value": "wasm"}},
                    {"key": "unsafe", "value": {"kind": "link", "value": "http://example.com"}},
                    {"key": "format", "value": {"kind": "tag", "value": "duplicate"}},
                    {"key": "custom", "value": {"kind": "unknown", "value": "ignored"}}
                ]
            }
        }));

        assert_eq!(surface.metadata.len(), 3);
        assert_eq!(surface.metadata[0].kind, "text");
        assert_eq!(surface.metadata[1].kind, "link");
        assert_eq!(surface.metadata[2].kind, "tag");
        assert!(surface.body.contains("docs: link https://example.com/docs"));
        assert!(surface.body.contains("format: #wasm"));
        assert!(!surface.body.contains("http://example.com"));
    }

    #[test]
    fn view_surface_keeps_bounded_interactive_slots_for_host_rendering() {
        let run = PluginFixtureRun {
            host_pid: 0,
            loaded: json!({"type":"loaded"}),
            opened: json!({"type":"view"}),
            updated: json!({
                "type":"view",
                "view": {
                    "kind":"action_panel",
                    "title":"Actions",
                    "actions":[
                        {"id":"copy","title":"Copy","role":"default","destructive":false},
                        {"id":"delete","title":"Delete","role":"secondary","destructive":true}
                    ]
                }
            }),
            closed: json!({"type":"ack"}),
        };
        let surface = run.updated_view_surface();
        assert_eq!(surface.actions.len(), 2);
        assert_eq!(surface.actions[0].id, "copy");
        assert!(surface.actions[0].is_default);
        assert!(!surface.actions[1].is_default);
        assert!(surface.actions[1].destructive);

        let list = PluginFixtureRun {
            updated: json!({
                "view": {
                    "kind":"list",
                    "title":"Results",
                    "next_cursor":"opaque-page-2",
                    "items":[{
                        "id":"one",
                        "title":"One",
                        "subtitle":"First",
                        "accessory": {
                            "status":"Ready",
                            "badge":"JSON",
                            "shortcut":"Enter",
                            "icon":{"id":"icon.json"}
                        }
                    }]
                }
            }),
            ..run
        };
        let surface = list.updated_view_surface();
        assert_eq!(surface.items[0].id, "one");
        let accessory = surface.items[0]
            .accessory
            .as_ref()
            .expect("bounded accessory is retained");
        assert_eq!(accessory.status, "Ready");
        assert_eq!(accessory.badge, "JSON");
        assert_eq!(accessory.shortcut, "Enter");
        assert_eq!(accessory.icon_id.as_deref(), Some("icon.json"));
        assert_eq!(surface.next_cursor.as_deref(), Some("opaque-page-2"));
        assert!(surface.body.contains("[JSON] (Enter)"));
    }

    #[test]
    fn action_surface_requires_one_bounded_default_action() {
        for actions in [
            json!([{"id":"copy","title":"Copy","role":"secondary"}]),
            json!([
                {"id":"copy","title":"Copy","role":"default"},
                {"id":"open","title":"Open","role":"default"}
            ]),
            json!([
                {"id":"copy","title":"Copy","role":"default"},
                {"id":"copy","title":"Duplicate","role":"secondary"}
            ]),
            json!([{"id":"copy","title":"Copy","role":"unknown"}]),
        ] {
            let surface = super::surface_from_view(&json!({
                "view": {
                    "kind": "action_panel",
                    "title": "Actions",
                    "actions": actions,
                }
            }));
            assert!(surface.actions.is_empty());
            assert_eq!(surface.body, "No actions");
        }
    }

    #[test]
    fn list_surface_drops_invalid_pagination_cursors() {
        for cursor in ["", "page\n2"] {
            let surface = super::surface_from_view(&json!({
                "view": {
                    "kind": "list",
                    "title": "Results",
                    "items": [],
                    "next_cursor": cursor,
                }
            }));
            assert_eq!(surface.next_cursor, None);
        }

        let surface = super::surface_from_view(&json!({
            "view": {
                "kind": "list",
                "title": "Results",
                "items": [],
                "next_cursor": "x".repeat(257),
            }
        }));
        assert_eq!(surface.next_cursor, None);
    }

    #[test]
    fn list_surface_retains_a_host_snapshot_beyond_the_visible_slot_count() {
        let items = (0..=PLUGIN_VIEW_SLOT_COUNT)
            .map(|index| json!({"id": format!("item-{index}"), "title": format!("Item {index}")}))
            .collect::<Vec<_>>();
        let run = PluginFixtureRun {
            host_pid: 0,
            loaded: json!({}),
            opened: json!({}),
            updated: json!({
                "view": {
                    "kind": "list",
                    "title": "Results",
                    "items": items,
                }
            }),
            closed: json!({}),
        };
        let surface = run.updated_view_surface();
        assert_eq!(surface.items.len(), PLUGIN_VIEW_SLOT_COUNT + 1);
        assert_eq!(
            surface.items.last().map(|item| item.id.as_str()),
            Some("item-6")
        );
    }

    #[cfg(windows)]
    #[test]
    fn host_without_a_response_hits_the_bounded_ipc_deadline() {
        let arguments = vec!["/c".into(), "ping 127.0.0.1 -n 5 > nul".into()];
        let mut client = PluginHostClient::spawn_with_args("cmd.exe", &arguments)
            .expect("silent command host starts");
        let error = client
            .load("timeout", "missing.component")
            .expect_err("non-responsive host must time out");
        assert!(
            matches!(&error, PluginHostError::Ipc(message) if
                message.contains("deadline") || message.contains("response read failed")),
            "unexpected error: {error:?}"
        );
    }

    #[cfg(windows)]
    #[test]
    fn supervisor_recovers_from_a_real_eof_process() {
        let mut supervisor = PluginHostSupervisor::new("cmd.exe");
        supervisor.arguments = vec!["/c".into(), "exit".into(), "1".into()];
        let result = supervisor.run_component_session("eof", "missing.component", "input");

        assert!(matches!(
            result,
            Err(PluginHostError::Io(_) | PluginHostError::Ipc(_))
        ));
    }

    #[cfg(windows)]
    #[test]
    fn supervisor_recovers_from_real_eof_then_runs_the_component() {
        let (Ok(host), Ok(fixture)) = (
            std::env::var("NOVAHUB_PLUGIN_HOST"),
            std::env::var("NOVAHUB_COMPONENT_FIXTURE"),
        ) else {
            return;
        };
        let marker = std::env::temp_dir().join(format!(
            "novahub-host-recovery-{}.marker",
            std::process::id()
        ));
        let script = marker.with_extension("cmd");
        let _ = std::fs::remove_file(&marker);
        let _ = std::fs::remove_file(&script);
        let command = format!(
            "@echo off\r\nif exist \"{}\" goto run\r\ntype nul > \"{}\"\r\nexit /b 1\r\n:run\r\n\"{}\"\r\n",
            marker.display(),
            marker.display(),
            host
        );
        std::fs::write(&script, command).expect("write real EOF recovery wrapper");
        let mut supervisor = PluginHostSupervisor::new("cmd.exe");
        supervisor.arguments = vec![
            "/d".into(),
            "/c".into(),
            script.to_string_lossy().into_owned(),
        ];

        let run = supervisor
            .run_component_session("real-eof-recovery", fixture, "recovered")
            .expect("second real Host attempt succeeds");
        assert!(marker.is_file());
        assert_eq!(run.loaded["type"], "loaded");
        assert_eq!(run.opened["revision"], 1);
        assert_eq!(run.updated["revision"], 2);
        assert_eq!(run.closed["type"], "ack");
        let _ = std::fs::remove_file(marker);
        let _ = std::fs::remove_file(script);
    }
}

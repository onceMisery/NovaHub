#![forbid(unsafe_code)]

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::sync::{
    Arc, Mutex,
    mpsc::{self, Receiver},
};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use novahub_core_domain::pet::{PetController, PetEvent, PetId, PetState};
use novahub_core_domain::pet_position::{DisplayWorkArea, LogicalPoint, PetPlacement, PetPosition};
use novahub_core_domain::{Action, CommandDescriptor};
use novahub_platform_api::{
    ApplicationInfo, ClipboardPayload, FileSearchResult, PlatformError, PlatformServices,
    SystemCommand,
};
#[cfg(target_os = "macos")]
use novahub_platform_macos::MacosAdapter;
#[cfg(windows)]
use novahub_platform_windows::WindowsAdapter;
use novahub_plugin_manager::{
    AuthorizedCapability, Capability, CapabilityBroker, CapabilityDenied, CapabilityRequest,
    DeclaredPermissions, EffectiveGrant, HostFileHandle, InstalledPet, PluginIdentity,
    PluginInteraction, PluginKind, PluginManifest, parse_manifest_toml, read_active_pet,
    read_active_plugin,
};
use novahub_providers::{
    BuiltinActionError, builtin_commands,
    clipboard::{
        ClipboardError, ClipboardItem, ClipboardKind, ClipboardPolicy, ClipboardPoller,
        ClipboardVault, CredentialStore, HISTORY_TTL_SECONDS, MAX_HISTORY_ITEMS,
    },
    execute_builtin_with_host,
};
use novahub_search::CommandMatcher;
use novahub_storage::{Storage, StorageInputError};
use novahub_ui_slint::{
    PetAction, PetActionShelf, PetActionState, PetFrameState, PetRenderer, RenderFrame, ShellState,
};
use pinyin::ToPinyin;

pub mod diagnostics;
pub mod plugin_client;

pub use diagnostics::{
    DiagnosticEvent, DiagnosticEventBuffer, DiagnosticExportOptions, DiagnosticPhase,
    DiagnosticStatus,
};
pub use plugin_client::{
    PluginCapabilityHandler, PluginCapabilityRequest, PluginCapabilityResponse, PluginFixtureRun,
    PluginHostClient, PluginHostError, PluginHostSupervisor, default_host_path,
};

#[cfg(test)]
mod diagnostics_tests {
    use super::{
        DiagnosticEvent, DiagnosticEventBuffer, DiagnosticExportOptions, DiagnosticPhase,
        DiagnosticStatus, NovaHubApp,
    };

    #[test]
    fn diagnostic_buffer_evicts_oldest_event_at_fixed_capacity() {
        let mut buffer = DiagnosticEventBuffer::with_capacity(2);
        buffer.push(DiagnosticEvent::success("request-1", DiagnosticPhase::Load));
        buffer.push(DiagnosticEvent::success("request-2", DiagnosticPhase::Open));
        buffer.push(DiagnosticEvent::failure(
            "request-3",
            DiagnosticPhase::Update,
            "timeout",
        ));

        let events = buffer.snapshot();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].request_id(), "request-2");
        assert_eq!(events[1].status(), DiagnosticStatus::Failure);
        assert_eq!(events[1].error_class(), Some("timeout"));
    }

    #[test]
    fn diagnostic_event_preview_omits_user_content_and_paths() {
        let mut buffer = DiagnosticEventBuffer::default();
        buffer.push(
            DiagnosticEvent::failure(
                "plugin:demo:run",
                DiagnosticPhase::Run,
                "component_run: user input secret",
            )
            .with_plugin("demo")
            .with_pointer_state("active")
            .with_version("1.2.3")
            .with_package_hash("sha256:abc")
            .with_signed(true),
        );

        let preview = buffer.preview_json().expect("diagnostic preview JSON");
        assert!(preview.contains("component_run"));
        assert!(!preview.contains("user input secret"));
        assert!(!preview.contains("C:/"));
        assert!(!preview.contains("https://"));
        let entries: serde_json::Value =
            serde_json::from_str(&preview).expect("preview remains valid JSON");
        assert_eq!(entries[0]["plugin_id"], "demo");
    }

    #[test]
    fn diagnostic_event_fields_are_bounded_and_control_free() {
        let event = DiagnosticEvent::success(" req\n", DiagnosticPhase::Close)
            .with_plugin("x".repeat(400))
            .with_error_class("bad\nvalue");
        assert_eq!(event.request_id(), "req");
        assert!(event.plugin_id().expect("plugin id").len() <= 128);
        assert!(
            !event
                .error_class()
                .expect("error class")
                .chars()
                .any(char::is_control)
        );
    }

    #[test]
    fn app_exposes_and_clears_a_local_diagnostic_snapshot() {
        let app = super::NovaHubApp::new();
        assert_eq!(app.diagnostic_summary(), "No plugin diagnostics");
        app.diagnostics.borrow_mut().push(
            DiagnosticEvent::success("plugin:demo:run", DiagnosticPhase::Run).with_plugin("demo"),
        );

        assert_eq!(app.diagnostic_events().len(), 1);
        assert!(
            app.diagnostic_preview()
                .expect("diagnostic preview")
                .contains("demo")
        );
        app.clear_diagnostics();
        assert!(app.diagnostic_events().is_empty());
    }

    #[test]
    fn diagnostic_export_applies_user_selected_reduction() {
        let app = NovaHubApp::new();
        app.diagnostics
            .borrow_mut()
            .push(DiagnosticEvent::success("request-1", DiagnosticPhase::Open));
        app.diagnostics.borrow_mut().push(DiagnosticEvent::failure(
            "request-2",
            DiagnosticPhase::Update,
            "timeout:first",
        ));
        app.diagnostics.borrow_mut().push(DiagnosticEvent::failure(
            "request-3",
            DiagnosticPhase::Close,
            "host-crash:details",
        ));

        let destination = std::env::temp_dir().join(format!(
            "novahub-diagnostics-export-{}.json",
            std::process::id()
        ));
        let bytes = app
            .export_diagnostics(
                &destination,
                DiagnosticExportOptions {
                    max_events: 1,
                    failures_only: true,
                },
            )
            .expect("export diagnostics");
        let exported = std::fs::read_to_string(&destination).expect("read diagnostic export");
        assert_eq!(bytes, exported.len());
        assert!(exported.contains("request-3"));
        assert!(!exported.contains("request-1"));
        assert!(!exported.contains("request-2"));
        assert!(!exported.contains("details"));
        let _ = std::fs::remove_file(destination);
    }
}

const MAX_APPLICATION_SNAPSHOT: usize = 128;
const MAX_APPLICATION_USAGE_ENTRIES: usize = 128;
const MAX_INSTALLED_PLUGIN_COMMANDS: usize = 128;
#[cfg(target_os = "macos")]
pub const DEFAULT_GLOBAL_HOTKEY: &str = "Option+Space";
#[cfg(not(target_os = "macos"))]
pub const DEFAULT_GLOBAL_HOTKEY: &str = "Alt+Space";
const GLOBAL_HOTKEY_SETTING: &str = "global_hotkey";
const PET_POSITION_SETTING: &str = "pet_position";
const ACTIVE_PET_SETTING: &str = "active_pet";
const CLIPBOARD_TTL_SETTING: &str = "clipboard_ttl_seconds";
const CLIPBOARD_MAX_ITEMS_SETTING: &str = "clipboard_max_items";
const MAX_PLUGIN_CAPABILITY_TEXT_BYTES: usize = 1024 * 1024;
const MAX_PLUGIN_FILE_BYTES: usize = 512 * 1024;
const MAX_PLUGIN_FILE_HANDLES: usize = 8;
const MAX_PLUGIN_HTTP_BYTES: usize = 512 * 1024;
const MAX_PLUGIN_HTTP_REDIRECTS: usize = 5;
const PLUGIN_HTTP_TOTAL_TIMEOUT: Duration = Duration::from_secs(10);

/// Resolves the host-owned data directory without introducing a platform
/// runtime dependency. The platform-specific adapters remain responsible for
/// OS capabilities; this function only chooses a writable data root.
///
/// # Errors
///
/// Returns an error when no supported per-user data location is available.
pub fn default_data_dir() -> Result<std::path::PathBuf, String> {
    if let Some(path) = std::env::var_os("NOVAHUB_DATA_DIR") {
        if path.is_empty() {
            return Err("NOVAHUB_DATA_DIR must not be empty".into());
        }
        return Ok(path.into());
    }

    #[cfg(windows)]
    if let Some(path) = std::env::var_os("LOCALAPPDATA") {
        return Ok(std::path::PathBuf::from(path).join("NovaHub"));
    }

    #[cfg(target_os = "macos")]
    if let Some(path) = std::env::var_os("HOME") {
        return Ok(std::path::PathBuf::from(path)
            .join("Library")
            .join("Application Support")
            .join("NovaHub"));
    }

    #[cfg(not(any(windows, target_os = "macos")))]
    if let Some(path) = std::env::var_os("XDG_DATA_HOME") {
        return Ok(std::path::PathBuf::from(path).join("NovaHub"));
    }

    #[cfg(not(windows))]
    if let Some(path) = std::env::var_os("HOME") {
        return Ok(std::path::PathBuf::from(path)
            .join(".local")
            .join("share")
            .join("NovaHub"));
    }

    Err("no per-user NovaHub data directory is available".into())
}

fn load_clipboard_policy(storage: &Storage) -> ClipboardPolicy {
    let max_items = storage
        .get_setting(CLIPBOARD_MAX_ITEMS_SETTING)
        .ok()
        .flatten()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(MAX_HISTORY_ITEMS);
    let ttl_seconds = storage
        .get_setting(CLIPBOARD_TTL_SETTING)
        .ok()
        .flatten()
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(HISTORY_TTL_SECONDS);
    ClipboardPolicy::new(max_items, ttl_seconds).unwrap_or_default()
}

fn command_belongs_to_provider(command_id: &str, provider: &str) -> bool {
    if provider == "plugins" {
        return command_id.starts_with("plugin:");
    }
    command_id.starts_with(&format!("{provider}."))
        || command_id.starts_with(&format!("{provider}:"))
}

#[derive(Clone)]
struct HostCredentialStore(HostPlatform);

impl CredentialStore for HostCredentialStore {
    fn encryption_key(&self) -> Result<[u8; 32], ClipboardError> {
        self.0
            .credential_key("clipboard-history")
            .map_err(|error| ClipboardError::CredentialStore(error.to_string()))
    }
}

/// Main-process model for the first local search path.
pub struct NovaHubApp {
    commands: Vec<CommandDescriptor>,
    plugin_interactions: BTreeMap<String, PluginInteraction>,
    shell: ShellState,
    storage: Storage,
    storage_path: Option<std::path::PathBuf>,
    pet_controller: PetController,
    pet_renderer: PetRenderer,
    active_pet_id: String,
    pet_frame_assets: BTreeMap<String, Vec<u8>>,
    pet_shelf: PetActionShelf,
    pet_placement: Option<PetPlacement>,
    saved_pet_position: Option<PetPosition>,
    clipboard: ClipboardVault<HostCredentialStore>,
    clipboard_poller: ClipboardPoller,
    clipboard_paused: bool,
    platform: HostPlatform,
    application_cache: RefCell<Option<Vec<ApplicationInfo>>>,
    application_usage: RefCell<std::collections::BTreeMap<String, u32>>,
    search_matcher: RefCell<CommandMatcher>,
    plugin_root: Option<std::path::PathBuf>,
    pending_confirmation: Option<Action>,
    diagnostics: RefCell<DiagnosticEventBuffer>,
    #[cfg(test)]
    test_file_selection: Option<Vec<u8>>,
    #[cfg(test)]
    test_http_responses: BTreeMap<String, novahub_platform_api::HttpResponse>,
}

/// Sendable execution context prepared from one enabled active plugin.
///
/// It carries only the verified component path, plugin identity, effective
/// grant, and main-process capability handler. The non-`Send` application and
/// `SQLite` connection stay on their owning thread.
#[derive(Clone)]
pub struct ActivePluginExecution {
    plugin_id: String,
    component_path: std::path::PathBuf,
    grant: EffectiveGrant,
    capability_handler: Arc<dyn PluginCapabilityHandler>,
}

/// Bounded host-owned permission state shown by native management surfaces.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PluginPermissionSnapshot {
    pub declared: Vec<String>,
    pub granted: Vec<String>,
}

impl ActivePluginExecution {
    /// Runs a view interaction through a short-lived sibling Plugin Host.
    ///
    /// # Errors
    ///
    /// Returns an IPC, serialization, or Plugin Host error.
    pub fn run_view(
        &self,
        session_id: &str,
        input: &str,
    ) -> Result<PluginFixtureRun, PluginHostError> {
        PluginHostSupervisor::from_default_path()?
            .with_capability_handler(Arc::clone(&self.capability_handler))
            .run_component_session_with_context(
                session_id,
                &self.component_path,
                &self.plugin_id,
                &self.grant,
                input,
            )
    }

    /// Runs a one-shot command through a short-lived sibling Plugin Host.
    ///
    /// # Errors
    ///
    /// Returns an IPC, serialization, or Plugin Host error.
    pub fn run_one_shot(
        &self,
        session_id: &str,
        command_id: &str,
        input: &str,
    ) -> Result<serde_json::Value, PluginHostError> {
        PluginHostSupervisor::from_default_path()?
            .with_capability_handler(Arc::clone(&self.capability_handler))
            .run_component_one_shot_with_context(
                session_id,
                &self.component_path,
                &self.plugin_id,
                &self.grant,
                command_id,
                input,
            )
    }

    #[must_use]
    pub fn component_path(&self) -> &std::path::Path {
        &self.component_path
    }
}

#[derive(Clone)]
enum HostPlatform {
    #[cfg(windows)]
    Windows(WindowsAdapter),
    #[cfg(target_os = "macos")]
    Macos(MacosAdapter),
    #[cfg(not(any(windows, target_os = "macos")))]
    Unsupported,
}

#[derive(Clone)]
struct HostPluginCapabilityHandler {
    plugin_id: String,
    grant: EffectiveGrant,
    platform: HostPlatform,
    issued_file_contents: Arc<Mutex<BTreeMap<String, Vec<u8>>>>,
    storage_path: Option<std::path::PathBuf>,
    #[cfg(test)]
    test_file_selection: Option<Vec<u8>>,
    #[cfg(test)]
    test_http_responses: BTreeMap<String, novahub_platform_api::HttpResponse>,
}

impl HostPluginCapabilityHandler {
    fn new(plugin_id: &str, grant: EffectiveGrant, platform: HostPlatform) -> Self {
        Self {
            plugin_id: plugin_id.to_owned(),
            grant,
            platform,
            issued_file_contents: Arc::new(Mutex::new(BTreeMap::new())),
            storage_path: None,
            #[cfg(test)]
            test_file_selection: None,
            #[cfg(test)]
            test_http_responses: BTreeMap::new(),
        }
    }

    fn with_storage_path(mut self, path: Option<std::path::PathBuf>) -> Self {
        self.storage_path = path;
        self
    }

    fn file_handle_key(handle: &HostFileHandle) -> String {
        format!(
            "{}\u{0}{}\u{0}{}",
            handle.plugin_id(),
            handle.session_id(),
            handle.token()
        )
    }

    #[cfg(test)]
    fn with_issued_file_content(self, token: &str, content: &[u8]) -> Self {
        self.issued_file_contents
            .lock()
            .expect("file content lock")
            .insert(
                Self::file_handle_key(&HostFileHandle::issue(&self.plugin_id, "session-1", token)),
                content.to_vec(),
            );
        self
    }

    #[cfg(test)]
    fn with_test_file_selection(mut self, content: &[u8]) -> Self {
        self.test_file_selection = Some(content.to_vec());
        self
    }

    #[cfg(test)]
    fn with_test_http_response(
        mut self,
        url: &str,
        response: novahub_platform_api::HttpResponse,
    ) -> Self {
        self.test_http_responses.insert(url.to_owned(), response);
        self
    }

    #[cfg(test)]
    fn with_test_http_responses(
        mut self,
        responses: &BTreeMap<String, novahub_platform_api::HttpResponse>,
    ) -> Self {
        self.test_http_responses.clone_from(responses);
        self
    }

    fn issue_file_token(&self, session_id: &str, content: Vec<u8>) -> Result<String, String> {
        if content.len() > MAX_PLUGIN_FILE_BYTES || std::str::from_utf8(&content).is_err() {
            return Err("file_payload_invalid".into());
        }
        let mut files = self
            .issued_file_contents
            .lock()
            .map_err(|_| "file_handle_store_unavailable".to_owned())?;
        if files.len() >= MAX_PLUGIN_FILE_HANDLES {
            return Err("file_handle_limit_exceeded".into());
        }
        let mut random = [0_u8; 16];
        getrandom::fill(&mut random).map_err(|error| format!("file_token_failed:{error}"))?;
        let mut token = String::with_capacity(random.len() * 2);
        for byte in random {
            write!(&mut token, "{byte:02x}").expect("writing to String cannot fail");
        }
        let handle = HostFileHandle::issue(&self.plugin_id, session_id, &token);
        files.insert(Self::file_handle_key(&handle), content);
        Ok(token)
    }

    fn http_get_once(
        &self,
        url: &str,
        timeout: Duration,
    ) -> Result<novahub_platform_api::HttpResponse, PlatformError> {
        #[cfg(test)]
        if let Some(response) = self.test_http_responses.get(url) {
            return Ok(response.clone());
        }
        self.platform
            .http_get_once(url, MAX_PLUGIN_HTTP_BYTES, timeout)
    }

    fn execute_http_get(
        &self,
        identity: &PluginIdentity,
        initial_url: &str,
    ) -> PluginCapabilityResponse {
        let started = Instant::now();
        let mut current_url = initial_url.to_owned();
        for redirect_count in 0..=MAX_PLUGIN_HTTP_REDIRECTS {
            if started.elapsed() >= PLUGIN_HTTP_TOTAL_TIMEOUT {
                return PluginCapabilityResponse::Error("http_deadline_exceeded".into());
            }
            if let Err(error) = CapabilityBroker.authorize(
                identity,
                &self.grant,
                CapabilityRequest::Http {
                    origin: current_url.clone(),
                },
            ) {
                return PluginCapabilityResponse::Error(capability_denial_code(error).into());
            }
            let remaining = PLUGIN_HTTP_TOTAL_TIMEOUT.saturating_sub(started.elapsed());
            let response = match self.http_get_once(&current_url, remaining) {
                Ok(response) => response,
                Err(error) => {
                    return PluginCapabilityResponse::Error(platform_error_code(&error).into());
                }
            };
            if (300..400).contains(&response.status) {
                if redirect_count == MAX_PLUGIN_HTTP_REDIRECTS {
                    return PluginCapabilityResponse::Error("http_redirect_limit_exceeded".into());
                }
                let Some(location) = response.location.as_deref() else {
                    return PluginCapabilityResponse::Error(
                        "http_redirect_location_missing".into(),
                    );
                };
                let Ok(next_url) =
                    url::Url::parse(&current_url).and_then(|base| base.join(location))
                else {
                    return PluginCapabilityResponse::Error("http_redirect_invalid".into());
                };
                current_url = next_url.to_string();
                continue;
            }
            if !(200..300).contains(&response.status) {
                return PluginCapabilityResponse::Error("http_status_error".into());
            }
            if response.body.len() > MAX_PLUGIN_HTTP_BYTES {
                return PluginCapabilityResponse::Error("http_response_too_large".into());
            }
            return match String::from_utf8(response.body) {
                Ok(value) => PluginCapabilityResponse::Text(value),
                Err(_) => PluginCapabilityResponse::Error("http_response_not_utf8".into()),
            };
        }
        PluginCapabilityResponse::Error("http_redirect_limit_exceeded".into())
    }

    fn handle_clipboard_read(&self) -> PluginCapabilityResponse {
        match self.platform.read_clipboard() {
            Ok(ClipboardPayload::Text(text)) if text.len() <= MAX_PLUGIN_CAPABILITY_TEXT_BYTES => {
                PluginCapabilityResponse::Text(text)
            }
            Ok(ClipboardPayload::Text(_)) => {
                PluginCapabilityResponse::Error("clipboard_payload_too_large".into())
            }
            Ok(ClipboardPayload::Image(_)) => {
                PluginCapabilityResponse::Error("clipboard_image_unsupported".into())
            }
            Err(error) => PluginCapabilityResponse::Error(platform_error_code(&error).into()),
        }
    }

    fn handle_clipboard_write(&self, payload: Option<&str>) -> PluginCapabilityResponse {
        let Some(text) = payload else {
            return PluginCapabilityResponse::Error("clipboard_payload_missing".into());
        };
        match self.platform.write_clipboard_text(text) {
            Ok(()) => PluginCapabilityResponse::Unit,
            Err(error) => PluginCapabilityResponse::Error(platform_error_code(&error).into()),
        }
    }

    fn handle_files_select(&self, session_id: &str) -> PluginCapabilityResponse {
        #[cfg(test)]
        let selected = self.test_file_selection.clone().map_or_else(
            || self.platform.pick_text_file(MAX_PLUGIN_FILE_BYTES),
            |content| Ok(Some(content)),
        );
        #[cfg(not(test))]
        let selected = self.platform.pick_text_file(MAX_PLUGIN_FILE_BYTES);

        match selected {
            Ok(Some(content)) => match self.issue_file_token(session_id, content) {
                Ok(token) => PluginCapabilityResponse::Text(token),
                Err(error) => PluginCapabilityResponse::Error(error),
            },
            Ok(None) => PluginCapabilityResponse::Unit,
            Err(error) => PluginCapabilityResponse::Error(platform_error_code(&error).into()),
        }
    }

    fn handle_storage_put(
        &self,
        key: &str,
        value: &[u8],
        quota_bytes: u64,
    ) -> PluginCapabilityResponse {
        match self.storage_path.as_deref() {
            Some(path) => match Storage::open(path)
                .map_err(StorageInputError::Database)
                .and_then(|storage| {
                    storage.set_plugin_kv(
                        &self.plugin_id,
                        key,
                        value,
                        quota_bytes,
                        current_unix_timestamp(),
                    )
                }) {
                Ok(()) => PluginCapabilityResponse::Unit,
                Err(error) => {
                    PluginCapabilityResponse::Error(storage_capability_error_code(&error).into())
                }
            },
            None => PluginCapabilityResponse::Error("storage_adapter_unavailable".into()),
        }
    }

    fn handle_storage_get(&self, key: &str) -> PluginCapabilityResponse {
        match self.storage_path.as_deref() {
            Some(path) => match Storage::open(path)
                .map_err(StorageInputError::Database)
                .and_then(|storage| storage.plugin_kv(&self.plugin_id, key))
            {
                Ok(Some(value)) => PluginCapabilityResponse::Bytes(value),
                Ok(None) => PluginCapabilityResponse::Error("storage_key_not_found".into()),
                Err(error) => {
                    PluginCapabilityResponse::Error(storage_capability_error_code(&error).into())
                }
            },
            None => PluginCapabilityResponse::Error("storage_adapter_unavailable".into()),
        }
    }

    fn handle_files_read(&self, handle: &HostFileHandle) -> PluginCapabilityResponse {
        let content = self
            .issued_file_contents
            .lock()
            .ok()
            .and_then(|files| files.get(&Self::file_handle_key(handle)).cloned());
        let Some(content) = content else {
            return PluginCapabilityResponse::Error("capability_handle_mismatch".into());
        };
        match String::from_utf8(content) {
            Ok(value) => PluginCapabilityResponse::Text(value),
            Err(_) => PluginCapabilityResponse::Error("file_payload_invalid".into()),
        }
    }
}

impl PluginCapabilityHandler for HostPluginCapabilityHandler {
    fn handle(&self, request: PluginCapabilityRequest) -> PluginCapabilityResponse {
        if request.plugin_id != self.plugin_id || request.session_id.is_empty() {
            return PluginCapabilityResponse::Error("capability_identity_mismatch".into());
        }

        let identity = PluginIdentity::new(&request.plugin_id, &request.session_id);
        let broker_request = capability_request_for_host(&request);
        let authorized = match CapabilityBroker.authorize(&identity, &self.grant, broker_request) {
            Ok(authorized) => authorized,
            Err(error) => {
                return PluginCapabilityResponse::Error(capability_denial_code(error).into());
            }
        };

        match authorized {
            AuthorizedCapability::ClipboardRead => self.handle_clipboard_read(),
            AuthorizedCapability::ClipboardWrite => {
                self.handle_clipboard_write(request.payload.as_deref())
            }
            AuthorizedCapability::FilesSelect => self.handle_files_select(&request.session_id),
            AuthorizedCapability::StorageWrite { .. } => {
                // Keep the original quota preflight for compatibility with
                // existing plugins; typed values use StoragePut/Get below.
                PluginCapabilityResponse::Unit
            }
            AuthorizedCapability::StoragePut {
                key,
                value,
                quota_bytes,
            } => self.handle_storage_put(&key, &value, quota_bytes),
            AuthorizedCapability::StorageGet { key } => self.handle_storage_get(&key),
            AuthorizedCapability::Http { origin } => {
                let url = request.payload.as_deref().unwrap_or(origin.as_str());
                self.execute_http_get(&identity, url)
            }
            AuthorizedCapability::FilesRead { handle } => self.handle_files_read(&handle),
            AuthorizedCapability::FilesWrite { .. } => {
                PluginCapabilityResponse::Error("file_write_unsupported".into())
            }
            AuthorizedCapability::Logging => {
                PluginCapabilityResponse::Error("logging_adapter_unsupported".into())
            }
            AuthorizedCapability::Notification => {
                PluginCapabilityResponse::Error("notification_adapter_unsupported".into())
            }
        }
    }

    fn end_session(&self, plugin_id: &str, session_id: &str) {
        if plugin_id != self.plugin_id || session_id.is_empty() {
            return;
        }
        let prefix = format!("{plugin_id}\u{0}{session_id}\u{0}");
        if let Ok(mut files) = self.issued_file_contents.lock() {
            files.retain(|key, _| !key.starts_with(&prefix));
        }
    }
}

fn capability_request_for_host(request: &PluginCapabilityRequest) -> CapabilityRequest {
    match &request.capability {
        AuthorizedCapability::ClipboardRead => CapabilityRequest::ClipboardRead,
        AuthorizedCapability::ClipboardWrite => CapabilityRequest::ClipboardWrite,
        AuthorizedCapability::FilesSelect => CapabilityRequest::FilesSelect,
        AuthorizedCapability::FilesRead { handle } => CapabilityRequest::FilesRead {
            handle: handle.clone(),
        },
        AuthorizedCapability::FilesWrite { handle } => CapabilityRequest::FilesWrite {
            handle: handle.clone(),
        },
        AuthorizedCapability::Http { origin } => CapabilityRequest::Http {
            origin: request.payload.clone().unwrap_or_else(|| origin.clone()),
        },
        AuthorizedCapability::StorageWrite { total_bytes } => CapabilityRequest::StorageWrite {
            total_bytes: *total_bytes,
        },
        AuthorizedCapability::StoragePut { key, value, .. } => CapabilityRequest::StoragePut {
            key: key.clone(),
            value: value.clone(),
        },
        AuthorizedCapability::StorageGet { key } => {
            CapabilityRequest::StorageGet { key: key.clone() }
        }
        AuthorizedCapability::Logging => CapabilityRequest::Logging,
        AuthorizedCapability::Notification => CapabilityRequest::Notification,
    }
}

const fn capability_denial_code(error: CapabilityDenied) -> &'static str {
    match error {
        CapabilityDenied::NotGranted => "capability_not_granted",
        CapabilityDenied::ScopeExceeded => "capability_scope_exceeded",
        CapabilityDenied::HandleMismatch => "capability_handle_mismatch",
        CapabilityDenied::InvalidOrigin => "capability_origin_invalid",
        CapabilityDenied::InvalidInput => "capability_input_invalid",
    }
}

fn platform_error_code(error: &PlatformError) -> &'static str {
    match error {
        PlatformError::Unavailable(_) => "platform_capability_unavailable",
        PlatformError::InvalidInput => "platform_input_invalid",
        PlatformError::LimitExceeded => "http_response_too_large",
        PlatformError::Timeout => "http_deadline_exceeded",
        PlatformError::Io(_) => "platform_io_failed",
    }
}

fn storage_capability_error_code(error: &StorageInputError) -> &'static str {
    match error {
        StorageInputError::InvalidInput(_) => "capability_input_invalid",
        StorageInputError::QuotaExceeded => "capability_scope_exceeded",
        StorageInputError::NotFound => "storage_key_not_found",
        StorageInputError::Database(_) => "storage_adapter_failed",
    }
}

impl HostPlatform {
    fn probe() -> Self {
        #[cfg(windows)]
        {
            Self::Windows(WindowsAdapter::probe())
        }
        #[cfg(target_os = "macos")]
        {
            Self::Macos(MacosAdapter::probe())
        }
        #[cfg(not(any(windows, target_os = "macos")))]
        Self::Unsupported
    }

    fn supports_command(&self, command_id: &str) -> bool {
        let capability = match command_id {
            "apps.launch" => Some(novahub_platform_api::Capability::ApplicationLaunch),
            "files.search" => Some(novahub_platform_api::Capability::FileSearch),
            "clipboard.history" | "clipboard.copy" => {
                Some(novahub_platform_api::Capability::Clipboard)
            }
            "system.command" => Some(novahub_platform_api::Capability::SystemCommand),
            "windows.layout" => Some(novahub_platform_api::Capability::WindowManagement),
            "hotkey.configure" => Some(novahub_platform_api::Capability::GlobalHotkey),
            _ => None,
        };
        match capability {
            Some(capability) => self.has_capability(capability),
            None => true,
        }
    }

    fn has_capability(&self, capability: novahub_platform_api::Capability) -> bool {
        match self {
            #[cfg(windows)]
            Self::Windows(adapter) => {
                adapter.capabilities().status(capability)
                    == novahub_platform_api::CapabilityStatus::Available
            }
            #[cfg(target_os = "macos")]
            Self::Macos(adapter) => {
                adapter.capabilities().status(capability)
                    == novahub_platform_api::CapabilityStatus::Available
            }
            #[cfg(not(any(windows, target_os = "macos")))]
            Self::Unsupported => false,
        }
    }
}

impl PlatformServices for HostPlatform {
    fn open_path(&self, path: &std::path::Path) -> Result<(), PlatformError> {
        match self {
            #[cfg(windows)]
            Self::Windows(adapter) => adapter.open_path(path),
            #[cfg(target_os = "macos")]
            Self::Macos(adapter) => adapter.open_path(path),
            #[cfg(not(any(windows, target_os = "macos")))]
            Self::Unsupported => Err(PlatformError::Unavailable(
                novahub_platform_api::Capability::FileOpen,
            )),
        }
    }

    fn launch_application(&self, path: &std::path::Path) -> Result<(), PlatformError> {
        match self {
            #[cfg(windows)]
            Self::Windows(adapter) => adapter.launch_application(path),
            #[cfg(target_os = "macos")]
            Self::Macos(adapter) => adapter.launch_application(path),
            #[cfg(not(any(windows, target_os = "macos")))]
            Self::Unsupported => Err(PlatformError::Unavailable(
                novahub_platform_api::Capability::ApplicationLaunch,
            )),
        }
    }

    fn write_clipboard_text(&self, text: &str) -> Result<(), PlatformError> {
        match self {
            #[cfg(windows)]
            Self::Windows(adapter) => adapter.write_clipboard_text(text),
            #[cfg(target_os = "macos")]
            Self::Macos(adapter) => adapter.write_clipboard_text(text),
            #[cfg(not(any(windows, target_os = "macos")))]
            Self::Unsupported => Err(PlatformError::Unavailable(
                novahub_platform_api::Capability::Clipboard,
            )),
        }
    }

    fn execute_system_command(&self, command: SystemCommand) -> Result<(), PlatformError> {
        match self {
            #[cfg(windows)]
            Self::Windows(adapter) => adapter.execute_system_command(command),
            #[cfg(target_os = "macos")]
            Self::Macos(adapter) => adapter.execute_system_command(command),
            #[cfg(not(any(windows, target_os = "macos")))]
            Self::Unsupported => Err(PlatformError::Unavailable(
                novahub_platform_api::Capability::SystemCommand,
            )),
        }
    }

    fn search_files(
        &self,
        root: &std::path::Path,
        query: &str,
        limit: usize,
    ) -> Result<Vec<FileSearchResult>, PlatformError> {
        match self {
            #[cfg(windows)]
            Self::Windows(adapter) => adapter.search_files(root, query, limit),
            #[cfg(target_os = "macos")]
            Self::Macos(adapter) => adapter.search_files(root, query, limit),
            #[cfg(not(any(windows, target_os = "macos")))]
            Self::Unsupported => Err(PlatformError::Unavailable(
                novahub_platform_api::Capability::FileSearch,
            )),
        }
    }

    fn discover_applications(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<ApplicationInfo>, PlatformError> {
        match self {
            #[cfg(windows)]
            Self::Windows(adapter) => adapter.discover_applications(query, limit),
            #[cfg(target_os = "macos")]
            Self::Macos(adapter) => adapter.discover_applications(query, limit),
            #[cfg(not(any(windows, target_os = "macos")))]
            Self::Unsupported => Err(PlatformError::Unavailable(
                novahub_platform_api::Capability::ApplicationLaunch,
            )),
        }
    }

    fn read_clipboard(&self) -> Result<ClipboardPayload, PlatformError> {
        match self {
            #[cfg(windows)]
            Self::Windows(adapter) => adapter.read_clipboard(),
            #[cfg(target_os = "macos")]
            Self::Macos(adapter) => adapter.read_clipboard(),
            #[cfg(not(any(windows, target_os = "macos")))]
            Self::Unsupported => Err(PlatformError::Unavailable(
                novahub_platform_api::Capability::Clipboard,
            )),
        }
    }

    fn pick_text_file(&self, max_bytes: usize) -> Result<Option<Vec<u8>>, PlatformError> {
        match self {
            #[cfg(windows)]
            Self::Windows(adapter) => adapter.pick_text_file(max_bytes),
            #[cfg(target_os = "macos")]
            Self::Macos(adapter) => adapter.pick_text_file(max_bytes),
            #[cfg(not(any(windows, target_os = "macos")))]
            Self::Unsupported => Err(PlatformError::Unavailable(
                novahub_platform_api::Capability::FileOpen,
            )),
        }
    }

    fn http_get_once(
        &self,
        url: &str,
        max_bytes: usize,
        timeout: std::time::Duration,
    ) -> Result<novahub_platform_api::HttpResponse, PlatformError> {
        match self {
            #[cfg(windows)]
            Self::Windows(adapter) => adapter.http_get_once(url, max_bytes, timeout),
            #[cfg(target_os = "macos")]
            Self::Macos(adapter) => adapter.http_get_once(url, max_bytes, timeout),
            #[cfg(not(any(windows, target_os = "macos")))]
            Self::Unsupported => Err(PlatformError::Unavailable(
                novahub_platform_api::Capability::Http,
            )),
        }
    }

    fn credential_key(&self, namespace: &str) -> Result<[u8; 32], PlatformError> {
        match self {
            #[cfg(windows)]
            Self::Windows(adapter) => adapter.credential_key(namespace),
            #[cfg(target_os = "macos")]
            Self::Macos(adapter) => adapter.credential_key(namespace),
            #[cfg(not(any(windows, target_os = "macos")))]
            Self::Unsupported => Err(PlatformError::Unavailable(
                novahub_platform_api::Capability::CredentialStore,
            )),
        }
    }
}

impl NovaHubApp {
    #[must_use]
    ///
    /// # Panics
    ///
    /// Panics only if the bundled in-memory `SQLite` schema cannot initialize.
    pub fn new() -> Self {
        Self::try_new().expect("NovaHub in-memory storage must initialize")
    }

    /// Creates a host model with isolated in-memory storage.
    ///
    /// # Errors
    ///
    /// Returns a storage initialization error.
    pub fn try_new() -> Result<Self, String> {
        Self::from_storage(
            Storage::open_in_memory().map_err(|error| error.to_string())?,
            None,
            None,
        )
    }

    /// Opens the default per-user database used by the desktop application.
    ///
    /// `NOVAHUB_DATA_DIR` is intentionally supported for portable installs,
    /// tests, and package smoke checks. The normal platform locations keep
    /// user data out of the application directory.
    ///
    /// # Errors
    ///
    /// Returns an error when the data directory cannot be created or `SQLite`
    /// cannot initialize its schema.
    pub fn open_default() -> Result<Self, String> {
        let data_dir = default_data_dir()?;
        std::fs::create_dir_all(&data_dir).map_err(|error| error.to_string())?;
        Self::open_with_plugin_root(data_dir.join("novahub.sqlite3"), data_dir.join("plugins"))
    }

    fn from_storage(
        storage: Storage,
        plugin_root: Option<std::path::PathBuf>,
        storage_path: Option<std::path::PathBuf>,
    ) -> Result<Self, String> {
        let platform = HostPlatform::probe();
        let mut commands = builtin_commands()
            .into_iter()
            .filter(|command| platform.supports_command(command.id.as_str()))
            .collect::<Vec<_>>();
        let installed =
            installed_plugin_commands(plugin_root.as_deref(), MAX_INSTALLED_PLUGIN_COMMANDS);
        commands.extend(installed.commands);
        storage
            .replace_commands(&commands)
            .map_err(|error| error.to_string())?;
        let saved_pet_position = load_pet_position(&storage);
        let clipboard_policy = load_clipboard_policy(&storage);
        let mut clipboard = ClipboardVault::new(HostCredentialStore(platform.clone()));
        clipboard.set_policy(clipboard_policy);
        let mut app = Self {
            commands,
            plugin_interactions: installed.interactions,
            shell: ShellState::default(),
            storage,
            storage_path,
            pet_controller: PetController::new(PetId::new("nova")),
            pet_renderer: PetRenderer::new("builtin://nova/idle"),
            active_pet_id: "nova".to_owned(),
            pet_frame_assets: BTreeMap::new(),
            pet_shelf: default_pet_shelf(),
            pet_placement: None,
            saved_pet_position,
            clipboard,
            clipboard_poller: ClipboardPoller::default(),
            clipboard_paused: false,
            platform,
            application_cache: RefCell::new(None),
            application_usage: RefCell::new(std::collections::BTreeMap::new()),
            search_matcher: RefCell::new(CommandMatcher::default()),
            plugin_root,
            pending_confirmation: None,
            diagnostics: RefCell::new(DiagnosticEventBuffer::default()),
            #[cfg(test)]
            test_file_selection: None,
            #[cfg(test)]
            test_http_responses: BTreeMap::new(),
        };
        app.restore_active_pet_setting();
        Ok(app)
    }

    /// Opens a persistent host database at `path`.
    ///
    /// # Errors
    ///
    /// Returns a storage initialization error.
    pub fn open(path: impl AsRef<std::path::Path>) -> Result<Self, String> {
        let path = path.as_ref();
        let storage = Storage::open(path).map_err(|error| error.to_string())?;
        let plugin_root = path.parent().map(|parent| parent.join("plugins"));
        Self::from_storage(storage, plugin_root, Some(path.to_owned()))
    }

    /// Opens a persistent database with an explicit installed-plugin root.
    ///
    /// Keeping the root explicit makes portable installs and tests independent
    /// from the database filename while production still uses one user data
    /// directory for both `SQLite` and versioned plugin packages.
    ///
    /// # Errors
    ///
    /// Returns a storage initialization error.
    pub fn open_with_plugin_root(
        path: impl AsRef<std::path::Path>,
        plugin_root: impl AsRef<std::path::Path>,
    ) -> Result<Self, String> {
        let path = path.as_ref();
        let storage = Storage::open(path).map_err(|error| error.to_string())?;
        Self::from_storage(
            storage,
            Some(plugin_root.as_ref().to_owned()),
            Some(path.to_owned()),
        )
    }

    #[cfg(test)]
    fn with_test_file_selection(mut self, content: &[u8]) -> Self {
        self.test_file_selection = Some(content.to_vec());
        self
    }

    #[cfg(test)]
    fn with_test_http_response(
        mut self,
        url: &str,
        response: novahub_platform_api::HttpResponse,
    ) -> Self {
        self.test_http_responses.insert(url.to_owned(), response);
        self
    }

    #[must_use]
    pub fn search(&self, query: impl AsRef<str>) -> Vec<CommandDescriptor> {
        let query = query.as_ref().trim();
        let normalized_query = query.to_ascii_lowercase();
        let mut results = self
            .search_matcher
            .borrow_mut()
            .match_commands(query, &self.commands);
        if !query.is_empty() {
            let mut seen_names = BTreeSet::new();
            let compact_query = compact_application_name(query);
            let initial_query = application_initials(query);
            // Search stays in memory while the Shell's discovery worker loads
            // the platform snapshot in the background.
            let mut applications = self.application_cache.borrow().clone().unwrap_or_default();
            applications.sort_by(|left, right| {
                let left_usage = self
                    .application_usage
                    .borrow()
                    .get(&left.name)
                    .copied()
                    .unwrap_or_default();
                let right_usage = self
                    .application_usage
                    .borrow()
                    .get(&right.name)
                    .copied()
                    .unwrap_or_default();
                right_usage.cmp(&left_usage).then_with(|| {
                    left.name
                        .to_ascii_lowercase()
                        .cmp(&right.name.to_ascii_lowercase())
                })
            });
            let matching_applications = applications
                .into_iter()
                .filter(|application| {
                    application_matches(
                        application,
                        &normalized_query,
                        &compact_query,
                        &initial_query,
                    )
                })
                .filter(|application| seen_names.insert(application.name.to_ascii_lowercase()))
                .collect::<Vec<_>>();
            let promote_applications = matching_applications.iter().any(|application| {
                application_has_exact_alias(
                    application,
                    &normalized_query,
                    &compact_query,
                    &initial_query,
                )
            });
            let application_results = matching_applications
                .into_iter()
                .map(application_command)
                .collect::<Vec<_>>();
            if promote_applications {
                let mut promoted = application_results;
                promoted.extend(results);
                results = promoted;
            } else {
                results.extend(application_results);
            }
        }
        results.truncate(50);
        results
    }

    /// Searches the host command snapshot while restricting results to one
    /// provider family. The filter is applied after normal ranking so the
    /// unfiltered search remains the single source of ordering semantics.
    #[must_use]
    pub fn search_provider(
        &self,
        query: impl AsRef<str>,
        provider: Option<&str>,
    ) -> Vec<CommandDescriptor> {
        let Some(provider) = provider.filter(|value| !value.is_empty() && *value != "all") else {
            return self.search(query);
        };
        self.search(query)
            .into_iter()
            .filter(|command| command_belongs_to_provider(command.id.as_str(), provider))
            .collect()
    }

    /// Returns the interaction registered with the host command snapshot.
    ///
    /// The registration is derived from the active manifest during app
    /// startup, so the Shell can choose its execution path without parsing
    /// plugin metadata or guessing from the command name.
    #[must_use]
    pub fn plugin_command_interaction(&self, command_ref: &str) -> Option<PluginInteraction> {
        self.plugin_interactions.get(command_ref).copied()
    }

    /// Discards the bounded application snapshot so the next search refreshes
    /// it from the platform index.
    pub fn refresh_applications(&self) {
        self.application_cache.borrow_mut().take();
    }

    /// Starts one bounded application-index read outside the UI thread.
    ///
    /// The worker owns only a cloned platform adapter and returns a bounded
    /// snapshot. It never receives the app model or a UI handle.
    ///
    /// # Errors
    ///
    /// Returns an error when the worker thread cannot be created.
    pub fn start_application_discovery(
        &self,
    ) -> Result<Receiver<Result<Vec<ApplicationInfo>, String>>, String> {
        let platform = self.platform.clone();
        let (sender, receiver) = mpsc::sync_channel(1);
        std::thread::Builder::new()
            .name("novahub-application-discovery".into())
            .spawn(move || {
                let result = platform
                    .discover_applications("", MAX_APPLICATION_SNAPSHOT)
                    .map_err(|error| error.to_string());
                let _ = sender.send(result);
            })
            .map_err(|error| error.to_string())?;
        Ok(receiver)
    }

    /// Starts one bounded indexed file search outside the UI thread.
    ///
    /// The platform adapter owns the search process and returns only bounded
    /// paths. The app model is updated by the Shell when the worker replies.
    ///
    /// # Errors
    ///
    /// Returns an error when the worker thread cannot be created.
    pub fn start_file_search(
        &self,
        root: std::path::PathBuf,
        query: String,
        limit: usize,
    ) -> Result<Receiver<Result<Vec<FileSearchResult>, String>>, String> {
        let platform = self.platform.clone();
        let (sender, receiver) = mpsc::sync_channel(1);
        std::thread::Builder::new()
            .name("novahub-file-search".into())
            .spawn(move || {
                let result = platform
                    .search_files(&root, &query, limit.min(50))
                    .map_err(|error| error.to_string());
                let _ = sender.send(result);
            })
            .map_err(|error| error.to_string())?;
        Ok(receiver)
    }

    /// Publishes a worker-produced application snapshot at the host boundary.
    pub fn apply_application_snapshot(&self, mut applications: Vec<ApplicationInfo>) {
        applications.truncate(MAX_APPLICATION_SNAPSHOT);
        *self.application_cache.borrow_mut() = Some(applications);
    }

    #[must_use]
    pub fn has_application_snapshot(&self) -> bool {
        self.application_cache.borrow().is_some()
    }

    fn application_snapshot(&self) -> Result<Vec<ApplicationInfo>, PlatformError> {
        if self.application_cache.borrow().is_none() {
            let applications = self
                .platform
                .discover_applications("", MAX_APPLICATION_SNAPSHOT)?;
            *self.application_cache.borrow_mut() = Some(applications);
        }
        Ok(self.application_cache.borrow().clone().unwrap_or_default())
    }

    /// Launches the exact application name resolved by the platform index.
    ///
    /// # Errors
    ///
    /// Returns an unavailable-host error when application discovery or launch
    /// is unsupported, or an unknown-command error when the name is not found.
    pub fn launch_application_by_name(
        &self,
        name: impl AsRef<str>,
    ) -> Result<String, BuiltinActionError> {
        let name = name.as_ref().trim();
        if name.is_empty() {
            return Err(BuiltinActionError::MissingArgument);
        }
        let applications = self
            .application_snapshot()
            .map_err(|_| BuiltinActionError::UnsupportedOnHost)?;
        let normalized_name = name.to_ascii_lowercase();
        let compact_name = compact_application_name(name);
        let initial_name = application_initials(name);
        let application = applications
            .into_iter()
            .find(|application| {
                application_matches(application, &normalized_name, &compact_name, &initial_name)
            })
            .ok_or(BuiltinActionError::UnknownCommand)?;
        let path = application.launch_path.to_string_lossy().into_owned();
        let result = self.execute_builtin(&Action::invoke("apps.launch", [path]), false);
        if result.is_ok() {
            let key = application.name.to_ascii_lowercase();
            let mut usage = self.application_usage.borrow_mut();
            if !usage.contains_key(&key) && usage.len() >= MAX_APPLICATION_USAGE_ENTRIES {
                let evicted = usage
                    .iter()
                    .min_by_key(|(_, count)| *count)
                    .map(|(name, _)| name.clone());
                if let Some(evicted) = evicted {
                    usage.remove(&evicted);
                }
            }
            let count = usage.entry(key).or_default();
            *count = count.saturating_add(1);
        }
        result
    }

    /// Executes a host-owned built-in action without crossing the plugin
    /// process boundary.
    ///
    /// # Errors
    ///
    /// Returns the bounded built-in action error from the provider layer.
    pub fn execute_builtin(
        &self,
        action: &Action,
        confirmed: bool,
    ) -> Result<String, BuiltinActionError> {
        execute_builtin_with_host(action, confirmed, &self.storage, &self.platform)
    }

    /// Records a destructive action until the user explicitly confirms it.
    pub fn request_confirmation(&mut self, action: Action) {
        self.pending_confirmation = Some(action);
    }

    #[must_use]
    pub fn has_pending_confirmation(&self) -> bool {
        self.pending_confirmation.is_some()
    }

    /// Executes and clears the pending action after an explicit confirmation.
    ///
    /// # Errors
    ///
    /// Returns the same built-in action error as a normal execution.
    pub fn confirm_pending_action(&mut self) -> Result<String, BuiltinActionError> {
        let Some(action) = self.pending_confirmation.take() else {
            return Err(BuiltinActionError::UnknownCommand);
        };
        self.execute_builtin(&action, true)
    }

    pub fn cancel_pending_confirmation(&mut self) -> bool {
        self.pending_confirmation.take().is_some()
    }

    /// Runs one development-only Component session through the sibling Host.
    ///
    /// This method deliberately accepts an explicit component path. Production
    /// callers should resolve that path from the Plugin Manager's active pointer
    /// after signature and manifest validation; the app never loads Wasmtime.
    ///
    /// # Errors
    ///
    /// Returns an IPC, serialization, or Plugin Host error.
    pub fn run_component_fixture(
        &self,
        session_id: &str,
        component_path: impl AsRef<std::path::Path>,
        input: &str,
    ) -> Result<PluginFixtureRun, PluginHostError> {
        let supervisor = PluginHostSupervisor::from_default_path()?;
        supervisor.run_component_session(session_id, component_path, input)
    }

    /// Runs the enabled installed version selected by the host-owned active
    /// pointer. The app never compiles or instantiates Wasmtime; it only passes
    /// the validated component path to the short-lived sibling Host.
    ///
    /// # Errors
    ///
    /// Returns a bounded path/activation error or a Plugin Host error string.
    pub fn run_active_plugin(
        &self,
        session_id: &str,
        plugin_id: &str,
        input: &str,
    ) -> Result<PluginFixtureRun, String> {
        let started = Instant::now();
        let execution = match self.prepare_active_plugin_execution(plugin_id) {
            Ok(execution) => execution,
            Err(error) => {
                self.record_plugin_error(plugin_id, DiagnosticPhase::Run, started, &error);
                return Err(error);
            }
        };
        let result = execution.run_view(session_id, input);
        self.record_plugin_result(
            plugin_id,
            DiagnosticPhase::Run,
            started,
            result.as_ref().ok().map(|run| run.host_pid),
            &result,
        );
        result.map_err(|error| error.to_string())
    }

    /// Runs a manifest-declared one-shot command through the sibling Host.
    ///
    /// The manifest is resolved from the same active pointer as the Component
    /// path, so callers cannot accidentally execute a command from a stale or
    /// disabled version. The result remains a bounded JSON payload owned by
    /// the host UI layer.
    ///
    /// # Errors
    ///
    /// Returns an activation, interaction, IPC, or Plugin Host error string.
    pub fn run_active_plugin_one_shot(
        &self,
        session_id: &str,
        plugin_id: &str,
        command_id: &str,
        input: &str,
    ) -> Result<serde_json::Value, String> {
        let started = Instant::now();
        if self.active_plugin_command_interaction(plugin_id, command_id)?
            != PluginInteraction::OneShot
        {
            let error = "plugin command is not declared as one-shot".to_owned();
            self.record_plugin_error(plugin_id, DiagnosticPhase::Run, started, &error);
            return Err(error);
        }
        let execution = match self.prepare_active_plugin_execution(plugin_id) {
            Ok(execution) => execution,
            Err(error) => {
                self.record_plugin_error(plugin_id, DiagnosticPhase::Run, started, &error);
                return Err(error);
            }
        };
        let result = execution
            .run_one_shot(session_id, command_id, input)
            .map_err(|error| error.to_string());
        self.record_plugin_result(plugin_id, DiagnosticPhase::Run, started, None, &result);
        result
    }

    /// Returns a bounded snapshot of host-owned plugin lifecycle diagnostics.
    #[must_use]
    pub fn diagnostic_events(&self) -> Vec<DiagnosticEvent> {
        self.diagnostics.borrow().snapshot()
    }

    /// Returns the user-visible summary used by the native diagnostics view.
    #[must_use]
    pub fn diagnostic_summary(&self) -> String {
        self.diagnostics.borrow().summary()
    }

    /// Creates a previewable local diagnostic bundle without writing user data.
    ///
    /// # Errors
    ///
    /// Returns an error if the bounded JSON preview cannot be serialized.
    pub fn diagnostic_preview(&self) -> Result<String, String> {
        self.diagnostics
            .borrow()
            .preview_json()
            .map_err(|error| error.to_string())
    }

    /// Creates a reduced preview selected by the user before export.
    ///
    /// # Errors
    ///
    /// Returns an error if the bounded JSON preview cannot be serialized.
    pub fn diagnostic_preview_with_options(
        &self,
        options: DiagnosticExportOptions,
    ) -> Result<String, String> {
        self.diagnostics
            .borrow()
            .preview_json_with_options(options)
            .map_err(|error| error.to_string())
    }

    /// Writes one explicitly requested diagnostic bundle to a local JSON file.
    ///
    /// The caller must obtain the destination through host UI. Plugins cannot
    /// invoke this method or choose the path.
    ///
    /// # Errors
    ///
    /// Returns an error for a non-JSON destination, missing parent directory,
    /// serialization failure, or filesystem write failure.
    pub fn export_diagnostics(
        &self,
        destination: &std::path::Path,
        options: DiagnosticExportOptions,
    ) -> Result<usize, String> {
        let is_json = destination
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("json"));
        if !is_json || destination.file_name().is_none() {
            return Err("diagnostic export destination must be a JSON file".into());
        }
        let parent = destination
            .parent()
            .ok_or_else(|| "diagnostic export destination has no parent".to_owned())?;
        if !parent.is_dir() {
            return Err("diagnostic export parent directory does not exist".into());
        }

        let preview = self.diagnostic_preview_with_options(options)?;
        std::fs::write(destination, preview.as_bytes()).map_err(|error| error.to_string())?;
        Ok(preview.len())
    }

    /// Clears only the in-memory diagnostic ring; plugin data and settings are
    /// not touched.
    pub fn clear_diagnostics(&self) {
        self.diagnostics.replace(DiagnosticEventBuffer::default());
    }

    fn record_plugin_result<T, E: std::fmt::Display>(
        &self,
        plugin_id: &str,
        phase: DiagnosticPhase,
        started: Instant,
        host_pid: Option<u32>,
        result: &Result<T, E>,
    ) {
        let request_id = format!("plugin:{plugin_id}:{}", phase.as_str());
        let duration_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        let event = match result {
            Ok(_) => DiagnosticEvent::success(request_id, phase),
            Err(error) => DiagnosticEvent::failure(request_id, phase, error.to_string()),
        }
        .with_plugin(plugin_id)
        .with_duration_ms(duration_ms);
        let event = match host_pid {
            Some(host_pid) => event.with_host_pid(host_pid),
            None => event,
        };
        self.diagnostics.borrow_mut().push(event);
    }

    fn record_plugin_error(
        &self,
        plugin_id: &str,
        phase: DiagnosticPhase,
        started: Instant,
        error: &str,
    ) {
        let result: Result<(), &str> = Err(error);
        self.record_plugin_result(plugin_id, phase, started, None, &result);
    }

    /// Returns the interaction declared for an enabled active plugin command.
    ///
    /// Keeping this lookup in the host model prevents the Shell from making a
    /// second, potentially divergent copy of manifest command metadata.
    ///
    /// # Errors
    ///
    /// Returns an activation, manifest, or unknown-command error.
    pub fn active_plugin_command_interaction(
        &self,
        plugin_id: &str,
        command_id: &str,
    ) -> Result<PluginInteraction, String> {
        let manifest = self.active_plugin_manifest(plugin_id)?;
        manifest
            .commands()
            .iter()
            .find(|command| command.id == command_id)
            .map(|command| command.interaction)
            .ok_or_else(|| format!("unknown plugin command: {command_id}"))
    }

    /// Prepares a sendable execution context from the enabled active pointer.
    ///
    /// Installation approval initializes the persistent user grant once.
    /// Later executions reuse the stored grant, so revocation survives process
    /// restarts and newly declared permissions are not implicitly added.
    ///
    /// # Errors
    ///
    /// Returns an activation or manifest error.
    pub fn prepare_active_plugin_execution(
        &self,
        plugin_id: &str,
    ) -> Result<ActivePluginExecution, String> {
        let component_path = self.active_plugin_component(plugin_id)?;
        let manifest = Self::active_plugin_manifest_from_component(plugin_id, &component_path)?;
        let declared = manifest.declared_permissions();
        let user_grant = self.load_or_initialize_plugin_user_grant(plugin_id, declared)?;
        let grant = EffectiveGrant::from_layers(declared, &user_grant, declared);
        let handler =
            HostPluginCapabilityHandler::new(plugin_id, grant.clone(), self.platform.clone())
                .with_storage_path(self.storage_path.clone());
        #[cfg(test)]
        let handler = if let Some(content) = self.test_file_selection.as_deref() {
            handler.with_test_file_selection(content)
        } else {
            handler
        };
        #[cfg(test)]
        let handler = handler.with_test_http_responses(&self.test_http_responses);
        let capability_handler: Arc<dyn PluginCapabilityHandler> = Arc::new(handler);
        Ok(ActivePluginExecution {
            plugin_id: plugin_id.to_owned(),
            component_path,
            grant,
            capability_handler,
        })
    }

    /// Replaces the persisted user grant with the permissions declared by the
    /// currently active plugin version.
    ///
    /// This is the explicit approval path after a user has reviewed an install
    /// or update permission diff. It never grants capabilities outside the
    /// active manifest.
    ///
    /// # Errors
    ///
    /// Returns an activation, manifest, serialization, or storage error.
    pub fn approve_declared_plugin_permissions(&self, plugin_id: &str) -> Result<(), String> {
        let declared = self.active_plugin_manifest(plugin_id)?;
        let normalized = declared
            .declared_permissions()
            .intersect(declared.declared_permissions());
        self.persist_plugin_user_grant(plugin_id, &normalized)
    }

    /// Persists an explicit empty user grant for the active plugin.
    ///
    /// Revocation uses a present empty record instead of deleting the row, so
    /// the next execution cannot mistake it for a first-install approval.
    ///
    /// # Errors
    ///
    /// Returns an activation, manifest, serialization, or storage error.
    pub fn revoke_plugin_permissions(&self, plugin_id: &str) -> Result<(), String> {
        self.active_plugin_manifest(plugin_id)?;
        self.persist_plugin_user_grant(plugin_id, &DeclaredPermissions::default())
    }

    /// Removes one declared capability from the persisted user grant.
    ///
    /// Other granted capabilities keep their normalized scopes. Re-approving
    /// the active declaration is always a separate explicit operation.
    ///
    /// # Errors
    ///
    /// Returns an activation, manifest, unknown-capability, serialization, or
    /// storage error.
    pub fn revoke_plugin_capability(
        &self,
        plugin_id: &str,
        capability: Capability,
    ) -> Result<(), String> {
        let manifest = self.active_plugin_manifest(plugin_id)?;
        let declared = manifest.declared_permissions();
        if !declared.contains(capability) {
            return Err(format!(
                "plugin does not declare capability: {}",
                capability.as_str()
            ));
        }
        let current = self.load_or_initialize_plugin_user_grant(plugin_id, declared)?;
        self.persist_plugin_user_grant(plugin_id, &current.without(capability))
    }

    /// Returns the active declaration and the currently granted intersection.
    ///
    /// Reading the snapshot initializes the first-install grant when no record
    /// exists, matching the execution path without starting Plugin Host.
    ///
    /// # Errors
    ///
    /// Returns an activation, manifest, persisted-data, or storage error.
    pub fn active_plugin_permission_snapshot(
        &self,
        plugin_id: &str,
    ) -> Result<PluginPermissionSnapshot, String> {
        let manifest = self.active_plugin_manifest(plugin_id)?;
        let declared = manifest.declared_permissions();
        let user_grant = self.load_or_initialize_plugin_user_grant(plugin_id, declared)?;
        let granted = EffectiveGrant::from_layers(declared, &user_grant, declared);
        Ok(PluginPermissionSnapshot {
            declared: capability_names(declared),
            granted: capability_names(granted.permissions()),
        })
    }

    fn load_or_initialize_plugin_user_grant(
        &self,
        plugin_id: &str,
        declared: &DeclaredPermissions,
    ) -> Result<DeclaredPermissions, String> {
        let stored = self
            .storage
            .plugin_user_grant(plugin_id)
            .map_err(|error| error.to_string())?;
        if let Some(json) = stored {
            return serde_json::from_str(&json)
                .map_err(|_| "persisted plugin user grant is invalid".to_owned());
        }

        let initial_grant = declared.intersect(declared);
        self.persist_plugin_user_grant(plugin_id, &initial_grant)?;
        Ok(initial_grant)
    }

    fn persist_plugin_user_grant(
        &self,
        plugin_id: &str,
        grant: &DeclaredPermissions,
    ) -> Result<(), String> {
        let json = serde_json::to_string(grant)
            .map_err(|_| "plugin user grant serialization failed".to_owned())?;
        self.storage
            .set_plugin_user_grant(plugin_id, &json, current_unix_timestamp())
            .map_err(|error| error.to_string())
    }

    fn active_plugin_manifest(&self, plugin_id: &str) -> Result<PluginManifest, String> {
        let component = self.active_plugin_component(plugin_id)?;
        Self::active_plugin_manifest_from_component(plugin_id, &component)
    }

    fn active_plugin_manifest_from_component(
        plugin_id: &str,
        component: &std::path::Path,
    ) -> Result<PluginManifest, String> {
        let manifest_path = component
            .parent()
            .ok_or_else(|| "active plugin has no version directory".to_owned())?
            .join("novahub.toml");
        let manifest_text = std::fs::read_to_string(&manifest_path)
            .map_err(|error| format!("failed to read active plugin manifest: {error}"))?;
        let manifest = parse_manifest_toml(&manifest_text)
            .map_err(|error| format!("active plugin manifest is invalid: {error}"))?;
        if manifest.id != plugin_id {
            return Err("active plugin manifest ID does not match its pointer".to_owned());
        }
        Ok(manifest)
    }

    /// Resolves the validated component path for an enabled active plugin.
    ///
    /// The path can be handed to a background Plugin Host task without
    /// moving the non-`Send` application model off the Slint event loop.
    ///
    /// # Errors
    ///
    /// Returns a bounded activation error when the plugin root, pointer, or
    /// component is missing or disabled.
    pub fn active_plugin_component(&self, plugin_id: &str) -> Result<std::path::PathBuf, String> {
        let root = self
            .plugin_root
            .as_deref()
            .ok_or_else(|| "installed plugin root is not configured".to_owned())?;
        let active = read_active_plugin(root, plugin_id).map_err(|error| format!("{error:?}"))?;
        if !active.enabled {
            return Err("plugin is disabled".to_owned());
        }
        let component = active.path.join("plugin.wasm");
        if !component.is_file() {
            return Err("active plugin has no plugin.wasm".to_owned());
        }
        Ok(component)
    }

    #[must_use]
    pub const fn shell(&self) -> ShellState {
        self.shell
    }

    /// Stores the user theme in the host database.
    ///
    /// # Errors
    ///
    /// Returns a storage error string when the write fails.
    pub fn set_theme(&self, theme: &str) -> Result<(), String> {
        self.storage
            .set_setting("theme", theme)
            .map_err(|error| error.to_string())
    }

    /// Reads the user theme from the host database.
    ///
    /// # Errors
    ///
    /// Returns a storage error string when the read fails.
    pub fn theme(&self) -> Result<Option<String>, String> {
        self.storage
            .get_setting("theme")
            .map_err(|error| error.to_string())
    }

    /// Returns the configured global launch shortcut, falling back to the
    /// platform-neutral MVP default when no setting has been stored.
    ///
    /// # Errors
    ///
    /// Returns a storage error when the setting cannot be read.
    pub fn global_hotkey(&self) -> Result<String, String> {
        Ok(self
            .storage
            .get_setting(GLOBAL_HOTKEY_SETTING)
            .map_err(|error| error.to_string())?
            .unwrap_or_else(|| DEFAULT_GLOBAL_HOTKEY.to_owned()))
    }

    /// Stores one bounded global shortcut. The native registration layer
    /// accepts the same canonical form, so malformed values fail before they
    /// can create a platform registration conflict.
    ///
    /// # Errors
    ///
    /// Returns a validation or storage error.
    pub fn set_global_hotkey(&self, binding: &str) -> Result<(), String> {
        validate_global_hotkey(binding)?;
        self.storage
            .set_setting(GLOBAL_HOTKEY_SETTING, binding.trim())
            .map_err(|error| error.to_string())
    }

    /// Saves a host-owned Quicklink template.
    ///
    /// # Errors
    ///
    /// Returns a validation or storage error.
    pub fn save_quicklink(&self, id: &str, title: &str, url_template: &str) -> Result<(), String> {
        self.storage
            .save_quicklink(id, title, url_template)
            .map_err(|error| error.to_string())
    }

    /// Saves a plain-text host-owned Snippet.
    ///
    /// # Errors
    ///
    /// Returns a validation or storage error.
    pub fn save_snippet(&self, id: &str, title: &str, body: &str) -> Result<(), String> {
        self.storage
            .save_snippet(id, title, body)
            .map_err(|error| error.to_string())
    }

    /// Captures the current platform clipboard into the encrypted host vault.
    ///
    /// # Errors
    ///
    /// Returns a platform, credential, encryption, or storage error. Sensitive
    /// sources are explicitly ignored and never enter `SQLite` metadata.
    pub fn capture_clipboard(
        &mut self,
        created_at: i64,
        sensitive: bool,
    ) -> Result<Option<ClipboardItem>, String> {
        if self.clipboard_paused {
            return Ok(None);
        }
        if sensitive {
            return Ok(None);
        }
        let payload = self
            .platform
            .read_clipboard()
            .map_err(|error| error.to_string())?;
        self.capture_clipboard_payload(payload, created_at, sensitive)
    }

    /// Captures a platform payload that was read by the clipboard worker.
    ///
    /// Pause, deduplication, encryption, and `SQLite` metadata policy remain in
    /// this host-owned method instead of being split across worker threads.
    ///
    /// # Errors
    ///
    /// Returns a credential, encryption, or storage error.
    pub fn capture_clipboard_payload(
        &mut self,
        payload: ClipboardPayload,
        created_at: i64,
        sensitive: bool,
    ) -> Result<Option<ClipboardItem>, String> {
        if self.clipboard_paused || sensitive {
            return Ok(None);
        }
        let (item, hash_bytes, kind) = match payload {
            novahub_platform_api::ClipboardPayload::Text(text) => (
                self.clipboard
                    .capture_text(&text, created_at, false)
                    .map_err(|error| error.to_string())?,
                text.into_bytes(),
                ClipboardKind::Text,
            ),
            novahub_platform_api::ClipboardPayload::Image(bytes) => (
                Some(
                    self.clipboard
                        .capture_image(&bytes, created_at)
                        .map_err(|error| error.to_string())?,
                ),
                bytes,
                ClipboardKind::Image,
            ),
        };
        if let Some(item) = &item {
            self.storage
                .record_clipboard(
                    &novahub_providers::clipboard::content_hash(&hash_bytes),
                    match kind {
                        ClipboardKind::Text => "text",
                        ClipboardKind::Image => "image",
                    },
                    item.created_at,
                    self.clipboard.policy().ttl_seconds(),
                    sensitive,
                )
                .map_err(|error| error.to_string())?;
        }
        Ok(item)
    }

    /// Returns whether the clipboard worker should start one read now.
    pub fn clipboard_poll_due(&mut self, now: Instant) -> bool {
        !self.clipboard_paused && self.clipboard_poller.due(now)
    }

    /// Starts one platform clipboard read outside the UI thread.
    ///
    /// # Errors
    ///
    /// Returns an error when the worker thread cannot be created.
    pub fn start_clipboard_read(
        &self,
    ) -> Result<Receiver<Result<ClipboardPayload, String>>, String> {
        let platform = self.platform.clone();
        let (sender, receiver) = mpsc::sync_channel(1);
        std::thread::Builder::new()
            .name("novahub-clipboard-reader".into())
            .spawn(move || {
                let result = platform.read_clipboard().map_err(|error| error.to_string());
                let _ = sender.send(result);
            })
            .map_err(|error| error.to_string())?;
        Ok(receiver)
    }

    /// Polls the platform clipboard only when the host cadence gate is due.
    ///
    /// # Errors
    ///
    /// Returns the same platform, credential, encryption, or storage errors as
    /// [`Self::capture_clipboard`].
    pub fn poll_clipboard(
        &mut self,
        now: Instant,
        created_at: i64,
    ) -> Result<Option<ClipboardItem>, String> {
        if !self.clipboard_poller.due(now) {
            return Ok(None);
        }
        self.capture_clipboard(created_at, false)
    }

    /// Lists recent clipboard metadata without exposing plaintext to plugins.
    pub fn clipboard_items(&mut self, now: i64) -> Vec<ClipboardItem> {
        self.clipboard.list(now)
    }

    /// Formats a bounded clipboard history summary for the host Shell.
    ///
    /// # Errors
    ///
    /// Returns a storage error when expired `SQLite` metadata cannot be purged.
    pub fn clipboard_history_summary(&mut self, now: i64) -> Result<String, String> {
        let items = self.clipboard.list(now);
        self.storage
            .purge_expired_clipboard(now)
            .map_err(|error| error.to_string())?;
        if items.is_empty() {
            return Ok("Clipboard history is empty".to_owned());
        }
        let summary = items
            .into_iter()
            .take(8)
            .enumerate()
            .map(|(index, item)| {
                let kind = match item.kind {
                    ClipboardKind::Text => "text",
                    ClipboardKind::Image => "image",
                };
                format!("{}. {kind} ({} bytes)", index + 1, item.size)
            })
            .collect::<Vec<_>>()
            .join("\n");
        Ok(summary)
    }

    /// Reads and decrypts one clipboard item for a host-owned UI view.
    ///
    /// # Errors
    ///
    /// Returns a bounded vault error.
    pub fn read_clipboard_item(&mut self, id: u64, now: i64) -> Result<Vec<u8>, String> {
        self.clipboard
            .read(id, now)
            .map_err(|error| error.to_string())
    }

    /// Pins or unpins one clipboard history item and persists only its hash.
    ///
    /// # Errors
    ///
    /// Returns a bounded history or storage error when the item is unavailable.
    pub fn set_clipboard_item_pinned(&mut self, id: u64, pinned: bool) -> Result<(), String> {
        let content_hash = self
            .clipboard
            .set_pinned(id, pinned)
            .map_err(|error| error.to_string())?;
        self.storage
            .set_clipboard_pinned(&content_hash, pinned)
            .map_err(|error| error.to_string())?;
        Ok(())
    }

    /// Copies one text history entry back to the host clipboard.
    ///
    /// The Shell exposes a one-based history ordinal instead of the vault's
    /// internal ID. Image entries remain read-only until the platform API has
    /// a safe, dimension-aware image write contract.
    ///
    /// # Errors
    ///
    /// Returns a bounded history, UTF-8, or platform capability error.
    pub fn copy_clipboard_item(&mut self, ordinal: usize, now: i64) -> Result<String, String> {
        let item = self
            .clipboard
            .list(now)
            .into_iter()
            .nth(
                ordinal
                    .checked_sub(1)
                    .ok_or_else(|| "clipboard item number must start at 1".to_owned())?,
            )
            .ok_or_else(|| "clipboard item was not found".to_owned())?;
        if item.kind != ClipboardKind::Text {
            return Err("image clipboard items cannot be restored yet".to_owned());
        }
        let bytes = self
            .clipboard
            .read(item.id, now)
            .map_err(|error| error.to_string())?;
        let text =
            String::from_utf8(bytes).map_err(|_| "clipboard text is not valid UTF-8".to_owned())?;
        self.platform
            .write_clipboard_text(&text)
            .map_err(|error| error.to_string())?;
        Ok("Clipboard item copied".to_owned())
    }

    /// Copies a text history item addressed by its host-owned vault ID.
    ///
    /// # Errors
    ///
    /// Returns a bounded history, UTF-8, or platform capability error.
    pub fn copy_clipboard_item_id(&mut self, id: u64, now: i64) -> Result<String, String> {
        let item = self
            .clipboard
            .list(now)
            .into_iter()
            .find(|item| item.id == id)
            .ok_or_else(|| "clipboard item was not found".to_owned())?;
        if item.kind != ClipboardKind::Text {
            return Err("image clipboard items cannot be restored yet".to_owned());
        }
        let bytes = self
            .clipboard
            .read(item.id, now)
            .map_err(|error| error.to_string())?;
        let text =
            String::from_utf8(bytes).map_err(|_| "clipboard text is not valid UTF-8".to_owned())?;
        self.platform
            .write_clipboard_text(&text)
            .map_err(|error| error.to_string())?;
        Ok("Clipboard item copied".to_owned())
    }

    /// Enables or pauses host clipboard capture.
    pub fn set_clipboard_paused(&mut self, paused: bool) {
        self.clipboard_paused = paused;
        self.clipboard_poller.set_paused(paused);
    }

    #[must_use]
    pub const fn clipboard_paused(&self) -> bool {
        self.clipboard_paused
    }

    /// Returns the bounded clipboard retention policy owned by the host.
    #[must_use]
    pub fn clipboard_policy(&self) -> ClipboardPolicy {
        self.clipboard.policy()
    }

    /// Updates and persists clipboard retention without exposing the vault to
    /// plugins. Invalid values are rejected before any in-memory state moves.
    ///
    /// # Errors
    ///
    /// Returns a policy validation or `SQLite` error.
    pub fn set_clipboard_policy(
        &mut self,
        max_items: usize,
        ttl_seconds: i64,
    ) -> Result<(), String> {
        let policy =
            ClipboardPolicy::new(max_items, ttl_seconds).map_err(|error| error.to_string())?;
        self.storage
            .set_clipboard_policy(max_items, ttl_seconds)
            .map_err(|error| error.to_string())?;
        self.clipboard.set_policy(policy);
        Ok(())
    }

    /// Clears encrypted clipboard payloads and their host metadata.
    ///
    /// # Errors
    ///
    /// Returns a storage error when the metadata cannot be removed.
    pub fn clear_clipboard(&mut self) -> Result<(), String> {
        self.clipboard.clear();
        self.storage
            .clear_clipboard()
            .map_err(|error| error.to_string())
    }

    /// Applies a host-owned desktop-pet event and returns the new state.
    ///
    /// # Errors
    ///
    /// Returns the event when it is not valid for the current pet state.
    pub fn dispatch_pet_event(&mut self, event: PetEvent) -> Result<PetState, PetEvent> {
        self.pet_controller.dispatch(event)
    }

    #[must_use]
    pub fn pet_state(&self) -> PetState {
        self.pet_controller.state()
    }

    #[must_use]
    pub fn pet_frame(&self) -> Option<RenderFrame> {
        self.pet_renderer.frame_for(self.pet_controller.state())
    }

    /// Returns the selected pet identifier used by the host renderer.
    #[must_use]
    pub fn active_pet_id(&self) -> &str {
        &self.active_pet_id
    }

    /// Returns the validated bytes for an installed frame, if the current
    /// renderer selected one. Built-in assets are embedded by `ui-slint` and
    /// therefore do not appear in this map.
    #[must_use]
    pub fn pet_frame_bytes(&self, asset: &str) -> Option<&[u8]> {
        self.pet_frame_assets.get(asset).map(Vec::as_slice)
    }

    /// Activates a built-in or installed declarative desktop pet.
    ///
    /// Installed packages are revalidated through the Plugin Manager before
    /// their bounded SVG bytes enter the host renderer. The selected pet ID is
    /// persisted by the host and never exposed as a plugin-owned setting.
    ///
    /// # Errors
    ///
    /// Returns an error when the requested package is unavailable, disabled,
    /// malformed, or outside the host resource boundary.
    pub fn activate_pet(&mut self, pet_id: impl AsRef<str>) -> Result<String, String> {
        let pet_id = pet_id.as_ref().trim();
        if pet_id.is_empty() {
            return Err("pet id must not be empty".into());
        }
        let result = match pet_id {
            "nova" | "pixel" | "waterman" => {
                self.apply_builtin_pet(pet_id)?;
                format!("Pet activated: {pet_id}")
            }
            _ => {
                let root = self
                    .plugin_root
                    .as_deref()
                    .ok_or_else(|| "installed plugin root is not configured".to_owned())?;
                let pet = read_active_pet(root, pet_id).map_err(|error| format!("{error:?}"))?;
                let display_name = pet.descriptor.display_name.clone();
                self.apply_installed_pet(&pet)?;
                format!("Pet activated: {display_name} ({pet_id})")
            }
        };
        self.storage
            .set_setting(ACTIVE_PET_SETTING, pet_id)
            .map_err(|error| error.to_string())?;
        Ok(result)
    }

    /// Lists bounded built-in and installed desktop-pet choices for the Shell.
    #[must_use]
    pub fn pet_choices(&self) -> Vec<String> {
        let mut choices = vec![
            "nova (built-in)".to_owned(),
            "pixel (built-in)".to_owned(),
            "waterman (built-in)".to_owned(),
        ];
        let Some(root) = self.plugin_root.as_deref() else {
            return choices;
        };
        let Ok(entries) = std::fs::read_dir(root) else {
            return choices;
        };
        for entry in entries.flatten().take(32) {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let Some(plugin_id) = path.file_name().and_then(|value| value.to_str()) else {
                continue;
            };
            if let Ok(pet) = read_active_pet(root, plugin_id) {
                choices.push(format!(
                    "{} ({})",
                    pet.plugin_id, pet.descriptor.display_name
                ));
            }
        }
        choices
    }

    fn restore_active_pet_setting(&mut self) {
        let Ok(Some(pet_id)) = self.storage.get_setting(ACTIVE_PET_SETTING) else {
            return;
        };
        let _ = self.activate_pet(pet_id);
    }

    fn apply_builtin_pet(&mut self, pet_id: &str) -> Result<(), String> {
        let (idle, success) = match pet_id {
            "nova" => ("builtin://nova/idle", "builtin://nova/success"),
            "pixel" => ("builtin://pixel/idle", "builtin://pixel/success"),
            "waterman" => ("builtin://waterman/idle", "builtin://waterman/success"),
            _ => return Err("unknown built-in pet".into()),
        };
        let visible = !matches!(self.pet_controller.state(), PetState::Hidden);
        let mut renderer = PetRenderer::new(idle);
        renderer
            .set_frame(PetFrameState::Success, success, 12)
            .map_err(str::to_owned)?;
        self.pet_renderer = renderer;
        self.pet_frame_assets.clear();
        pet_id.clone_into(&mut self.active_pet_id);
        self.reset_pet_controller(pet_id, visible);
        self.pet_shelf = default_pet_shelf();
        Ok(())
    }

    fn apply_installed_pet(&mut self, pet: &InstalledPet) -> Result<(), String> {
        let Some(idle) = pet.frames.first() else {
            return Err("desktop-pet has no frame assets".into());
        };
        let visible = !matches!(self.pet_controller.state(), PetState::Hidden);
        let mut renderer = PetRenderer::new(installed_pet_asset(&pet.plugin_id, &idle.name));
        let mut assets = BTreeMap::new();
        for frame in &pet.frames {
            let asset = installed_pet_asset(&pet.plugin_id, &frame.name);
            let state = pet_frame_state(&frame.name);
            renderer
                .set_frame(state, asset.clone(), 12)
                .map_err(str::to_owned)?;
            assets.insert(asset, frame.bytes.clone());
        }
        self.pet_renderer = renderer;
        self.pet_frame_assets = assets;
        self.active_pet_id.clone_from(&pet.plugin_id);
        self.reset_pet_controller(&pet.descriptor.pet_id, visible);
        let recommendations = pet
            .descriptor
            .recommended_actions
            .iter()
            .map(|target| PetAction {
                id: target.clone(),
                title: target.clone(),
                target: target.clone(),
            })
            .collect();
        self.pet_shelf = default_pet_shelf();
        self.pet_shelf.set_recommended(recommendations);
        Ok(())
    }

    fn reset_pet_controller(&mut self, pet_id: &str, visible: bool) {
        self.pet_controller = PetController::new(PetId::new(pet_id));
        if visible {
            let _ = self.pet_controller.dispatch(PetEvent::Show);
        }
    }

    /// Restores a saved logical position and clamps it to a connected display.
    ///
    /// The position is kept in the host model so a platform adapter only has
    /// to convert display metrics and apply the native window position.
    pub fn restore_pet_position(
        &mut self,
        saved: PetPosition,
        size: (u32, u32),
        displays: &[DisplayWorkArea],
    ) -> Option<PetPosition> {
        self.pet_placement = PetPlacement::restore(saved, size, displays);
        self.saved_pet_position = self.pet_placement.map(|placement| placement.position);
        self.persist_pet_position();
        self.pet_placement.map(|placement| placement.position)
    }

    /// Snaps the current pet window to the nearest edge of its display.
    pub fn snap_pet_to_edge(&mut self, area: DisplayWorkArea) -> Option<PetPosition> {
        let placement = self.pet_placement.take()?;
        let snapped = placement.snap_to_edge(area);
        let position = snapped.position;
        self.pet_placement = Some(snapped);
        Some(position)
    }

    #[must_use]
    pub fn pet_position(&self) -> Option<PetPosition> {
        self.pet_placement.map(|placement| placement.position)
    }

    /// Returns the last logical position even before a native monitor has been
    /// attached to the pet window during startup.
    #[must_use]
    pub const fn saved_pet_position(&self) -> Option<PetPosition> {
        self.saved_pet_position
    }

    /// Sets a logical position after a drag, keeping it bounded to the current
    /// work area. This is the only mutation path used by platform adapters.
    ///
    /// # Panics
    ///
    /// Panics only if the caller supplies a display that cannot be restored;
    /// callers must pass the same display snapshot used for the drag.
    pub fn set_pet_position(
        &mut self,
        display: DisplayWorkArea,
        point: LogicalPoint,
        size: (u32, u32),
    ) -> PetPosition {
        let placement = PetPlacement::restore(
            PetPosition {
                display_id: display.display_id,
                point,
            },
            size,
            std::slice::from_ref(&display),
        )
        .expect("the current display must be available");
        let position = placement.position;
        self.pet_placement = Some(placement);
        self.saved_pet_position = Some(position);
        self.persist_pet_position();
        position
    }

    fn persist_pet_position(&self) {
        let Some(position) = self.saved_pet_position else {
            return;
        };
        let value = serde_json::json!({
            "display_id": position.display_id,
            "x": position.point.x,
            "y": position.point.y,
        });
        let _ = self
            .storage
            .set_setting(PET_POSITION_SETTING, &value.to_string());
    }

    pub fn set_pet_animation_mode(&mut self, mode: novahub_ui_slint::AnimationMode) {
        self.pet_renderer.set_animation_mode(mode);
    }

    pub fn set_pet_recommendations(&mut self, actions: Vec<PetAction>) {
        self.pet_shelf.set_recommended(actions);
    }

    #[must_use]
    pub fn pet_visible_actions(&self) -> Vec<PetAction> {
        self.pet_shelf.visible_actions()
    }

    #[must_use]
    pub fn pet_action_at(&self, index: usize) -> Option<PetAction> {
        self.pet_shelf.visible_actions().into_iter().nth(index)
    }

    /// Pins an available host-resolved action in the user's pet shelf.
    ///
    /// # Errors
    ///
    /// Returns an error when the target is unavailable or the shelf is full.
    pub fn pin_pet_action(
        &mut self,
        action: PetAction,
        state: &PetActionState,
    ) -> Result<(), &'static str> {
        self.pet_shelf.pin(action, state)
    }

    /// Pins one of the currently visible host-resolved actions by stable ID.
    ///
    /// # Errors
    ///
    /// Returns an error when the action is not visible or the shelf is full.
    pub fn pin_pet_action_by_id(&mut self, id: &str) -> Result<(), &'static str> {
        let action = self
            .pet_shelf
            .visible_actions()
            .into_iter()
            .find(|action| action.id == id)
            .ok_or("pet action target is unavailable")?;
        self.pin_pet_action(action, &PetActionState::Available)
    }

    pub fn unpin_pet_action(&mut self, id: &str) -> bool {
        self.pet_shelf.unpin(id)
    }

    #[must_use]
    pub fn pet_actions(&self) -> &PetActionShelf {
        &self.pet_shelf
    }
}

struct InstalledPluginCommands {
    commands: Vec<CommandDescriptor>,
    interactions: BTreeMap<String, PluginInteraction>,
}

fn installed_plugin_commands(
    plugin_root: Option<&std::path::Path>,
    limit: usize,
) -> InstalledPluginCommands {
    let Some(plugin_root) = plugin_root else {
        return InstalledPluginCommands {
            commands: Vec::new(),
            interactions: BTreeMap::new(),
        };
    };
    let Ok(entries) = std::fs::read_dir(plugin_root) else {
        return InstalledPluginCommands {
            commands: Vec::new(),
            interactions: BTreeMap::new(),
        };
    };
    let mut commands = Vec::new();
    let mut interactions = BTreeMap::new();
    for entry in entries.flatten().take(64) {
        if commands.len() >= limit || !entry.path().is_dir() {
            break;
        }
        let Some(plugin_id) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        let Ok(active) = read_active_plugin(plugin_root, &plugin_id) else {
            continue;
        };
        if !active.enabled {
            continue;
        }
        let Ok(manifest_text) = std::fs::read_to_string(active.path.join("novahub.toml")) else {
            continue;
        };
        let Ok(manifest) = parse_manifest_toml(&manifest_text) else {
            continue;
        };
        if manifest.kind() != PluginKind::Command {
            continue;
        }
        let summary = manifest.summary();
        for command in manifest.commands() {
            if commands.len() >= limit {
                break;
            }
            let command_id = format!("plugin:{plugin_id}:{}", command.id);
            let title = if command.title.trim().is_empty() {
                summary.name.clone()
            } else {
                command.title.clone()
            };
            commands.push(CommandDescriptor {
                id: novahub_core_domain::CommandId::new(command_id),
                title,
                subtitle: if command.subtitle.trim().is_empty() {
                    format!("{} plugin command", summary.name)
                } else {
                    command.subtitle.clone()
                },
            });
            interactions.insert(
                format!("plugin:{plugin_id}:{}", command.id),
                command.interaction,
            );
        }
    }
    InstalledPluginCommands {
        commands,
        interactions,
    }
}

impl Default for NovaHubApp {
    fn default() -> Self {
        Self::new()
    }
}

fn application_command(application: ApplicationInfo) -> CommandDescriptor {
    CommandDescriptor {
        id: novahub_core_domain::CommandId::new(format!("apps.launch:{}", application.name)),
        title: application.name,
        subtitle: "Launch application".to_owned(),
    }
}

fn compact_application_name(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn application_initials(value: &str) -> String {
    let mut initials = String::new();
    let mut at_word_start = true;
    for character in value.chars() {
        if !character.is_alphanumeric() {
            at_word_start = true;
            continue;
        }
        if let Some(pinyin) = character.to_pinyin() {
            initials.push_str(pinyin.first_letter());
            at_word_start = false;
        } else if at_word_start {
            initials.extend(character.to_lowercase());
            at_word_start = false;
        }
    }
    initials
}

fn pinyin_application_name(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            character
                .to_pinyin()
                .map_or_else(|| character.to_string(), |pinyin| pinyin.plain().to_owned())
        })
        .collect()
}

fn application_matches(
    application: &ApplicationInfo,
    normalized_query: &str,
    compact_query: &str,
    initial_query: &str,
) -> bool {
    let normalized_name = application.name.to_ascii_lowercase();
    let launch_name = application
        .launch_path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let compact_launch_name = compact_application_name(&launch_name);
    let launch_initials = application_initials(&launch_name);
    let pinyin_name = pinyin_application_name(&application.name);
    let compact_pinyin_name = compact_application_name(&pinyin_name);
    let pinyin_launch_name = pinyin_application_name(&launch_name);
    let compact_pinyin_launch_name = compact_application_name(&pinyin_launch_name);
    normalized_name.contains(normalized_query)
        || launch_name.contains(normalized_query)
        || (!compact_query.is_empty()
            && compact_application_name(&application.name).contains(compact_query))
        || (!compact_query.is_empty() && compact_launch_name.contains(compact_query))
        || (!compact_query.is_empty() && compact_pinyin_name.contains(compact_query))
        || (!compact_query.is_empty() && compact_pinyin_launch_name.contains(compact_query))
        || (!compact_query.is_empty()
            && (application_initials(&application.name).starts_with(compact_query)
                || launch_initials.starts_with(compact_query)))
        || (!initial_query.is_empty()
            && (application_initials(&application.name).starts_with(initial_query)
                || launch_initials.starts_with(initial_query)))
}

fn application_has_exact_alias(
    application: &ApplicationInfo,
    normalized_query: &str,
    compact_query: &str,
    initial_query: &str,
) -> bool {
    let normalized_name = application.name.to_ascii_lowercase();
    let launch_name = application
        .launch_path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let compact_name = compact_application_name(&application.name);
    let compact_launch_name = compact_application_name(&launch_name);
    let name_initials = application_initials(&application.name);
    let launch_initials = application_initials(&launch_name);

    (!normalized_query.is_empty()
        && (normalized_name == normalized_query || launch_name == normalized_query))
        || (!compact_query.is_empty()
            && (compact_name == compact_query || compact_launch_name == compact_query))
        || (!compact_query.is_empty()
            && (name_initials == compact_query || launch_initials == compact_query))
        || (!initial_query.is_empty()
            && (name_initials == initial_query || launch_initials == initial_query))
}

fn installed_pet_asset(plugin_id: &str, frame_name: &str) -> String {
    format!("installed://{plugin_id}/{frame_name}")
}

fn pet_frame_state(frame_name: &str) -> PetFrameState {
    let name = frame_name.to_ascii_lowercase();
    if name.contains("working") || name.contains("busy") {
        PetFrameState::Working
    } else if name.contains("success") || name.contains("done") {
        PetFrameState::Success
    } else if name.contains("error") || name.contains("fail") {
        PetFrameState::Error
    } else {
        PetFrameState::Idle
    }
}

fn default_pet_shelf() -> PetActionShelf {
    let mut shelf = PetActionShelf::default();
    shelf.set_recommended(vec![
        PetAction {
            id: "clipboard.history".into(),
            title: "Clipboard".into(),
            target: "clipboard".into(),
        },
        PetAction {
            id: "calculator.evaluate".into(),
            title: "Calculator".into(),
            target: "calc 1 + 1".into(),
        },
        PetAction {
            id: "system.settings".into(),
            title: "Settings".into(),
            target: "system settings".into(),
        },
    ]);
    shelf
}

fn validate_global_hotkey(binding: &str) -> Result<(), String> {
    let mut parts = binding.trim().split('+');
    let modifier = parts.next().unwrap_or_default();
    let key = parts.next().unwrap_or_default();
    if parts.next().is_some()
        || !matches!(modifier, "Alt" | "Option" | "Control" | "Shift" | "Meta")
        || key != "Space"
    {
        return Err("shortcut must use one modifier and Space (for example Alt+Space)".into());
    }
    Ok(())
}

fn load_pet_position(storage: &Storage) -> Option<PetPosition> {
    let value = storage
        .get_setting(PET_POSITION_SETTING)
        .ok()
        .flatten()
        .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok())?;
    Some(PetPosition {
        display_id: value.get("display_id")?.as_u64()?,
        point: LogicalPoint {
            x: i32::try_from(value.get("x")?.as_i64()?).ok()?,
            y: i32::try_from(value.get("y")?.as_i64()?).ok()?,
        },
    })
}

fn current_unix_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|duration| i64::try_from(duration.as_secs()).ok())
        .unwrap_or(0)
}

fn capability_names(permissions: &DeclaredPermissions) -> Vec<String> {
    permissions
        .iter()
        .map(|permission| permission.capability.as_str().to_owned())
        .collect()
}

#[cfg(test)]
mod tests {
    use std::env;
    use std::time::Duration;

    use super::{
        HISTORY_TTL_SECONDS, HostPlatform, HostPluginCapabilityHandler, MAX_HISTORY_ITEMS,
        MAX_PLUGIN_HTTP_BYTES, NovaHubApp, PluginCapabilityHandler, PluginCapabilityRequest,
        PluginCapabilityResponse,
    };
    use novahub_core_domain::Action;
    use novahub_platform_api::{ApplicationInfo, HttpResponse, PlatformError};
    use novahub_plugin_manager::{
        ArchiveLimits, AuthorizedCapability, Capability, EffectiveGrant, HostFileHandle,
        PluginInteraction, install_archive_verified, pack_directory, parse_manifest_toml,
        set_plugin_enabled, sign_archive, uninstall_plugin,
    };

    fn capability_handler(plugin_id: &str, permission_toml: &str) -> HostPluginCapabilityHandler {
        let manifest = parse_manifest_toml(&format!(
            "id = \"{plugin_id}\"\nversion = \"1.0.0\"\nhost_api = \">=1.0, <2.0\"\n{permission_toml}"
        ))
        .expect("test manifest parses");
        let declared = manifest.declared_permissions();
        let grant = EffectiveGrant::from_layers(declared, declared, declared);
        HostPluginCapabilityHandler::new(plugin_id, grant, HostPlatform::probe())
    }

    fn capability_request(
        plugin_id: &str,
        capability: AuthorizedCapability,
        payload: Option<&str>,
    ) -> PluginCapabilityRequest {
        PluginCapabilityRequest {
            request_id: "request-1".into(),
            plugin_id: plugin_id.into(),
            session_id: "session-1".into(),
            capability,
            payload: payload.map(str::to_owned),
        }
    }

    #[test]
    fn plugin_capability_handler_rejects_identity_and_missing_grants() {
        let handler = capability_handler("demo", "permissions = []\n");
        let wrong_identity =
            capability_request("another-plugin", AuthorizedCapability::ClipboardRead, None);
        assert_eq!(
            handler.handle(wrong_identity),
            PluginCapabilityResponse::Error("capability_identity_mismatch".into())
        );

        let missing_grant = capability_request("demo", AuthorizedCapability::ClipboardRead, None);
        assert_eq!(
            handler.handle(missing_grant),
            PluginCapabilityResponse::Error("capability_not_granted".into())
        );
    }

    #[test]
    fn plugin_storage_preflight_enforces_the_effective_quota() {
        let handler =
            capability_handler("demo", "[permissions]\nstorage = { quota_bytes = 1024 }\n");
        assert_eq!(
            handler.handle(capability_request(
                "demo",
                AuthorizedCapability::StorageWrite { total_bytes: 1024 },
                None,
            )),
            PluginCapabilityResponse::Unit
        );
        assert_eq!(
            handler.handle(capability_request(
                "demo",
                AuthorizedCapability::StorageWrite { total_bytes: 1025 },
                None,
            )),
            PluginCapabilityResponse::Error("capability_scope_exceeded".into())
        );
    }

    #[test]
    fn plugin_http_returns_host_owned_response_after_origin_authorization() {
        let handler = capability_handler(
            "demo",
            "[permissions]\nhttp = { origins = [\"https://api.example.test\"] }\n",
        )
        .with_test_http_response(
            "https://api.example.test/resource",
            HttpResponse::ok(b"response".to_vec()),
        );
        assert_eq!(
            handler.handle(capability_request(
                "demo",
                AuthorizedCapability::Http {
                    origin: "https://api.example.test".into(),
                },
                Some("https://api.example.test/resource"),
            )),
            PluginCapabilityResponse::Text("response".into())
        );
    }

    #[test]
    fn plugin_http_rechecks_each_redirect_and_returns_bounded_text() {
        let handler = capability_handler(
            "demo",
            "[permissions]\nhttp = { origins = [\"https://api.example.test\", \"https://cdn.example.test\"] }\n",
        )
        .with_test_http_response(
            "https://api.example.test/start",
            HttpResponse::redirect("https://cdn.example.test/final"),
        )
        .with_test_http_response(
            "https://cdn.example.test/final",
            HttpResponse::ok(b"hello".to_vec()),
        );
        assert_eq!(
            handler.handle(capability_request(
                "demo",
                AuthorizedCapability::Http {
                    origin: "https://api.example.test".into(),
                },
                Some("https://api.example.test/start"),
            )),
            PluginCapabilityResponse::Text("hello".into())
        );
    }

    #[test]
    fn plugin_http_rejects_an_undeclared_redirect_target() {
        let handler = capability_handler(
            "demo",
            "[permissions]\nhttp = { origins = [\"https://api.example.test\"] }\n",
        )
        .with_test_http_response(
            "https://api.example.test/start",
            HttpResponse::redirect("https://evil.example.test/final"),
        );
        assert_eq!(
            handler.handle(capability_request(
                "demo",
                AuthorizedCapability::Http {
                    origin: "https://api.example.test".into(),
                },
                Some("https://api.example.test/start"),
            )),
            PluginCapabilityResponse::Error("capability_scope_exceeded".into())
        );
    }

    #[test]
    fn plugin_http_rejects_oversized_and_non_utf8_bodies() {
        let handler = capability_handler(
            "demo",
            "[permissions]\nhttp = { origins = [\"https://api.example.test\"] }\n",
        )
        .with_test_http_response(
            "https://api.example.test/large",
            HttpResponse::ok(vec![b'x'; MAX_PLUGIN_HTTP_BYTES + 1]),
        )
        .with_test_http_response(
            "https://api.example.test/binary",
            HttpResponse::ok(vec![0xff]),
        );
        assert_eq!(
            handler.handle(capability_request(
                "demo",
                AuthorizedCapability::Http {
                    origin: "https://api.example.test".into(),
                },
                Some("https://api.example.test/large"),
            )),
            PluginCapabilityResponse::Error("http_response_too_large".into())
        );
        assert_eq!(
            handler.handle(capability_request(
                "demo",
                AuthorizedCapability::Http {
                    origin: "https://api.example.test".into(),
                },
                Some("https://api.example.test/binary"),
            )),
            PluginCapabilityResponse::Error("http_response_not_utf8".into())
        );
    }

    #[test]
    fn platform_http_body_limit_has_a_stable_error_code() {
        assert_eq!(
            super::platform_error_code(&PlatformError::LimitExceeded),
            "http_response_too_large"
        );
        assert_eq!(
            super::platform_error_code(&PlatformError::Timeout),
            "http_deadline_exceeded"
        );
    }

    #[test]
    fn plugin_file_handle_must_be_host_issued_for_the_current_session() {
        let handler = capability_handler(
            "demo",
            "[permissions]\nfiles = { read = \"user-selected\" }\n",
        )
        .with_issued_file_content("issued-token", b"hello");
        assert_eq!(
            handler.handle(capability_request(
                "demo",
                AuthorizedCapability::FilesRead {
                    handle: HostFileHandle::issue("demo", "session-1", "forged-token"),
                },
                Some("forged-token"),
            )),
            PluginCapabilityResponse::Error("capability_handle_mismatch".into())
        );
        assert_eq!(
            handler.handle(capability_request(
                "demo",
                AuthorizedCapability::FilesRead {
                    handle: HostFileHandle::issue("demo", "session-1", "issued-token"),
                },
                Some("issued-token"),
            )),
            PluginCapabilityResponse::Text("hello".into())
        );
    }

    #[test]
    fn plugin_file_picker_issues_a_session_bound_token() {
        let handler = capability_handler(
            "demo",
            "[permissions]\nfiles = { read = \"user-selected\" }\n",
        )
        .with_test_file_selection(b"picked text");
        let token = match handler.handle(capability_request(
            "demo",
            AuthorizedCapability::FilesSelect,
            None,
        )) {
            PluginCapabilityResponse::Text(token) => token,
            other => panic!("expected opaque token, got {other:?}"),
        };
        assert!(!token.is_empty());
        assert_eq!(
            handler.handle(capability_request(
                "demo",
                AuthorizedCapability::FilesRead {
                    handle: HostFileHandle::issue("demo", "session-1", token.clone()),
                },
                Some(&token),
            )),
            PluginCapabilityResponse::Text("picked text".into())
        );

        handler.end_session("demo", "session-1");
        assert_eq!(
            handler.handle(capability_request(
                "demo",
                AuthorizedCapability::FilesRead {
                    handle: HostFileHandle::issue("demo", "session-1", token.clone()),
                },
                Some(&token),
            )),
            PluginCapabilityResponse::Error("capability_handle_mismatch".into())
        );
    }

    #[test]
    fn app_searches_builtin_commands_without_plugin_host() {
        let app = NovaHubApp::new();
        let results = app.search("calc");
        assert!(
            results
                .iter()
                .any(|result| result.id.as_str() == "calculator.evaluate")
        );
    }

    #[test]
    fn app_searches_host_owned_diagnostics_without_starting_plugin_host() {
        let app = NovaHubApp::new();
        assert!(
            app.search("diagnostics")
                .iter()
                .any(|result| result.id.as_str() == "diagnostics.view")
        );
    }

    #[test]
    fn provider_filter_keeps_ranked_results_inside_one_family() {
        let app = NovaHubApp::new();
        let calculator = app.search_provider("", Some("calculator"));
        assert!(!calculator.is_empty());
        assert!(
            calculator
                .iter()
                .all(|command| command.id.as_str().starts_with("calculator."))
        );
        assert!(app.search_provider("", Some("plugins")).is_empty());
        assert_eq!(app.search_provider("", None), app.search(""));
    }

    #[test]
    fn app_hides_platform_commands_without_an_advertised_capability() {
        let app = NovaHubApp::new();
        assert!(
            !app.search("window manager")
                .iter()
                .any(|result| result.id.as_str() == "windows.layout")
        );
    }

    #[test]
    fn application_matches_keep_a_host_owned_launch_identity() {
        let descriptor = super::application_command(ApplicationInfo {
            name: "Example App".to_owned(),
            launch_path: std::path::PathBuf::from("C:/Example App.exe"),
        });
        assert_eq!(descriptor.id.as_str(), "apps.launch:Example App");
        assert_eq!(descriptor.title, "Example App");
        assert_eq!(descriptor.subtitle, "Launch application");
    }

    #[test]
    fn application_matching_supports_compact_names_and_initials() {
        let application = ApplicationInfo {
            name: "Visual Studio Code".to_owned(),
            launch_path: std::path::PathBuf::from("C:/Code.exe"),
        };
        assert!(super::application_matches(
            &application,
            "visualstudio",
            "visualstudio",
            "vs"
        ));
        assert!(super::application_matches(
            &application,
            "vsc",
            "vsc",
            "vsc"
        ));
        assert!(!super::application_matches(
            &application,
            "unknown",
            "unknown",
            "unknown"
        ));
    }

    #[test]
    fn application_matching_supports_launch_name_aliases() {
        let application = ApplicationInfo {
            name: "Photo Editor Pro".to_owned(),
            launch_path: std::path::PathBuf::from("C:/Tools/pep.exe"),
        };
        assert!(super::application_matches(
            &application,
            "pep",
            "pep",
            "pep"
        ));
    }

    #[test]
    fn application_matching_supports_pinyin_full_names_and_initials() {
        let application = ApplicationInfo {
            name: "微信".to_owned(),
            launch_path: std::path::PathBuf::from("C:/WeChat.exe"),
        };
        assert!(super::application_matches(
            &application,
            "weixin",
            "weixin",
            "w"
        ));
        assert!(super::application_matches(&application, "wx", "wx", "wx"));
    }

    #[test]
    fn application_snapshot_is_published_only_at_the_host_boundary() {
        let app = NovaHubApp::new();
        assert!(!app.has_application_snapshot());
        app.apply_application_snapshot(vec![ApplicationInfo {
            name: "Visual Studio Code".into(),
            launch_path: std::path::PathBuf::from("C:/Code.exe"),
        }]);
        assert!(app.has_application_snapshot());
        assert_eq!(app.search("vsc")[0].title, "Visual Studio Code");
    }

    #[test]
    fn application_discovery_worker_returns_a_bounded_result() {
        let app = NovaHubApp::new();
        let receiver = app
            .start_application_discovery()
            .expect("discovery worker should start");
        let result = receiver
            .recv_timeout(Duration::from_secs(5))
            .expect("discovery worker should respond");
        if let Ok(applications) = result {
            assert!(applications.len() <= super::MAX_APPLICATION_SNAPSHOT);
        }
    }

    #[test]
    fn app_persists_theme_in_host_storage() {
        let app = NovaHubApp::new();
        app.set_theme("dark").expect("theme write");
        assert_eq!(app.theme().expect("theme read").as_deref(), Some("dark"));
    }

    #[test]
    fn app_persists_and_validates_global_hotkey() {
        let app = NovaHubApp::new();
        assert_eq!(
            app.global_hotkey().expect("default hotkey"),
            super::DEFAULT_GLOBAL_HOTKEY
        );
        app.set_global_hotkey("Control+Space")
            .expect("valid hotkey");
        assert_eq!(app.global_hotkey().expect("stored hotkey"), "Control+Space");
        assert!(app.set_global_hotkey("Shell::Command").is_err());
    }

    #[test]
    fn app_persists_bounded_clipboard_policy() {
        let mut app = NovaHubApp::new();
        let default = app.clipboard_policy();
        assert_eq!(default.max_items(), MAX_HISTORY_ITEMS);
        assert_eq!(default.ttl_seconds(), HISTORY_TTL_SECONDS);

        app.set_clipboard_policy(25, 86_400)
            .expect("bounded policy write");
        assert_eq!(app.clipboard_policy().max_items(), 25);
        assert_eq!(app.clipboard_policy().ttl_seconds(), 86_400);
        assert!(app.set_clipboard_policy(0, 86_400).is_err());
    }

    #[test]
    fn clipboard_policy_survives_host_restart() {
        let path = env::temp_dir().join(format!(
            "novahub-clipboard-policy-{}.sqlite3",
            std::process::id()
        ));
        {
            let mut app = NovaHubApp::open(&path).expect("open policy database");
            app.set_clipboard_policy(40, 172_800)
                .expect("persist policy");
        }
        let app = NovaHubApp::open(&path).expect("reopen policy database");
        assert_eq!(app.clipboard_policy().max_items(), 40);
        assert_eq!(app.clipboard_policy().ttl_seconds(), 172_800);
        let _ = std::fs::remove_file(path);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_uses_option_label_for_the_default_shortcut() {
        assert_eq!(super::DEFAULT_GLOBAL_HOTKEY, "Option+Space");
        assert!(super::validate_global_hotkey("Option+Space").is_ok());
    }

    #[test]
    fn app_executes_calculator_in_the_host_process() {
        let app = NovaHubApp::new();
        let action = Action::invoke("calculator.evaluate", ["6 * 7"]);
        assert_eq!(app.execute_builtin(&action, false), Ok("42".into()));
    }

    #[test]
    fn app_executes_host_owned_quicklinks_and_snippets() {
        let app = NovaHubApp::new();
        app.save_quicklink("docs", "Docs", "https://example.test/?q={query}")
            .expect("save quicklink");
        app.save_snippet("hello", "Hello", "Plain text")
            .expect("save snippet");
        assert_eq!(
            app.execute_builtin(&Action::invoke("quicklinks.open", ["docs", "Rust"]), false,),
            Ok("https://example.test/?q=Rust".into())
        );
        assert_eq!(
            app.execute_builtin(&Action::invoke("snippets.library", ["hello"]), false),
            Ok("Plain text".into())
        );
    }

    #[test]
    fn app_owns_pet_state_renderer_and_action_shelf_without_plugin_host() {
        use novahub_core_domain::pet::PetEvent;
        use novahub_ui_slint::{PetAction, PetActionState};

        let mut app = NovaHubApp::new();
        assert_eq!(app.pet_state(), novahub_core_domain::pet::PetState::Hidden);
        app.dispatch_pet_event(PetEvent::Show).expect("show pet");
        assert_eq!(
            app.pet_frame().expect("idle frame").asset,
            "builtin://nova/idle"
        );
        app.set_pet_recommendations(vec![PetAction {
            id: "calculator".into(),
            title: "Calculator".into(),
            target: "calculator.evaluate".into(),
        }]);
        app.pin_pet_action(
            PetAction {
                id: "calculator".into(),
                title: "Calculator".into(),
                target: "calculator.evaluate".into(),
            },
            &PetActionState::Available,
        )
        .expect("pin pet action");
        assert_eq!(app.pet_actions().fixed().len(), 1);
    }

    #[test]
    fn app_restores_and_snaps_pet_position_in_logical_coordinates() {
        use novahub_core_domain::pet_position::{DisplayWorkArea, LogicalPoint, PetPosition};

        let display = DisplayWorkArea {
            display_id: 7,
            left: 0,
            top: 0,
            width: 1920,
            height: 1080,
            dpi: 144,
        };
        let mut app = NovaHubApp::new();
        assert_eq!(
            app.restore_pet_position(
                PetPosition {
                    display_id: 99,
                    point: LogicalPoint { x: 5000, y: -10 },
                },
                (160, 120),
                &[display],
            ),
            Some(PetPosition {
                display_id: 7,
                point: LogicalPoint { x: 1760, y: 0 },
            })
        );
        assert_eq!(
            app.snap_pet_to_edge(display),
            Some(PetPosition {
                display_id: 7,
                point: LogicalPoint { x: 1760, y: 0 },
            })
        );
    }

    #[test]
    fn pet_position_roundtrips_through_host_storage() {
        use novahub_core_domain::pet_position::{DisplayWorkArea, LogicalPoint, PetPosition};

        let path = env::temp_dir().join(format!(
            "novahub-pet-position-{}.sqlite3",
            std::process::id()
        ));
        let display = DisplayWorkArea {
            display_id: 3,
            left: 0,
            top: 0,
            width: 1920,
            height: 1080,
            dpi: 144,
        };
        {
            let mut app = NovaHubApp::open(&path).expect("open position database");
            assert_eq!(
                app.set_pet_position(display, LogicalPoint { x: 120, y: 240 }, (360, 160)),
                PetPosition {
                    display_id: 3,
                    point: LogicalPoint { x: 120, y: 240 },
                }
            );
        }
        let app = NovaHubApp::open(&path).expect("reopen position database");
        assert_eq!(
            app.saved_pet_position(),
            Some(PetPosition {
                display_id: 3,
                point: LogicalPoint { x: 120, y: 240 },
            })
        );
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn app_plugin_client_roundtrips_a_component_when_configured() {
        let (Ok(_host), Ok(fixture)) = (
            env::var("NOVAHUB_PLUGIN_HOST"),
            env::var("NOVAHUB_COMPONENT_FIXTURE"),
        ) else {
            return;
        };
        let app = NovaHubApp::new();
        let run = app
            .run_component_fixture("app-fixture", fixture, r#"{"from":"app"}"#)
            .expect("run component session");
        assert_eq!(run.loaded["type"], "loaded");
        assert_eq!(run.opened["revision"], 1);
        assert_eq!(run.updated["revision"], 2);
        assert_eq!(run.closed["type"], "ack");
    }

    #[test]
    fn app_capability_component_roundtrips_storage_and_persisted_revocation_when_configured() {
        let Some((app, root)) = configured_capability_component_app() else {
            return;
        };

        assert_capability_component_results(&app);
        app.revoke_plugin_capability("capability.fixture", Capability::Storage)
            .expect("revoke storage grant");
        let revoked = app
            .run_active_plugin(
                "capability-revoked",
                "capability.fixture",
                "capability:storage:1024",
            )
            .expect("revoked call returns a bounded plugin view");
        assert_eq!(
            revoked.updated["view"]["markdown"],
            "storage:error:capability not granted"
        );
        drop(app);
        let _ = std::fs::remove_dir_all(root);
    }

    fn configured_capability_component_app() -> Option<(NovaHubApp, std::path::PathBuf)> {
        let (Ok(_host), Ok(fixture)) = (
            env::var("NOVAHUB_PLUGIN_HOST"),
            env::var("NOVAHUB_COMPONENT_FIXTURE"),
        ) else {
            return None;
        };
        let root = env::temp_dir().join(format!(
            "novahub-capability-component-e2e-{}",
            std::process::id()
        ));
        let plugin_root = root.join("plugins");
        let version = plugin_root.join("capability.fixture/versions/1.0.0");
        std::fs::create_dir_all(&version).expect("plugin version directory");
        std::fs::write(
            version.join("novahub.toml"),
            "id = \"capability.fixture\"\nversion = \"1.0.0\"\nhost_api = \">=1.0, <2.0\"\n\n[permissions]\nfiles = { read = \"user-selected\" }\nhttp = { origins = [\"https://api.example.test\", \"https://cdn.example.test\"] }\nstorage = { quota_bytes = 1024 }\n",
        )
        .expect("plugin manifest");
        std::fs::copy(fixture, version.join("plugin.wasm")).expect("fixture component");
        std::fs::write(
            plugin_root.join("capability.fixture/active.json"),
            serde_json::to_vec(&serde_json::json!({
                "plugin_id": "capability.fixture",
                "version": "1.0.0",
                "path": version,
                "enabled": true,
            }))
            .expect("active pointer JSON"),
        )
        .expect("active pointer");

        let app = NovaHubApp::open_with_plugin_root(root.join("novahub.sqlite3"), &plugin_root)
            .expect("open capability E2E app")
            .with_test_file_selection(b"picked from host picker")
            .with_test_http_response(
                "https://api.example.test/start",
                HttpResponse::redirect("https://cdn.example.test/final"),
            )
            .with_test_http_response(
                "https://cdn.example.test/final",
                HttpResponse::ok(b"redirected response".to_vec()),
            );
        Some((app, root))
    }

    fn assert_capability_component_results(app: &NovaHubApp) {
        let allowed = app
            .run_active_plugin(
                "capability-allowed",
                "capability.fixture",
                "capability:storage:1024",
            )
            .expect("granted storage preflight");
        assert_eq!(
            allowed.opened["view"]["items"][0]["accessory"]["badge"],
            "FIXTURE"
        );
        assert_eq!(
            allowed.opened["view"]["items"][0]["accessory"]["icon"]["id"],
            "fixture.icon"
        );
        assert_eq!(allowed.opened["view"]["next_cursor"], "fixture-page-2");
        assert_eq!(allowed.updated["view"]["markdown"], "storage:ok");

        let next_page = app
            .run_active_plugin(
                "capability-pagination",
                "capability.fixture",
                r#"{"event":"load_more","cursor":"fixture-page-2"}"#,
            )
            .expect("cursor pagination roundtrip");
        assert_eq!(
            next_page.updated["view"]["items"][0]["id"],
            "fixture.page-2"
        );
        assert!(next_page.updated["view"]["next_cursor"].is_null());

        let actions = app
            .run_active_plugin("capability-actions", "capability.fixture", "view:actions")
            .expect("typed action panel roundtrip");
        assert_eq!(actions.updated["view"]["actions"][0]["role"], "default");
        assert_eq!(actions.updated["view"]["actions"][1]["role"], "secondary");
        assert_eq!(actions.updated["view"]["actions"][1]["destructive"], true);

        let typed_kv = app
            .run_active_plugin(
                "capability-kv",
                "capability.fixture",
                "capability:kv:greeting=hello",
            )
            .expect("typed plugin KV roundtrip");
        assert_eq!(typed_kv.updated["view"]["markdown"], "kv:ok:hello");
        assert_eq!(
            app.storage
                .plugin_kv("capability.fixture", "greeting")
                .expect("read host-owned plugin KV"),
            Some(b"hello".to_vec())
        );

        let forged_file = app
            .run_active_plugin(
                "capability-file-mismatch",
                "capability.fixture",
                "capability:file:forged-token",
            )
            .expect("forged file handle returns a bounded plugin view");
        assert_eq!(
            forged_file.updated["view"]["markdown"],
            "file:error:capability_handle_mismatch"
        );

        let picked_file = app
            .run_active_plugin(
                "capability-file-pick",
                "capability.fixture",
                "capability:file:pick",
            )
            .expect("host-issued file token roundtrip");
        assert_eq!(
            picked_file.updated["view"]["markdown"],
            "file:ok:picked from host picker"
        );

        let http = app
            .run_active_plugin(
                "capability-http",
                "capability.fixture",
                "capability:http:https://api.example.test/start",
            )
            .expect("bounded HTTP redirect roundtrip");
        assert_eq!(
            http.updated["view"]["markdown"],
            "http:ok:redirected response"
        );
    }

    #[test]
    fn app_resolves_disabled_active_plugin_without_starting_the_host() {
        let root =
            env::temp_dir().join(format!("novahub-active-plugin-test-{}", std::process::id()));
        let version = root.join("demo/versions/1.0.0");
        std::fs::create_dir_all(&version).expect("plugin version directory");
        std::fs::write(
            root.join("demo/active.json"),
            serde_json::to_vec(&serde_json::json!({
                "plugin_id": "demo",
                "version": "1.0.0",
                "path": version,
                "enabled": false,
            }))
            .expect("active pointer JSON"),
        )
        .expect("active pointer");
        let app = NovaHubApp::open_with_plugin_root(root.join("novahub.sqlite3"), &root)
            .expect("app opens with plugin root");
        assert_eq!(
            app.run_active_plugin("disabled", "demo", "input")
                .expect_err("disabled plugin must not start Host"),
            "plugin is disabled"
        );
        assert_eq!(app.diagnostic_events().len(), 1);
        assert_eq!(
            app.diagnostic_events()[0].error_class(),
            Some("plugin is disabled")
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn plugin_user_grant_is_initialized_narrowed_and_restored_after_restart() {
        use novahub_plugin_manager::Capability;

        let root = env::temp_dir().join(format!(
            "novahub-plugin-grant-lifecycle-{}",
            std::process::id()
        ));
        let version = root.join("demo/versions/1.0.0");
        std::fs::create_dir_all(&version).expect("plugin version directory");
        std::fs::write(
            version.join("novahub.toml"),
            "id = \"demo\"\nversion = \"1.0.0\"\nhost_api = \">=1.0, <2.0\"\n\n[permissions]\nclipboard = { access = [\"read\"] }\nhttp = { origins = [\"https://api.example.test\"] }\n",
        )
        .expect("plugin manifest");
        std::fs::write(
            version.join("plugin.wasm"),
            b"validated component placeholder",
        )
        .expect("plugin component placeholder");
        std::fs::write(
            root.join("demo/active.json"),
            serde_json::to_vec(&serde_json::json!({
                "plugin_id": "demo",
                "version": "1.0.0",
                "path": version,
                "enabled": true,
            }))
            .expect("active pointer JSON"),
        )
        .expect("active pointer");

        {
            let app = NovaHubApp::open_with_plugin_root(root.join("novahub.sqlite3"), &root)
                .expect("open app");
            let initial = app
                .prepare_active_plugin_execution("demo")
                .expect("initial grant is created");
            assert!(initial.grant.allows(Capability::Clipboard));
            assert!(initial.grant.allows(Capability::Http));
            app.revoke_plugin_capability("demo", Capability::Http)
                .expect("revoke one capability");
            assert_eq!(
                app.active_plugin_permission_snapshot("demo")
                    .expect("read permission snapshot"),
                super::PluginPermissionSnapshot {
                    declared: vec!["clipboard".into(), "http".into()],
                    granted: vec!["clipboard".into()],
                }
            );
            let narrowed = app
                .prepare_active_plugin_execution("demo")
                .expect("narrowed plugin still prepares");
            assert!(narrowed.grant.allows(Capability::Clipboard));
            assert!(!narrowed.grant.allows(Capability::Http));
            app.revoke_plugin_permissions("demo")
                .expect("revoke plugin grant");
            let revoked = app
                .prepare_active_plugin_execution("demo")
                .expect("revoked plugin still prepares");
            assert!(!revoked.grant.allows(Capability::Clipboard));
        }
        {
            let app = NovaHubApp::open_with_plugin_root(root.join("novahub.sqlite3"), &root)
                .expect("reopen app");
            let revoked = app
                .prepare_active_plugin_execution("demo")
                .expect("revocation survives restart");
            assert!(!revoked.grant.allows(Capability::Clipboard));
            app.approve_declared_plugin_permissions("demo")
                .expect("approve declared permissions");
            let restored = app
                .prepare_active_plugin_execution("demo")
                .expect("approval restores declared grant");
            assert!(restored.grant.allows(Capability::Clipboard));
            assert!(restored.grant.allows(Capability::Http));
            app.storage
                .set_plugin_user_grant("demo", "not-json", 30)
                .expect("inject malformed persisted grant");
            let Err(error) = app.prepare_active_plugin_execution("demo") else {
                panic!("malformed grants must fail closed");
            };
            assert_eq!(error, "persisted plugin user grant is invalid");
        }
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn plugin_update_does_not_implicitly_expand_the_persisted_user_grant() {
        let root = env::temp_dir().join(format!(
            "novahub-plugin-grant-update-{}",
            std::process::id()
        ));
        let version = root.join("demo/versions/1.0.0");
        std::fs::create_dir_all(&version).expect("plugin version directory");
        std::fs::write(
            version.join("novahub.toml"),
            "id = \"demo\"\nversion = \"1.0.0\"\nhost_api = \">=1.0, <2.0\"\n\n[permissions]\nclipboard = { access = [\"read\"] }\n",
        )
        .expect("initial plugin manifest");
        std::fs::write(
            version.join("plugin.wasm"),
            b"validated component placeholder",
        )
        .expect("plugin component placeholder");
        std::fs::write(
            root.join("demo/active.json"),
            serde_json::to_vec(&serde_json::json!({
                "plugin_id": "demo",
                "version": "1.0.0",
                "path": version,
                "enabled": true,
            }))
            .expect("active pointer JSON"),
        )
        .expect("active pointer");

        {
            let app = NovaHubApp::open_with_plugin_root(root.join("novahub.sqlite3"), &root)
                .expect("open app");
            let snapshot = app
                .active_plugin_permission_snapshot("demo")
                .expect("initialize grant");
            assert_eq!(snapshot.declared, vec!["clipboard"]);
            assert_eq!(snapshot.granted, vec!["clipboard"]);
        }

        std::fs::write(
            version.join("novahub.toml"),
            "id = \"demo\"\nversion = \"1.0.0\"\nhost_api = \">=1.0, <2.0\"\n\n[permissions]\nclipboard = { access = [\"read\"] }\nhttp = { origins = [\"https://api.example.test\"] }\n",
        )
        .expect("expanded plugin manifest");
        {
            let app = NovaHubApp::open_with_plugin_root(root.join("novahub.sqlite3"), &root)
                .expect("reopen app after update");
            assert_eq!(
                app.active_plugin_permission_snapshot("demo")
                    .expect("read updated permission state"),
                super::PluginPermissionSnapshot {
                    declared: vec!["clipboard".into(), "http".into()],
                    granted: vec!["clipboard".into()],
                }
            );
        }
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn app_indexes_enabled_plugin_commands_in_the_host_search() {
        let root = env::temp_dir().join(format!(
            "novahub-plugin-command-test-{}",
            std::process::id()
        ));
        let version = root.join("demo/versions/1.0.0");
        std::fs::create_dir_all(&version).expect("plugin version directory");
        std::fs::write(
            version.join("novahub.toml"),
            "id = \"demo\"\nversion = \"1.0.0\"\nhost_api = \">=1.0, <2.0\"\n\n[[commands]]\nid = \"demo.run\"\ntitle = \"Demo command\"\nsubtitle = \"Run the demo component\"\ninteraction = \"one-shot\"\n",
        )
        .expect("plugin manifest");
        std::fs::write(version.join("plugin.wasm"), b"component fixture")
            .expect("plugin component");
        std::fs::write(
            root.join("demo/active.json"),
            serde_json::to_vec(&serde_json::json!({
                "plugin_id": "demo",
                "version": "1.0.0",
                "path": version,
                "enabled": true,
            }))
            .expect("active pointer JSON"),
        )
        .expect("active pointer");

        let app = NovaHubApp::open_with_plugin_root(root.join("novahub.sqlite3"), &root)
            .expect("app opens with plugin root");
        let command = app
            .search("demo command")
            .into_iter()
            .find(|command| command.id.as_str() == "plugin:demo:demo.run")
            .expect("installed plugin command is searchable");
        assert_eq!(command.title, "Demo command");
        assert_eq!(
            app.active_plugin_command_interaction("demo", "demo.run"),
            Ok(PluginInteraction::OneShot)
        );
        assert_eq!(
            app.plugin_command_interaction("plugin:demo:demo.run"),
            Some(PluginInteraction::OneShot)
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn plugin_install_call_disable_uninstall_lifecycle_stays_host_owned() {
        let (Ok(_host), Ok(fixture)) = (
            env::var("NOVAHUB_PLUGIN_HOST"),
            env::var("NOVAHUB_COMPONENT_FIXTURE"),
        ) else {
            return;
        };
        let root = env::temp_dir().join(format!(
            "novahub-plugin-lifecycle-test-{}",
            std::process::id()
        ));
        let source = root.join("source");
        let plugin_root = root.join("plugins");
        std::fs::create_dir_all(&source).expect("source directory");
        std::fs::write(
            source.join("novahub.toml"),
            "id = \"com.example.e2e\"\nname = \"E2E Plugin\"\nversion = \"1.0.0\"\nhost_api = \">=1.0, <2.0\"\n\n[[commands]]\nid = \"e2e.run\"\ntitle = \"E2E command\"\nsubtitle = \"Install lifecycle command\"\n",
        )
        .expect("manifest");
        std::fs::copy(fixture, source.join("plugin.wasm")).expect("fixture component");
        let archive = root.join("e2e.novahub-plugin");
        pack_directory(&source, &archive, ArchiveLimits::default()).expect("pack plugin");
        let archive_bytes = std::fs::read(&archive).expect("read archive");
        let (signature, public_key) = sign_archive(&archive_bytes, &[11_u8; 32]).expect("sign");
        install_archive_verified(
            &archive_bytes,
            &plugin_root,
            "1.1.0",
            None,
            Some((&public_key, &signature)),
            ArchiveLimits::default(),
        )
        .expect("install signed plugin");

        let database = root.join("novahub.sqlite3");
        let app = NovaHubApp::open_with_plugin_root(&database, &plugin_root)
            .expect("open app after install");
        assert!(
            app.search("E2E command")
                .iter()
                .any(|command| command.id.as_str() == "plugin:com.example.e2e:e2e.run")
        );
        let component = app
            .active_plugin_component("com.example.e2e")
            .expect("active pointer resolves component");
        let run = app
            .run_active_plugin("e2e", "com.example.e2e", "lifecycle")
            .expect("installed component runs through Plugin Host");
        assert_eq!(run.updated["view"]["kind"], "detail");
        assert!(component.is_file());
        drop(app);

        set_plugin_enabled(&plugin_root, "com.example.e2e", false).expect("disable plugin");
        let disabled = NovaHubApp::open_with_plugin_root(&database, &plugin_root)
            .expect("reopen app after disable");
        assert!(
            !disabled
                .search("E2E command")
                .iter()
                .any(|command| command.id.as_str() == "plugin:com.example.e2e:e2e.run")
        );
        assert_eq!(
            disabled
                .active_plugin_component("com.example.e2e")
                .expect_err("disabled plugin must not resolve"),
            "plugin is disabled"
        );
        drop(disabled);

        uninstall_plugin(&plugin_root, "com.example.e2e").expect("uninstall plugin");
        assert!(!plugin_root.join("com.example.e2e").exists());
        let removed = NovaHubApp::open_with_plugin_root(&database, &plugin_root)
            .expect("reopen app after uninstall");
        assert!(
            !removed
                .search("E2E command")
                .iter()
                .any(|command| command.id.as_str() == "plugin:com.example.e2e:e2e.run")
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn app_activates_and_persists_a_validated_custom_pet() {
        use novahub_core_domain::pet::PetEvent;

        let root = env::temp_dir().join(format!("novahub-custom-pet-test-{}", std::process::id()));
        let version = root.join("custom.pet/versions/1.0.0");
        std::fs::create_dir_all(version.join("assets")).expect("pet assets directory");
        std::fs::write(
            version.join("novahub.toml"),
            "id = \"custom.pet\"\nversion = \"1.0.0\"\nhost_api = \">=1.0, <2.0\"\nkind = \"desktop-pet\"\n",
        )
        .expect("pet manifest");
        std::fs::write(
            version.join("pet.json"),
            br#"{"petId":"custom","displayName":"Custom","fallbackPetId":"nova","frames":["assets/idle.svg","assets/success.svg"],"recommendedActions":["calculator.evaluate"]}"#,
        )
        .expect("pet descriptor");
        std::fs::write(
            version.join("assets/idle.svg"),
            b"<svg xmlns=\"http://www.w3.org/2000/svg\"></svg>",
        )
        .expect("idle frame");
        std::fs::write(
            version.join("assets/success.svg"),
            b"<svg xmlns=\"http://www.w3.org/2000/svg\"></svg>",
        )
        .expect("success frame");
        std::fs::write(
            root.join("custom.pet/active.json"),
            serde_json::to_vec(&serde_json::json!({
                "plugin_id": "custom.pet",
                "version": "1.0.0",
                "path": version,
                "enabled": true,
            }))
            .expect("active pointer JSON"),
        )
        .expect("active pointer");

        let database = root.join("novahub.sqlite3");
        {
            let mut app = NovaHubApp::open_with_plugin_root(&database, &root)
                .expect("open app with custom pet");
            assert_eq!(app.active_pet_id(), "nova");
            assert!(
                app.activate_pet("custom.pet")
                    .expect("activate custom pet")
                    .contains("Custom")
            );
            assert_eq!(app.active_pet_id(), "custom.pet");
            app.dispatch_pet_event(PetEvent::Show).expect("show pet");
            let frame = app.pet_frame().expect("custom idle frame");
            assert!(frame.asset.starts_with("installed://custom.pet/"));
            assert!(app.pet_frame_bytes(&frame.asset).is_some());
            assert!(
                app.pet_choices()
                    .iter()
                    .any(|choice| choice.contains("custom.pet (Custom)"))
            );
        }
        let app = NovaHubApp::open_with_plugin_root(&database, &root)
            .expect("reopen app with custom pet");
        assert_eq!(app.active_pet_id(), "custom.pet");
        let _ = std::fs::remove_dir_all(root);
    }
}

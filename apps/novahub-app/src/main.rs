#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use std::cell::RefCell;
use std::ffi::OsStr;
use std::io::Write;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use novahub_app::DEFAULT_GLOBAL_HOTKEY;
use novahub_app::diagnostics::DiagnosticExportOptions;
use novahub_app::plugin_client::{
    PLUGIN_VIEW_SLOT_COUNT, PluginHostClient, PluginViewAction, PluginViewSurface,
};
use novahub_app::{
    ActivePluginExecution, NovaHubApp, PluginFixtureRun, PluginHostError, PluginHostSupervisor,
    default_data_dir,
};
use novahub_core_domain::Action;
use novahub_core_domain::pet::PetEvent;
use novahub_core_domain::pet_position::LogicalPoint;
use novahub_plugin_manager::{Capability, PluginInteraction};
use novahub_ui_slint::{
    CLIPBOARD_HISTORY_SLOT_COUNT, ClipboardWindow, ComponentHandle, PhysicalPosition,
    PluginFieldSlot, PluginItemSlot, SEARCH_RESULT_SLOT_COUNT, ShellWindow, TrayIcon,
    builtin_tray_image, center_shell_window, clear_clipboard_rows, clear_result_rows,
    create_clipboard_window, create_pet_window, create_settings_window, create_shell_window,
    create_tray_icon, current_monitor_work_area, initialize_shell_backend, on_shell_first_frame,
    on_shell_next_frame, place_pet_window, quit_event_loop, set_action_panel, set_clipboard_row,
    set_pet_frame_with_bytes, set_pet_shelf, set_pet_shelf_actions, set_pet_visible,
    set_plugin_default_action_available, set_plugin_view, set_plugin_view_action,
    set_plugin_view_field, set_plugin_view_item, set_plugin_view_paging, set_plugin_view_progress,
    set_provider_filter_label, set_result_list_visible, set_result_row, set_result_text,
    set_settings_theme, set_shell_theme, set_status_text,
};
use serde_json::json;

#[derive(Debug)]
enum PluginExecutionResult {
    View(PluginFixtureRun),
    OneShot(serde_json::Value),
}

type PluginTaskResult = Result<PluginExecutionResult, String>;
type PluginTaskReceiver = Receiver<PluginTaskResult>;
type PendingPluginTask = Rc<RefCell<Option<PluginTaskReceiver>>>;
type ApplicationTaskReceiver = Receiver<Result<Vec<novahub_platform_api::ApplicationInfo>, String>>;
type PendingApplicationTask = Rc<RefCell<Option<ApplicationTaskReceiver>>>;
type ClipboardTaskReceiver = Receiver<Result<novahub_platform_api::ClipboardPayload, String>>;
type PendingClipboardTask = Rc<RefCell<Option<ClipboardTaskReceiver>>>;
type FileTaskReceiver = Receiver<Result<Vec<novahub_platform_api::FileSearchResult>, String>>;
type PendingFileTask = Rc<RefCell<Option<FileTaskReceiver>>>;
type ActivePluginSourceState = Rc<RefCell<Option<ActivePluginSource>>>;
type ActivePluginSurface = Rc<RefCell<Option<PluginViewSurface>>>;
type PluginFormValues = Rc<RefCell<Vec<String>>>;
type PluginFormDirty = Rc<RefCell<bool>>;
type PluginItemOffset = Rc<RefCell<usize>>;
type PendingPluginAction = Rc<RefCell<Option<PluginViewAction>>>;
type SettingsWindowState = Rc<RefCell<Option<novahub_ui_slint::SettingsWindow>>>;
type ProviderFilterState = Rc<RefCell<Option<String>>>;
type ClipboardWindowState = Rc<RefCell<Option<ClipboardWindow>>>;
type ClipboardFilterState = Rc<RefCell<ClipboardFilter>>;

const APPLICATION_REFRESH_INTERVAL: Duration = Duration::from_secs(30);
const MAX_SMOKE_HOLD_MS: u64 = 60_000;
const HOT_WAKE_SETTLE: Duration = Duration::from_millis(50);
const MAX_REFERENCE_PLUGIN_SESSIONS: usize = 4;
const REFERENCE_SESSION_READY_TIMEOUT: Duration = Duration::from_secs(35);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ShellSmokeMode {
    ColdStart(Duration),
    HotWake(Duration),
}

impl ShellSmokeMode {
    const fn hold(self) -> Duration {
        match self {
            Self::ColdStart(hold) | Self::HotWake(hold) => hold,
        }
    }
}

/// Identifies how the current plugin view must be executed.
///
/// Installed plugins retain the capability-aware execution context prepared
/// by the application. Development fixtures deliberately keep the unbound,
/// deny-by-default supervisor path.
#[derive(Clone)]
enum ActivePluginSource {
    Fixture(PathBuf),
    Installed(ActivePluginExecution),
}

impl ActivePluginSource {
    fn run_view(&self, session_id: &str, input: &str) -> Result<PluginFixtureRun, PluginHostError> {
        match self {
            Self::Fixture(component_path) => PluginHostSupervisor::from_default_path()?
                .run_component_session(session_id, component_path, input),
            Self::Installed(execution) => execution.run_view(session_id, input),
        }
    }

    fn run_one_shot(
        &self,
        session_id: &str,
        command_id: &str,
        input: &str,
    ) -> Result<serde_json::Value, PluginHostError> {
        match self {
            Self::Fixture(component_path) => PluginHostSupervisor::from_default_path()?
                .run_component_one_shot(session_id, component_path, command_id, input),
            Self::Installed(execution) => execution.run_one_shot(session_id, command_id, input),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ClipboardFilter {
    All,
    Text,
    Image,
}

impl ClipboardFilter {
    fn next(self) -> Self {
        match self {
            Self::All => Self::Text,
            Self::Text => Self::Image,
            Self::Image => Self::All,
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::All => "All types",
            Self::Text => "Text only",
            Self::Image => "Images only",
        }
    }

    const fn matches(self, kind: novahub_providers::clipboard::ClipboardKind) -> bool {
        match self {
            Self::All => true,
            Self::Text => matches!(kind, novahub_providers::clipboard::ClipboardKind::Text),
            Self::Image => matches!(kind, novahub_providers::clipboard::ClipboardKind::Image),
        }
    }
}

fn theme_is_dark(theme: &str) -> bool {
    theme.eq_ignore_ascii_case("dark")
}

fn shell_theme_is_dark(app: &NovaHubApp) -> bool {
    app.theme()
        .ok()
        .flatten()
        .is_some_and(|theme| theme_is_dark(&theme))
}

#[cfg(any(windows, target_os = "macos"))]
type GlobalHotkeyState = Rc<RefCell<Option<HotkeyRegistration>>>;
#[cfg(not(any(windows, target_os = "macos")))]
type GlobalHotkeyState = Rc<RefCell<()>>;

#[cfg(any(windows, target_os = "macos"))]
struct HotkeyRegistration {
    manager: GlobalHotKeyManager,
    hotkey: Option<HotKey>,
}

const PROVIDER_FILTERS: &[(&str, &str)] = &[
    ("apps", "Applications"),
    ("files", "Files"),
    ("clipboard", "Clipboard"),
    ("snippets", "Snippets"),
    ("quicklinks", "Quicklinks"),
    ("windows", "Window Manager"),
    ("system", "System"),
    ("calculator", "Calculator"),
    ("preferences", "Preferences"),
    ("units", "Unit conversion"),
    ("plugins", "Plugins"),
];

fn provider_filter_label(provider: Option<&str>) -> &'static str {
    provider
        .and_then(|value| {
            PROVIDER_FILTERS
                .iter()
                .find(|(id, _)| *id == value)
                .map(|(_, label)| *label)
        })
        .unwrap_or("All providers")
}

fn next_provider_filter(current: Option<&str>) -> Option<String> {
    let next_index = current
        .and_then(|value| PROVIDER_FILTERS.iter().position(|(id, _)| *id == value))
        .map_or(0, |index| index + 1);
    PROVIDER_FILTERS
        .get(next_index)
        .map(|(id, _)| (*id).to_owned())
}

#[derive(Clone)]
struct PluginUiState {
    pending_task: PendingPluginTask,
    pending_file_task: PendingFileTask,
    source: ActivePluginSourceState,
    surface: ActivePluginSurface,
    form_values: PluginFormValues,
    form_dirty: PluginFormDirty,
    item_offset: PluginItemOffset,
    pending_confirmation: PendingPluginAction,
}

impl PluginUiState {
    fn new() -> Self {
        Self {
            pending_task: Rc::new(RefCell::new(None)),
            pending_file_task: Rc::new(RefCell::new(None)),
            source: Rc::new(RefCell::new(None)),
            surface: Rc::new(RefCell::new(None)),
            form_values: Rc::new(RefCell::new(Vec::new())),
            form_dirty: Rc::new(RefCell::new(false)),
            item_offset: Rc::new(RefCell::new(0)),
            pending_confirmation: Rc::new(RefCell::new(None)),
        }
    }

    fn accept_surface(&self, surface: &PluginViewSurface) {
        *self.surface.borrow_mut() = Some(surface.clone());
        *self.item_offset.borrow_mut() = 0;
        *self.form_values.borrow_mut() = surface
            .fields
            .iter()
            .map(plugin_initial_field_value)
            .collect();
        *self.form_dirty.borrow_mut() = false;
        self.pending_confirmation.borrow_mut().take();
    }

    fn clear_active_view(&self) {
        self.source.borrow_mut().take();
        self.surface.borrow_mut().take();
        self.form_values.borrow_mut().clear();
        *self.form_dirty.borrow_mut() = false;
        *self.item_offset.borrow_mut() = 0;
        self.pending_confirmation.borrow_mut().take();
    }
}

static NEXT_PLUGIN_SESSION_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

#[cfg(any(windows, target_os = "macos"))]
use global_hotkey::{
    GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState,
    hotkey::{Code, HotKey, Modifiers},
};

#[allow(clippy::too_many_lines)]
fn main() {
    let process_started = Instant::now();
    if std::env::var_os("NOVAHUB_HEADLESS").is_some() {
        let app = Rc::new(RefCell::new(NovaHubApp::new()));
        run_headless(&app.borrow(), process_started);
        return;
    }

    let shell_smoke = match parse_shell_smoke_config(
        std::env::var_os("NOVAHUB_SMOKE_HOLD_MS").as_deref(),
        std::env::var_os("NOVAHUB_HOT_WAKE_HOLD_MS").as_deref(),
    ) {
            Ok(value) => value,
            Err(error) => {
                eprintln!("invalid NovaHub Shell smoke configuration: {error}");
                std::process::exit(1);
            }
        };

    let frame_observer = match initialize_shell_backend() {
        Ok(observer) => observer,
        Err(error) => {
            eprintln!("failed to initialize NovaHub rendering backend: {error}");
            std::process::exit(1);
        }
    };

    let app = match NovaHubApp::open_default() {
        Ok(app) => Rc::new(RefCell::new(app)),
        Err(error) => {
            eprintln!("failed to open NovaHub data store: {error}");
            std::process::exit(1);
        }
    };

    let window = match create_shell_window() {
        Ok(window) => window,
        Err(error) => {
            eprintln!("failed to create NovaHub Shell: {error}");
            std::process::exit(1);
        }
    };
    set_shell_theme(&window, shell_theme_is_dark(&app.borrow()));
    let pet_window = match create_pet_window() {
        Ok(window) => window,
        Err(error) => {
            eprintln!("failed to create NovaHub pet window: {error}");
            std::process::exit(1);
        }
    };
    let tray = match create_tray_icon() {
        Ok(tray) => Some(tray),
        Err(error) => {
            eprintln!("NovaHub tray is unavailable: {error}");
            None
        }
    };
    if let Some(tray) = &tray {
        if let Some(image) = builtin_tray_image() {
            tray.set_tray_image(image);
        }
        wire_tray_handler(&app, &window, &pet_window, tray);
    }
    window.invoke_focus_search_requested();
    if let Err(error) = sync_pet_window(&mut app.borrow_mut(), &pet_window) {
        eprintln!("NovaHub pet window is unavailable: {error}");
    }
    let query = Rc::new(RefCell::new(String::new()));
    let selected = Rc::new(RefCell::new(0_usize));
    let provider_filter = Rc::new(RefCell::new(None));
    let plugin_ui = PluginUiState::new();
    let settings_window = Rc::new(RefCell::new(None));
    let clipboard_window = Rc::new(RefCell::new(None));
    let clipboard_filter = Rc::new(RefCell::new(ClipboardFilter::All));
    let (hotkey_timer, hotkey_state) = start_global_hotkey_timer(&app, &window);
    wire_query_handler(
        &app,
        &window,
        &query,
        &selected,
        &provider_filter,
        &plugin_ui,
    );
    wire_selection_handlers(&app, &window, &query, &selected, &provider_filter);
    wire_result_handlers(&window, &selected);
    wire_action_panel_handlers(
        &app,
        &window,
        &query,
        &selected,
        &provider_filter,
        &plugin_ui,
    );
    wire_provider_filter_handler(&app, &window, &query, &selected, &provider_filter);
    wire_focus_handlers(&app, &window, &query, &selected, &plugin_ui);
    wire_settings_handler(&app, &window, &settings_window, &hotkey_state);
    wire_clipboard_history_handler(&app, &window, &clipboard_window, &clipboard_filter);
    wire_diagnostics_handlers(&app, &window);
    wire_run_handler(
        &window,
        &RunHandlerContext {
            app: &app,
            query: &query,
            selected: &selected,
            provider_filter: &provider_filter,
            pet_window: &pet_window,
            plugin_ui: &plugin_ui,
            clipboard_window: &clipboard_window,
            clipboard_filter: &clipboard_filter,
        },
    );
    wire_plugin_view_handlers(&window, &plugin_ui);
    wire_pet_handler(&app, &pet_window);
    wire_pet_drag_handler(&app, &pet_window);
    wire_pet_action_handler(&app, &window, &pet_window, &plugin_ui);
    let _application_timer = start_application_snapshot_timer(&app, &window);
    let _clipboard_timer = start_clipboard_timer(&app);
    let _file_search_timer = start_file_search_timer(&window, &plugin_ui);
    let _hotkey_timer = hotkey_timer;
    let _plugin_timer = start_plugin_task_timer(&app, &window, &pet_window, &plugin_ui);

    if let Some(mode) = shell_smoke
        && let Err(error) = configure_shell_smoke(&window, &frame_observer, process_started, mode)
    {
        eprintln!("NovaHub Shell smoke could not observe the first frame: {error}");
        std::process::exit(1);
    }

    if let Err(error) = window.show() {
        eprintln!("NovaHub Shell could not be shown: {error}");
    } else {
        let _ = center_shell_window(&window);
        window.invoke_focus_search_requested();
    }

    if let Err(error) = window.run() {
        eprintln!("NovaHub Shell stopped with an error: {error}");
        std::process::exit(1);
    }
    if let Some(mode) = shell_smoke {
        write_smoke_marker("exit", process_started.elapsed(), Some(mode.hold())).unwrap_or_else(|error| {
            eprintln!("NovaHub Shell smoke marker could not be written: {error}");
        });
    }
    drop(tray);
}

fn parse_shell_smoke_hold(value: Option<&OsStr>) -> Result<Option<Duration>, String> {
    parse_bounded_duration("NOVAHUB_SMOKE_HOLD_MS", value)
}

fn parse_shell_smoke_config(
    cold_start: Option<&OsStr>,
    hot_wake: Option<&OsStr>,
) -> Result<Option<ShellSmokeMode>, String> {
    let cold_start = parse_shell_smoke_hold(cold_start)?;
    let hot_wake = parse_bounded_duration("NOVAHUB_HOT_WAKE_HOLD_MS", hot_wake)?;
    match (cold_start, hot_wake) {
        (Some(_), Some(_)) => Err(
            "NOVAHUB_SMOKE_HOLD_MS and NOVAHUB_HOT_WAKE_HOLD_MS are mutually exclusive".to_owned(),
        ),
        (Some(hold), None) => Ok(Some(ShellSmokeMode::ColdStart(hold))),
        (None, Some(hold)) => Ok(Some(ShellSmokeMode::HotWake(hold))),
        (None, None) => Ok(None),
    }
}

fn parse_bounded_duration(name: &str, value: Option<&OsStr>) -> Result<Option<Duration>, String> {
    let Some(value) = value else {
        return Ok(None);
    };
    let value = value
        .to_str()
        .ok_or_else(|| format!("{name} must be UTF-8"))?;
    let milliseconds = value
        .parse::<u64>()
        .map_err(|_| format!("{name} must be an integer"))?;
    if milliseconds > MAX_SMOKE_HOLD_MS {
        return Err(format!("{name} must not exceed {MAX_SMOKE_HOLD_MS}"));
    }
    Ok(Some(Duration::from_millis(milliseconds)))
}

fn configure_shell_smoke(
    window: &ShellWindow,
    frame_observer: &novahub_ui_slint::ShellFrameObserver,
    process_started: Instant,
    mode: ShellSmokeMode,
) -> Result<(), String> {
    match mode {
        ShellSmokeMode::ColdStart(hold) => on_shell_first_frame(window, frame_observer, move || {
            if let Err(error) =
                write_smoke_marker("first_frame", process_started.elapsed(), Some(hold))
            {
                eprintln!("NovaHub Shell smoke marker could not be written: {error}");
            }
            novahub_ui_slint::Timer::single_shot(hold, quit_event_loop);
        }),
        ShellSmokeMode::HotWake(hold) => {
            configure_hot_wake_smoke(window, frame_observer, hold)
        }
    }
}

fn configure_hot_wake_smoke(
    window: &ShellWindow,
    frame_observer: &novahub_ui_slint::ShellFrameObserver,
    hold: Duration,
) -> Result<(), String> {
    let first_window = window.as_weak();
    let observer = frame_observer.clone();
    on_shell_first_frame(window, frame_observer, move || {
        let Some(window) = first_window.upgrade() else {
            quit_event_loop();
            return;
        };
        let _ = window.hide();
        let next_observer = observer.clone();
        novahub_ui_slint::Timer::single_shot(HOT_WAKE_SETTLE, move || {
            let Some(window) = window.as_weak().upgrade() else {
                quit_event_loop();
                return;
            };
            let frame_started = Instant::now();
            if let Err(error) = on_shell_next_frame(&window, &next_observer, move || {
                if let Err(error) = write_hot_wake_marker(frame_started.elapsed(), hold) {
                    eprintln!("NovaHub hot-wake marker could not be written: {error}");
                }
                novahub_ui_slint::Timer::single_shot(hold, quit_event_loop);
            }) {
                eprintln!("NovaHub hot-wake frame observer failed: {error}");
                quit_event_loop();
                return;
            }
            if let Err(error) = activate_shell_window(&window) {
                eprintln!("NovaHub hot-wake activation failed: {error}");
                quit_event_loop();
            }
        });
    })
}

fn write_smoke_marker(
    event: &str,
    elapsed: Duration,
    hold: Option<Duration>,
) -> std::io::Result<()> {
    let marker = json!({
        "event": event,
        "elapsed_ms": elapsed.as_secs_f64() * 1_000.0,
        "hold_ms": hold.map(|duration| duration.as_millis()),
    });
    write_json_marker("NOVAHUB_SMOKE", &marker)
}

fn write_hot_wake_marker(elapsed: Duration, hold: Duration) -> std::io::Result<()> {
    let marker = json!({
        "event": "hot_wake",
        "elapsed_ms": elapsed.as_secs_f64() * 1_000.0,
        "hold_ms": hold.as_millis(),
        "settle_ms": HOT_WAKE_SETTLE.as_millis(),
        "measurement": "activation_dispatch_to_rendered_frame",
        "hotkey_poll_interval_ms": 40,
    });
    write_json_marker("NOVAHUB_SMOKE", &marker)
}

fn write_json_marker(prefix: &str, marker: &serde_json::Value) -> std::io::Result<()> {
    let mut stdout = std::io::stdout().lock();
    writeln!(stdout, "{prefix} {marker}")?;
    stdout.flush()
}

fn activate_shell_window(window: &ShellWindow) -> Result<(), slint::PlatformError> {
    window.show()?;
    window.window().set_minimized(false);
    window.invoke_focus_search_requested();
    Ok(())
}

fn wire_settings_handler(
    app: &Rc<RefCell<NovaHubApp>>,
    shell: &ShellWindow,
    state: &SettingsWindowState,
    hotkey_state: &GlobalHotkeyState,
) {
    let app = Rc::clone(app);
    let shell = shell.as_weak();
    let state = Rc::clone(state);
    let hotkey_state = Rc::clone(hotkey_state);
    shell
        .upgrade()
        .expect("ShellWindow remains alive")
        .on_settings_requested(move || {
            if state.borrow().is_none() {
                let Ok(settings) = create_settings_window() else {
                    if let Some(shell) = shell.upgrade() {
                        set_status_text(&shell, "Settings window is unavailable");
                    }
                    return;
                };
                let Some(shell_window) = shell.upgrade() else {
                    return;
                };
                wire_settings_window_handlers(&app, &shell_window, &settings, &hotkey_state);
                state.borrow_mut().replace(settings);
            }

            let settings_state = state.borrow();
            let Some(settings) = settings_state.as_ref() else {
                return;
            };
            let app_ref = app.borrow();
            let theme = app_ref
                .theme()
                .ok()
                .flatten()
                .unwrap_or_else(|| "system".to_owned());
            settings.set_theme(theme.clone().into());
            set_settings_theme(settings, theme_is_dark(&theme));
            settings.set_hotkey(
                app_ref
                    .global_hotkey()
                    .unwrap_or_else(|_| String::new())
                    .into(),
            );
            let policy = app_ref.clipboard_policy();
            settings.set_clipboard_ttl(policy.ttl_seconds().to_string().into());
            settings.set_clipboard_limit(policy.max_items().to_string().into());
            settings.set_status_text("".into());
            if let Some(shell) = shell.upgrade() {
                let _ = shell.hide();
            }
            if let Err(error) = settings.show()
                && let Some(shell) = shell.upgrade()
            {
                set_status_text(
                    &shell,
                    format!("Settings window could not be shown: {error}"),
                );
            }
        });
}

fn wire_settings_window_handlers(
    app: &Rc<RefCell<NovaHubApp>>,
    shell: &ShellWindow,
    settings: &novahub_ui_slint::SettingsWindow,
    hotkey_state: &GlobalHotkeyState,
) {
    let app = Rc::clone(app);
    let hotkey_state = Rc::clone(hotkey_state);
    let shell_for_save = shell.as_weak();
    let settings_weak = settings.as_weak();
    settings.on_settings_save_requested(move || {
        let Some(settings) = settings_weak.upgrade() else {
            return;
        };
        let theme = settings.get_theme().to_string();
        let hotkey = settings.get_hotkey().to_string();
        let Ok(max_items) = settings.get_clipboard_limit().parse::<usize>() else {
            settings.set_status_text("Clipboard item limit must be an integer".into());
            return;
        };
        let Ok(ttl_seconds) = settings.get_clipboard_ttl().parse::<i64>() else {
            settings.set_status_text("Clipboard retention must be an integer".into());
            return;
        };
        if !matches!(theme.as_str(), "system" | "light" | "dark") {
            settings.set_status_text("Theme must be system, light, or dark".into());
            return;
        }

        let result = {
            let mut app = app.borrow_mut();
            let previous_hotkey = app.global_hotkey().unwrap_or_default();
            apply_global_hotkey(&hotkey_state, &hotkey)
                .and_then(|()| app.set_global_hotkey(&hotkey))
                .inspect_err(|_| {
                    let _ = apply_global_hotkey(&hotkey_state, &previous_hotkey);
                })
                .and_then(|()| app.set_theme(&theme))
                .and_then(|()| app.set_clipboard_policy(max_items, ttl_seconds))
        };
        match result {
            Ok(()) => {
                settings.set_status_text("Settings saved".into());
                let _ = settings.hide();
                if let Some(shell) = shell_for_save.upgrade() {
                    set_shell_theme(&shell, theme_is_dark(&theme));
                    let _ = shell.show();
                    shell.invoke_focus_search_requested();
                    set_status_text(&shell, "Settings saved");
                }
            }
            Err(error) => settings.set_status_text(error.into()),
        }
    });

    let shell_for_dismiss = shell.as_weak();
    let settings_weak = settings.as_weak();
    settings.on_settings_dismiss_requested(move || {
        if let Some(settings) = settings_weak.upgrade() {
            let _ = settings.hide();
        }
        if let Some(shell) = shell_for_dismiss.upgrade() {
            let _ = shell.show();
            shell.invoke_focus_search_requested();
        }
    });
}

fn wire_clipboard_history_handler(
    app: &Rc<RefCell<NovaHubApp>>,
    shell: &ShellWindow,
    state: &ClipboardWindowState,
    filter: &ClipboardFilterState,
) {
    let app = Rc::clone(app);
    let shell = shell.as_weak();
    let state = Rc::clone(state);
    let filter = Rc::clone(filter);
    shell
        .upgrade()
        .expect("ShellWindow remains alive")
        .on_clipboard_history_requested(move || {
            let Some(shell) = shell.upgrade() else {
                return;
            };
            show_clipboard_window(&app, &shell, &state, &filter);
        });
}

fn show_clipboard_window(
    app: &Rc<RefCell<NovaHubApp>>,
    shell: &ShellWindow,
    state: &ClipboardWindowState,
    filter: &ClipboardFilterState,
) {
    if state.borrow().is_none() {
        let Ok(window) = create_clipboard_window() else {
            set_status_text(shell, "Clipboard history window is unavailable");
            return;
        };
        wire_clipboard_window_handlers(app, shell, &window, filter);
        state.borrow_mut().replace(window);
    }
    let clipboard_state = state.borrow();
    let Some(window) = clipboard_state.as_ref() else {
        return;
    };
    window.set_filter_label(filter.borrow().label().into());
    window.set_paused(app.borrow().clipboard_paused());
    refresh_clipboard_window(&mut app.borrow_mut(), window, *filter.borrow());
    let _ = shell.hide();
    if let Err(error) = window.show() {
        set_status_text(
            shell,
            format!("Clipboard history could not be shown: {error}"),
        );
    }
}

fn wire_clipboard_window_handlers(
    app: &Rc<RefCell<NovaHubApp>>,
    shell: &ShellWindow,
    window: &ClipboardWindow,
    filter: &ClipboardFilterState,
) {
    wire_clipboard_filter_handler(app, window, filter);
    wire_clipboard_preview_handler(app, window, filter);
    wire_clipboard_copy_handler(app, window, filter);
    wire_clipboard_pin_handler(app, window, filter);
    wire_clipboard_clear_handler(app, window);
    wire_clipboard_pause_handler(app, window);
    wire_clipboard_dismiss_handler(shell, window);
}

fn wire_clipboard_filter_handler(
    app: &Rc<RefCell<NovaHubApp>>,
    window: &ClipboardWindow,
    filter: &ClipboardFilterState,
) {
    let app = Rc::clone(app);
    let filter = Rc::clone(filter);
    let window_weak = window.as_weak();
    window.on_filter_requested(move || {
        let next_filter = filter.borrow().next();
        *filter.borrow_mut() = next_filter;
        let Some(window) = window_weak.upgrade() else {
            return;
        };
        window.set_filter_label(next_filter.label().into());
        refresh_clipboard_window(&mut app.borrow_mut(), &window, next_filter);
    });
}

fn wire_clipboard_preview_handler(
    app: &Rc<RefCell<NovaHubApp>>,
    window: &ClipboardWindow,
    filter: &ClipboardFilterState,
) {
    let app = Rc::clone(app);
    let filter = Rc::clone(filter);
    let window_weak = window.as_weak();
    window.on_item_activated(move |index| {
        let Ok(index) = usize::try_from(index) else {
            return;
        };
        let Some(window) = window_weak.upgrade() else {
            return;
        };
        preview_clipboard_item(&mut app.borrow_mut(), &window, *filter.borrow(), index);
    });
}

fn wire_clipboard_copy_handler(
    app: &Rc<RefCell<NovaHubApp>>,
    window: &ClipboardWindow,
    filter: &ClipboardFilterState,
) {
    let app = Rc::clone(app);
    let filter = Rc::clone(filter);
    let window_weak = window.as_weak();
    window.on_item_copy_requested(move |index| {
        let Ok(index) = usize::try_from(index) else {
            return;
        };
        let filter = *filter.borrow();
        let item = filtered_clipboard_items(&mut app.borrow_mut(), filter)
            .into_iter()
            .nth(index);
        let Some(item) = item else {
            return;
        };
        let status = if item.kind == novahub_providers::clipboard::ClipboardKind::Text {
            app.borrow_mut()
                .copy_clipboard_item_id(item.id, unix_timestamp())
                .unwrap_or_else(|error| format!("Clipboard copy failed: {error}"))
        } else {
            "Image clipboard items cannot be restored yet".to_owned()
        };
        if let Some(window) = window_weak.upgrade() {
            window.set_status_text(status.into());
        }
    });
}

fn wire_clipboard_pin_handler(
    app: &Rc<RefCell<NovaHubApp>>,
    window: &ClipboardWindow,
    filter: &ClipboardFilterState,
) {
    let app = Rc::clone(app);
    let filter = Rc::clone(filter);
    let window_weak = window.as_weak();
    window.on_item_pin_requested(move |index| {
        let Ok(index) = usize::try_from(index) else {
            return;
        };
        let filter = *filter.borrow();
        let item = filtered_clipboard_items(&mut app.borrow_mut(), filter)
            .into_iter()
            .nth(index);
        let Some(item) = item else {
            return;
        };
        let status = app
            .borrow_mut()
            .set_clipboard_item_pinned(item.id, !item.pinned)
            .map_or_else(
                |error| format!("Clipboard pin failed: {error}"),
                |()| "Clipboard pin updated".to_owned(),
            );
        if let Some(window) = window_weak.upgrade() {
            window.set_status_text(status.into());
            refresh_clipboard_window(&mut app.borrow_mut(), &window, filter);
        }
    });
}

fn wire_clipboard_clear_handler(app: &Rc<RefCell<NovaHubApp>>, window: &ClipboardWindow) {
    let app = Rc::clone(app);
    let window_weak = window.as_weak();
    window.on_clear_requested(move || {
        let status = app.borrow_mut().clear_clipboard().map_or_else(
            |error| format!("Clipboard clear failed: {error}"),
            |()| "Clipboard history cleared".to_owned(),
        );
        if let Some(window) = window_weak.upgrade() {
            window.set_status_text(status.into());
            clear_clipboard_rows(&window);
            window.set_preview_kind("".into());
            window.set_preview_text("Clipboard history is empty".into());
        }
    });
}

fn wire_clipboard_pause_handler(app: &Rc<RefCell<NovaHubApp>>, window: &ClipboardWindow) {
    let app = Rc::clone(app);
    let window_weak = window.as_weak();
    window.on_pause_requested(move || {
        let paused = !app.borrow().clipboard_paused();
        app.borrow_mut().set_clipboard_paused(paused);
        if let Some(window) = window_weak.upgrade() {
            window.set_paused(paused);
            window.set_status_text(
                if paused {
                    "Clipboard capture paused"
                } else {
                    "Clipboard capture resumed"
                }
                .into(),
            );
        }
    });
}

fn wire_clipboard_dismiss_handler(shell: &ShellWindow, window: &ClipboardWindow) {
    let shell_weak = shell.as_weak();
    let window_weak = window.as_weak();
    window.on_dismiss_requested(move || {
        if let Some(window) = window_weak.upgrade() {
            let _ = window.hide();
        }
        if let Some(shell) = shell_weak.upgrade() {
            let _ = shell.show();
            shell.invoke_focus_search_requested();
        }
    });
}

fn filtered_clipboard_items(
    app: &mut NovaHubApp,
    filter: ClipboardFilter,
) -> Vec<novahub_providers::clipboard::ClipboardItem> {
    app.clipboard_items(unix_timestamp())
        .into_iter()
        .filter(|item| filter.matches(item.kind))
        .take(CLIPBOARD_HISTORY_SLOT_COUNT)
        .collect()
}

fn refresh_clipboard_window(
    app: &mut NovaHubApp,
    window: &ClipboardWindow,
    filter: ClipboardFilter,
) {
    let items = filtered_clipboard_items(app, filter);
    clear_clipboard_rows(window);
    for (index, item) in items.iter().enumerate() {
        let kind = match item.kind {
            novahub_providers::clipboard::ClipboardKind::Text => "Text",
            novahub_providers::clipboard::ClipboardKind::Image => "Image",
        };
        let title = format!("{kind} - {} bytes", item.size);
        let subtitle = if item.pinned {
            format!("Captured {} - Pinned", item.created_at)
        } else {
            format!("Captured {}", item.created_at)
        };
        set_clipboard_row(
            window,
            index,
            title,
            subtitle,
            true,
            item.pinned,
            item.kind == novahub_providers::clipboard::ClipboardKind::Text,
        );
    }
    if items.is_empty() {
        window.set_preview_kind("".into());
        window.set_preview_text("Clipboard history is empty".into());
    }
    window.set_status_text(format!("{} item(s)", items.len()).into());
}

fn preview_clipboard_item(
    app: &mut NovaHubApp,
    window: &ClipboardWindow,
    filter: ClipboardFilter,
    index: usize,
) {
    let Some(item) = filtered_clipboard_items(app, filter).into_iter().nth(index) else {
        return;
    };
    match item.kind {
        novahub_providers::clipboard::ClipboardKind::Text => {
            match app.read_clipboard_item(item.id, unix_timestamp()) {
                Ok(bytes) => {
                    window.set_preview_kind("Text preview".into());
                    if let Ok(text) = String::from_utf8(bytes) {
                        let preview = text.chars().take(4_096).collect::<String>();
                        window.set_preview_text(preview.into());
                    } else {
                        window.set_preview_text("Clipboard text is not valid UTF-8".into());
                    }
                }
                Err(error) => {
                    window.set_preview_text(format!("Clipboard preview failed: {error}").into());
                }
            }
        }
        novahub_providers::clipboard::ClipboardKind::Image => {
            window.set_preview_kind("Image preview".into());
            window.set_preview_text(
                "Image payload is encrypted and preview decoding is unavailable in this host"
                    .into(),
            );
        }
    }
}

fn wire_tray_handler(
    app: &Rc<RefCell<NovaHubApp>>,
    shell: &ShellWindow,
    pet: &novahub_ui_slint::PetWindow,
    tray: &TrayIcon,
) {
    let shell_window = shell.as_weak();
    tray.on_show_shell_requested(move || {
        if let Some(window) = shell_window.upgrade() {
            let _ = window.show();
            window.window().set_minimized(false);
            window.invoke_focus_search_requested();
        }
    });

    let show_pet_app = Rc::clone(app);
    let pet_window = pet.as_weak();
    tray.on_show_pet_requested(move || {
        let mut app = show_pet_app.borrow_mut();
        let _ = app.dispatch_pet_event(PetEvent::Show);
        if let Some(window) = pet_window.upgrade() {
            let _ = sync_pet_window(&mut app, &window);
        }
    });

    let hide_pet_app = Rc::clone(app);
    let pet_window = pet.as_weak();
    tray.on_hide_pet_requested(move || {
        let mut app = hide_pet_app.borrow_mut();
        let _ = app.dispatch_pet_event(PetEvent::Hide);
        if let Some(window) = pet_window.upgrade() {
            let _ = sync_pet_window(&mut app, &window);
        }
    });

    let shell_window = shell.as_weak();
    let pet_window = pet.as_weak();
    tray.on_quit_requested(move || {
        if let Some(window) = shell_window.upgrade() {
            let _ = window.hide();
        }
        if let Some(window) = pet_window.upgrade() {
            let _ = window.hide();
        }
        quit_event_loop();
    });
}

fn start_application_snapshot_timer(
    app: &Rc<RefCell<NovaHubApp>>,
    window: &ShellWindow,
) -> novahub_ui_slint::Timer {
    let pending: PendingApplicationTask = Rc::new(RefCell::new(None));
    let last_refresh = Rc::new(RefCell::new(None::<Instant>));
    let pending_task = Rc::clone(&pending);
    let last_refresh_task = Rc::clone(&last_refresh);
    let app = Rc::clone(app);
    let window = window.as_weak();
    let timer = novahub_ui_slint::Timer::default();
    timer.start(
        novahub_ui_slint::TimerMode::Repeated,
        Duration::from_millis(50),
        move || {
            let result = {
                let pending = pending_task.borrow();
                pending
                    .as_ref()
                    .and_then(|receiver| match receiver.try_recv() {
                        Ok(result) => Some(result),
                        Err(std::sync::mpsc::TryRecvError::Empty) => None,
                        Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                            Some(Err("application discovery worker stopped".to_owned()))
                        }
                    })
            };
            let Some(result) = result else {
                let refresh_due = pending_task.borrow().is_none()
                    && last_refresh_task.borrow().is_none_or(|last_refresh| {
                        last_refresh.elapsed() >= APPLICATION_REFRESH_INTERVAL
                    });
                if refresh_due {
                    match app.borrow().start_application_discovery() {
                        Ok(receiver) => {
                            pending_task.borrow_mut().replace(receiver);
                        }
                        Err(error) => {
                            *last_refresh_task.borrow_mut() = Some(Instant::now());
                            if let Some(window) = window.upgrade() {
                                set_status_text(
                                    &window,
                                    format!("Application index unavailable: {error}"),
                                );
                            }
                        }
                    }
                }
                return;
            };
            pending_task.borrow_mut().take();
            *last_refresh_task.borrow_mut() = Some(Instant::now());
            match result {
                Ok(applications) => app.borrow().apply_application_snapshot(applications),
                Err(error) => {
                    if let Some(window) = window.upgrade() {
                        set_status_text(&window, format!("Application index unavailable: {error}"));
                    }
                }
            }
        },
    );
    timer
}

fn start_clipboard_timer(app: &Rc<RefCell<NovaHubApp>>) -> novahub_ui_slint::Timer {
    let pending: PendingClipboardTask = Rc::new(RefCell::new(None));
    let pending_task = Rc::clone(&pending);
    let app = Rc::clone(app);
    let timer = novahub_ui_slint::Timer::default();
    timer.start(
        novahub_ui_slint::TimerMode::Repeated,
        Duration::from_millis(100),
        move || {
            let completed = {
                let pending = pending_task.borrow();
                pending
                    .as_ref()
                    .and_then(|receiver| match receiver.try_recv() {
                        Ok(result) => Some(result),
                        Err(std::sync::mpsc::TryRecvError::Empty) => None,
                        Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                            Some(Err("clipboard worker stopped".to_owned()))
                        }
                    })
            };
            if let Some(result) = completed {
                pending_task.borrow_mut().take();
                match result {
                    Ok(payload) => {
                        let _ = app.borrow_mut().capture_clipboard_payload(
                            payload,
                            unix_timestamp(),
                            false,
                        );
                    }
                    Err(error) => {
                        eprintln!("NovaHub clipboard read failed: {error}");
                    }
                }
            }

            if pending_task.borrow().is_none()
                && app.borrow_mut().clipboard_poll_due(Instant::now())
                && let Ok(receiver) = app.borrow().start_clipboard_read()
            {
                pending_task.borrow_mut().replace(receiver);
            }
        },
    );
    timer
}

fn start_file_search_timer(
    window: &ShellWindow,
    plugin_ui: &PluginUiState,
) -> novahub_ui_slint::Timer {
    let pending = Rc::clone(&plugin_ui.pending_file_task);
    let window = window.as_weak();
    let timer = novahub_ui_slint::Timer::default();
    timer.start(
        novahub_ui_slint::TimerMode::Repeated,
        Duration::from_millis(16),
        move || {
            let result = {
                let pending = pending.borrow();
                pending
                    .as_ref()
                    .and_then(|receiver| match receiver.try_recv() {
                        Ok(result) => Some(result),
                        Err(std::sync::mpsc::TryRecvError::Empty) => None,
                        Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                            Some(Err("file search worker stopped".to_owned()))
                        }
                    })
            };
            let Some(result) = result else {
                return;
            };
            pending.borrow_mut().take();
            let (summary, status) = match result {
                Ok(results) => (
                    format_file_results(&results),
                    "File search completed".to_owned(),
                ),
                Err(error) => (
                    "File search failed".to_owned(),
                    format!("File search failed: {error}"),
                ),
            };
            if let Some(window) = window.upgrade() {
                clear_result_rows(&window);
                set_plugin_view(&window, "", "", "");
                set_result_text(&window, summary);
                set_status_text(&window, status);
            }
        },
    );
    timer
}

/// Registers the host launch shortcut only on the supported desktop targets.
/// The manager is captured by the timer so the native registration remains
/// alive for the complete Slint event loop. Registration conflicts are
/// reported and leave the rest of the Shell usable.
#[cfg(any(windows, target_os = "macos"))]
fn start_global_hotkey_timer(
    app: &Rc<RefCell<NovaHubApp>>,
    window: &ShellWindow,
) -> (Option<novahub_ui_slint::Timer>, GlobalHotkeyState) {
    let state = Rc::new(RefCell::new(None));
    let binding = app.borrow().global_hotkey().unwrap_or_else(|error| {
        eprintln!("failed to read global hotkey setting: {error}");
        DEFAULT_GLOBAL_HOTKEY.to_owned()
    });
    let Some(hotkey) = parse_global_hotkey(&binding) else {
        eprintln!("invalid global hotkey setting {binding:?}; using {DEFAULT_GLOBAL_HOTKEY}");
        return (None, state);
    };
    let manager = match GlobalHotKeyManager::new() {
        Ok(manager) => manager,
        Err(error) => {
            eprintln!("global hotkey backend unavailable: {error}");
            return (None, state);
        }
    };
    let registered_hotkey = if let Err(error) = manager.register(hotkey) {
        eprintln!("failed to register {binding}: {error}");
        None
    } else {
        Some(hotkey)
    };
    state.borrow_mut().replace(HotkeyRegistration {
        manager,
        hotkey: registered_hotkey,
    });

    let window = window.as_weak();
    let state_for_timer = Rc::clone(&state);
    let timer = novahub_ui_slint::Timer::default();
    timer.start(
        novahub_ui_slint::TimerMode::Repeated,
        Duration::from_millis(40),
        move || {
            let current_hotkey = state_for_timer
                .borrow()
                .as_ref()
                .and_then(|entry| entry.hotkey);
            for event in GlobalHotKeyEvent::receiver().try_iter() {
                if current_hotkey.is_some_and(|hotkey| event.id == hotkey.id())
                    && event.state == HotKeyState::Released
                    && let Some(window) = window.upgrade()
                {
                    let _ = activate_shell_window(&window);
                }
            }
        },
    );
    (Some(timer), state)
}

#[cfg(any(windows, target_os = "macos"))]
fn parse_global_hotkey(binding: &str) -> Option<HotKey> {
    let mut parts = binding.split('+');
    let modifier = parts.next()?;
    if parts.next()? != "Space" || parts.next().is_some() {
        return None;
    }
    let modifiers = match modifier {
        "Alt" | "Option" => Modifiers::ALT,
        "Control" => Modifiers::CONTROL,
        "Shift" => Modifiers::SHIFT,
        "Meta" => Modifiers::META,
        _ => return None,
    };
    Some(HotKey::new(Some(modifiers), Code::Space))
}

#[cfg(any(windows, target_os = "macos"))]
fn apply_global_hotkey(state: &GlobalHotkeyState, binding: &str) -> Result<(), String> {
    let new_hotkey =
        parse_global_hotkey(binding).ok_or_else(|| "invalid global hotkey binding".to_owned())?;
    let mut registration = state.borrow_mut();
    let Some(registration) = registration.as_mut() else {
        return Err("global hotkey backend is unavailable".to_owned());
    };
    if registration.hotkey == Some(new_hotkey) {
        return Ok(());
    }
    registration
        .manager
        .register(new_hotkey)
        .map_err(|error| format!("failed to register global hotkey: {error}"))?;
    if let Some(old_hotkey) = registration.hotkey
        && let Err(error) = registration.manager.unregister(old_hotkey)
    {
        let _ = registration.manager.unregister(new_hotkey);
        return Err(format!("failed to replace global hotkey: {error}"));
    }
    registration.hotkey = Some(new_hotkey);
    Ok(())
}

#[cfg(not(any(windows, target_os = "macos")))]
fn apply_global_hotkey(_state: &GlobalHotkeyState, _binding: &str) -> Result<(), String> {
    Ok(())
}

#[cfg(not(any(windows, target_os = "macos")))]
fn start_global_hotkey_timer(
    _app: &Rc<RefCell<NovaHubApp>>,
    _window: &ShellWindow,
) -> (Option<novahub_ui_slint::Timer>, GlobalHotkeyState) {
    (None, Rc::new(RefCell::new(())))
}

fn sync_pet_window(
    app: &mut NovaHubApp,
    window: &novahub_ui_slint::PetWindow,
) -> Result<(), String> {
    if let Some(frame) = app.pet_frame() {
        set_pet_frame_with_bytes(window, &frame, app.pet_frame_bytes(&frame.asset));
    }
    let actions = app.pet_visible_actions();
    let shelf_label = actions
        .iter()
        .map(|action| action.title.as_str())
        .collect::<Vec<_>>()
        .join(" | ");
    set_pet_shelf(
        window,
        matches!(app.pet_state(), novahub_core_domain::pet::PetState::Shelf),
        if shelf_label.is_empty() {
            "No pinned actions"
        } else {
            &shelf_label
        },
    );
    set_pet_shelf_actions(window, &actions);
    let visible = !matches!(app.pet_state(), novahub_core_domain::pet::PetState::Hidden);
    set_pet_visible(window, visible).map_err(|error| error.to_string())?;
    if visible {
        ensure_pet_position(app, window);
    }
    Ok(())
}

fn ensure_pet_position(app: &mut NovaHubApp, window: &novahub_ui_slint::PetWindow) {
    if app.pet_position().is_some() {
        return;
    }
    let _ = place_pet_window(window);
    let Some(area) = current_monitor_work_area(window) else {
        return;
    };
    let size = (360_u32, 160_u32);
    if let Some(saved) = app.saved_pet_position() {
        let _ = app.restore_pet_position(saved, size, std::slice::from_ref(&area));
        set_native_pet_position(window, saved.point);
        return;
    }
    let position = window.window().position();
    let scale = window.window().scale_factor().max(1.0);
    let point = LogicalPoint {
        x: logical_from_physical(position.x, scale),
        y: logical_from_physical(position.y, scale),
    };
    let _ = app.set_pet_position(area, point, size);
}

fn set_native_pet_position(window: &novahub_ui_slint::PetWindow, point: LogicalPoint) {
    let scale = window.window().scale_factor().max(1.0);
    window.window().set_position(PhysicalPosition::new(
        physical_from_logical(point.x, scale),
        physical_from_logical(point.y, scale),
    ));
}

#[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
fn logical_from_physical(value: i32, scale: f32) -> i32 {
    (value as f32 / scale).round() as i32
}

#[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
fn physical_from_logical(value: i32, scale: f32) -> i32 {
    (value as f32 * scale).round() as i32
}

fn wire_pet_handler(app: &Rc<RefCell<NovaHubApp>>, window: &novahub_ui_slint::PetWindow) {
    let app = Rc::clone(app);
    let pet_window = window.as_weak();
    window.on_pet_clicked(move || {
        let mut app = app.borrow_mut();
        let event = match app.pet_state() {
            novahub_core_domain::pet::PetState::Success
            | novahub_core_domain::pet::PetState::Error => PetEvent::Dismiss,
            _ => PetEvent::OpenShelf,
        };
        let _ = app.dispatch_pet_event(event);
        if let Some(window) = pet_window.upgrade()
            && let Err(error) = sync_pet_window(&mut app, &window)
        {
            eprintln!("NovaHub pet window update failed: {error}");
        }
    });
}

fn wire_pet_drag_handler(app: &Rc<RefCell<NovaHubApp>>, window: &novahub_ui_slint::PetWindow) {
    let start_app = Rc::clone(app);
    let end_app = Rc::clone(app);
    let origin = Rc::new(RefCell::new(None::<PhysicalPosition>));

    let start_origin = Rc::clone(&origin);
    let start_window = window.as_weak();
    window.on_pet_drag_started(move || {
        if let Some(window) = start_window.upgrade() {
            start_origin
                .borrow_mut()
                .replace(window.window().position());
        }
        let _ = start_app
            .borrow_mut()
            .dispatch_pet_event(PetEvent::DragStart);
    });

    let move_origin = Rc::clone(&origin);
    let drag_window = window.as_weak();
    window.on_pet_drag_moved(move |delta_x, delta_y| {
        let Some(start) = *move_origin.borrow() else {
            return;
        };
        if let Some(window) = drag_window.upgrade() {
            window.window().set_position(PhysicalPosition::new(
                add_drag_delta(start.x, delta_x),
                add_drag_delta(start.y, delta_y),
            ));
        }
    });

    let end_origin = Rc::clone(&origin);
    let end_window = window.as_weak();
    window.on_pet_drag_ended(move || {
        end_origin.borrow_mut().take();
        let mut app = end_app.borrow_mut();
        let _ = app.dispatch_pet_event(PetEvent::DragEnd);
        if let Some(window) = end_window.upgrade()
            && let Some(area) = current_monitor_work_area(&window)
        {
            let scale = window.window().scale_factor().max(1.0);
            let physical = window.window().position();
            let point = LogicalPoint {
                x: logical_from_physical(physical.x, scale),
                y: logical_from_physical(physical.y, scale),
            };
            let _ = app.set_pet_position(area, point, (360, 160));
        }
    });
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn add_drag_delta(origin: i32, delta: f32) -> i32 {
    let value = f64::from(origin) + f64::from(delta.round());
    value.clamp(f64::from(i32::MIN), f64::from(i32::MAX)) as i32
}

fn wire_pet_action_handler(
    app: &Rc<RefCell<NovaHubApp>>,
    shell: &ShellWindow,
    window: &novahub_ui_slint::PetWindow,
    plugin_ui: &PluginUiState,
) {
    let app = Rc::clone(app);
    let shell = shell.as_weak();
    let pet_window = window.as_weak();
    let pending_plugin_task = Rc::clone(&plugin_ui.pending_task);
    let active_plugin_source = Rc::clone(&plugin_ui.source);
    window.on_shelf_action_clicked(move |index| {
        let mut app = app.borrow_mut();
        let Some(action) = usize::try_from(index)
            .ok()
            .and_then(|index| app.pet_action_at(index))
        else {
            return;
        };
        if app.dispatch_pet_event(PetEvent::StartAction).is_err() {
            return;
        }
        let status = if plugin_command_plugin_id(&action.target).is_some() {
            start_plugin_command_by_id(
                &app,
                &action.target,
                "",
                &pending_plugin_task,
                &active_plugin_source,
            )
        } else {
            run_query(&mut app, &action.target)
        };
        if status == "Plugin command started" {
            if let Some(shell) = shell.upgrade() {
                set_status_text(&shell, status);
            }
            return;
        }
        let failed = status.contains("failed") || status.contains("Failed");
        let event = if failed {
            PetEvent::ActionFailed
        } else {
            PetEvent::ActionSucceeded
        };
        let _ = app.dispatch_pet_event(event);
        if let Some(window) = pet_window.upgrade()
            && let Err(error) = sync_pet_window(&mut app, &window)
        {
            eprintln!("NovaHub pet window update failed: {error}");
        }
        if let Some(shell) = shell.upgrade() {
            set_status_text(&shell, status);
        }
    });
}

fn run_headless(app: &NovaHubApp, process_started: Instant) {
    let component_path = std::env::var_os("NOVAHUB_COMPONENT_FIXTURE").map(PathBuf::from);
    let reference_sessions = parse_reference_session_count(
        std::env::var_os("NOVAHUB_HEADLESS_PLUGIN_SESSIONS").as_deref(),
    )
    .unwrap_or_else(|error| exit_with_headless_error(&error));
    let reference_hold = parse_bounded_duration(
        "NOVAHUB_HEADLESS_HOLD_MS",
        std::env::var_os("NOVAHUB_HEADLESS_HOLD_MS").as_deref(),
    )
    .unwrap_or_else(|error| exit_with_headless_error(&error))
    .unwrap_or(Duration::ZERO);

    if let Some(session_count) = reference_sessions {
        let component_path = component_path.as_deref().unwrap_or_else(|| {
            exit_with_headless_error(
                "NOVAHUB_COMPONENT_FIXTURE is required for reference plugin sessions",
            )
        });
        match run_reference_plugin_sessions(
            component_path,
            session_count,
            reference_hold,
            process_started,
        ) {
            Ok(runs) => {
                for run in runs {
                    println!(
                        "NovaHub plugin fixture completed: {}",
                        format_plugin_run(&run)
                    );
                }
            }
            Err(error) => exit_with_headless_error(&error),
        }
    } else if let Some(component_path) = component_path {
        match app.run_component_fixture(
            "headless-fixture",
            component_path,
            r#"{"source":"headless"}"#,
        ) {
            Ok(run) => println!(
                "NovaHub plugin fixture completed: {}",
                format_plugin_run(&run)
            ),
            Err(error) => {
                exit_with_headless_error(&format!("NovaHub plugin fixture failed: {error}"))
            }
        }
    } else if !reference_hold.is_zero() {
        exit_with_headless_error(
            "NOVAHUB_HEADLESS_HOLD_MS requires NOVAHUB_HEADLESS_PLUGIN_SESSIONS",
        );
    }
    println!(
        "NovaHub host bootstrap ({} builtin command)",
        app.search("").len()
    );
}

fn parse_reference_session_count(value: Option<&OsStr>) -> Result<Option<usize>, String> {
    let Some(value) = value else {
        return Ok(None);
    };
    let value = value
        .to_str()
        .ok_or_else(|| "NOVAHUB_HEADLESS_PLUGIN_SESSIONS must be UTF-8".to_owned())?;
    let count = value
        .parse::<usize>()
        .map_err(|_| "NOVAHUB_HEADLESS_PLUGIN_SESSIONS must be an integer".to_owned())?;
    if !(1..=MAX_REFERENCE_PLUGIN_SESSIONS).contains(&count) {
        return Err(format!(
            "NOVAHUB_HEADLESS_PLUGIN_SESSIONS must be between 1 and {MAX_REFERENCE_PLUGIN_SESSIONS}"
        ));
    }
    Ok(Some(count))
}

fn run_reference_plugin_sessions(
    component_path: &std::path::Path,
    session_count: usize,
    hold: Duration,
    process_started: Instant,
) -> Result<Vec<PluginFixtureRun>, String> {
    let (ready_sender, ready_receiver) = mpsc::channel::<Result<u32, String>>();
    let mut releases = Vec::with_capacity(session_count);
    let mut workers = Vec::with_capacity(session_count);

    for index in 0..session_count {
        let component_path = component_path.to_owned();
        let ready_sender = ready_sender.clone();
        let (release_sender, release_receiver) = mpsc::sync_channel(1);
        releases.push(release_sender);
        workers.push(std::thread::spawn(move || {
            let prepared = prepare_reference_plugin_session(index, &component_path);
            let ready = prepared
                .as_ref()
                .map(|session| session.host_pid)
                .map_err(Clone::clone);
            let _ = ready_sender.send(ready);
            let _ = release_receiver.recv();
            prepared.and_then(PreparedReferenceSession::close)
        }));
    }
    drop(ready_sender);

    let ready_deadline = Instant::now() + REFERENCE_SESSION_READY_TIMEOUT;
    let mut ready_pids = Vec::with_capacity(session_count);
    let mut ready_error = None;
    for _ in 0..session_count {
        let remaining = ready_deadline.saturating_duration_since(Instant::now());
        match ready_receiver.recv_timeout(remaining) {
            Ok(Ok(pid)) => ready_pids.push(pid),
            Ok(Err(error)) => {
                ready_error.get_or_insert(error);
            }
            Err(error) => {
                ready_error.get_or_insert_with(|| {
                    format!("reference plugin sessions did not become ready: {error}")
                });
                break;
            }
        }
    }

    if ready_error.is_none() {
        write_json_marker(
            "NOVAHUB_HEADLESS",
            &json!({
                "event": "sessions_ready",
                "elapsed_ms": process_started.elapsed().as_secs_f64() * 1_000.0,
                "hold_ms": hold.as_millis(),
                "host_pids": ready_pids,
                "sessions": session_count,
            }),
        )
        .map_err(|error| error.to_string())?;
        std::thread::sleep(hold);
    }

    for release in releases {
        let _ = release.send(());
    }
    let mut runs = Vec::with_capacity(session_count);
    for worker in workers {
        match worker.join() {
            Ok(Ok(run)) => runs.push(run),
            Ok(Err(error)) => {
                ready_error.get_or_insert(error);
            }
            Err(_) => {
                ready_error
                    .get_or_insert_with(|| "reference plugin session worker panicked".to_owned());
            }
        }
    }
    ready_error.map_or(Ok(runs), Err)
}

struct PreparedReferenceSession {
    client: PluginHostClient,
    session_id: String,
    host_pid: u32,
    loaded: serde_json::Value,
    opened: serde_json::Value,
    updated: serde_json::Value,
}

impl PreparedReferenceSession {
    fn close(mut self) -> Result<PluginFixtureRun, String> {
        let closed = self
            .client
            .close(&self.session_id)
            .map_err(|error| error.to_string())?;
        Ok(PluginFixtureRun {
            host_pid: self.host_pid,
            loaded: self.loaded,
            opened: self.opened,
            updated: self.updated,
            closed,
        })
    }
}

fn prepare_reference_plugin_session(
    index: usize,
    component_path: &std::path::Path,
) -> Result<PreparedReferenceSession, String> {
    let session_id = format!("headless-reference-{index}");
    let mut client = PluginHostClient::spawn_default().map_err(|error| error.to_string())?;
    let host_pid = client.process_id();
    let loaded = client
        .load(&session_id, component_path)
        .map_err(|error| error.to_string())?;
    let opened = client
        .open(&session_id)
        .map_err(|error| error.to_string())?;
    let revision = opened
        .get("revision")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| "reference plugin open response has no revision".to_owned())?;
    let input = json!({ "session": index, "source": "headless-reference" }).to_string();
    let updated = client
        .update(&session_id, revision, &input)
        .map_err(|error| error.to_string())?;
    Ok(PreparedReferenceSession {
        client,
        session_id,
        host_pid,
        loaded,
        opened,
        updated,
    })
}

fn exit_with_headless_error(error: &str) -> ! {
    eprintln!("NovaHub headless probe failed: {error}");
    std::process::exit(1);
}

fn wire_query_handler(
    app: &Rc<RefCell<NovaHubApp>>,
    window: &ShellWindow,
    query: &Rc<RefCell<String>>,
    selected: &Rc<RefCell<usize>>,
    provider_filter: &ProviderFilterState,
    plugin_ui: &PluginUiState,
) {
    let app = Rc::clone(app);
    let query = Rc::clone(query);
    let selected = Rc::clone(selected);
    let provider_filter = Rc::clone(provider_filter);
    let plugin_ui = plugin_ui.clone();
    let query_window = window.as_weak();
    window.on_query_changed(move |value| {
        let value = value.to_string();
        query.borrow_mut().clone_from(&value);
        *selected.borrow_mut() = 0;
        plugin_ui.clear_active_view();
        if let Some(window) = query_window.upgrade() {
            set_action_panel(&window, false, "Actions", "", "", "");
            set_plugin_view(&window, "", "", "");
            let filter = provider_filter.borrow().clone();
            set_search_results(&window, &app.borrow(), &value, 0, filter.as_deref());
        }
    });
}

fn wire_provider_filter_handler(
    app: &Rc<RefCell<NovaHubApp>>,
    window: &ShellWindow,
    query: &Rc<RefCell<String>>,
    selected: &Rc<RefCell<usize>>,
    provider_filter: &ProviderFilterState,
) {
    let app = Rc::clone(app);
    let query = Rc::clone(query);
    let selected = Rc::clone(selected);
    let provider_filter = Rc::clone(provider_filter);
    let filter_window = window.as_weak();
    window.on_provider_filter_requested(move || {
        let next_filter = next_provider_filter(provider_filter.borrow().as_deref());
        *provider_filter.borrow_mut() = next_filter;
        *selected.borrow_mut() = 0;

        let Some(window) = filter_window.upgrade() else {
            return;
        };
        let query_text = query.borrow().clone();
        let filter = provider_filter.borrow().clone();
        set_provider_filter_label(&window, provider_filter_label(filter.as_deref()));
        set_search_results(&window, &app.borrow(), &query_text, 0, filter.as_deref());
    });
}

fn wire_selection_handlers(
    app: &Rc<RefCell<NovaHubApp>>,
    window: &ShellWindow,
    query: &Rc<RefCell<String>>,
    selected: &Rc<RefCell<usize>>,
    provider_filter: &ProviderFilterState,
) {
    let app = Rc::clone(app);
    let query = Rc::clone(query);
    let selected = Rc::clone(selected);
    let provider_filter = Rc::clone(provider_filter);
    let result_window = window.as_weak();
    window.on_selection_moved(move |delta| {
        let query_text = query.borrow().clone();
        let filter = provider_filter.borrow().clone();
        let result_count = app
            .borrow()
            .search_provider(&query_text, filter.as_deref())
            .len()
            .min(8);
        if result_count == 0 {
            *selected.borrow_mut() = 0;
        } else {
            let current = *selected.borrow();
            let next = if delta < 0 {
                current.saturating_sub(1)
            } else {
                current.saturating_add(1).min(result_count - 1)
            };
            *selected.borrow_mut() = next;
        }
        if let Some(window) = result_window.upgrade() {
            set_search_results(
                &window,
                &app.borrow(),
                &query_text,
                *selected.borrow(),
                filter.as_deref(),
            );
        }
    });
}

fn wire_result_handlers(window: &ShellWindow, selected: &Rc<RefCell<usize>>) {
    let selected = Rc::clone(selected);
    let shell = window.as_weak();
    window.on_result_row_activated(move |index| {
        let Ok(index) = usize::try_from(index) else {
            return;
        };
        *selected.borrow_mut() = index.min(SEARCH_RESULT_SLOT_COUNT - 1);
        if let Some(window) = shell.upgrade() {
            window.invoke_run_requested();
        }
    });
}

fn wire_action_panel_handlers(
    app: &Rc<RefCell<NovaHubApp>>,
    window: &ShellWindow,
    query: &Rc<RefCell<String>>,
    selected: &Rc<RefCell<usize>>,
    provider_filter: &ProviderFilterState,
    plugin_ui: &PluginUiState,
) {
    let app_for_panel = Rc::clone(app);
    let query = Rc::clone(query);
    let selected = Rc::clone(selected);
    let provider_filter = Rc::clone(provider_filter);
    let plugin_confirmation = Rc::clone(&plugin_ui.pending_confirmation);
    let shell = window.as_weak();
    window.on_action_panel_requested(move || {
        let Some(window) = shell.upgrade() else {
            return;
        };
        if app_for_panel.borrow().has_pending_confirmation() {
            show_pending_confirmation(&window, "Confirm or cancel the pending system action");
            return;
        }
        if let Some(action) = plugin_confirmation.borrow().as_ref() {
            show_plugin_confirmation(&window, action);
            return;
        }
        if window.get_action_panel_visible() {
            set_action_panel(&window, false, "Actions", "", "", "");
            return;
        }
        let query_text = query.borrow().clone();
        let selected_index = *selected.borrow();
        let filter = provider_filter.borrow().clone();
        let command = app_for_panel
            .borrow()
            .search_provider(&query_text, filter.as_deref())
            .into_iter()
            .take(SEARCH_RESULT_SLOT_COUNT)
            .nth(selected_index);
        let Some(command) = command else {
            set_status_text(&window, "No command is selected");
            return;
        };
        set_action_panel(
            &window,
            true,
            "Actions",
            format!("{} - {}", command.title, command.subtitle),
            "Run selected command",
            "Close action panel",
        );
    });

    let app_for_dismiss = Rc::clone(app);
    let plugin_confirmation = Rc::clone(&plugin_ui.pending_confirmation);
    let shell = window.as_weak();
    window.on_action_panel_dismiss_requested(move || {
        if let Some(window) = shell.upgrade() {
            set_action_panel(&window, false, "Actions", "", "", "");
            if plugin_confirmation.borrow_mut().take().is_some() {
                set_status_text(&window, "Plugin action cancelled");
            } else if app_for_dismiss.borrow_mut().cancel_pending_confirmation() {
                set_status_text(&window, "System action cancelled");
            }
        }
    });

    let app_for_action = Rc::clone(app);
    let plugin_confirmation = Rc::clone(&plugin_ui.pending_confirmation);
    let plugin_source = Rc::clone(&plugin_ui.source);
    let plugin_pending_task = Rc::clone(&plugin_ui.pending_task);
    let shell = window.as_weak();
    window.on_action_panel_action_requested(move |index| {
        let Some(window) = shell.upgrade() else {
            return;
        };
        set_action_panel(&window, false, "Actions", "", "", "");
        if let Some(action) = plugin_confirmation.borrow_mut().take() {
            let status = if index == 0 {
                start_plugin_event_task(
                    &plugin_source,
                    &plugin_pending_task,
                    &json!({"event": "action", "id": action.id}),
                )
            } else {
                "Plugin action cancelled".to_owned()
            };
            set_status_text(&window, status);
            return;
        }
        if app_for_action.borrow().has_pending_confirmation() {
            let status = if index == 0 {
                app_for_action
                    .borrow_mut()
                    .confirm_pending_action()
                    .unwrap_or_else(|error| format!("System action failed: {error:?}"))
            } else if app_for_action.borrow_mut().cancel_pending_confirmation() {
                "System action cancelled".to_owned()
            } else {
                "No system action is pending".to_owned()
            };
            set_status_text(&window, status);
            return;
        }
        if index == 0 {
            window.invoke_run_requested();
        } else {
            window.invoke_focus_search_requested();
        }
    });
}

fn show_pending_confirmation(window: &ShellWindow, description: impl AsRef<str>) {
    set_action_panel(
        window,
        true,
        "Confirm system action",
        description,
        "Confirm",
        "Cancel",
    );
}

fn show_plugin_confirmation(window: &ShellWindow, action: &PluginViewAction) {
    set_action_panel(
        window,
        true,
        "Confirm plugin action",
        format!("{} requests: {}", action.id, action.title),
        "Confirm",
        "Cancel",
    );
}

fn wire_focus_handlers(
    app: &Rc<RefCell<NovaHubApp>>,
    window: &ShellWindow,
    query: &Rc<RefCell<String>>,
    selected: &Rc<RefCell<usize>>,
    plugin_ui: &PluginUiState,
) {
    let dismiss_window = window.as_weak();
    let dismiss_app = Rc::clone(app);
    let query = Rc::clone(query);
    let selected = Rc::clone(selected);
    let plugin_surface = Rc::clone(&plugin_ui.surface);
    let plugin_form_dirty = Rc::clone(&plugin_ui.form_dirty);
    let plugin_confirmation = Rc::clone(&plugin_ui.pending_confirmation);
    window.on_dismiss_requested(move || {
        let query_text = query.borrow().clone();
        let has_dirty_form = plugin_surface
            .borrow()
            .as_ref()
            .is_some_and(|surface| surface.kind == "form" && *plugin_form_dirty.borrow());
        match dismiss_intent(
            dismiss_app.borrow().has_pending_confirmation()
                || plugin_confirmation.borrow().is_some(),
            has_dirty_form,
            &query_text,
        ) {
            DismissIntent::CancelConfirmation => {
                let plugin_cancelled = plugin_confirmation.borrow_mut().take().is_some();
                if !plugin_cancelled {
                    dismiss_app.borrow_mut().cancel_pending_confirmation();
                }
                if let Some(window) = dismiss_window.upgrade() {
                    set_action_panel(&window, false, "Actions", "", "", "");
                    set_status_text(
                        &window,
                        if plugin_cancelled {
                            "Plugin action cancelled"
                        } else {
                            "System action cancelled"
                        },
                    );
                    window.invoke_focus_search_requested();
                }
            }
            DismissIntent::ClearQueryAndRefocus => {
                query.borrow_mut().clear();
                *selected.borrow_mut() = 0;
                if let Some(window) = dismiss_window.upgrade() {
                    window.set_query_text("".into());
                    set_action_panel(&window, false, "Actions", "", "", "");
                    clear_result_rows(&window);
                    set_result_text(&window, "Start typing to search NovaHub");
                    window.invoke_focus_search_requested();
                }
            }
            DismissIntent::KeepFormOpen => {
                if let Some(window) = dismiss_window.upgrade() {
                    set_status_text(
                        &window,
                        "Form edits are preserved until you submit or clear them",
                    );
                    window.invoke_focus_search_requested();
                }
            }
            DismissIntent::HideWindow => {
                if let Some(window) = dismiss_window.upgrade() {
                    let _ = window.hide();
                }
            }
        }
    });

    let blur_window = window.as_weak();
    let blur_app = Rc::clone(app);
    let blur_surface = Rc::clone(&plugin_ui.surface);
    let blur_form_dirty = Rc::clone(&plugin_ui.form_dirty);
    let blur_plugin_confirmation = Rc::clone(&plugin_ui.pending_confirmation);
    window.on_shell_focus_lost(move || {
        let has_dirty_form = blur_surface
            .borrow()
            .as_ref()
            .is_some_and(|surface| surface.kind == "form" && *blur_form_dirty.borrow());
        let has_pending_confirmation = blur_app.borrow().has_pending_confirmation()
            || blur_plugin_confirmation.borrow().is_some();
        if should_hide_on_focus_loss(has_pending_confirmation, has_dirty_form)
            && let Some(window) = blur_window.upgrade()
        {
            let _ = window.hide();
        } else if has_dirty_form && let Some(window) = blur_window.upgrade() {
            set_status_text(
                &window,
                "Form edits are preserved while NovaHub is unfocused",
            );
        }
    });
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DismissIntent {
    CancelConfirmation,
    ClearQueryAndRefocus,
    KeepFormOpen,
    HideWindow,
}

fn dismiss_intent(
    has_pending_confirmation: bool,
    has_dirty_form: bool,
    query: &str,
) -> DismissIntent {
    if has_pending_confirmation {
        DismissIntent::CancelConfirmation
    } else if has_dirty_form {
        DismissIntent::KeepFormOpen
    } else if !query.trim().is_empty() {
        DismissIntent::ClearQueryAndRefocus
    } else {
        DismissIntent::HideWindow
    }
}

const fn should_hide_on_focus_loss(has_pending_confirmation: bool, has_dirty_form: bool) -> bool {
    !has_pending_confirmation && !has_dirty_form
}

struct RunHandlerContext<'a> {
    app: &'a Rc<RefCell<NovaHubApp>>,
    query: &'a Rc<RefCell<String>>,
    selected: &'a Rc<RefCell<usize>>,
    provider_filter: &'a ProviderFilterState,
    pet_window: &'a novahub_ui_slint::PetWindow,
    plugin_ui: &'a PluginUiState,
    clipboard_window: &'a ClipboardWindowState,
    clipboard_filter: &'a ClipboardFilterState,
}

// Keeping the complete Run callback in one place makes command precedence
// auditable across plugin, file, host and selected-result paths.
#[allow(clippy::too_many_lines)]
fn wire_run_handler(window: &ShellWindow, context: &RunHandlerContext<'_>) {
    let app = Rc::clone(context.app);
    let query = Rc::clone(context.query);
    let selected = Rc::clone(context.selected);
    let provider_filter = Rc::clone(context.provider_filter);
    let pet_window = context.pet_window.as_weak();
    let run_window = window.as_weak();
    let pending_plugin_task = Rc::clone(&context.plugin_ui.pending_task);
    let pending_file_task = Rc::clone(&context.plugin_ui.pending_file_task);
    let active_plugin_source = Rc::clone(&context.plugin_ui.source);
    let active_plugin_surface = Rc::clone(&context.plugin_ui.surface);
    let plugin_form_values = Rc::clone(&context.plugin_ui.form_values);
    let plugin_form_dirty = Rc::clone(&context.plugin_ui.form_dirty);
    let plugin_item_offset = Rc::clone(&context.plugin_ui.item_offset);
    let plugin_confirmation = Rc::clone(&context.plugin_ui.pending_confirmation);
    let clipboard_window = Rc::clone(context.clipboard_window);
    let clipboard_filter = Rc::clone(context.clipboard_filter);
    window.on_run_requested(move || {
        let raw_query = query.borrow().trim().to_owned();
        if let Some(window) = run_window.upgrade() {
            set_action_panel(&window, false, "Actions", "", "", "");
        }
        *plugin_form_dirty.borrow_mut() = false;
        *plugin_item_offset.borrow_mut() = 0;
        plugin_confirmation.borrow_mut().take();
        if let Some(status) = run_plugin_permission_query(&app.borrow(), &raw_query) {
            if let Some(window) = run_window.upgrade() {
                clear_result_rows(&window);
                set_plugin_view(&window, "", "", "");
                set_status_text(&window, status);
            }
            return;
        }
        if let Some(invocation) = parse_plugin_query(&raw_query) {
            let status = start_plugin_invocation(
                &app.borrow(),
                invocation,
                &pending_plugin_task,
                &active_plugin_source,
            );
            if let Some(window) = run_window.upgrade() {
                clear_result_rows(&window);
                set_plugin_view(&window, "loading", "Plugin", "Loading plugin view...");
                set_status_text(&window, status);
            }
            return;
        }
        if let Some((root, file_query)) = parse_file_search_query(&raw_query) {
            let status =
                start_file_search_task(&app.borrow(), root, file_query, &pending_file_task);
            if let Some(window) = run_window.upgrade() {
                clear_result_rows(&window);
                set_result_text(&window, "Searching indexed files...");
                set_status_text(&window, status);
            }
            return;
        }
        let filter = provider_filter.borrow().clone();
        let selected_command = app
            .borrow()
            .search_provider(&raw_query, filter.as_deref())
            .into_iter()
            .take(8)
            .nth(*selected.borrow());
        if let Some(window) = run_window.upgrade()
            && handle_selected_host_command(
                selected_command.as_ref(),
                &app,
                &window,
                &clipboard_window,
                &clipboard_filter,
            )
        {
            return;
        }
        if selected_command
            .as_ref()
            .is_some_and(|command| plugin_command_plugin_id(command.id.as_str()).is_some())
        {
            let input = plugin_command_input(&raw_query);
            let command_id = selected_command
                .as_ref()
                .map_or("", |command| command.id.as_str());
            let status = start_plugin_command_by_id(
                &app.borrow(),
                command_id,
                input,
                &pending_plugin_task,
                &active_plugin_source,
            );
            if let Some(window) = run_window.upgrade() {
                set_plugin_view(&window, "loading", "Plugin", "Loading plugin view...");
                set_status_text(&window, status);
            }
            return;
        }
        let mut app = app.borrow_mut();
        let status = run_query_with_selection(&mut app, &raw_query, selected_command.as_ref());
        let confirmation_pending = app.has_pending_confirmation();
        if let Some(pet_window) = pet_window.upgrade()
            && let Err(error) = sync_pet_window(&mut app, &pet_window)
        {
            eprintln!("NovaHub pet window update failed: {error}");
        }
        if let Some(window) = run_window.upgrade() {
            clear_result_rows(&window);
            set_plugin_view(&window, "", "", "");
            set_status_text(&window, &status);
            if confirmation_pending {
                show_pending_confirmation(&window, &status);
            }
        }
        active_plugin_source.borrow_mut().take();
        active_plugin_surface.borrow_mut().take();
        plugin_form_values.borrow_mut().clear();
        *plugin_form_dirty.borrow_mut() = false;
        *plugin_item_offset.borrow_mut() = 0;
    });
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SearchResultRowData {
    title: String,
    subtitle: String,
    selected: bool,
}

fn show_diagnostics(window: &ShellWindow, app: &Rc<RefCell<NovaHubApp>>) {
    clear_result_rows(window);
    window.set_diagnostics_failures_only(false);
    set_plugin_view(window, "diagnostics", "Diagnostics", "");
    refresh_diagnostics_preview(window, app, false);
    set_status_text(window, "Diagnostics opened");
}

const MAX_DIAGNOSTIC_PREVIEW_CHARS: usize = 12_000;

fn refresh_diagnostics_preview(
    window: &ShellWindow,
    app: &Rc<RefCell<NovaHubApp>>,
    failures_only: bool,
) {
    let options = DiagnosticExportOptions {
        max_events: DiagnosticExportOptions::default().max_events,
        failures_only,
    };
    let preview = app
        .borrow()
        .diagnostic_preview_with_options(options)
        .map_or_else(
            |error| format!("Diagnostic preview failed: {error}"),
            |preview| preview.chars().take(MAX_DIAGNOSTIC_PREVIEW_CHARS).collect(),
        );
    set_plugin_view(window, "diagnostics", "Diagnostics", preview);
}

fn wire_diagnostics_handlers(app: &Rc<RefCell<NovaHubApp>>, shell: &ShellWindow) {
    let app_for_preview = Rc::clone(app);
    let shell_for_preview = shell.as_weak();
    shell.on_diagnostics_preview_requested(move || {
        if let Some(shell) = shell_for_preview.upgrade() {
            let failures_only = shell.get_diagnostics_failures_only();
            refresh_diagnostics_preview(&shell, &app_for_preview, failures_only);
            set_status_text(&shell, "Diagnostics preview refreshed");
        }
    });

    let app_for_filter = Rc::clone(app);
    let shell_for_filter = shell.as_weak();
    shell.on_diagnostics_filter_requested(move || {
        if let Some(shell) = shell_for_filter.upgrade() {
            let failures_only = !shell.get_diagnostics_failures_only();
            shell.set_diagnostics_failures_only(failures_only);
            refresh_diagnostics_preview(&shell, &app_for_filter, failures_only);
            set_status_text(
                &shell,
                if failures_only {
                    "Showing diagnostic failures only"
                } else {
                    "Showing all diagnostic events"
                },
            );
        }
    });

    let app_for_clear = Rc::clone(app);
    let shell_for_clear = shell.as_weak();
    shell.on_diagnostics_clear_requested(move || {
        app_for_clear.borrow().clear_diagnostics();
        if let Some(shell) = shell_for_clear.upgrade() {
            refresh_diagnostics_preview(
                &shell,
                &app_for_clear,
                shell.get_diagnostics_failures_only(),
            );
            set_status_text(&shell, "Diagnostics cleared");
        }
    });

    let app_for_export = Rc::clone(app);
    let shell_for_export = shell.as_weak();
    shell.on_diagnostics_export_requested(move || {
        let result = (|| {
            let directory = default_data_dir()?;
            std::fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
            let destination = directory.join(format!("diagnostics-{}.json", unix_timestamp()));
            let failures_only = shell_for_export
                .upgrade()
                .as_ref()
                .is_some_and(ShellWindow::get_diagnostics_failures_only);
            let bytes = app_for_export.borrow().export_diagnostics(
                &destination,
                DiagnosticExportOptions {
                    max_events: DiagnosticExportOptions::default().max_events,
                    failures_only,
                },
            )?;
            Ok::<_, String>((destination, bytes))
        })();
        if let Some(shell) = shell_for_export.upgrade() {
            match result {
                Ok((destination, bytes)) => set_status_text(
                    &shell,
                    format!(
                        "Diagnostics exported: {} ({} bytes)",
                        destination.display(),
                        bytes
                    ),
                ),
                Err(error) => {
                    set_status_text(&shell, format!("Diagnostics export failed: {error}"));
                }
            }
        }
    });
}

fn handle_selected_host_command(
    selected: Option<&novahub_core_domain::CommandDescriptor>,
    app: &Rc<RefCell<NovaHubApp>>,
    window: &ShellWindow,
    clipboard_window: &ClipboardWindowState,
    clipboard_filter: &ClipboardFilterState,
) -> bool {
    match selected.map(|command| command.id.as_str()) {
        Some("clipboard.history") => {
            show_clipboard_window(app, window, clipboard_window, clipboard_filter);
            true
        }
        Some("diagnostics.view") => {
            show_diagnostics(window, app);
            true
        }
        _ => false,
    }
}

fn search_result_rows(
    app: &NovaHubApp,
    query: &str,
    selected: usize,
    provider_filter: Option<&str>,
) -> Vec<SearchResultRowData> {
    app.search_provider(query, provider_filter)
        .into_iter()
        .take(SEARCH_RESULT_SLOT_COUNT)
        .enumerate()
        .map(|(index, command)| SearchResultRowData {
            title: command.title,
            subtitle: command.subtitle,
            selected: index == selected,
        })
        .collect()
}

fn set_search_results(
    window: &ShellWindow,
    app: &NovaHubApp,
    query: &str,
    selected: usize,
    provider_filter: Option<&str>,
) {
    let results = search_result_rows(app, query, selected, provider_filter);
    set_result_list_visible(window, !results.is_empty());
    for index in 0..SEARCH_RESULT_SLOT_COUNT {
        if let Some(row) = results.get(index) {
            set_result_row(window, index, &row.title, &row.subtitle, true, row.selected);
        } else {
            set_result_row(window, index, "", "", false, false);
        }
    }
    if results.is_empty() {
        let text = if query.trim().is_empty() {
            "Start typing to search NovaHub"
        } else {
            "No matching command"
        };
        set_result_text(window, text);
    } else {
        set_result_text(window, "");
    }
}

fn format_file_results(results: &[novahub_platform_api::FileSearchResult]) -> String {
    if results.is_empty() {
        "No files found".to_owned()
    } else {
        results
            .iter()
            .map(|result| result.path.display().to_string())
            .collect::<Vec<_>>()
            .join("\n")
    }
}

fn run_query_with_selection(
    app: &mut NovaHubApp,
    raw_query: &str,
    selected: Option<&novahub_core_domain::CommandDescriptor>,
) -> String {
    let Some(command) = selected else {
        return run_query(app, raw_query);
    };
    if command.id.as_str() == "clipboard.history" {
        return app
            .clipboard_history_summary(unix_timestamp())
            .unwrap_or_else(|error| format!("Clipboard history failed: {error}"));
    }
    if command.id.as_str() == "diagnostics.view" {
        return app.diagnostic_summary();
    }
    if command.id.as_str() == "calculator.evaluate" {
        let expression = raw_query.strip_prefix("calc ").unwrap_or(raw_query).trim();
        return execute_action(app, "calculator.evaluate", [expression]);
    }
    if command.id.as_str() == "units.convert" {
        return run_unit_conversion(app, raw_query);
    }
    if let Some(name) = command.id.as_str().strip_prefix("apps.launch:") {
        return launch_application(app, name);
    }
    if plugin_command_plugin_id(command.id.as_str()).is_some() {
        let input = raw_query
            .strip_prefix("plugin ")
            .and_then(|value| value.split_once(char::is_whitespace))
            .map_or("", |(_, value)| value.trim());
        return run_selected_plugin_command(app, command.id.as_str(), input);
    }
    run_query(app, raw_query)
}

fn parse_file_search_query(raw_query: &str) -> Option<(String, String)> {
    let input = raw_query.strip_prefix("files ")?;
    if input.starts_with("open ") || input.starts_with("reveal ") || input.starts_with("copy ") {
        return None;
    }
    let (root, query) = input.split_once(char::is_whitespace)?;
    let root = root.trim();
    let query = query.trim();
    (!root.is_empty() && !query.is_empty()).then(|| (root.to_owned(), query.to_owned()))
}

fn start_file_search_task(
    app: &NovaHubApp,
    root: String,
    query: String,
    pending: &PendingFileTask,
) -> String {
    if pending.borrow().is_some() {
        return "File search is already running".to_owned();
    }
    let receiver = match app.start_file_search(root.into(), query, 20) {
        Ok(receiver) => receiver,
        Err(error) => return format!("File search failed: {error}"),
    };
    pending.borrow_mut().replace(receiver);
    "File search started".to_owned()
}

fn plugin_command_plugin_id(command_id: &str) -> Option<&str> {
    plugin_command_parts(command_id).map(|(plugin_id, _)| plugin_id)
}

fn plugin_command_parts(command_id: &str) -> Option<(&str, &str)> {
    let value = command_id.strip_prefix("plugin:")?;
    let (plugin_id, command_id) = value.split_once(':')?;
    (!plugin_id.is_empty() && !command_id.is_empty()).then_some((plugin_id, command_id))
}

fn plugin_command_input(raw_query: &str) -> &str {
    raw_query
        .strip_prefix("plugin ")
        .and_then(|value| value.split_once(char::is_whitespace))
        .map_or("", |(_, input)| input.trim())
}

fn run_selected_plugin_command(app: &NovaHubApp, command_ref: &str, input: &str) -> String {
    let Some((plugin_id, command_id)) = plugin_command_parts(command_ref) else {
        return "Plugin command failed: invalid command identity".to_owned();
    };
    match app.active_plugin_command_interaction(plugin_id, command_id) {
        Ok(PluginInteraction::View) => app
            .run_active_plugin("shell-plugin", plugin_id, input)
            .map_or_else(
                |error| format!("Plugin command failed: {error}"),
                |run| format_plugin_run(&run),
            ),
        Ok(PluginInteraction::OneShot) => app
            .run_active_plugin_one_shot("shell-plugin", plugin_id, command_id, input)
            .map_or_else(
                |error| format!("Plugin command failed: {error}"),
                |result| format_one_shot_result(&result),
            ),
        Err(error) => format!("Plugin command failed: {error}"),
    }
}

enum PluginInvocation {
    Active {
        plugin_id: String,
        input: String,
    },
    Fixture {
        component_path: std::path::PathBuf,
        input: String,
    },
}

fn parse_plugin_query(raw_query: &str) -> Option<PluginInvocation> {
    let input = raw_query.strip_prefix("plugin ")?;
    if matches!(
        input.split_whitespace().next(),
        Some("permissions" | "revoke" | "approve")
    ) {
        return None;
    }
    if let Some(active) = input.strip_prefix("active ") {
        let (plugin_id, plugin_input) = active
            .split_once(char::is_whitespace)
            .map_or((active.trim(), ""), |(id, value)| (id.trim(), value.trim()));
        if plugin_id.is_empty() {
            return None;
        }
        return Some(PluginInvocation::Active {
            plugin_id: plugin_id.to_owned(),
            input: plugin_input.to_owned(),
        });
    }
    let component_path = std::env::var_os("NOVAHUB_COMPONENT_FIXTURE")?;
    Some(PluginInvocation::Fixture {
        component_path: component_path.into(),
        input: input.to_owned(),
    })
}

fn start_plugin_invocation(
    app: &NovaHubApp,
    invocation: PluginInvocation,
    pending_plugin_task: &PendingPluginTask,
    active_plugin_source: &ActivePluginSourceState,
) -> String {
    match invocation {
        PluginInvocation::Active { plugin_id, input } => start_plugin_command(
            app,
            &plugin_id,
            &input,
            pending_plugin_task,
            active_plugin_source,
        ),
        PluginInvocation::Fixture {
            component_path,
            input,
        } => {
            if pending_plugin_task.borrow().is_some() {
                return "Plugin command is already running".to_owned();
            }
            active_plugin_source
                .borrow_mut()
                .replace(ActivePluginSource::Fixture(component_path.clone()));
            start_component_task(
                ActivePluginSource::Fixture(component_path),
                input,
                pending_plugin_task,
            )
        }
    }
}

fn start_plugin_command(
    app: &NovaHubApp,
    plugin_id: &str,
    input: &str,
    pending_plugin_task: &PendingPluginTask,
    active_plugin_source: &ActivePluginSourceState,
) -> String {
    if pending_plugin_task.borrow().is_some() {
        return "Plugin command is already running".to_owned();
    }
    let execution = match app.prepare_active_plugin_execution(plugin_id) {
        Ok(execution) => execution,
        Err(error) => return format!("Plugin command failed: {error}"),
    };
    let source = ActivePluginSource::Installed(execution);
    active_plugin_source.borrow_mut().replace(source.clone());
    start_component_task(source, input.to_owned(), pending_plugin_task)
}

fn start_plugin_command_by_id(
    app: &NovaHubApp,
    command_ref: &str,
    input: &str,
    pending_plugin_task: &PendingPluginTask,
    active_plugin_source: &ActivePluginSourceState,
) -> String {
    let Some((plugin_id, command_id)) = plugin_command_parts(command_ref) else {
        return "Plugin command failed: invalid command identity".to_owned();
    };
    if pending_plugin_task.borrow().is_some() {
        return "Plugin command is already running".to_owned();
    }
    let Some(interaction) = app.plugin_command_interaction(command_ref) else {
        return format!("Plugin command failed: unknown command: {command_ref}");
    };
    if !matches!(
        app.active_plugin_command_interaction(plugin_id, command_id),
        Ok(active) if active == interaction
    ) {
        return "Plugin command failed: active manifest interaction changed".to_owned();
    }
    let execution = match app.prepare_active_plugin_execution(plugin_id) {
        Ok(execution) => execution,
        Err(error) => return format!("Plugin command failed: {error}"),
    };
    let source = ActivePluginSource::Installed(execution);
    match interaction {
        PluginInteraction::View => {
            active_plugin_source.borrow_mut().replace(source.clone());
            start_component_task(source, input.to_owned(), pending_plugin_task)
        }
        PluginInteraction::OneShot => start_one_shot_task(
            source,
            command_id.to_owned(),
            input.to_owned(),
            pending_plugin_task,
        ),
    }
}

fn start_component_task(
    source: ActivePluginSource,
    input: String,
    pending_plugin_task: &PendingPluginTask,
) -> String {
    if pending_plugin_task.borrow().is_some() {
        return "Plugin command is already running".to_owned();
    }
    let (sender, receiver) = mpsc::sync_channel(1);
    pending_plugin_task.borrow_mut().replace(receiver);
    let session_id = format!(
        "shell-plugin-{}",
        NEXT_PLUGIN_SESSION_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    );
    std::thread::spawn(move || {
        let result = source
            .run_view(&session_id, &input)
            .map(PluginExecutionResult::View)
            .map_err(|error| error.to_string());
        let _ = sender.send(result);
    });
    "Plugin command started".to_owned()
}

fn start_one_shot_task(
    source: ActivePluginSource,
    command_id: String,
    input: String,
    pending_plugin_task: &PendingPluginTask,
) -> String {
    if pending_plugin_task.borrow().is_some() {
        return "Plugin command is already running".to_owned();
    }
    let (sender, receiver) = mpsc::sync_channel(1);
    pending_plugin_task.borrow_mut().replace(receiver);
    let session_id = format!(
        "shell-plugin-{}",
        NEXT_PLUGIN_SESSION_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    );
    std::thread::spawn(move || {
        let result = source
            .run_one_shot(&session_id, &command_id, &input)
            .map(PluginExecutionResult::OneShot)
            .map_err(|error| error.to_string());
        let _ = sender.send(result);
    });
    "Plugin command started".to_owned()
}

fn start_plugin_task_timer(
    app: &Rc<RefCell<NovaHubApp>>,
    shell: &ShellWindow,
    pet_window: &novahub_ui_slint::PetWindow,
    plugin_ui: &PluginUiState,
) -> novahub_ui_slint::Timer {
    let app = Rc::clone(app);
    let shell = shell.as_weak();
    let pet_window = pet_window.as_weak();
    let plugin_ui = plugin_ui.clone();
    let timer = novahub_ui_slint::Timer::default();
    timer.start(
        novahub_ui_slint::TimerMode::Repeated,
        Duration::from_millis(16),
        move || {
            let result = {
                let pending = plugin_ui.pending_task.borrow();
                match pending.as_ref().map(Receiver::try_recv) {
                    Some(Ok(result)) => Some(result),
                    Some(Err(TryRecvError::Disconnected)) => {
                        Some(Err("plugin task channel closed".to_owned()))
                    }
                    Some(Err(TryRecvError::Empty)) | None => None,
                }
            };
            let Some(result) = result else {
                return;
            };
            plugin_ui.pending_task.borrow_mut().take();
            let (status, succeeded, surface) = match result {
                Ok(PluginExecutionResult::View(run)) => {
                    let surface = run.updated_view_surface();
                    (format_plugin_run(&run), true, Some(surface))
                }
                Ok(PluginExecutionResult::OneShot(result)) => {
                    plugin_ui.clear_active_view();
                    (format_one_shot_result(&result), true, None)
                }
                Err(error) => {
                    let message = error.chars().take(256).collect::<String>();
                    (
                        format!("Plugin command failed: {message}"),
                        false,
                        Some(PluginViewSurface {
                            kind: "error".into(),
                            title: "Plugin command failed".into(),
                            body: message,
                            progress: None,
                            items: Vec::new(),
                            next_cursor: None,
                            metadata: Vec::new(),
                            fields: Vec::new(),
                            actions: Vec::new(),
                        }),
                    )
                }
            };
            if succeeded {
                if let Some(surface) = &surface {
                    plugin_ui.accept_surface(surface);
                }
            } else {
                plugin_ui.clear_active_view();
            }
            let mut app = app.borrow_mut();
            if app.pet_state() == novahub_core_domain::pet::PetState::Working {
                let event = if succeeded {
                    PetEvent::ActionSucceeded
                } else {
                    PetEvent::ActionFailed
                };
                let _ = app.dispatch_pet_event(event);
            }
            if let Some(window) = pet_window.upgrade()
                && let Err(error) = sync_pet_window(&mut app, &window)
            {
                eprintln!("NovaHub pet window update failed: {error}");
            }
            if let Some(window) = shell.upgrade() {
                set_status_text(&window, status);
                if let Some(surface) = surface {
                    let values = plugin_ui.form_values.borrow();
                    apply_plugin_surface(
                        &window,
                        &surface,
                        &values,
                        *plugin_ui.item_offset.borrow(),
                    );
                }
            }
        },
    );
    timer
}

fn apply_plugin_surface(
    window: &ShellWindow,
    surface: &PluginViewSurface,
    form_values: &[String],
    requested_offset: usize,
) {
    set_plugin_view(window, &surface.kind, &surface.title, &surface.body);
    set_plugin_view_progress(window, surface.progress);
    set_plugin_default_action_available(
        window,
        surface.actions.iter().any(|action| action.is_default),
    );
    let item_window = plugin_item_window(surface, requested_offset);
    set_plugin_view_paging(
        window,
        item_window.previous_visible,
        item_window.next_visible,
        item_window.next_loads_more,
    );
    for index in 0..PLUGIN_VIEW_SLOT_COUNT {
        if let Some(item) = surface.items.get(item_window.start + index) {
            let accessory = item.accessory.as_ref();
            set_plugin_view_item(
                window,
                index,
                &PluginItemSlot {
                    title: &item.title,
                    subtitle: &item.subtitle,
                    status: accessory.map_or("", |accessory| accessory.status.as_str()),
                    badge: accessory.map_or("", |accessory| accessory.badge.as_str()),
                    shortcut: accessory.map_or("", |accessory| accessory.shortcut.as_str()),
                    icon_id: accessory
                        .and_then(|accessory| accessory.icon_id.as_deref())
                        .unwrap_or_default(),
                    visible: true,
                },
            );
        } else {
            set_plugin_view_item(window, index, &PluginItemSlot::hidden());
        }
        if let Some(field) = surface.fields.get(index) {
            let stored_value = form_values.get(index).map_or("", String::as_str);
            let (display_value, option_index) = plugin_field_display_value(field, stored_value);
            let option_labels = field
                .options
                .iter()
                .map(|option| option.label.as_str())
                .collect::<Vec<_>>();
            set_plugin_view_field(
                window,
                index,
                &PluginFieldSlot {
                    label: &field.label,
                    kind: &field.control,
                    helper: &field.helper,
                    error: &field.error,
                    options: &option_labels,
                    option_index,
                    value: &display_value,
                    required: field.required,
                    visible: true,
                },
            );
        } else {
            set_plugin_view_field(window, index, &PluginFieldSlot::hidden());
        }
        if let Some(action) = surface.actions.get(index) {
            let prefix = match (action.is_default, action.destructive) {
                (true, true) => "Enter + Confirm: ",
                (true, false) => "Enter: ",
                (false, true) => "Confirm: ",
                (false, false) => "",
            };
            let label = format!("{prefix}{}", action.title);
            set_plugin_view_action(window, index, label, true);
        } else {
            set_plugin_view_action(window, index, "", false);
        }
    }
}

fn plugin_field_display_value(
    field: &novahub_app::plugin_client::PluginViewField,
    stored_value: &str,
) -> (String, usize) {
    if field.control == "select" {
        if let Some((index, option)) = field
            .options
            .iter()
            .enumerate()
            .find(|(_, option)| option.id == stored_value)
        {
            return (option.label.clone(), index);
        }
        return (String::new(), 0);
    }
    (stored_value.to_owned(), 0)
}

fn plugin_initial_field_value(field: &novahub_app::plugin_client::PluginViewField) -> String {
    if field.control == "select" && field.initial_value.is_empty() {
        return field
            .options
            .first()
            .map_or_else(String::new, |option| option.id.clone());
    }
    field.initial_value.clone()
}

fn plugin_field_value_is_missing(
    field: &novahub_app::plugin_client::PluginViewField,
    value: &str,
) -> bool {
    if field.control == "checkbox" || field.control == "switch" {
        field.required && value != "true"
    } else {
        field.required && value.trim().is_empty()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PluginItemWindow {
    start: usize,
    previous_visible: bool,
    next_visible: bool,
    next_loads_more: bool,
}

fn plugin_item_window(surface: &PluginViewSurface, requested_offset: usize) -> PluginItemWindow {
    if !matches!(surface.kind.as_str(), "list" | "grid") || surface.items.is_empty() {
        return PluginItemWindow {
            start: 0,
            previous_visible: false,
            next_visible: false,
            next_loads_more: false,
        };
    }
    let rows = surface
        .items
        .iter()
        .map(|item| novahub_ui_slint::StableRow::new(&item.id, &item.title))
        .collect();
    let list = novahub_ui_slint::StableList::new(rows);
    let offset = requested_offset.min(surface.items.len().saturating_sub(PLUGIN_VIEW_SLOT_COUNT));
    let start = list
        .window(offset, PLUGIN_VIEW_SLOT_COUNT, 0)
        .map_or(offset, novahub_ui_slint::StableListWindow::start_index);
    let has_local_next = start.saturating_add(PLUGIN_VIEW_SLOT_COUNT) < surface.items.len();
    let next_loads_more = !has_local_next && surface.next_cursor.is_some();
    PluginItemWindow {
        start,
        previous_visible: start > 0,
        next_visible: has_local_next || next_loads_more,
        next_loads_more,
    }
}

// Generated Slint callbacks share one small state object; keeping their
// registration together makes the host-to-renderer event boundary explicit.
#[allow(clippy::too_many_lines)]
fn wire_plugin_view_handlers(window: &ShellWindow, plugin_ui: &PluginUiState) {
    let shell = window.as_weak();
    let pending = Rc::clone(&plugin_ui.pending_task);
    let source = Rc::clone(&plugin_ui.source);
    let surface = Rc::clone(&plugin_ui.surface);
    let item_offset = Rc::clone(&plugin_ui.item_offset);
    window.on_plugin_item_activated(move |index| {
        let offset = *item_offset.borrow();
        let Some(item) = surface
            .borrow()
            .as_ref()
            .and_then(|surface| {
                usize::try_from(index)
                    .ok()
                    .and_then(|i| surface.items.get(offset.saturating_add(i)))
            })
            .cloned()
        else {
            return;
        };
        let status = start_plugin_event_task(
            &source,
            &pending,
            &json!({"event": "activate", "id": item.id}),
        );
        if let Some(window) = shell.upgrade() {
            set_status_text(&window, status);
        }
    });

    let shell = window.as_weak();
    let pending = Rc::clone(&plugin_ui.pending_task);
    let source = Rc::clone(&plugin_ui.source);
    let surface = Rc::clone(&plugin_ui.surface);
    let confirmation = Rc::clone(&plugin_ui.pending_confirmation);
    window.on_plugin_action_requested(move |index| {
        let Some(action) = surface
            .borrow()
            .as_ref()
            .and_then(|surface| {
                usize::try_from(index)
                    .ok()
                    .and_then(|i| surface.actions.get(i))
            })
            .cloned()
        else {
            return;
        };
        let Some(window) = shell.upgrade() else {
            return;
        };
        let status = request_plugin_action(&window, action, &confirmation, &source, &pending);
        set_status_text(&window, status);
    });

    let shell = window.as_weak();
    let pending = Rc::clone(&plugin_ui.pending_task);
    let source = Rc::clone(&plugin_ui.source);
    let surface = Rc::clone(&plugin_ui.surface);
    let confirmation = Rc::clone(&plugin_ui.pending_confirmation);
    window.on_plugin_default_action_requested(move || {
        let Some(action) = surface
            .borrow()
            .as_ref()
            .and_then(|surface| surface.actions.iter().find(|action| action.is_default))
            .cloned()
        else {
            return;
        };
        let Some(window) = shell.upgrade() else {
            return;
        };
        let status = request_plugin_action(&window, action, &confirmation, &source, &pending);
        set_status_text(&window, status);
    });

    let values = Rc::clone(&plugin_ui.form_values);
    let form_dirty = Rc::clone(&plugin_ui.form_dirty);
    let surface = Rc::clone(&plugin_ui.surface);
    window.on_plugin_form_value_changed(move |index, value| {
        let Some(index) = usize::try_from(index).ok() else {
            return;
        };
        let mut values = values.borrow_mut();
        if values.len() <= index {
            values.resize(index + 1, String::new());
        }
        values[index] = surface
            .borrow()
            .as_ref()
            .and_then(|surface| surface.fields.get(index))
            .and_then(|field| {
                (field.control == "select")
                    .then(|| {
                        field
                            .options
                            .iter()
                            .find(|option| option.label == value.as_str())
                            .map(|option| option.id.clone())
                    })
                    .flatten()
            })
            .unwrap_or_else(|| value.to_string());
        *form_dirty.borrow_mut() = true;
    });

    let shell = window.as_weak();
    let pending = Rc::clone(&plugin_ui.pending_task);
    let source = Rc::clone(&plugin_ui.source);
    let surface = Rc::clone(&plugin_ui.surface);
    let values = Rc::clone(&plugin_ui.form_values);
    window.on_plugin_form_submit_requested(move || {
        let Some(surface) = surface.borrow().clone() else {
            return;
        };
        let form_values = values.borrow();
        if let Some((_, field)) = surface.fields.iter().enumerate().find(|(index, field)| {
            form_values
                .get(*index)
                .is_none_or(|value| plugin_field_value_is_missing(field, value))
        }) {
            if let Some(window) = shell.upgrade() {
                set_status_text(
                    &window,
                    format!("Required field is missing: {}", field.label),
                );
            }
            return;
        }
        let mut fields = serde_json::Map::new();
        for (index, field) in surface.fields.iter().enumerate() {
            fields.insert(
                field.id.clone(),
                serde_json::Value::String(form_values.get(index).cloned().unwrap_or_default()),
            );
        }
        let status = start_plugin_event_task(
            &source,
            &pending,
            &json!({"event": "submit", "values": fields}),
        );
        if let Some(window) = shell.upgrade() {
            set_status_text(&window, status);
        }
    });

    let shell = window.as_weak();
    let surface = Rc::clone(&plugin_ui.surface);
    let offset = Rc::clone(&plugin_ui.item_offset);
    let values = Rc::clone(&plugin_ui.form_values);
    window.on_plugin_list_previous_requested(move || {
        let current = *offset.borrow();
        *offset.borrow_mut() = current.saturating_sub(PLUGIN_VIEW_SLOT_COUNT);
        if let (Some(shell), Some(surface)) = (shell.upgrade(), surface.borrow().clone()) {
            let form_values = values.borrow();
            apply_plugin_surface(&shell, &surface, &form_values, *offset.borrow());
        }
    });

    let shell = window.as_weak();
    let pending = Rc::clone(&plugin_ui.pending_task);
    let source = Rc::clone(&plugin_ui.source);
    let surface = Rc::clone(&plugin_ui.surface);
    let offset = Rc::clone(&plugin_ui.item_offset);
    let values = Rc::clone(&plugin_ui.form_values);
    window.on_plugin_list_next_requested(move || {
        let Some(surface_snapshot) = surface.borrow().clone() else {
            return;
        };
        let current_offset = *offset.borrow();
        let item_window = plugin_item_window(&surface_snapshot, current_offset);
        if item_window.next_loads_more {
            let Some(cursor) = surface_snapshot.next_cursor.as_deref() else {
                return;
            };
            let status = start_plugin_event_task(
                &source,
                &pending,
                &json!({"event": "load_more", "cursor": cursor}),
            );
            if let Some(shell) = shell.upgrade() {
                set_status_text(&shell, status);
            }
            return;
        }
        let max_offset = surface_snapshot
            .items
            .len()
            .saturating_sub(PLUGIN_VIEW_SLOT_COUNT);
        let next = offset
            .borrow()
            .saturating_add(PLUGIN_VIEW_SLOT_COUNT)
            .min(max_offset);
        *offset.borrow_mut() = next;
        if let Some(shell) = shell.upgrade() {
            let form_values = values.borrow();
            apply_plugin_surface(&shell, &surface_snapshot, &form_values, next);
        }
    });
}

fn request_plugin_action(
    window: &ShellWindow,
    action: PluginViewAction,
    pending_confirmation: &PendingPluginAction,
    active_plugin_source: &ActivePluginSourceState,
    pending_plugin_task: &PendingPluginTask,
) -> String {
    if action.destructive {
        pending_confirmation.borrow_mut().replace(action);
        if let Some(action) = pending_confirmation.borrow().as_ref() {
            show_plugin_confirmation(window, action);
        }
        "Plugin action requires confirmation".to_owned()
    } else {
        start_plugin_event_task(
            active_plugin_source,
            pending_plugin_task,
            &json!({"event": "action", "id": action.id}),
        )
    }
}

fn start_plugin_event_task(
    active_plugin_source: &ActivePluginSourceState,
    pending_plugin_task: &PendingPluginTask,
    event: &serde_json::Value,
) -> String {
    let Some(source) = active_plugin_source.borrow().clone() else {
        return "Plugin view is no longer active".to_owned();
    };
    start_component_task(source, event.to_string(), pending_plugin_task)
}

fn run_query(app: &mut NovaHubApp, raw_query: &str) -> String {
    if raw_query == "confirm" {
        return app.confirm_pending_action().map_or_else(
            |error| format!("Confirmation failed: {error:?}"),
            |value| format!("Result: {value}"),
        );
    }
    if raw_query == "cancel" {
        return if app.cancel_pending_confirmation() {
            "Confirmation cancelled".to_owned()
        } else {
            "No pending confirmation".to_owned()
        };
    }
    if let Some(result) = run_pet_query(app, raw_query) {
        return result;
    }
    if let Some(result) = run_shortcut_query(app, raw_query) {
        return result;
    }
    if let Some(result) = run_clipboard_query(app, raw_query) {
        return result;
    }
    if let Some(result) = run_provider_query(app, raw_query) {
        return result;
    }
    if let Some(binding) = raw_query.strip_prefix("hotkey ") {
        return configure_hotkey(app, binding);
    }
    if let Some(application) = app
        .search(raw_query)
        .into_iter()
        .find(|command| command.id.as_str().starts_with("apps.launch:"))
    {
        return launch_application(app, &application.title);
    }
    run_calculator_query(app, raw_query)
}

fn configure_hotkey(app: &NovaHubApp, binding: &str) -> String {
    match app.set_global_hotkey(binding.trim()) {
        Ok(()) => format!(
            "Global shortcut saved as {}; restart NovaHub to register it",
            binding.trim()
        ),
        Err(error) => format!("Global shortcut rejected: {error}"),
    }
}

fn run_pet_query(app: &mut NovaHubApp, raw_query: &str) -> Option<String> {
    if raw_query == "pet list" {
        return Some(app.pet_choices().join("\n"));
    }
    if let Some(pet_id) = raw_query.strip_prefix("pet use ") {
        return Some(
            app.activate_pet(pet_id.trim())
                .unwrap_or_else(|error| format!("Pet activation failed: {error}")),
        );
    }
    if raw_query == "pet show" {
        return Some(app.dispatch_pet_event(PetEvent::Show).map_or_else(
            |_| "Pet is already visible".to_owned(),
            |_| "Pet shown".to_owned(),
        ));
    }
    if raw_query == "pet hide" {
        return Some(app.dispatch_pet_event(PetEvent::Hide).map_or_else(
            |_| "Pet is already hidden".to_owned(),
            |_| "Pet hidden".to_owned(),
        ));
    }
    if let Some(id) = raw_query.strip_prefix("pet pin ") {
        return Some(app.pin_pet_action_by_id(id.trim()).map_or_else(
            |error| format!("Pet action pin failed: {error}"),
            |()| "Pet action pinned".to_owned(),
        ));
    }
    raw_query.strip_prefix("pet unpin ").map(|id| {
        if app.unpin_pet_action(id.trim()) {
            "Pet action unpinned".to_owned()
        } else {
            "Pet action was not pinned".to_owned()
        }
    })
}

fn run_shortcut_query(app: &mut NovaHubApp, raw_query: &str) -> Option<String> {
    if let Some(result) = run_plugin_permission_query(app, raw_query) {
        return Some(result);
    }
    if let Some(input) = raw_query.strip_prefix("plugin ") {
        return Some(run_plugin_fixture(app, input));
    }
    if let Some(input) = raw_query.strip_prefix("quicklink ") {
        let mut parts = input.splitn(2, char::is_whitespace);
        let id = parts.next().unwrap_or_default();
        let query = parts.next().unwrap_or_default().trim();
        return Some(execute_action(app, "quicklinks.open", [id, query]));
    }
    if let Some(id) = raw_query.strip_prefix("snippet ") {
        return Some(execute_action(app, "snippets.library", [id.trim(), ""]));
    }
    if let Some(path) = raw_query.strip_prefix("open ") {
        return Some(execute_action(app, "apps.launch", [path.trim()]));
    }
    raw_query
        .strip_prefix("app ")
        .map(|name| launch_application(app, name))
}

fn run_plugin_permission_query(app: &NovaHubApp, raw_query: &str) -> Option<String> {
    let input = raw_query.strip_prefix("plugin ")?;
    let mut parts = input.split_whitespace();
    match parts.next()? {
        "permissions" => {
            let Some(plugin_id) = parts.next() else {
                return Some("Usage: plugin permissions <id>".into());
            };
            if parts.next().is_some() {
                return Some("Usage: plugin permissions <id>".into());
            }
            Some(
                app.active_plugin_permission_snapshot(plugin_id)
                    .map_or_else(
                        |error| format!("Plugin permission lookup failed: {error}"),
                        |snapshot| {
                            let declared = permission_list_label(&snapshot.declared);
                            let granted = permission_list_label(&snapshot.granted);
                            format!("Plugin permissions: declared={declared}; granted={granted}")
                        },
                    ),
            )
        }
        "approve" => {
            let Some(plugin_id) = parts.next() else {
                return Some("Usage: plugin approve <id>".into());
            };
            if parts.next().is_some() {
                return Some("Usage: plugin approve <id>".into());
            }
            Some(
                app.approve_declared_plugin_permissions(plugin_id)
                    .map_or_else(
                        |error| format!("Plugin approval failed: {error}"),
                        |()| "Plugin declared permissions approved".to_owned(),
                    ),
            )
        }
        "revoke" => {
            let Some(plugin_id) = parts.next() else {
                return Some("Usage: plugin revoke <id> [capability|all]".into());
            };
            let capability = parts.next().unwrap_or("all");
            if parts.next().is_some() {
                return Some("Usage: plugin revoke <id> [capability|all]".into());
            }
            let result = if capability == "all" {
                app.revoke_plugin_permissions(plugin_id)
            } else {
                let Some(capability) = capability_from_name(capability) else {
                    return Some(format!("Unknown plugin capability: {capability}"));
                };
                app.revoke_plugin_capability(plugin_id, capability)
            };
            Some(result.map_or_else(
                |error| format!("Plugin revocation failed: {error}"),
                |()| "Plugin permissions revoked".to_owned(),
            ))
        }
        _ => None,
    }
}

fn capability_from_name(name: &str) -> Option<Capability> {
    match name {
        "clipboard" => Some(Capability::Clipboard),
        "files" => Some(Capability::Files),
        "http" => Some(Capability::Http),
        "logging" => Some(Capability::Logging),
        "notification" => Some(Capability::Notification),
        "storage" => Some(Capability::Storage),
        _ => None,
    }
}

fn permission_list_label(permissions: &[String]) -> String {
    if permissions.is_empty() {
        "none".to_owned()
    } else {
        permissions.join(",")
    }
}

fn run_clipboard_query(app: &mut NovaHubApp, raw_query: &str) -> Option<String> {
    if let Some(text) = raw_query.strip_prefix("copy ") {
        let text = text.trim();
        return Some(if text.is_empty() {
            "Usage: copy <text>".to_owned()
        } else {
            execute_action(app, "clipboard.copy", [text])
        });
    }
    if let Some(ordinal) = raw_query.strip_prefix("clipboard copy ") {
        return Some(
            ordinal
                .trim()
                .parse::<usize>()
                .map_err(|_| "clipboard item number must be an integer".to_owned())
                .and_then(|ordinal| app.copy_clipboard_item(ordinal, unix_timestamp()))
                .unwrap_or_else(|error| format!("Clipboard copy failed: {error}")),
        );
    }
    match raw_query {
        "clipboard pause" => {
            app.set_clipboard_paused(true);
            Some("Clipboard capture paused".to_owned())
        }
        "clipboard resume" => {
            app.set_clipboard_paused(false);
            Some("Clipboard capture resumed".to_owned())
        }
        "clipboard clear" => Some(app.clear_clipboard().map_or_else(
            |error| format!("Clipboard clear failed: {error}"),
            |()| "Clipboard history cleared".to_owned(),
        )),
        "clipboard capture" => Some(capture_clipboard(app)),
        "clipboard" | "clipboard history" => Some(
            app.clipboard_history_summary(unix_timestamp())
                .unwrap_or_else(|error| format!("Clipboard history failed: {error}")),
        ),
        _ => None,
    }
}

fn run_provider_query(app: &mut NovaHubApp, raw_query: &str) -> Option<String> {
    for (prefix, command) in [
        ("files open ", "files.open"),
        ("files reveal ", "files.reveal"),
        ("files copy ", "files.copy_path"),
    ] {
        if let Some(path) = raw_query.strip_prefix(prefix) {
            let path = path.trim();
            return Some(if path.is_empty() {
                format!("Usage: {prefix}<path>")
            } else {
                execute_action(app, command, [path])
            });
        }
    }
    if let Some(input) = raw_query.strip_prefix("files ") {
        let mut parts = input.splitn(2, char::is_whitespace);
        let root = parts.next().unwrap_or_default();
        let query = parts.next().unwrap_or_default().trim();
        return Some(if root.is_empty() || query.is_empty() {
            "Usage: files <root> <query>".to_owned()
        } else {
            execute_action(app, "files.search", [root, query])
        });
    }
    if let Some(command) = raw_query.strip_prefix("system ") {
        return Some(match system_command_id(command.trim()) {
            Some(command_id) => execute_action(app, "system.command", [command_id]),
            None => "Unknown system command".to_owned(),
        });
    }
    raw_query
        .strip_prefix("convert ")
        .map(|input| run_unit_conversion(app, input))
}

fn run_calculator_query(app: &mut NovaHubApp, raw_query: &str) -> String {
    let expression = raw_query
        .strip_prefix("calc ")
        .unwrap_or(raw_query)
        .trim()
        .to_owned();
    app.search(raw_query)
        .into_iter()
        .find(|command| command.id.as_str() == "calculator.evaluate")
        .map_or_else(
            || "Select a built-in command first".to_owned(),
            |command| {
                let action = Action::invoke(command.id.as_str().to_owned(), [expression]);
                execute_action_value(app, &action, "Result")
            },
        )
}

fn run_unit_conversion(app: &mut NovaHubApp, input: &str) -> String {
    let mut parts = input.split_whitespace();
    let Some(value) = parts.next() else {
        return "Usage: convert <value> <from> to <unit>".to_owned();
    };
    let Some(from) = parts.next() else {
        return "Usage: convert <value> <from> to <unit>".to_owned();
    };
    let marker = parts.next().unwrap_or_default();
    let to = if marker.eq_ignore_ascii_case("to") {
        parts.next().unwrap_or_default()
    } else {
        marker
    };
    if to.is_empty() || parts.next().is_some() {
        return "Usage: convert <value> <from> to <unit>".to_owned();
    }
    execute_action(app, "units.convert", [value, from, to])
}

fn launch_application(app: &mut NovaHubApp, name: &str) -> String {
    app.launch_application_by_name(name.trim())
        .unwrap_or_else(|error| format!("Application launch failed: {error:?}"))
}

fn capture_clipboard(app: &mut NovaHubApp) -> String {
    match app.capture_clipboard(unix_timestamp(), false) {
        Ok(Some(item)) => {
            let kind = match item.kind {
                novahub_providers::clipboard::ClipboardKind::Text => "text",
                novahub_providers::clipboard::ClipboardKind::Image => "image",
            };
            format!("Clipboard captured: {kind} ({} bytes)", item.size)
        }
        Ok(None) => "Clipboard capture skipped".to_owned(),
        Err(error) => format!("Clipboard capture failed: {error}"),
    }
}

fn unix_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|duration| i64::try_from(duration.as_secs()).ok())
        .unwrap_or(i64::MAX)
}

fn system_command_id(command: &str) -> Option<&'static str> {
    match command {
        "settings" => Some("system.settings"),
        "lock" => Some("system.lock"),
        "sleep" => Some("system.sleep"),
        "logout" => Some("system.logout"),
        "restart" => Some("system.restart"),
        _ => None,
    }
}

fn execute_action<const N: usize>(app: &mut NovaHubApp, command: &str, args: [&str; N]) -> String {
    let action = novahub_core_domain::Action::invoke(command, args);
    execute_action_value(app, &action, "Result")
}

fn execute_action_value(app: &mut NovaHubApp, action: &Action, label: &str) -> String {
    match app.execute_builtin(action, false) {
        Ok(value) => format!("{label}: {value}"),
        Err(novahub_providers::BuiltinActionError::ConfirmationRequired) => {
            app.request_confirmation(action.clone());
            "Confirmation required: type confirm to continue or cancel to stop".to_owned()
        }
        Err(error) => format!("Command failed: {error:?}"),
    }
}

fn run_plugin_fixture(app: &NovaHubApp, input: &str) -> String {
    if let Some(active_input) = input.strip_prefix("active ")
        && let Some((plugin_id, plugin_input)) = active_input.split_once(char::is_whitespace)
    {
        let plugin_id = plugin_id.trim();
        let plugin_input = plugin_input.trim();
        if !plugin_id.is_empty() && !plugin_input.is_empty() {
            return app
                .run_active_plugin("shell-plugin", plugin_id, plugin_input)
                .map_or_else(
                    |error| format!("Plugin command failed: {error}"),
                    |run| format_plugin_run(&run),
                );
        }
    }
    let Some(component_path) = std::env::var_os("NOVAHUB_COMPONENT_FIXTURE") else {
        return "Usage: plugin active <id> <input>".to_owned();
    };
    app.run_component_fixture("shell-fixture", component_path, input)
        .map_or_else(
            |error| format!("Plugin command failed: {error}"),
            |run| format_plugin_run(&run),
        )
}

fn format_plugin_run(run: &novahub_app::PluginFixtureRun) -> String {
    let surface = run.updated_view_surface();
    format!(
        "Plugin view updated to revision {}: {} [{}]\n{}",
        run.updated_revision().unwrap_or_default(),
        surface.title,
        surface.kind,
        surface.body,
    )
}

fn format_one_shot_result(result: &serde_json::Value) -> String {
    let text = result
        .get("result")
        .and_then(|value| value.get("text"))
        .and_then(serde_json::Value::as_str)
        .map_or_else(|| result.to_string(), str::to_owned);
    let text = text.chars().take(512).collect::<String>();
    format!("Plugin command completed: {text}")
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, ffi::OsStr, rc::Rc, time::Duration};

    use super::{
        ActivePluginSource, ClipboardFilter, DismissIntent, PluginInvocation, dismiss_intent,
        next_provider_filter, parse_file_search_query, parse_plugin_query,
        parse_reference_session_count, parse_shell_smoke_hold, plugin_item_window,
        provider_filter_label, run_plugin_permission_query, run_query, search_result_rows,
        should_hide_on_focus_loss, start_component_task, start_plugin_command,
    };
    use novahub_app::NovaHubApp;
    use novahub_app::plugin_client::{PluginViewItem, PluginViewSurface};

    #[test]
    fn shell_smoke_hold_is_optional_and_bounded() {
        assert_eq!(parse_shell_smoke_hold(None), Ok(None));
        assert_eq!(
            parse_shell_smoke_hold(Some(OsStr::new("2500"))),
            Ok(Some(Duration::from_millis(2_500)))
        );
        assert!(parse_shell_smoke_hold(Some(OsStr::new("60001"))).is_err());
    }

    #[test]
    fn shell_smoke_hold_rejects_non_numeric_values() {
        assert!(parse_shell_smoke_hold(Some(OsStr::new("later"))).is_err());
    }

    #[test]
    fn reference_plugin_session_count_is_limited_to_mvp_scenarios() {
        assert_eq!(
            parse_reference_session_count(Some(OsStr::new("1"))),
            Ok(Some(1))
        );
        assert_eq!(
            parse_reference_session_count(Some(OsStr::new("4"))),
            Ok(Some(4))
        );
        assert!(parse_reference_session_count(Some(OsStr::new("0"))).is_err());
        assert!(parse_reference_session_count(Some(OsStr::new("5"))).is_err());
    }

    #[test]
    fn clipboard_shell_commands_keep_state_in_the_host() {
        let mut app = NovaHubApp::new();
        assert_eq!(
            run_query(&mut app, "clipboard"),
            "Clipboard history is empty"
        );
        assert_eq!(
            run_query(&mut app, "clipboard pause"),
            "Clipboard capture paused"
        );
        assert!(app.clipboard_paused());
        assert_eq!(
            run_query(&mut app, "clipboard resume"),
            "Clipboard capture resumed"
        );
        assert!(!app.clipboard_paused());
        assert_eq!(
            run_query(&mut app, "clipboard clear"),
            "Clipboard history cleared"
        );
        assert!(run_query(&mut app, "clipboard copy 0").contains("Clipboard copy failed"));
    }

    #[test]
    fn copy_command_is_explicit_and_host_owned() {
        let mut app = NovaHubApp::new();
        assert_eq!(run_query(&mut app, "copy "), "Usage: copy <text>");
        assert!(run_query(&mut app, "copy bad\ntext").contains("Command failed"));
    }

    #[test]
    fn explicit_application_command_reports_unavailable_platform() {
        let mut app = NovaHubApp::new();
        let status = run_query(&mut app, "app Missing Application");
        assert!(status.contains("Application launch failed"));
    }

    #[test]
    fn file_result_actions_have_explicit_host_commands() {
        let mut app = NovaHubApp::new();
        assert_eq!(
            run_query(&mut app, "files open "),
            "Usage: files open <path>"
        );
        assert_eq!(
            run_query(&mut app, "files reveal "),
            "Usage: files reveal <path>"
        );
        assert_eq!(
            run_query(&mut app, "files copy "),
            "Usage: files copy <path>"
        );
        assert!(run_query(&mut app, "files open bad\npath").contains("Command failed"));
    }

    #[test]
    fn file_search_query_parser_keeps_open_actions_out_of_the_worker_path() {
        assert_eq!(
            parse_file_search_query("files C:/Users report"),
            Some(("C:/Users".into(), "report".into()))
        );
        assert!(parse_file_search_query("files open C:/Users/report.txt").is_none());
        assert!(parse_file_search_query("files C:/Users").is_none());
    }

    #[test]
    fn pet_pin_commands_use_the_host_shelf_owner() {
        let mut app = NovaHubApp::new();
        assert_eq!(
            run_query(&mut app, "pet pin clipboard.history"),
            "Pet action pinned"
        );
        assert_eq!(app.pet_actions().fixed().len(), 1);
        assert_eq!(
            run_query(&mut app, "pet unpin clipboard.history"),
            "Pet action unpinned"
        );
        assert!(app.pet_actions().fixed().is_empty());
    }

    #[test]
    fn pet_selection_commands_use_the_host_renderer_owner() {
        let mut app = NovaHubApp::new();
        assert!(run_query(&mut app, "pet list").contains("nova (built-in)"));
        assert_eq!(run_query(&mut app, "pet use pixel"), "Pet activated: pixel");
        assert_eq!(app.active_pet_id(), "pixel");
    }

    #[test]
    fn unit_conversion_command_uses_the_host_provider() {
        let mut app = NovaHubApp::new();
        assert_eq!(run_query(&mut app, "convert 1 km to m"), "Result: 1000");
    }

    #[test]
    fn search_result_rows_are_bounded_and_keep_the_selected_command() {
        let app = NovaHubApp::new();
        let rows = search_result_rows(&app, "calc", 0, None);
        assert!(!rows.is_empty());
        assert!(rows.len() <= super::SEARCH_RESULT_SLOT_COUNT);
        assert!(rows[0].selected);
        assert!(rows.iter().skip(1).all(|row| !row.selected));
    }

    #[test]
    fn empty_search_result_rows_are_explicitly_empty() {
        let app = NovaHubApp::new();
        assert!(search_result_rows(&app, "query-that-does-not-exist", 0, None).is_empty());
    }

    #[test]
    fn provider_filter_cycles_from_all_through_known_families() {
        assert_eq!(provider_filter_label(None), "All providers");
        assert_eq!(provider_filter_label(Some("calculator")), "Calculator");
        assert_eq!(next_provider_filter(None).as_deref(), Some("apps"));
        assert_eq!(next_provider_filter(Some("apps")).as_deref(), Some("files"));
        assert_eq!(next_provider_filter(Some("plugins")), None);
    }

    #[test]
    fn clipboard_filter_cycles_with_explicit_labels() {
        assert_eq!(ClipboardFilter::All.next(), ClipboardFilter::Text);
        assert_eq!(ClipboardFilter::Text.next(), ClipboardFilter::Image);
        assert_eq!(ClipboardFilter::Image.next(), ClipboardFilter::All);
        assert_eq!(ClipboardFilter::Image.label(), "Images only");
    }

    #[test]
    fn destructive_system_commands_keep_a_pending_confirmation_in_the_host() {
        let mut app = NovaHubApp::new();
        let status = run_query(&mut app, "system restart");
        assert!(status.contains("Confirmation required"));
        assert!(app.has_pending_confirmation());
        assert_eq!(run_query(&mut app, "cancel"), "Confirmation cancelled");
        assert!(!app.has_pending_confirmation());
    }

    #[test]
    fn shell_escape_clears_query_before_hiding_and_cancels_confirmation_first() {
        assert_eq!(
            dismiss_intent(true, false, "restart"),
            DismissIntent::CancelConfirmation
        );
        assert_eq!(
            dismiss_intent(false, false, "restart"),
            DismissIntent::ClearQueryAndRefocus
        );
        assert_eq!(
            dismiss_intent(false, false, "  "),
            DismissIntent::HideWindow
        );
        assert_eq!(
            dismiss_intent(false, true, "restart"),
            DismissIntent::KeepFormOpen
        );
    }

    #[test]
    fn shell_blur_keeps_pending_confirmation_visible() {
        assert!(!should_hide_on_focus_loss(true, false));
        assert!(!should_hide_on_focus_loss(false, true));
        assert!(should_hide_on_focus_loss(false, false));
    }

    #[test]
    fn plugin_item_window_loads_cursor_only_after_local_slots_are_exhausted() {
        let items = (0..=super::PLUGIN_VIEW_SLOT_COUNT)
            .map(|index| PluginViewItem {
                id: format!("item-{index}"),
                title: format!("Item {index}"),
                subtitle: String::new(),
                accessory: None,
            })
            .collect();
        let surface = PluginViewSurface {
            kind: "list".into(),
            title: "Results".into(),
            body: String::new(),
            progress: None,
            items,
            next_cursor: Some("opaque-page-2".into()),
            metadata: Vec::new(),
            fields: Vec::new(),
            actions: Vec::new(),
        };

        let first = plugin_item_window(&surface, 0);
        assert!(first.next_visible);
        assert!(!first.next_loads_more);

        let last = plugin_item_window(&surface, 1);
        assert!(last.previous_visible);
        assert!(last.next_visible);
        assert!(last.next_loads_more);
    }

    #[test]
    fn plugin_active_query_keeps_the_active_pointer_boundary() {
        assert!(matches!(
            parse_plugin_query("plugin active demo {\"input\":true}"),
            Some(PluginInvocation::Active { plugin_id, input })
                if plugin_id == "demo" && input == "{\"input\":true}"
        ));
    }

    #[test]
    fn plugin_command_identity_and_one_shot_result_are_bounded() {
        assert_eq!(
            super::plugin_command_parts("plugin:demo:format"),
            Some(("demo", "format"))
        );
        assert!(super::plugin_command_parts("plugin:demo").is_none());
        let result = serde_json::json!({
            "type": "command_result",
            "result": {"text": "formatted"}
        });
        assert_eq!(
            super::format_one_shot_result(&result),
            "Plugin command completed: formatted"
        );
    }

    #[test]
    fn plugin_permission_commands_reject_invalid_input_before_storage_access() {
        let app = NovaHubApp::new();
        assert_eq!(
            run_plugin_permission_query(&app, "plugin revoke demo websocket"),
            Some("Unknown plugin capability: websocket".into())
        );
        assert_eq!(
            run_plugin_permission_query(&app, "plugin revoke"),
            Some("Usage: plugin revoke <id> [capability|all]".into())
        );
        assert_eq!(run_plugin_permission_query(&app, "plugin run demo"), None);
    }

    #[test]
    fn plugin_host_work_is_queued_without_blocking_the_shell_callback() {
        let pending = Rc::new(RefCell::new(None));
        assert_eq!(
            start_component_task(
                ActivePluginSource::Fixture("missing-component.wasm".into()),
                "input".to_owned(),
                &pending,
            ),
            "Plugin command started"
        );
        let receiver = pending.borrow_mut().take().expect("queued plugin task");
        let result = receiver
            .recv_timeout(Duration::from_secs(1))
            .expect("background task reports its spawn error");
        assert!(result.is_err());
    }

    #[test]
    fn installed_plugin_shell_path_retains_the_capability_execution_context() {
        let root = std::env::temp_dir().join(format!(
            "novahub-shell-execution-context-{}",
            std::process::id()
        ));
        let version = root.join("demo/versions/1.0.0");
        std::fs::create_dir_all(&version).expect("plugin version directory");
        std::fs::write(
            version.join("novahub.toml"),
            "id = \"demo\"\nversion = \"1.0.0\"\nhost_api = \">=1.0, <2.0\"\n\n[permissions]\nclipboard = { access = [\"read\"] }\n",
        )
        .expect("plugin manifest");
        std::fs::write(version.join("plugin.wasm"), b"invalid test component")
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

        let app = NovaHubApp::open_with_plugin_root(root.join("novahub.sqlite3"), &root)
            .expect("app opens with plugin root");
        assert_eq!(
            run_plugin_permission_query(&app, "plugin permissions demo"),
            Some("Plugin permissions: declared=clipboard; granted=clipboard".into())
        );
        assert_eq!(
            run_plugin_permission_query(&app, "plugin revoke demo clipboard"),
            Some("Plugin permissions revoked".into())
        );
        assert_eq!(
            run_plugin_permission_query(&app, "plugin permissions demo"),
            Some("Plugin permissions: declared=clipboard; granted=none".into())
        );
        assert_eq!(
            run_plugin_permission_query(&app, "plugin approve demo"),
            Some("Plugin declared permissions approved".into())
        );
        let pending = Rc::new(RefCell::new(None));
        let active_source = Rc::new(RefCell::new(None));
        assert_eq!(
            start_plugin_command(&app, "demo", "input", &pending, &active_source),
            "Plugin command started"
        );
        assert!(matches!(
            active_source.borrow().as_ref(),
            Some(ActivePluginSource::Installed(_))
        ));

        let receiver = pending.borrow_mut().take().expect("queued plugin task");
        let _ = receiver.recv_timeout(Duration::from_secs(5));
        drop(app);
        let _ = std::fs::remove_dir_all(root);
    }
}

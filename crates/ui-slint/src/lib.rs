#![deny(unsafe_code)]

mod generated {
    #![allow(unsafe_code)]

    slint::include_modules!();
}

pub use generated::{ClipboardWindow, PetWindow, SettingsWindow, ShellWindow, TrayIcon};
pub use slint::ComponentHandle;
pub use slint::{Image, ModelRc, PhysicalPosition, SharedString, Timer, TimerMode, VecModel};

use novahub_core_domain::pet_position::DisplayWorkArea;
use slint::winit_030::WinitWindowAccessor;

mod pet_action_shelf;
mod pet_renderer;
mod render_backend;

pub use pet_action_shelf::{PetAction, PetActionShelf, PetActionState};
pub use pet_renderer::{AnimationMode, PetFrameState, PetRenderer, RenderFrame};
pub use render_backend::{
    ShellFrameObserver, initialize_shell_backend, on_shell_first_frame, on_shell_next_frame,
};

/// Number of fixed result rows exposed by the launcher Shell.
pub const SEARCH_RESULT_SLOT_COUNT: usize = 8;
/// Number of fixed clipboard rows exposed by the history window.
pub const CLIPBOARD_HISTORY_SLOT_COUNT: usize = 8;

/// Creates the generated Shell window from the host-owned Slint component.
///
/// # Errors
///
/// Returns the native window creation error reported by Slint.
pub fn create_shell_window() -> Result<ShellWindow, slint::PlatformError> {
    ShellWindow::new()
}

/// Creates the on-demand host settings window. The application keeps no
/// settings window alive until the user requests it with `Cmd/Ctrl+,`.
///
/// # Errors
///
/// Returns the native window creation error reported by Slint.
pub fn create_settings_window() -> Result<SettingsWindow, slint::PlatformError> {
    SettingsWindow::new()
}

/// Creates the on-demand host clipboard history window.
///
/// # Errors
///
/// Returns the native window creation error reported by Slint.
pub fn create_clipboard_window() -> Result<ClipboardWindow, slint::PlatformError> {
    ClipboardWindow::new()
}

/// Creates the host-owned desktop-pet window. The caller controls whether the
/// native window is shown; the pet package never receives a window handle.
///
/// # Errors
///
/// Returns the native window creation error reported by Slint.
pub fn create_pet_window() -> Result<PetWindow, slint::PlatformError> {
    PetWindow::new()
}

/// Creates the native tray icon used to recover hidden windows and quit the
/// host without adding a helper process.
///
/// # Errors
///
/// Returns the native tray creation error reported by Slint.
pub fn create_tray_icon() -> Result<TrayIcon, slint::PlatformError> {
    TrayIcon::new()
}

/// Centers the Shell on the monitor currently associated with its native
/// window. The operation is best effort because some backends create native
/// windows lazily or do not expose monitor geometry.
#[must_use]
pub fn center_shell_window(window: &ShellWindow) -> bool {
    center_native_window(window.window())
}

/// Places the pet at the lower-right corner of its current monitor. The
/// caller owns persistence and can immediately convert the returned monitor
/// geometry into the host's logical position model.
#[must_use]
pub fn place_pet_window(window: &PetWindow) -> bool {
    place_native_window(window.window())
}

/// Returns the current monitor in logical coordinates for host-side clamping
/// and DPI-aware position persistence.
#[must_use]
pub fn current_monitor_work_area(window: &PetWindow) -> Option<DisplayWorkArea> {
    monitor_work_area(window.window())
}

fn center_native_window(window: &slint::Window) -> bool {
    window
        .with_winit_window(|native| {
            let Some(monitor) = native.current_monitor() else {
                return false;
            };
            let monitor_position = monitor.position();
            let monitor_size = monitor.size();
            let window_size = native.outer_size();
            let x = monitor_position
                .x
                .saturating_add(center_offset(monitor_size.width, window_size.width));
            let y = monitor_position
                .y
                .saturating_add(center_offset(monitor_size.height, window_size.height));
            native.set_outer_position(slint::winit_030::winit::dpi::PhysicalPosition::new(x, y));
            true
        })
        .unwrap_or(false)
}

fn place_native_window(window: &slint::Window) -> bool {
    window
        .with_winit_window(|native| {
            let Some(monitor) = native.current_monitor() else {
                return false;
            };
            let monitor_position = monitor.position();
            let monitor_size = monitor.size();
            let window_size = native.outer_size();
            let margin = 24_u32;
            let x = monitor_position.x.saturating_add(edge_offset(
                monitor_size.width,
                window_size.width,
                margin,
            ));
            let y = monitor_position.y.saturating_add(edge_offset(
                monitor_size.height,
                window_size.height,
                margin,
            ));
            native.set_outer_position(slint::winit_030::winit::dpi::PhysicalPosition::new(x, y));
            true
        })
        .unwrap_or(false)
}

#[allow(clippy::cast_possible_truncation)]
fn monitor_work_area(window: &slint::Window) -> Option<DisplayWorkArea> {
    window
        .with_winit_window(|native| {
            let monitor = native.current_monitor()?;
            let position = monitor.position();
            let physical_size = monitor.size();
            let scale = native.scale_factor() as f32;
            Some(DisplayWorkArea {
                display_id: monitor_id(&monitor, position, physical_size),
                left: logical_coordinate(position.x, scale),
                top: logical_coordinate(position.y, scale),
                width: logical_dimension(physical_size.width, scale),
                height: logical_dimension(physical_size.height, scale),
                dpi: logical_dpi(scale),
            })
        })
        .flatten()
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss
)]
fn center_offset(monitor: u32, window: u32) -> i32 {
    monitor
        .saturating_sub(window)
        .saturating_div(2)
        .min(i32::MAX as u32) as i32
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss
)]
fn edge_offset(monitor: u32, window: u32, margin: u32) -> i32 {
    monitor
        .saturating_sub(window)
        .saturating_sub(margin)
        .min(i32::MAX as u32) as i32
}

#[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
fn logical_coordinate(value: i32, scale: f32) -> i32 {
    (value as f32 / scale.max(1.0)).round() as i32
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)]
fn logical_dimension(value: u32, scale: f32) -> u32 {
    ((value as f32 / scale.max(1.0)).round().max(1.0)) as u32
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)]
fn logical_dpi(scale: f32) -> u16 {
    (scale.max(1.0) * 96.0).round().min(f32::from(u16::MAX)) as u16
}

fn monitor_id(
    monitor: &slint::winit_030::winit::monitor::MonitorHandle,
    position: slint::winit_030::winit::dpi::PhysicalPosition<i32>,
    size: slint::winit_030::winit::dpi::PhysicalSize<u32>,
) -> u64 {
    use std::hash::{Hash, Hasher};

    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    monitor.name().hash(&mut hasher);
    position.x.hash(&mut hasher);
    position.y.hash(&mut hasher);
    size.width.hash(&mut hasher);
    size.height.hash(&mut hasher);
    hasher.finish()
}

/// Loads the small host-owned icon shared by the system tray and pet fallback.
#[must_use]
pub fn builtin_tray_image() -> Option<Image> {
    Image::load_from_svg_data(include_bytes!(
        "../../../plugins/official/pets/nova/assets/nova-idle.svg"
    ))
    .ok()
}

/// Stops the Slint event loop from a native tray action.
pub fn quit_event_loop() {
    let _ = slint::quit_event_loop();
}

/// Applies one bounded renderer frame to the pet window.
pub fn set_pet_frame(window: &PetWindow, frame: &RenderFrame) {
    set_pet_frame_with_bytes(window, frame, None);
}

/// Applies a host-owned renderer frame and, when present, its validated SVG
/// bytes. The package path is intentionally never passed to Slint.
pub fn set_pet_frame_with_bytes(
    window: &PetWindow,
    frame: &RenderFrame,
    asset_bytes: Option<&[u8]>,
) {
    window.set_frame_asset(frame.asset.as_str().into());
    if let Some(image) = asset_bytes
        .and_then(|bytes| Image::load_from_svg_data(bytes).ok())
        .or_else(|| builtin_pet_image(frame.asset.as_str()))
    {
        window.set_pet_image(image);
        window.set_pet_label("".into());
    } else {
        window.set_pet_label("N".into());
    }
}

fn builtin_pet_image(asset: &str) -> Option<Image> {
    let bytes: &[u8] = match asset {
        "builtin://nova/idle" => {
            include_bytes!("../../../plugins/official/pets/nova/assets/nova-idle.svg")
        }
        "builtin://nova/success" => {
            include_bytes!("../../../plugins/official/pets/nova/assets/nova-success.svg")
        }
        "builtin://pixel/idle" => {
            include_bytes!("../../../plugins/official/pets/pixel/assets/pixel-idle.svg")
        }
        "builtin://pixel/success" => {
            include_bytes!("../../../plugins/official/pets/pixel/assets/pixel-success.svg")
        }
        "builtin://waterman/idle" => {
            include_bytes!("../../../plugins/official/pets/waterman/assets/waterman-idle.svg")
        }
        "builtin://waterman/success" => {
            include_bytes!("../../../plugins/official/pets/waterman/assets/waterman-success.svg")
        }
        _ => return None,
    };
    Image::load_from_svg_data(bytes).ok()
}

/// Updates the bounded host-owned action-shelf summary.
pub fn set_pet_shelf(window: &PetWindow, visible: bool, label: impl AsRef<str>) {
    window.set_shelf_visible(visible);
    window.set_shelf_label(label.as_ref().into());
}

/// Updates the five bounded host-owned action slots exposed by the pet shelf.
pub fn set_pet_shelf_actions(window: &PetWindow, actions: &[PetAction]) {
    let labels = [
        actions.first().map(|action| action.title.as_str()),
        actions.get(1).map(|action| action.title.as_str()),
        actions.get(2).map(|action| action.title.as_str()),
        actions.get(3).map(|action| action.title.as_str()),
        actions.get(4).map(|action| action.title.as_str()),
    ];
    window.set_shelf_action_1(labels[0].unwrap_or_default().into());
    window.set_shelf_action_2(labels[1].unwrap_or_default().into());
    window.set_shelf_action_3(labels[2].unwrap_or_default().into());
    window.set_shelf_action_4(labels[3].unwrap_or_default().into());
    window.set_shelf_action_5(labels[4].unwrap_or_default().into());
    window.set_shelf_action_1_visible(labels[0].is_some());
    window.set_shelf_action_2_visible(labels[1].is_some());
    window.set_shelf_action_3_visible(labels[2].is_some());
    window.set_shelf_action_4_visible(labels[3].is_some());
    window.set_shelf_action_5_visible(labels[4].is_some());
}

/// Shows or hides the native pet window without changing its persisted
/// position.
///
/// # Errors
///
/// Returns the native window error when the backend rejects the request. The
/// caller can keep the main Shell running when this optional window fails.
pub fn set_pet_visible(window: &PetWindow, visible: bool) -> Result<(), slint::PlatformError> {
    if visible {
        window.show()
    } else {
        window.hide()
    }
}

/// Updates the bounded result text displayed by the generated Shell.
pub fn set_result_text(window: &ShellWindow, text: impl AsRef<str>) {
    window.set_result_text(text.as_ref().into());
}

/// Shows or hides the fixed search-result row surface. The row count is kept
/// stable so asynchronous provider results cannot resize the launcher.
pub fn set_result_list_visible(window: &ShellWindow, visible: bool) {
    window.set_result_list_visible(visible);
}

/// Updates one bounded search-result row owned by the Shell.
pub fn set_result_row(
    window: &ShellWindow,
    index: usize,
    title: impl AsRef<str>,
    subtitle: impl AsRef<str>,
    visible: bool,
    selected: bool,
) {
    let title = title.as_ref().into();
    let subtitle = subtitle.as_ref().into();
    match index {
        0 => {
            window.set_result_1_title(title);
            window.set_result_1_subtitle(subtitle);
            window.set_result_1_visible(visible);
            window.set_result_1_selected(selected);
        }
        1 => {
            window.set_result_2_title(title);
            window.set_result_2_subtitle(subtitle);
            window.set_result_2_visible(visible);
            window.set_result_2_selected(selected);
        }
        2 => {
            window.set_result_3_title(title);
            window.set_result_3_subtitle(subtitle);
            window.set_result_3_visible(visible);
            window.set_result_3_selected(selected);
        }
        3 => {
            window.set_result_4_title(title);
            window.set_result_4_subtitle(subtitle);
            window.set_result_4_visible(visible);
            window.set_result_4_selected(selected);
        }
        4 => {
            window.set_result_5_title(title);
            window.set_result_5_subtitle(subtitle);
            window.set_result_5_visible(visible);
            window.set_result_5_selected(selected);
        }
        5 => {
            window.set_result_6_title(title);
            window.set_result_6_subtitle(subtitle);
            window.set_result_6_visible(visible);
            window.set_result_6_selected(selected);
        }
        6 => {
            window.set_result_7_title(title);
            window.set_result_7_subtitle(subtitle);
            window.set_result_7_visible(visible);
            window.set_result_7_selected(selected);
        }
        7 => {
            window.set_result_8_title(title);
            window.set_result_8_subtitle(subtitle);
            window.set_result_8_visible(visible);
            window.set_result_8_selected(selected);
        }
        _ => {}
    }
}

/// Clears every result row and restores the text-only status surface used for
/// command feedback, file-search summaries, and errors.
pub fn clear_result_rows(window: &ShellWindow) {
    set_result_list_visible(window, false);
    for index in 0..SEARCH_RESULT_SLOT_COUNT {
        set_result_row(window, index, "", "", false, false);
    }
}

/// Updates one bounded clipboard history row without creating dynamic Slint
/// children from protected clipboard content.
pub fn set_clipboard_row(
    window: &ClipboardWindow,
    index: usize,
    title: impl AsRef<str>,
    subtitle: impl AsRef<str>,
    visible: bool,
    pinned: bool,
    copyable: bool,
) {
    let title = title.as_ref().into();
    let subtitle = subtitle.as_ref().into();
    match index {
        0 => {
            window.set_item_1_title(title);
            window.set_item_1_subtitle(subtitle);
            window.set_item_1_visible(visible);
            window.set_item_1_pinned(pinned);
            window.set_item_1_copyable(copyable);
        }
        1 => {
            window.set_item_2_title(title);
            window.set_item_2_subtitle(subtitle);
            window.set_item_2_visible(visible);
            window.set_item_2_pinned(pinned);
            window.set_item_2_copyable(copyable);
        }
        2 => {
            window.set_item_3_title(title);
            window.set_item_3_subtitle(subtitle);
            window.set_item_3_visible(visible);
            window.set_item_3_pinned(pinned);
            window.set_item_3_copyable(copyable);
        }
        3 => {
            window.set_item_4_title(title);
            window.set_item_4_subtitle(subtitle);
            window.set_item_4_visible(visible);
            window.set_item_4_pinned(pinned);
            window.set_item_4_copyable(copyable);
        }
        4 => {
            window.set_item_5_title(title);
            window.set_item_5_subtitle(subtitle);
            window.set_item_5_visible(visible);
            window.set_item_5_pinned(pinned);
            window.set_item_5_copyable(copyable);
        }
        5 => {
            window.set_item_6_title(title);
            window.set_item_6_subtitle(subtitle);
            window.set_item_6_visible(visible);
            window.set_item_6_pinned(pinned);
            window.set_item_6_copyable(copyable);
        }
        6 => {
            window.set_item_7_title(title);
            window.set_item_7_subtitle(subtitle);
            window.set_item_7_visible(visible);
            window.set_item_7_pinned(pinned);
            window.set_item_7_copyable(copyable);
        }
        7 => {
            window.set_item_8_title(title);
            window.set_item_8_subtitle(subtitle);
            window.set_item_8_visible(visible);
            window.set_item_8_pinned(pinned);
            window.set_item_8_copyable(copyable);
        }
        _ => {}
    }
}

/// Clears all clipboard rows before applying a new bounded snapshot.
pub fn clear_clipboard_rows(window: &ClipboardWindow) {
    for index in 0..CLIPBOARD_HISTORY_SLOT_COUNT {
        set_clipboard_row(window, index, "", "", false, false, false);
    }
}

/// Updates the bounded host-owned action panel for the currently selected
/// command. The panel never accepts arbitrary plugin UI or dynamic children.
pub fn set_action_panel(
    window: &ShellWindow,
    visible: bool,
    title: impl AsRef<str>,
    description: impl AsRef<str>,
    primary: impl AsRef<str>,
    secondary: impl AsRef<str>,
) {
    window.set_action_panel_visible(visible);
    window.set_action_panel_title(title.as_ref().into());
    window.set_action_panel_description(description.as_ref().into());
    window.set_action_panel_1_title(primary.as_ref().into());
    window.set_action_panel_1_visible(visible && !primary.as_ref().is_empty());
    window.set_action_panel_2_title(secondary.as_ref().into());
    window.set_action_panel_2_visible(visible && !secondary.as_ref().is_empty());
}

/// Updates the status text displayed by the generated Shell.
pub fn set_status_text(window: &ShellWindow, text: impl AsRef<str>) {
    window.set_status_text(text.as_ref().into());
}

/// Applies the host-selected semantic theme to the Shell surface.
pub fn set_shell_theme(window: &ShellWindow, dark: bool) {
    window.set_dark_theme(dark);
}

/// Applies the host-selected semantic theme to the settings surface.
pub fn set_settings_theme(window: &SettingsWindow, dark: bool) {
    window.set_dark_theme(dark);
}

/// Updates the active provider filter shown beside the search field.
pub fn set_provider_filter_label(window: &ShellWindow, label: impl AsRef<str>) {
    window.set_provider_filter_label(label.as_ref().into());
}

/// Updates the bounded renderer-neutral plugin surface shown by the Shell.
/// Empty values clear the previous View before a normal command starts.
pub fn set_plugin_view(
    window: &ShellWindow,
    kind: impl AsRef<str>,
    title: impl AsRef<str>,
    body: impl AsRef<str>,
) {
    clear_plugin_view_slots(window);
    window.set_plugin_view_kind(kind.as_ref().into());
    window.set_plugin_view_title(title.as_ref().into());
    window.set_plugin_view_body(body.as_ref().into());
}

/// Updates the determinate progress value for the active plugin View.
pub fn set_plugin_view_progress(window: &ShellWindow, progress: Option<f32>) {
    window.set_plugin_progress(progress.unwrap_or(0.0));
}

/// Controls whether Enter routes to the active `ActionPanel` default action.
pub fn set_plugin_default_action_available(window: &ShellWindow, available: bool) {
    window.set_plugin_default_action_visible(available);
}

/// Describes one bounded list/grid item before it is copied into Slint.
pub struct PluginItemSlot<'a> {
    pub title: &'a str,
    pub subtitle: &'a str,
    pub status: &'a str,
    pub badge: &'a str,
    pub shortcut: &'a str,
    pub icon_id: &'a str,
    pub visible: bool,
}

impl PluginItemSlot<'_> {
    #[must_use]
    pub const fn hidden() -> Self {
        Self {
            title: "",
            subtitle: "",
            status: "",
            badge: "",
            shortcut: "",
            icon_id: "",
            visible: false,
        }
    }
}

/// Updates one of the six bounded list/grid slots exposed by the Shell.
pub fn set_plugin_view_item(window: &ShellWindow, index: usize, item: &PluginItemSlot<'_>) {
    let title = item.title.into();
    let subtitle = item.subtitle.into();
    let status = item.status.into();
    let badge = item.badge.into();
    let shortcut = item.shortcut.into();
    let icon_id = item.icon_id.into();
    match index {
        0 => {
            window.set_plugin_item_1_title(title);
            window.set_plugin_item_1_subtitle(subtitle);
            window.set_plugin_item_1_status(status);
            window.set_plugin_item_1_badge(badge);
            window.set_plugin_item_1_shortcut(shortcut);
            window.set_plugin_item_1_icon_id(icon_id);
            window.set_plugin_item_1_visible(item.visible);
        }
        1 => {
            window.set_plugin_item_2_title(title);
            window.set_plugin_item_2_subtitle(subtitle);
            window.set_plugin_item_2_status(status);
            window.set_plugin_item_2_badge(badge);
            window.set_plugin_item_2_shortcut(shortcut);
            window.set_plugin_item_2_icon_id(icon_id);
            window.set_plugin_item_2_visible(item.visible);
        }
        2 => {
            window.set_plugin_item_3_title(title);
            window.set_plugin_item_3_subtitle(subtitle);
            window.set_plugin_item_3_status(status);
            window.set_plugin_item_3_badge(badge);
            window.set_plugin_item_3_shortcut(shortcut);
            window.set_plugin_item_3_icon_id(icon_id);
            window.set_plugin_item_3_visible(item.visible);
        }
        3 => {
            window.set_plugin_item_4_title(title);
            window.set_plugin_item_4_subtitle(subtitle);
            window.set_plugin_item_4_status(status);
            window.set_plugin_item_4_badge(badge);
            window.set_plugin_item_4_shortcut(shortcut);
            window.set_plugin_item_4_icon_id(icon_id);
            window.set_plugin_item_4_visible(item.visible);
        }
        4 => {
            window.set_plugin_item_5_title(title);
            window.set_plugin_item_5_subtitle(subtitle);
            window.set_plugin_item_5_status(status);
            window.set_plugin_item_5_badge(badge);
            window.set_plugin_item_5_shortcut(shortcut);
            window.set_plugin_item_5_icon_id(icon_id);
            window.set_plugin_item_5_visible(item.visible);
        }
        5 => {
            window.set_plugin_item_6_title(title);
            window.set_plugin_item_6_subtitle(subtitle);
            window.set_plugin_item_6_status(status);
            window.set_plugin_item_6_badge(badge);
            window.set_plugin_item_6_shortcut(shortcut);
            window.set_plugin_item_6_icon_id(icon_id);
            window.set_plugin_item_6_visible(item.visible);
        }
        _ => {}
    }
}

/// Updates the bounded list pagination controls exposed by the Shell.
pub fn set_plugin_view_paging(
    window: &ShellWindow,
    previous_visible: bool,
    next_visible: bool,
    next_loads_more: bool,
) {
    window.set_plugin_list_previous_visible(previous_visible);
    window.set_plugin_list_next_visible(next_visible);
    window.set_plugin_list_next_loads_more(next_loads_more);
}

/// Describes one bounded form field before it is copied into the Slint model.
///
/// The host owns the values and option IDs. This UI bridge only receives
/// borrowed display data and translates it into the fixed Shell slots.
pub struct PluginFieldSlot<'a> {
    pub label: &'a str,
    pub kind: &'a str,
    pub helper: &'a str,
    pub error: &'a str,
    pub options: &'a [&'a str],
    pub option_index: usize,
    pub value: &'a str,
    pub required: bool,
    pub visible: bool,
}

impl PluginFieldSlot<'_> {
    /// Returns an empty hidden slot used while clearing the Shell.
    #[must_use]
    pub const fn hidden() -> Self {
        Self {
            label: "",
            kind: "text",
            helper: "",
            error: "",
            options: &[],
            option_index: 0,
            value: "",
            required: false,
            visible: false,
        }
    }
}

/// Updates one of the six bounded form fields exposed by the Shell.
pub fn set_plugin_view_field(window: &ShellWindow, index: usize, field: &PluginFieldSlot<'_>) {
    let label = field.label.into();
    let kind = field.kind.into();
    let helper = field.helper.into();
    let error = field.error.into();
    let options = ModelRc::new(
        field
            .options
            .iter()
            .map(|option| SharedString::from(*option))
            .collect::<VecModel<_>>(),
    );
    let option_index = i32::try_from(field.option_index).unwrap_or(i32::MAX);
    let value = field.value.into();
    match index {
        0 => {
            window.set_plugin_field_1_label(label);
            window.set_plugin_field_1_kind(kind);
            window.set_plugin_field_1_value(value);
            window.set_plugin_field_1_helper(helper);
            window.set_plugin_field_1_error(error);
            window.set_plugin_field_1_options(options);
            window.set_plugin_field_1_option_index(option_index);
            window.set_plugin_field_1_required(field.required);
            window.set_plugin_field_1_visible(field.visible);
        }
        1 => {
            window.set_plugin_field_2_label(label);
            window.set_plugin_field_2_kind(kind);
            window.set_plugin_field_2_value(value);
            window.set_plugin_field_2_helper(helper);
            window.set_plugin_field_2_error(error);
            window.set_plugin_field_2_options(options);
            window.set_plugin_field_2_option_index(option_index);
            window.set_plugin_field_2_required(field.required);
            window.set_plugin_field_2_visible(field.visible);
        }
        2 => {
            window.set_plugin_field_3_label(label);
            window.set_plugin_field_3_kind(kind);
            window.set_plugin_field_3_value(value);
            window.set_plugin_field_3_helper(helper);
            window.set_plugin_field_3_error(error);
            window.set_plugin_field_3_options(options);
            window.set_plugin_field_3_option_index(option_index);
            window.set_plugin_field_3_required(field.required);
            window.set_plugin_field_3_visible(field.visible);
        }
        3 => {
            window.set_plugin_field_4_label(label);
            window.set_plugin_field_4_kind(kind);
            window.set_plugin_field_4_value(value);
            window.set_plugin_field_4_helper(helper);
            window.set_plugin_field_4_error(error);
            window.set_plugin_field_4_options(options);
            window.set_plugin_field_4_option_index(option_index);
            window.set_plugin_field_4_required(field.required);
            window.set_plugin_field_4_visible(field.visible);
        }
        4 => {
            window.set_plugin_field_5_label(label);
            window.set_plugin_field_5_kind(kind);
            window.set_plugin_field_5_value(value);
            window.set_plugin_field_5_helper(helper);
            window.set_plugin_field_5_error(error);
            window.set_plugin_field_5_options(options);
            window.set_plugin_field_5_option_index(option_index);
            window.set_plugin_field_5_required(field.required);
            window.set_plugin_field_5_visible(field.visible);
        }
        5 => {
            window.set_plugin_field_6_label(label);
            window.set_plugin_field_6_kind(kind);
            window.set_plugin_field_6_value(value);
            window.set_plugin_field_6_helper(helper);
            window.set_plugin_field_6_error(error);
            window.set_plugin_field_6_options(options);
            window.set_plugin_field_6_option_index(option_index);
            window.set_plugin_field_6_required(field.required);
            window.set_plugin_field_6_visible(field.visible);
        }
        _ => {}
    }
}

/// Updates one of the six bounded action-panel buttons exposed by the Shell.
pub fn set_plugin_view_action(
    window: &ShellWindow,
    index: usize,
    title: impl AsRef<str>,
    visible: bool,
) {
    let title = title.as_ref().into();
    match index {
        0 => {
            window.set_plugin_action_1_title(title);
            window.set_plugin_action_1_visible(visible);
        }
        1 => {
            window.set_plugin_action_2_title(title);
            window.set_plugin_action_2_visible(visible);
        }
        2 => {
            window.set_plugin_action_3_title(title);
            window.set_plugin_action_3_visible(visible);
        }
        3 => {
            window.set_plugin_action_4_title(title);
            window.set_plugin_action_4_visible(visible);
        }
        4 => {
            window.set_plugin_action_5_title(title);
            window.set_plugin_action_5_visible(visible);
        }
        5 => {
            window.set_plugin_action_6_title(title);
            window.set_plugin_action_6_visible(visible);
        }
        _ => {}
    }
}

/// Clears all interactive plugin slots before a new View is applied.
pub fn clear_plugin_view_slots(window: &ShellWindow) {
    set_plugin_view_paging(window, false, false, false);
    set_plugin_view_progress(window, None);
    set_plugin_default_action_available(window, false);
    for index in 0..6 {
        set_plugin_view_item(window, index, &PluginItemSlot::hidden());
        set_plugin_view_field(window, index, &PluginFieldSlot::hidden());
        set_plugin_view_action(window, index, "", false);
    }
}

/// Top-level launcher state owned by the host shell.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ShellMode {
    Idle,
    Searching,
    Actions,
}

/// Small state machine for focus and action-panel ownership.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ShellState {
    mode: ShellMode,
}

impl Default for ShellState {
    fn default() -> Self {
        Self::new()
    }
}

impl ShellState {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            mode: ShellMode::Idle,
        }
    }

    #[must_use]
    pub const fn mode(self) -> ShellMode {
        self.mode
    }

    #[must_use]
    pub const fn input_focused(self) -> bool {
        matches!(self.mode, ShellMode::Searching)
    }

    pub fn begin_query(&mut self) {
        self.mode = ShellMode::Searching;
    }

    pub fn show_actions(&mut self) {
        self.mode = ShellMode::Actions;
    }

    pub fn dismiss(&mut self) {
        self.mode = ShellMode::Idle;
    }
}

/// Stable row data used by virtualized list adapters.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StableRow {
    id: String,
    title: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StableListOp {
    Insert { index: usize, row: StableRow },
    Remove { index: usize, id: String },
    Move { from: usize, to: usize, id: String },
    Update { index: usize, row: StableRow },
}

/// Computes bounded host-side list changes by stable ID. The protocol still
/// exchanges complete snapshots; this operation list is an internal renderer
/// optimization and is never exposed as a second plugin ABI.
///
/// # Errors
///
/// Returns an error when either snapshot exceeds host row limits or contains
/// an empty or duplicate stable ID.
pub fn diff_stable_rows(
    previous: &[StableRow],
    next: &[StableRow],
) -> Result<Vec<StableListOp>, String> {
    const MAX_ROWS: usize = 10_000;
    if previous.len() > MAX_ROWS || next.len() > MAX_ROWS {
        return Err("stable list exceeds the host row limit".into());
    }
    validate_stable_rows(previous)?;
    validate_stable_rows(next)?;
    let previous_by_id = previous
        .iter()
        .enumerate()
        .map(|(index, row)| (row.id().to_owned(), (index, row)))
        .collect::<std::collections::BTreeMap<_, _>>();
    let next_by_id = next
        .iter()
        .enumerate()
        .map(|(index, row)| (row.id().to_owned(), (index, row)))
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut operations = Vec::new();
    for (index, row) in previous.iter().enumerate() {
        if !next_by_id.contains_key(row.id()) {
            operations.push(StableListOp::Remove {
                index,
                id: row.id().to_owned(),
            });
        }
    }
    for (index, row) in next.iter().enumerate() {
        match previous_by_id.get(row.id()) {
            None => operations.push(StableListOp::Insert {
                index,
                row: row.clone(),
            }),
            Some((previous_index, previous_row)) => {
                if *previous_index != index {
                    operations.push(StableListOp::Move {
                        from: *previous_index,
                        to: index,
                        id: row.id().to_owned(),
                    });
                }
                if *previous_row != row {
                    operations.push(StableListOp::Update {
                        index,
                        row: row.clone(),
                    });
                }
            }
        }
    }
    Ok(operations)
}

fn validate_stable_rows(rows: &[StableRow]) -> Result<(), String> {
    let mut ids = std::collections::BTreeSet::new();
    if rows
        .iter()
        .any(|row| row.id().trim().is_empty() || !ids.insert(row.id()))
    {
        return Err("stable list row IDs must be non-empty and unique".into());
    }
    Ok(())
}

impl StableRow {
    pub fn new(id: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
        }
    }

    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    #[must_use]
    pub fn title(&self) -> &str {
        &self.title
    }
}

/// Bounded host-side list model that preserves selection by stable ID.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct StableList {
    rows: Vec<StableRow>,
    selected_id: Option<String>,
    focused_id: Option<String>,
    scroll_anchor_id: Option<String>,
}

/// Borrowed row window exposed to the native renderer.
///
/// The complete snapshot stays in the host model. Slint receives only this
/// bounded slice, so list memory and component count do not scale together.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StableListWindow<'a> {
    start_index: usize,
    rows: &'a [StableRow],
}

const MAX_VISIBLE_ROWS: usize = 128;
const MAX_OVERSCAN_ROWS: usize = 32;

impl StableList {
    #[must_use]
    pub fn new(rows: Vec<StableRow>) -> Self {
        Self {
            rows,
            selected_id: None,
            focused_id: None,
            scroll_anchor_id: None,
        }
    }

    pub fn select(&mut self, id: impl AsRef<str>) {
        self.selected_id = retained_row_id(&self.rows, id.as_ref());
    }

    pub fn focus(&mut self, id: impl AsRef<str>) {
        self.focused_id = retained_row_id(&self.rows, id.as_ref());
    }

    pub fn set_scroll_anchor(&mut self, id: impl AsRef<str>) {
        self.scroll_anchor_id = retained_row_id(&self.rows, id.as_ref());
    }

    pub fn replace(&mut self, rows: Vec<StableRow>) {
        let selected = self.selected_id.clone();
        let focused = self.focused_id.clone();
        let scroll_anchor = self.scroll_anchor_id.clone();
        self.rows = rows;
        self.selected_id = selected.and_then(|id| retained_row_id(&self.rows, &id));
        self.focused_id = focused.and_then(|id| retained_row_id(&self.rows, &id));
        self.scroll_anchor_id = scroll_anchor.and_then(|id| retained_row_id(&self.rows, &id));
    }

    /// Applies a complete snapshot while returning the host-only stable-ID
    /// operations used by a virtualized renderer.
    ///
    /// # Errors
    ///
    /// Returns the same validation errors as [`diff_stable_rows`]. The current
    /// rows remain unchanged when validation fails.
    pub fn replace_with_diff(&mut self, rows: Vec<StableRow>) -> Result<Vec<StableListOp>, String> {
        let operations = diff_stable_rows(&self.rows, &rows)?;
        self.replace(rows);
        Ok(operations)
    }

    #[must_use]
    pub fn rows(&self) -> &[StableRow] {
        &self.rows
    }

    #[must_use]
    pub fn selected_id(&self) -> Option<&str> {
        self.selected_id.as_deref()
    }

    #[must_use]
    pub fn focused_id(&self) -> Option<&str> {
        self.focused_id.as_deref()
    }

    #[must_use]
    pub fn scroll_anchor_id(&self) -> Option<&str> {
        self.scroll_anchor_id.as_deref()
    }

    /// Returns a bounded viewport with symmetric overscan.
    ///
    /// # Errors
    ///
    /// Returns an error when the viewport is empty or exceeds the renderer
    /// limits. An out-of-date first row is clamped to the current tail.
    pub fn window(
        &self,
        first_visible: usize,
        visible_rows: usize,
        overscan_rows: usize,
    ) -> Result<StableListWindow<'_>, String> {
        if visible_rows == 0 || visible_rows > MAX_VISIBLE_ROWS {
            return Err("stable list viewport is outside the host limit".into());
        }
        if overscan_rows > MAX_OVERSCAN_ROWS {
            return Err("stable list overscan is outside the host limit".into());
        }
        if self.rows.is_empty() {
            return Ok(StableListWindow {
                start_index: 0,
                rows: &[],
            });
        }

        let first_visible = first_visible.min(self.rows.len().saturating_sub(1));
        let start_index = first_visible.saturating_sub(overscan_rows);
        let end_index = first_visible
            .saturating_add(visible_rows)
            .saturating_add(overscan_rows)
            .min(self.rows.len());
        Ok(StableListWindow {
            start_index,
            rows: &self.rows[start_index..end_index],
        })
    }
}

impl<'a> StableListWindow<'a> {
    #[must_use]
    pub const fn start_index(self) -> usize {
        self.start_index
    }

    #[must_use]
    pub const fn rows(self) -> &'a [StableRow] {
        self.rows
    }
}

fn retained_row_id(rows: &[StableRow], id: &str) -> Option<String> {
    rows.iter().any(|row| row.id() == id).then(|| id.to_owned())
}

#[cfg(test)]
mod tests {
    use super::{
        ShellMode, ShellState, StableList, StableListOp, StableRow, center_offset,
        diff_stable_rows, edge_offset, logical_coordinate, logical_dimension, logical_dpi,
    };

    #[test]
    fn shell_state_owns_focus_and_panel_transitions() {
        let mut state = ShellState::new();
        assert_eq!(state.mode(), ShellMode::Idle);
        assert!(!state.input_focused());

        state.begin_query();
        assert_eq!(state.mode(), ShellMode::Searching);
        assert!(state.input_focused());

        state.show_actions();
        assert_eq!(state.mode(), ShellMode::Actions);
        assert!(!state.input_focused());

        state.dismiss();
        assert_eq!(state.mode(), ShellMode::Idle);
    }

    #[test]
    fn stable_list_replaces_rows_without_losing_selected_id() {
        let mut list = StableList::new(vec![StableRow::new("a", "A"), StableRow::new("b", "B")]);
        list.select("b");
        list.focus("b");
        list.set_scroll_anchor("b");
        list.replace(vec![
            StableRow::new("b", "B updated"),
            StableRow::new("c", "C"),
        ]);
        assert_eq!(list.selected_id(), Some("b"));
        assert_eq!(list.focused_id(), Some("b"));
        assert_eq!(list.scroll_anchor_id(), Some("b"));
        assert_eq!(list.rows()[0].title(), "B updated");
    }

    #[test]
    fn stable_list_window_bounds_rendered_rows_for_large_snapshots() {
        for row_count in [100, 1_000, 10_000] {
            let list = StableList::new(
                (0..row_count)
                    .map(|index| StableRow::new(format!("row-{index}"), format!("Row {index}")))
                    .collect(),
            );
            let window = list.window(row_count / 2, 12, 5).expect("bounded window");
            assert!(window.rows().len() <= 22);
            assert_eq!(window.start_index(), row_count / 2 - 5);
            assert_eq!(window.rows()[5].id(), format!("row-{}", row_count / 2));
        }

        let list = StableList::new(vec![StableRow::new("only", "Only")]);
        assert!(list.window(0, 0, 0).is_err());
        assert!(list.window(0, 129, 0).is_err());
        assert!(list.window(0, 1, 33).is_err());
    }

    #[test]
    fn stable_list_diff_preserves_ids_for_insert_move_and_update() {
        let previous = vec![StableRow::new("a", "A"), StableRow::new("b", "B")];
        let next = vec![StableRow::new("b", "B updated"), StableRow::new("c", "C")];
        let operations = diff_stable_rows(&previous, &next).expect("stable diff");
        assert!(operations.contains(&StableListOp::Remove {
            index: 0,
            id: "a".into()
        }));
        assert!(operations.contains(&StableListOp::Insert {
            index: 1,
            row: StableRow::new("c", "C")
        }));
        assert!(operations.contains(&StableListOp::Move {
            from: 1,
            to: 0,
            id: "b".into()
        }));
        assert!(operations.contains(&StableListOp::Update {
            index: 0,
            row: StableRow::new("b", "B updated")
        }));
    }

    #[test]
    fn window_offsets_clamp_when_the_window_is_larger_than_the_monitor() {
        assert_eq!(center_offset(1_920, 960), 480);
        assert_eq!(center_offset(800, 1_000), 0);
        assert_eq!(edge_offset(1_920, 360, 24), 1_536);
        assert_eq!(edge_offset(300, 360, 24), 0);
    }

    #[test]
    fn display_geometry_converts_physical_values_to_logical_values() {
        assert_eq!(logical_coordinate(-1_440, 1.5), -960);
        assert_eq!(logical_dimension(2_880, 1.5), 1_920);
        assert_eq!(logical_dimension(0, 2.0), 1);
        assert_eq!(logical_dpi(1.25), 120);
        assert_eq!(logical_dpi(0.0), 96);
    }
}

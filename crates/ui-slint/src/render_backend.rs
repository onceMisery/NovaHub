use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use slint::ComponentHandle;
use slint::winit_030::{CustomApplicationHandler, EventResult, winit};

use crate::ShellWindow;

const SHELL_WINDOW_TITLE: &str = "NovaHub";

type FrameCallback = Box<dyn FnOnce()>;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum ObserverMode {
    #[default]
    Uninitialized,
    RenderingNotifier,
    WinitRedraw,
}

#[derive(Default)]
struct ObserverState {
    mode: ObserverMode,
    callback: Option<FrameCallback>,
}

/// Coordinates first-frame observation across Slint renderers.
///
/// GPU renderers use Slint's precise `AfterRendering` notification. The
/// software renderer falls back to a zero-delay callback scheduled from the
/// Shell's first Winit redraw event, after Slint completes that synchronous
/// redraw.
#[derive(Clone, Default)]
pub struct ShellFrameObserver {
    state: Rc<RefCell<ObserverState>>,
}

impl ShellFrameObserver {
    fn register(&self, callback: impl FnOnce() + 'static) -> Result<(), String> {
        let mut state = self.state.borrow_mut();
        if state.callback.is_some() {
            return Err("Shell first-frame observer is already registered".to_owned());
        }
        state.callback = Some(Box::new(callback));
        Ok(())
    }

    fn set_mode(&self, mode: ObserverMode) {
        self.state.borrow_mut().mode = mode;
    }

    fn mode(&self) -> ObserverMode {
        self.state.borrow().mode
    }

    fn clear_callback(&self) {
        self.state.borrow_mut().callback.take();
    }
}

struct ShellRedrawHandler {
    observer: ShellFrameObserver,
}

impl CustomApplicationHandler for ShellRedrawHandler {
    fn window_event(
        &mut self,
        _event_loop: &winit::event_loop::ActiveEventLoop,
        _window_id: winit::window::WindowId,
        winit_window: Option<&winit::window::Window>,
        _slint_window: Option<&slint::Window>,
        event: &winit::event::WindowEvent,
    ) -> EventResult {
        let is_shell_redraw = matches!(event, winit::event::WindowEvent::RedrawRequested)
            && winit_window.is_some_and(|window| window.title() == SHELL_WINDOW_TITLE);
        if is_shell_redraw {
            let callback = {
                let mut state = self.observer.state.borrow_mut();
                if state.mode == ObserverMode::WinitRedraw {
                    state.callback.take()
                } else {
                    None
                }
            };
            if let Some(callback) = callback {
                slint::Timer::single_shot(Duration::ZERO, callback);
            }
        }
        EventResult::Propagate
    }
}

/// Selects the native Slint backend before any component is created.
///
/// Windows defaults to the software renderer because the reference-machine
/// probe showed a materially smaller resident set. `SLINT_BACKEND` remains an
/// explicit diagnostic override. Other platforms keep Slint's renderer
/// default until their own reference-machine probes are available.
///
/// # Errors
///
/// Returns an error when the requested Winit backend or renderer cannot be
/// initialized.
pub fn initialize_shell_backend() -> Result<ShellFrameObserver, String> {
    let observer = ShellFrameObserver::default();
    let selector = slint::BackendSelector::new()
        .backend_name("winit".into())
        .with_winit_custom_application_handler(ShellRedrawHandler {
            observer: observer.clone(),
        });
    #[cfg(windows)]
    let selector = if std::env::var_os("SLINT_BACKEND").is_none() {
        selector.renderer_name("software".into())
    } else {
        selector
    };
    selector.select().map_err(|error| error.to_string())?;
    Ok(observer)
}

/// Invokes one callback after the Shell has rendered its next frame.
///
/// # Errors
///
/// Returns an error when another callback is already registered or when the
/// rendering notifier cannot be installed.
pub fn on_shell_next_frame(
    window: &ShellWindow,
    observer: &ShellFrameObserver,
    callback: impl FnOnce() + 'static,
) -> Result<(), String> {
    observer.register(callback)?;
    if observer.mode() == ObserverMode::RenderingNotifier
        || observer.mode() == ObserverMode::WinitRedraw
    {
        return Ok(());
    }
    let notifier_observer = observer.clone();
    match window
        .window()
        .set_rendering_notifier(move |state, _graphics_api| {
            if matches!(state, slint::RenderingState::AfterRendering) {
                let callback = {
                    let mut state = notifier_observer.state.borrow_mut();
                    state.callback.take()
                };
                if let Some(callback) = callback {
                    callback();
                }
            }
        }) {
        Ok(()) => {
            observer.set_mode(ObserverMode::RenderingNotifier);
            Ok(())
        }
        Err(slint::SetRenderingNotifierError::Unsupported) => {
            observer.set_mode(ObserverMode::WinitRedraw);
            Ok(())
        }
        Err(error) => {
            observer.clear_callback();
            Err(error.to_string())
        }
    }
}

/// Compatibility name for the first-frame call site. The observer itself is
/// reusable, so later calls can observe another rendered frame as well.
pub fn on_shell_first_frame(
    window: &ShellWindow,
    observer: &ShellFrameObserver,
    callback: impl FnOnce() + 'static,
) -> Result<(), String> {
    on_shell_next_frame(window, observer, callback)
}

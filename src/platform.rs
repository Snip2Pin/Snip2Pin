use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

use eframe::egui;
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState, hotkey::HotKey};
use snip2pin::i18n::{Message, Text};
use snip2pin::settings::Binding;

#[path = "platform/pin_menu.rs"]
pub mod pin_menu;

#[path = "platform/pin_window.rs"]
pub mod pin_window;

/// Convert the native viewport's observed coordinates at the desktop seam.
pub fn pin_input_space(ui: &egui::Ui) -> snip2pin::ui::pin::InputSpace {
    snip2pin::ui::pin::InputSpace {
        origin: ui.input(|input| input.viewport().inner_rect.map(|rect| rect.min)),
        scale: if cfg!(target_os = "macos") {
            ui.ctx().zoom_factor()
        } else {
            ui.ctx().pixels_per_point()
        },
    }
}

/// Lay out controls using the native window's presentation constraints.
pub fn pin_toolbar(
    _id: u64,
    image: egui::Rect,
    screens: &[egui::Rect],
    fallback: egui::Rect,
    ratio: f32,
) -> Option<snip2pin::pin::Toolbar> {
    #[cfg(target_os = "macos")]
    if let Some(area) = macos::pin_presentation_area(_id) {
        return snip2pin::pin::toolbar_geometry(image, &[], area, ratio);
    }
    snip2pin::pin::toolbar_geometry(image, screens, fallback, ratio)
}

#[path = "platform/pin_zoom.rs"]
pub mod pin_zoom;

#[path = "platform/quit_prompt.rs"]
pub mod quit_prompt;

#[cfg(target_os = "macos")]
#[path = "platform/macos_drag.rs"]
pub mod macos_drag;

#[cfg(any(target_os = "windows", target_os = "linux"))]
#[path = "platform/work_area.rs"]
mod work_area;

#[cfg(target_os = "macos")]
#[path = "platform/macos.rs"]
mod macos;

#[cfg(target_os = "macos")]
pub fn activate_application() -> Result<(), Message> {
    macos::activate_application()
}

#[cfg(target_os = "macos")]
pub use macos::PinLock;

#[cfg(target_os = "macos")]
pub fn set_pin_locked(id: u64, locked: bool, saved: &mut Option<PinLock>) -> Result<(), Message> {
    let window = pin_window::find(id)?;
    macos::set_locked(&window, locked, saved);
    Ok(())
}

#[cfg(target_os = "macos")]
#[path = "platform/macos_icon.rs"]
pub mod macos_icon;

#[cfg(target_os = "macos")]
#[path = "platform/macos_access.rs"]
pub mod macos_access;

/// macOS window content uses whole native points. Snap selection in source
/// pixels when a point contains an integral number of pixels. Synthetic or
/// non-integral capture densities retain the source-pixel grid instead.
pub fn selection_pixel_step(snapshot: &snip2pin::capture::Snapshot) -> [u32; 2] {
    #[cfg(target_os = "macos")]
    {
        let pixels = snapshot.pixel_size();
        let size = snapshot.display.logical_size();
        std::array::from_fn(|axis| {
            let density = f64::from(pixels[axis]) / f64::from(size[axis]);
            let rounded = density.round();
            // Display dimensions enter as f32; allow their representation error,
            // without treating a genuinely fractional density as an integer.
            let tolerance = f64::from(f32::EPSILON) * density.abs();
            if density.is_finite()
                && rounded >= 1.0
                && rounded <= f64::from(u32::MAX)
                && (density - rounded).abs() <= tolerance
            {
                rounded as u32
            } else {
                1
            }
        })
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = snapshot;
        [1, 1]
    }
}

pub fn capture_permission() -> bool {
    #[cfg(target_os = "macos")]
    return objc2_core_graphics::CGPreflightScreenCaptureAccess();
    #[cfg(not(target_os = "macos"))]
    true
}

pub fn request_capture_permission() -> bool {
    #[cfg(target_os = "macos")]
    return macos_access::request();
    #[cfg(not(target_os = "macos"))]
    true
}

pub fn open_capture_settings() -> Result<(), Message> {
    #[cfg(target_os = "macos")]
    return macos_access::open_settings();
    #[cfg(not(target_os = "macos"))]
    Err(Text::NoSettingsEntry.into())
}

/// Prepare a user-requested relaunch; the caller runs it after the app has
/// dropped all native windows and global shortcuts. No waiting or polling.
pub fn relaunch_command() -> Result<std::process::Command, Message> {
    let executable = std::env::current_exe().map_err(|error| error.to_string())?;
    #[cfg(target_os = "macos")]
    if let Some(bundle) = executable
        .ancestors()
        .find(|path| path.extension().is_some_and(|extension| extension == "app"))
    {
        let mut command = std::process::Command::new("/usr/bin/open");
        command.arg("-n").arg(bundle);
        return Ok(command);
    }
    Ok(std::process::Command::new(executable))
}

#[cfg(target_os = "macos")]
const DISPLAY_ORIGIN_TOLERANCE: f32 = 1.0;

#[cfg(target_os = "macos")]
fn matching_work_areas(
    bounds: Vec<egui::Rect>,
    areas: &[(egui::Rect, egui::Rect)],
) -> Vec<egui::Rect> {
    bounds
        .into_iter()
        .map(|bounds| {
            areas
                .iter()
                .find(|(frame, _)| frame.min.distance(bounds.min) < DISPLAY_ORIGIN_TOLERANCE)
                .map_or(bounds, |(_, area)| *area)
        })
        .collect()
}

/// Query each desktop once and return usable bounds in the input display order.
pub fn pin_work_areas(displays: &[snip2pin::capture::Display]) -> Vec<egui::Rect> {
    if displays.is_empty() {
        return Vec::new();
    }
    let bounds: Vec<_> = displays
        .iter()
        .map(|display| {
            egui::Rect::from_min_size(display.position().into(), display.logical_size().into())
        })
        .collect();
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    {
        work_area::read_many(displays)
            .into_iter()
            .zip(bounds)
            .map(|(area, bounds)| area.unwrap_or(bounds))
            .collect()
    }
    #[cfg(target_os = "macos")]
    if let Some(thread) = objc2::MainThreadMarker::new() {
        let screens = objc2_app_kit::NSScreen::screens(thread);
        if let Some(primary) = screens.iter().next() {
            let top = macos::primary_top(primary.frame());
            let areas: Vec<_> = screens
                .iter()
                .map(|screen| {
                    (
                        macos::backend_rect(screen.frame(), top),
                        macos::backend_rect(screen.visibleFrame(), top),
                    )
                })
                .collect();
            return matching_work_areas(bounds, &areas);
        }
    }
    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    bounds
}

/// Read the cursor on capture entry or a user drag event, in the same coordinates
/// as xcap's monitor bounds. No event taps, global hooks, or polling are used.
#[cfg(target_os = "macos")]
pub fn pointer_position() -> Option<[f32; 2]> {
    let event = objc2_core_graphics::CGEvent::new(None)?;
    let point = objc2_core_graphics::CGEvent::location(Some(&event));
    Some([point.x as f32, point.y as f32])
}

#[cfg(target_os = "windows")]
#[allow(unsafe_code)]
pub fn pointer_position() -> Option<[f32; 2]> {
    use windows_sys::Win32::{Foundation::POINT, UI::WindowsAndMessaging::GetPhysicalCursorPos};
    let mut point = POINT { x: 0, y: 0 };
    // SAFETY: A valid, writable POINT stays alive for this synchronous API call.
    (unsafe { GetPhysicalCursorPos(&mut point) } != 0).then_some([point.x as f32, point.y as f32])
}

#[cfg(target_os = "linux")]
pub fn pointer_position() -> Option<[f32; 2]> {
    use x11rb::{connection::Connection, protocol::xproto::ConnectionExt};
    let (connection, screen) = x11rb::connect(None).ok()?;
    let root = connection.setup().roots.get(screen)?.root;
    let pointer = connection.query_pointer(root).ok()?.reply().ok()?;
    pointer
        .same_screen
        .then_some([f32::from(pointer.root_x), f32::from(pointer.root_y)])
}

pub fn set_app_icon(
    ctx: &egui::Context,
    variant: snip2pin::icon::Variant,
    data: Arc<egui::IconData>,
) -> Result<(), Message> {
    let viewports = ctx.input(|input| input.raw.viewports.keys().copied().collect::<Vec<_>>());
    for viewport in viewports {
        ctx.send_viewport_cmd_to(viewport, egui::ViewportCommand::Icon(Some(data.clone())));
    }
    #[cfg(target_os = "macos")]
    macos_icon::update(variant)?;
    #[cfg(not(target_os = "macos"))]
    let _ = variant;
    Ok(())
}

/// Selection windows can cover the menu bar and fullscreen applications without
/// asking macOS to create a Space. All calls stay on the verified main thread.
pub fn prepare_floating_windows() -> Result<(), Message> {
    #[cfg(target_os = "macos")]
    if let Some(main_thread) = objc2::MainThreadMarker::new() {
        use objc2_app_kit::{NSApplication, NSWindowCollectionBehavior};
        let application = NSApplication::sharedApplication(main_thread);
        for window in application.windows() {
            let title = window.title().to_string();
            let pin_or_menu =
                pin_window::id_from_title(&title).is_some() || title == "Snip2Pin · 操作";
            if title == snip2pin::SELECTION_WINDOW_TITLE || pin_or_menu {
                let behavior = NSWindowCollectionBehavior::CanJoinAllSpaces
                    | NSWindowCollectionBehavior::Stationary
                    | NSWindowCollectionBehavior::IgnoresCycle
                    | NSWindowCollectionBehavior::FullScreenAuxiliary;
                if window.collectionBehavior() != behavior {
                    window.setCollectionBehavior(behavior);
                }
                if pin_or_menu {
                    macos::prepare_pin(&window)?;
                }
                if title == snip2pin::SELECTION_WINDOW_TITLE {
                    // The demo is a regular small test window; it need not
                    // cover system UI or block other apps during development.
                    let level = if std::env::args().any(|argument| argument == "--demo") {
                        0
                    } else {
                        (objc2_core_graphics::kCGStatusWindowLevel + 1) as isize
                    };
                    if window.level() != level {
                        window.setLevel(level);
                    }
                    if level != 0
                        && let Some(screen) = window.screen()
                        && window.frame() != screen.frame()
                    {
                        // Raising the level does not undo an earlier work-area
                        // constraint. Restore the entire screen, including the
                        // menu bar and Dock, without a fullscreen transition.
                        window.setFrame_display(screen.frame(), false);
                    }
                }
            }
        }
    }
    Ok(())
}

/// Raise this process's existing transparent layers without focus or geometry changes.
pub fn raise_screen_hint() -> Result<(), Message> {
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    pin_window::raise_selection()?;
    // macOS selection layers already have a higher native level than pins.
    Ok(())
}

pub fn finish_capture(ctx: &egui::Context) {
    #[cfg(target_os = "macos")]
    {
        let _ = ctx;
        if let Some(main_thread) = objc2::MainThreadMarker::new() {
            objc2_app_kit::NSCursor::arrowCursor().set();
            let application = objc2_app_kit::NSApplication::sharedApplication(main_thread);
            for window in application.windows() {
                if window.title().to_string() == snip2pin::SELECTION_WINDOW_TITLE
                    && window.isKeyWindow()
                {
                    window.resignKeyWindow();
                }
            }
            application.deactivate();
        }
    }
    #[cfg(not(target_os = "macos"))]
    ctx.send_viewport_cmd_to(egui::ViewportId::ROOT, egui::ViewportCommand::Focus);
}

/// A GUI-subsystem executable has no visible stderr when opened from Explorer.
#[cfg(target_os = "windows")]
pub fn show_startup_error(error: &str) {
    use snip2pin::{i18n::Text, settings::Preferences};
    use windows_sys::Win32::UI::WindowsAndMessaging::{MB_ICONERROR, MB_OK, MessageBoxW};

    let language = snip2pin::settings::path()
        .and_then(|path| Preferences::load(&path).ok())
        .unwrap_or_default()
        .language;
    let locale = language.resolve(snip2pin::i18n::Locale::system());
    let title: Vec<u16> = locale
        .text(Text::StartupFailed)
        .encode_utf16()
        .chain([0])
        .collect();
    let body: Vec<u16> = error.encode_utf16().chain([0]).collect();
    // SAFETY: This ownerless native dialog can run even when graphics setup
    // failed. Both null-terminated buffers remain owned until it returns.
    #[allow(unsafe_code)]
    unsafe {
        MessageBoxW(
            std::ptr::null_mut(),
            body.as_ptr(),
            title.as_ptr(),
            MB_OK | MB_ICONERROR,
        );
    }
}

pub fn window_options(options: eframe::NativeOptions) -> eframe::NativeOptions {
    #[cfg(target_os = "macos")]
    if std::env::args().any(|arg| arg == "--background") {
        use winit::platform::macos::EventLoopBuilderExtMacOS;
        let mut options = options;
        options.event_loop_builder = Some(Box::new(|builder| {
            builder.with_activate_ignoring_other_apps(false);
        }));
        return options;
    }
    // XWayland preserves window-level requests that native Wayland cannot make.
    #[cfg(target_os = "linux")]
    if std::env::var_os("DISPLAY").is_some() {
        use winit::platform::x11::EventLoopBuilderExtX11;
        let mut options = options;
        options.event_loop_builder = Some(Box::new(|builder| {
            builder.with_x11();
        }));
        return options;
    }
    options
}

pub struct Shortcut {
    manager: GlobalHotKeyManager,
    key: Option<HotKey>,
    recovery: Option<HotKey>,
    active_id: Arc<AtomicU64>,
}

pub fn global_shortcuts_supported() -> bool {
    !(cfg!(target_os = "linux") && std::env::var_os("WAYLAND_DISPLAY").is_some())
}

impl Shortcut {
    pub fn capture_registered(&self) -> bool {
        self.key.is_some()
    }
    pub fn register(
        binding: Binding,
        action: impl Fn(u32) + Send + Sync + 'static,
        unlock: impl Fn() + Send + Sync + 'static,
    ) -> Result<(Self, Option<Message>), Message> {
        if !global_shortcuts_supported() {
            return Err(Text::WaylandShortcut.into());
        }
        let manager = GlobalHotKeyManager::new().map_err(|error| error.to_string())?;
        // Independent registration: a conflict must never disable the other
        // shortcut, especially the recovery path for inaccessible locked pins.
        let (key, recovery, warning) = register_shortcuts(binding, |key| {
            manager.register(key).map_err(|error| error.to_string())
        });
        let active_id = Arc::new(AtomicU64::new(
            key.map_or(u64::MAX, |key| u64::from(key.id())),
        ));
        let handler_id = active_id.clone();
        GlobalHotKeyEvent::set_event_handler(Some(move |event: GlobalHotKeyEvent| {
            if event.state != HotKeyState::Pressed {
                return;
            }
            if recovery.is_some_and(|key| event.id == key.id()) {
                unlock();
            } else if u64::from(event.id) == handler_id.load(Ordering::Acquire) {
                action(event.id);
            }
        }));
        Ok((
            Self {
                manager,
                key,
                recovery,
                active_id,
            },
            warning,
        ))
    }

    pub fn replace(&mut self, binding: Binding) -> Result<(), Message> {
        binding.validate_capture()?;
        let key = binding.0;
        if Some(key) == self.key {
            return Ok(());
        }
        // Register first so a conflict leaves the existing shortcut working.
        self.manager
            .register(key)
            .map_err(|error| Text::ShortcutError.with(error))?;
        if let Some(previous) = self.key
            && let Err(error) = self.manager.unregister(previous)
        {
            let _ = self.manager.unregister(key);
            return Err(Text::UpdateShortcutError.with(error));
        }
        self.key = Some(key);
        self.active_id.store(u64::from(key.id()), Ordering::Release);
        Ok(())
    }
}

impl Drop for Shortcut {
    fn drop(&mut self) {
        for key in [self.key, self.recovery].into_iter().flatten() {
            let _ = self.manager.unregister(key);
        }
        GlobalHotKeyEvent::set_event_handler(None::<fn(GlobalHotKeyEvent)>);
    }
}

fn register_shortcuts(
    binding: Binding,
    mut register: impl FnMut(HotKey) -> Result<(), String>,
) -> (Option<HotKey>, Option<HotKey>, Option<Message>) {
    let mut warnings = Vec::new();
    let capture = binding
        .validate_capture()
        .and_then(|()| register(binding.0).map_err(|error| Text::ShortcutError.with(error)));
    let key = match capture {
        Ok(()) => Some(binding.0),
        Err(error) => {
            warnings.push(error);
            None
        }
    };
    let recovery = Binding::unlock_all().0;
    let recovery = match register(recovery) {
        Ok(()) => Some(recovery),
        Err(error) => {
            warnings.push(Text::UnlockShortcutError.with(error));
            None
        }
    };
    let warning = (!warnings.is_empty()).then_some(Message::Combined(warnings));
    (key, recovery, warning)
}

#[cfg(test)]
mod shortcut_tests {

    #[test]
    fn registration_conflicts_never_disable_the_other_shortcut() {
        use super::*;
        for (capture_fails, recovery_fails) in
            [(false, false), (false, true), (true, false), (true, true)]
        {
            let mut attempted = Vec::new();
            let (capture, recovery, warning) = register_shortcuts(Binding::default(), |key| {
                attempted.push(key);
                if (key == Binding::default().0 && capture_fails)
                    || (key == Binding::unlock_all().0 && recovery_fails)
                {
                    Err("generated conflict".into())
                } else {
                    Ok(())
                }
            });
            assert_eq!(attempted, [Binding::default().0, Binding::unlock_all().0]);
            assert_eq!(capture.is_none(), capture_fails);
            assert_eq!(recovery.is_none(), recovery_fails);
            assert_eq!(warning.is_some(), capture_fails || recovery_fails);
            if capture_fails && recovery_fails {
                let Message::Combined(errors) = warning.expect("both failures") else {
                    panic!("typed failures")
                };
                assert_eq!(errors.len(), 2);
            }
        }
    }
}

pub fn install_fonts(ctx: &egui::Context) {
    let candidates = [
        "/System/Library/Fonts/STHeiti Light.ttc",
        "/System/Library/Fonts/PingFang.ttc",
        "C:\\Windows\\Fonts\\msyh.ttc",
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/truetype/wqy/wqy-microhei.ttc",
        "/usr/share/fonts/wenquanyi/wqy-microhei/wqy-microhei.ttc",
    ];
    for path in candidates {
        if let Ok(bytes) = std::fs::read(path) {
            let mut fonts = egui::FontDefinitions::default();
            fonts.font_data.insert(
                "system-cjk".into(),
                Arc::new(egui::FontData::from_owned(bytes)),
            );
            if let Some(family) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
                family.insert(0, "system-cjk".into());
            }
            ctx.set_fonts(fonts);
            break;
        }
    }
}

#[cfg(test)]
mod selection_grid_tests {
    #[cfg(target_os = "macos")]
    #[test]
    fn work_areas_match_origins_in_input_order_and_preserve_unavailable_fallbacks() {
        use eframe::egui::{Rect, pos2, vec2};
        let left = Rect::from_min_size(pos2(-1280.0, 56.0), vec2(1280.0, 1024.0));
        let above = Rect::from_min_size(pos2(0.0, -900.0), vec2(1600.0, 900.0));
        let unmatched = Rect::from_min_size(pos2(3000.0, 0.0), vec2(100.0, 100.0));
        let usable_left = Rect::from_min_max(left.min + vec2(0.0, 24.0), left.max);
        let usable_above = Rect::from_min_max(above.min, above.max - vec2(0.0, 40.0));
        let areas = [(left, usable_left), (above, usable_above)];
        assert_eq!(
            super::matching_work_areas(
                vec![above.translate(vec2(0.5, 0.0)), unmatched, left],
                &areas
            ),
            vec![usable_above, unmatched, usable_left]
        );
        assert_eq!(super::matching_work_areas(vec![left], &[]), vec![left]);
        let outside_tolerance = left.translate(vec2(1.0, 0.0));
        assert_eq!(
            super::matching_work_areas(vec![outside_tolerance], &areas),
            vec![outside_tolerance]
        );
    }

    #[test]
    fn source_density_selects_native_steps_without_native_services() {
        let mut snapshot = snip2pin::capture::demo();
        for (size, mac_step) in [
            ([480.0, 300.0], [2, 2]),
            ([480.0, 200.0], [2, 3]),
            ([680.0, 420.0], [1, 1]),
            ([0.0, f32::NAN], [1, 1]),
        ] {
            snapshot.display.size = size;
            let expected = if cfg!(target_os = "macos") {
                mac_step
            } else {
                [1, 1]
            };
            assert_eq!(super::selection_pixel_step(&snapshot), expected);
        }
    }
}

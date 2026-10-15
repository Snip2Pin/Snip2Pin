use std::sync::{
    Arc,
    atomic::{AtomicU32, Ordering},
};

use eframe::egui;
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState, hotkey::HotKey};
use snip2pin::settings::Binding;

#[path = "platform/pin_menu.rs"]
pub mod pin_menu;

#[cfg(any(target_os = "windows", target_os = "linux"))]
#[path = "platform/work_area.rs"]
mod work_area;

#[cfg(target_os = "macos")]
#[path = "platform/macos.rs"]
mod macos;

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

pub fn open_capture_settings() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    return macos_access::open_settings();
    #[cfg(not(target_os = "macos"))]
    Err("当前平台没有统一的屏幕录制授权设置入口。".into())
}

/// Prepare a user-requested relaunch; the caller runs it after the app has
/// dropped all native windows and global shortcuts. No waiting or polling.
pub fn relaunch_command() -> Result<std::process::Command, String> {
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

/// Read usable display bounds on the UI thread; never show or activate windows.
pub fn pin_work_area(display: &snip2pin::capture::Display) -> egui::Rect {
    let bounds =
        egui::Rect::from_min_size(display.position().into(), display.logical_size().into());
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    if let Some(area) = work_area::read(display) {
        return area;
    }
    #[cfg(target_os = "macos")]
    if let Some(thread) = objc2::MainThreadMarker::new() {
        let screens = objc2_app_kit::NSScreen::screens(thread);
        if let Some(primary) = screens.iter().next() {
            let frame = primary.frame();
            let desktop_top = frame.origin.y + frame.size.height;
            for screen in screens.iter() {
                let frame = screen.frame();
                let origin = egui::pos2(
                    frame.origin.x as f32,
                    (desktop_top - frame.origin.y - frame.size.height) as f32,
                );
                if origin.distance(bounds.min) < DISPLAY_ORIGIN_TOLERANCE {
                    let visible = screen.visibleFrame();
                    return egui::Rect::from_min_size(
                        egui::pos2(
                            visible.origin.x as f32,
                            (desktop_top - visible.origin.y - visible.size.height) as f32,
                        ),
                        egui::vec2(visible.size.width as f32, visible.size.height as f32),
                    );
                }
            }
        }
    }
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
) -> Result<(), String> {
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
pub fn prepare_floating_windows() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    if let Some(main_thread) = objc2::MainThreadMarker::new() {
        use objc2_app_kit::{NSApplication, NSWindowCollectionBehavior};
        let application = NSApplication::sharedApplication(main_thread);
        for window in application.windows() {
            let title = window.title().to_string();
            let pin_or_menu = title.starts_with("Snip2Pin · 贴图") || title == "Snip2Pin · 操作";
            if title == "Snip2Pin · 框选" || pin_or_menu {
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
                if title == "Snip2Pin · 框选" {
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

pub fn finish_capture(ctx: &egui::Context) {
    #[cfg(target_os = "macos")]
    {
        let _ = ctx;
        if let Some(main_thread) = objc2::MainThreadMarker::new() {
            objc2_app_kit::NSCursor::arrowCursor().set();
            let application = objc2_app_kit::NSApplication::sharedApplication(main_thread);
            for window in application.windows() {
                if window.title().to_string() == "Snip2Pin · 框选" && window.isKeyWindow() {
                    window.resignKeyWindow();
                }
            }
            application.deactivate();
        }
    }
    #[cfg(not(target_os = "macos"))]
    ctx.send_viewport_cmd_to(egui::ViewportId::ROOT, egui::ViewportCommand::Focus);
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
    key: HotKey,
    active_id: Arc<AtomicU32>,
}

pub fn global_shortcuts_supported() -> bool {
    !(cfg!(target_os = "linux") && std::env::var_os("WAYLAND_DISPLAY").is_some())
}

impl Shortcut {
    pub fn register(
        binding: Binding,
        action: impl Fn() + Send + Sync + 'static,
    ) -> Result<Self, String> {
        if !global_shortcuts_supported() {
            return Err("Wayland 下请从应用窗口截图，或在桌面设置里配置启动快捷键。".into());
        }
        let manager = GlobalHotKeyManager::new().map_err(|error| error.to_string())?;
        let key = binding.0;
        manager.register(key).map_err(|error| error.to_string())?;
        let active_id = Arc::new(AtomicU32::new(key.id()));
        let handler_id = active_id.clone();
        GlobalHotKeyEvent::set_event_handler(Some(move |event: GlobalHotKeyEvent| {
            if event.id == handler_id.load(Ordering::Acquire) && event.state == HotKeyState::Pressed
            {
                action();
            }
        }));
        Ok(Self {
            manager,
            key,
            active_id,
        })
    }

    pub fn replace(&mut self, binding: Binding) -> Result<(), String> {
        let key = binding.0;
        if key == self.key {
            return Ok(());
        }
        // Register first so a conflict leaves the existing shortcut working.
        self.manager
            .register(key)
            .map_err(|error| format!("快捷键不可用，请换一个组合：{error}"))?;
        if let Err(error) = self.manager.unregister(self.key) {
            let _ = self.manager.unregister(key);
            return Err(format!("更新快捷键失败：{error}"));
        }
        self.key = key;
        self.active_id.store(key.id(), Ordering::Release);
        Ok(())
    }
}

impl Drop for Shortcut {
    fn drop(&mut self) {
        let _ = self.manager.unregister(self.key);
    }
}

pub fn install_fonts(ctx: &egui::Context) {
    let candidates = [
        "/System/Library/Fonts/STHeiti Light.ttc",
        "/System/Library/Fonts/PingFang.ttc",
        "C:\\Windows\\Fonts\\msyh.ttc",
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
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

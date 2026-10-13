use std::sync::{
    Arc,
    atomic::{AtomicU32, Ordering},
};

use eframe::egui;
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState, hotkey::HotKey};
use snipsprig::settings::Binding;

/// Selection windows can cover the menu bar and fullscreen applications without
/// asking macOS to create a Space. All calls stay on the verified main thread.
pub fn prepare_floating_windows() {
    #[cfg(target_os = "macos")]
    if let Some(main_thread) = objc2::MainThreadMarker::new() {
        use objc2_app_kit::{NSApplication, NSWindowCollectionBehavior};
        let application = NSApplication::sharedApplication(main_thread);
        for window in application.windows() {
            let title = window.title().to_string();
            if title == "SnipSprig · 框选" || title == "SnipSprig · 贴图" {
                window.setCollectionBehavior(
                    NSWindowCollectionBehavior::CanJoinAllSpaces
                        | NSWindowCollectionBehavior::Stationary
                        | NSWindowCollectionBehavior::IgnoresCycle
                        | NSWindowCollectionBehavior::FullScreenAuxiliary,
                );
                if title == "SnipSprig · 框选" {
                    // The demo is a regular small test window; it need not
                    // cover system UI or block other apps during development.
                    let level = if std::env::args().any(|argument| argument == "--demo") {
                        0
                    } else {
                        (objc2_core_graphics::kCGMainMenuWindowLevel + 1) as isize
                    };
                    window.setLevel(level);
                }
            }
        }
    }
}

pub fn finish_capture(ctx: &egui::Context) {
    #[cfg(target_os = "macos")]
    {
        let _ = ctx;
        if let Some(main_thread) = objc2::MainThreadMarker::new() {
            let application = objc2_app_kit::NSApplication::sharedApplication(main_thread);
            for window in application.windows() {
                if window.title().to_string() == "SnipSprig · 框选" && window.isKeyWindow() {
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

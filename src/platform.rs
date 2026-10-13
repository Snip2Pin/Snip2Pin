use std::sync::Arc;

use eframe::egui;
use global_hotkey::{
    GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState,
    hotkey::{Code, HotKey, Modifiers},
};

pub const SHORTCUT: &str = if cfg!(target_os = "macos") {
    "Command + Shift + 2"
} else {
    "Ctrl + Shift + 2"
};

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
}

impl Shortcut {
    pub fn register(action: impl Fn() + Send + Sync + 'static) -> Result<Self, String> {
        if cfg!(target_os = "linux") && std::env::var_os("WAYLAND_DISPLAY").is_some() {
            return Err("Wayland 下请从应用窗口截图，或在桌面设置里配置启动快捷键。".into());
        }
        let manager = GlobalHotKeyManager::new().map_err(|error| error.to_string())?;
        let modifier = if cfg!(target_os = "macos") {
            Modifiers::SUPER
        } else {
            Modifiers::CONTROL
        };
        let key = HotKey::new(Some(modifier | Modifiers::SHIFT), Code::Digit2);
        manager.register(key).map_err(|error| error.to_string())?;
        GlobalHotKeyEvent::set_event_handler(Some(move |event: GlobalHotKeyEvent| {
            if event.id == key.id() && event.state == HotKeyState::Pressed {
                action();
            }
        }));
        Ok(Self { manager, key })
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

/// The same small card icon is used in windows and packaged app assets.
pub fn icon(size: u32) -> image::RgbaImage {
    image::RgbaImage::from_fn(size, size, |x, y| {
        let x = x as f32 / size as f32;
        let y = y as f32 / size as f32;
        let inset = (x - 0.5).abs().max((y - 0.5).abs());
        let corner =
            ((x - 0.5).abs() - 0.28).max(0.0).powi(2) + ((y - 0.5).abs() - 0.28).max(0.0).powi(2);
        if inset > 0.44 || corner > 0.16_f32.powi(2) {
            image::Rgba([0, 0, 0, 0])
        } else if (0.23..0.77).contains(&x) && (0.35..0.73).contains(&y) {
            image::Rgba([240, 245, 239, 255])
        } else if (0.45..0.55).contains(&x) && (0.23..0.46).contains(&y) {
            image::Rgba([242, 213, 137, 255])
        } else {
            image::Rgba([48, (122.0 - y * 24.0) as u8, 91, 255])
        }
    })
}

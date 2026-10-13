use image::RgbaImage;
use xcap::Monitor;

use crate::geometry::PixelRect;

#[derive(Clone, Debug)]
pub struct Display {
    pub id: u32,
    /// Desktop origin in native points on macOS, physical pixels elsewhere.
    pub origin: [f32; 2],
    pub size: [f32; 2],
    pub scale: f32,
}

pub struct Snapshot {
    pub display: Display,
    pub pixels: RgbaImage,
}

impl Display {
    pub fn position(&self) -> [f32; 2] {
        self.origin.map(|value| {
            if cfg!(target_os = "macos") {
                value
            } else {
                value / self.scale
            }
        })
    }

    pub fn logical_size(&self) -> [f32; 2] {
        self.size.map(|value| {
            if cfg!(target_os = "macos") {
                value
            } else {
                value / self.scale
            }
        })
    }
}

fn describe(monitor: &Monitor) -> Result<Display, String> {
    let result = || -> xcap::XCapResult<Display> {
        let scale = monitor.scale_factor()?;
        Ok(Display {
            id: monitor.id()?,
            origin: [monitor.x()? as f32, monitor.y()? as f32],
            size: [monitor.width()? as f32, monitor.height()? as f32],
            scale: if scale.is_finite() && scale > 0.0 {
                scale
            } else {
                1.0
            },
        })
    };
    result().map_err(|error| error.to_string())
}

pub fn displays() -> Result<Vec<Display>, String> {
    let monitors = Monitor::all().map_err(|error| error.to_string())?;
    monitors
        .into_iter()
        .map(|monitor| describe(&monitor))
        .collect()
}

pub fn desktop() -> Result<Vec<Snapshot>, String> {
    #[cfg(target_os = "macos")]
    if !objc2_core_graphics::CGPreflightScreenCaptureAccess()
        && !objc2_core_graphics::CGRequestScreenCaptureAccess()
    {
        return Err("需要屏幕录制权限。请在系统设置的「隐私与安全性 → 屏幕与系统音频录制」中允许 Snip2Pin，再重新打开应用。".into());
    }
    let monitors = Monitor::all().map_err(|error| error.to_string())?;
    if monitors.is_empty() {
        return Err("没有可用屏幕。".into());
    }
    monitors
        .into_iter()
        .map(|monitor| {
            let display = describe(&monitor)?;
            let pixels = monitor.capture_image().map_err(|error| {
                format!("截图失败：{error}。macOS 请检查屏幕录制权限；Linux 请检查桌面截图权限。")
            })?;
            if pixels.width() == 0 || pixels.height() == 0 {
                return Err("系统没有返回有效的截图。".into());
            }
            Ok(Snapshot { display, pixels })
        })
        .collect()
}

pub fn crop(image: &RgbaImage, rect: PixelRect) -> Option<RgbaImage> {
    let right = rect.x.checked_add(rect.width)?;
    let bottom = rect.y.checked_add(rect.height)?;
    if rect.width == 0 || rect.height == 0 || right > image.width() || bottom > image.height() {
        return None;
    }
    Some(image::imageops::crop_imm(image, rect.x, rect.y, rect.width, rect.height).to_image())
}

/// Generated pixels for development/CI: this never reads the desktop.
pub fn demo() -> Snapshot {
    Snapshot {
        display: Display {
            id: 0,
            origin: [80.0, 100.0],
            size: [680.0, 420.0],
            scale: 1.0,
        },
        pixels: RgbaImage::from_fn(960, 600, |x, y| {
            if (x / 80 + y / 80) % 2 == 0 {
                image::Rgba([70, 132, 110, 255])
            } else {
                image::Rgba([225, 238, 228, 255])
            }
        }),
    }
}

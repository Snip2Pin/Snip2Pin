use image::RgbaImage;
use xcap::Monitor;

use crate::geometry::PixelRect;

#[derive(Clone, Debug)]
pub struct Display {
    pub id: u32,
    pub name: String,
    pub primary: bool,
}

pub struct Snapshot {
    pub pixels: RgbaImage,
    pub scale: f32,
}

pub fn displays() -> Result<Vec<Display>, String> {
    let monitors = Monitor::all().map_err(|error| error.to_string())?;
    monitors
        .into_iter()
        .map(|monitor| {
            Ok(Display {
                id: monitor.id().map_err(|error| error.to_string())?,
                name: monitor.friendly_name().unwrap_or_else(|_| "屏幕".into()),
                primary: monitor.is_primary().unwrap_or(false),
            })
        })
        .collect()
}

pub fn monitor(id: u32) -> Result<Snapshot, String> {
    #[cfg(target_os = "macos")]
    if !objc2_core_graphics::CGPreflightScreenCaptureAccess()
        && !objc2_core_graphics::CGRequestScreenCaptureAccess()
    {
        return Err("需要屏幕录制权限。请在系统设置的「隐私与安全性 → 屏幕与系统音频录制」中允许 SnipSprig，再重新打开应用。".into());
    }
    let monitor = Monitor::all()
        .map_err(|error| error.to_string())?
        .into_iter()
        .find(|monitor| monitor.id().ok() == Some(id))
        .ok_or_else(|| "找不到所选屏幕，请重新打开应用。".to_owned())?;
    let pixels = monitor.capture_image().map_err(|error| {
        format!("截图失败：{error}。macOS 请检查屏幕录制权限；Linux 请检查桌面截图权限。")
    })?;
    if pixels.width() == 0 || pixels.height() == 0 {
        return Err("系统没有返回有效的截图。".into());
    }
    Ok(Snapshot {
        pixels,
        scale: monitor.scale_factor().unwrap_or(1.0),
    })
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
        pixels: RgbaImage::from_fn(960, 600, |x, y| {
            if (x / 80 + y / 80) % 2 == 0 {
                image::Rgba([70, 132, 110, 255])
            } else {
                image::Rgba([225, 238, 228, 255])
            }
        }),
        scale: 1.0,
    }
}

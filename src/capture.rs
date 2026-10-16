use eframe::egui::ColorImage;
use image::RgbaImage;
use std::sync::Arc;
#[cfg(not(target_arch = "wasm32"))]
use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(not(target_arch = "wasm32"))]
use xcap::Monitor;

use crate::geometry::PixelRect;
#[cfg(not(target_arch = "wasm32"))]
use crate::i18n::{Message, Text};

#[cfg(not(target_arch = "wasm32"))]
mod worker;
#[cfg(not(target_arch = "wasm32"))]
pub use worker::Worker;

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
    pub pixels: Arc<ColorImage>,
}

impl Snapshot {
    /// Convert on the capture worker. Selection and GPU upload share this allocation.
    pub fn from_rgba(display: Display, pixels: RgbaImage) -> Self {
        let size = [pixels.width() as usize, pixels.height() as usize];
        Self {
            display,
            pixels: Arc::new(ColorImage::from_rgba_unmultiplied(size, pixels.as_raw())),
        }
    }
    pub fn pixel_size(&self) -> [u32; 2] {
        self.pixels.size.map(|length| length as u32)
    }
}

impl Display {
    pub fn contains(&self, point: [f32; 2]) -> bool {
        (0..2).all(|axis| {
            point[axis] >= self.origin[axis] && point[axis] < self.origin[axis] + self.size[axis]
        })
    }

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

#[cfg(not(target_arch = "wasm32"))]
fn describe(monitor: &Monitor) -> Result<Display, Message> {
    let result = || -> xcap::XCapResult<Display> {
        Ok(from_backend(
            monitor.id()?,
            [monitor.x()? as f32, monitor.y()? as f32],
            [monitor.width()? as f32, monitor.height()? as f32],
            monitor.scale_factor()?,
        ))
    };
    result().map_err(|error| error.to_string().into())
}

#[cfg(not(target_arch = "wasm32"))]
fn from_backend(id: u32, origin: [f32; 2], size: [f32; 2], scale: f32) -> Display {
    let scale = if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        1.0
    };
    Display {
        id,
        origin: origin.map(|value| value * backend_units(scale)),
        size: size.map(|value| value * backend_units(scale)),
        scale,
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn displays() -> Result<Vec<Display>, Message> {
    let monitors = Monitor::all().map_err(|error| error.to_string())?;
    monitors
        .into_iter()
        .map(|monitor| describe(&monitor))
        .collect()
}

#[cfg(not(target_arch = "wasm32"))]
pub fn desktop(cancelled: &AtomicBool) -> Result<Vec<Snapshot>, Message> {
    #[cfg(target_os = "macos")]
    if !objc2_core_graphics::CGPreflightScreenCaptureAccess() {
        // The UI owns authorization. A worker must never present a permission
        // dialog or capture while access is absent/revoked.
        return Err(Text::NoCaptureAccess.into());
    }
    let monitors = Monitor::all().map_err(|error| error.to_string())?;
    if monitors.is_empty() {
        return Err(Text::NoDisplays.into());
    }
    let displays = monitors
        .iter()
        .map(describe)
        .collect::<Result<Vec<_>, _>>()?;
    if let ([monitor], [display]) = (monitors.as_slice(), displays.as_slice()) {
        return capture_monitor(monitor, display.clone(), cancelled).map(|snapshot| vec![snapshot]);
    }
    // Portal capture on Linux is coordinated by the desktop; do not request
    // concurrent permission dialogs. Windows/macOS capture independent displays.
    collect_displays(
        displays,
        !cfg!(target_os = "linux"),
        |display| {
            let monitor = Monitor::all()
                .map_err(|error| error.to_string())?
                .into_iter()
                .find(|monitor| monitor.id().ok() == Some(display.id))
                .ok_or(Text::DisplayDisconnected)?;
            capture_monitor(&monitor, display, cancelled)
        },
        cancelled,
    )
}

#[cfg(not(target_arch = "wasm32"))]
fn capture_monitor(
    monitor: &Monitor,
    display: Display,
    cancelled: &AtomicBool,
) -> Result<Snapshot, Message> {
    if cancelled.load(Ordering::Acquire) {
        return Err(Text::CaptureCanceled.into());
    }
    let pixels = monitor
        .capture_image()
        .map_err(|error| Text::CaptureFailed.with(error))?;
    if pixels.width() == 0 || pixels.height() == 0 {
        return Err(Text::EmptyCapture.into());
    }
    Ok(Snapshot::from_rgba(display, pixels))
}

/// xcap's X11 descriptor divides XRandR coordinates by Xft scale, whereas our
/// non-macOS native domain (including the X11 pointer) uses physical pixels.
#[cfg(not(target_arch = "wasm32"))]
fn backend_units(scale: f32) -> f32 {
    if cfg!(target_os = "linux") {
        scale
    } else {
        1.0
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn collect_displays(
    displays: Vec<Display>,
    parallel: bool,
    capture: impl Fn(Display) -> Result<Snapshot, Message> + Sync,
    cancelled: &AtomicBool,
) -> Result<Vec<Snapshot>, Message> {
    let capture = |display| {
        if cancelled.load(Ordering::Acquire) {
            Err(Text::CaptureCanceled.into())
        } else {
            capture(display)
        }
    };
    if !parallel || displays.len() < 2 {
        return displays.into_iter().map(capture).collect();
    }
    std::thread::scope(|scope| {
        let jobs: Vec<_> = displays
            .into_iter()
            .map(|display| {
                let capture = &capture;
                scope.spawn(move || capture(display))
            })
            .collect();
        jobs.into_iter()
            .map(|job| {
                job.join()
                    .map_err(|_| Message::from(Text::CaptureCrashed))?
            })
            .collect()
    })
}

pub fn crop(image: &ColorImage, rect: PixelRect) -> Option<ColorImage> {
    let right = rect.x.checked_add(rect.width)?;
    let bottom = rect.y.checked_add(rect.height)?;
    if rect.width == 0
        || rect.height == 0
        || right as usize > image.size[0]
        || bottom as usize > image.size[1]
    {
        return None;
    }
    let mut pixels = Vec::with_capacity(rect.width as usize * rect.height as usize);
    for y in rect.y..bottom {
        let start = y as usize * image.size[0] + rect.x as usize;
        pixels.extend_from_slice(&image.pixels[start..start + rect.width as usize]);
    }
    Some(ColorImage::new(
        [rect.width as usize, rect.height as usize],
        pixels,
    ))
}

/// Generated pixels for development/CI: this never reads the desktop.
pub fn demo() -> Snapshot {
    const DISPLAY: Display = Display {
        id: 0,
        origin: [80.0, 100.0],
        size: [680.0, 420.0],
        scale: 1.0,
    };
    const PIXELS: [u32; 2] = [960, 600];
    const CELL_SIZE: u32 = 80;
    const COLORS: [eframe::egui::Color32; 2] = [
        eframe::egui::Color32::from_rgb(70, 132, 110),
        eframe::egui::Color32::from_rgb(225, 238, 228),
    ];
    Snapshot {
        display: DISPLAY,
        pixels: Arc::new(ColorImage::new(
            PIXELS.map(|size| size as usize),
            (0..PIXELS[1])
                .flat_map(|y| {
                    (0..PIXELS[0]).map(move |x| {
                        COLORS[((x / CELL_SIZE + y / CELL_SIZE) % COLORS.len() as u32) as usize]
                    })
                })
                .collect(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, mpsc};
    use std::time::Duration;

    #[test]
    fn backend_descriptors_use_one_native_domain_and_validate_scale_first() {
        let display = from_backend(1, [-1920.0, 40.0], [1920.0, 1080.0], 2.0);
        let units = if cfg!(target_os = "linux") { 2.0 } else { 1.0 };
        assert_eq!(display.origin, [-1920.0 * units, 40.0 * units]);
        assert_eq!(display.size, [1920.0 * units, 1080.0 * units]);
        if cfg!(target_os = "linux") {
            assert_eq!(display.position(), [-1920.0, 40.0]);
            assert_eq!(display.logical_size(), [1920.0, 1080.0]);
            assert!(display.contains([-3800.0, 100.0]));
        }
        for scale in [0.0, -1.0, f32::NAN, f32::INFINITY] {
            let display = from_backend(1, [-100.0, 20.0], [200.0, 100.0], scale);
            assert_eq!(display.scale, 1.0);
            assert_eq!(display.origin, [-100.0, 20.0]);
            assert_eq!(display.size, [200.0, 100.0]);
        }
    }

    #[test]
    fn independent_displays_start_together_and_keep_topology_order() {
        let (entered, started) = mpsc::channel();
        let (release, wait) = mpsc::channel();
        let capture = std::thread::spawn(move || {
            let wait = Mutex::new(wait);
            let first = demo().display;
            let mut second = first.clone();
            second.id = 1;
            collect_displays(
                vec![first, second],
                true,
                |display| {
                    entered.send(display.id).expect("entered");
                    wait.lock().expect("wait").recv().expect("release");
                    Ok(Snapshot {
                        display,
                        pixels: Arc::new(ColorImage::filled([1, 1], eframe::egui::Color32::BLACK)),
                    })
                },
                &AtomicBool::new(false),
            )
        });
        let first = started.recv_timeout(Duration::from_secs(5));
        let second = started.recv_timeout(Duration::from_secs(5));
        release.send(()).expect("release");
        release.send(()).expect("release");
        assert!(
            first.is_ok() && second.is_ok(),
            "both backends must enter before either finishes"
        );
        let frames = capture.join().expect("capture thread").expect("frames");
        assert_eq!(
            frames
                .iter()
                .map(|frame| frame.display.id)
                .collect::<Vec<_>>(),
            vec![0, 1]
        );
    }

    #[test]
    fn cancelling_a_serial_backend_stops_before_the_next_display() {
        let cancelled = AtomicBool::new(false);
        let calls = std::sync::atomic::AtomicUsize::new(0);
        let display = demo().display;
        let result = collect_displays(
            vec![display.clone(), display],
            false,
            |display| {
                calls.fetch_add(1, Ordering::Relaxed);
                cancelled.store(true, Ordering::Release);
                Ok(Snapshot {
                    display,
                    pixels: Arc::new(ColorImage::filled([1, 1], eframe::egui::Color32::BLACK)),
                })
            },
            &cancelled,
        );
        assert!(result.is_err());
        assert_eq!(calls.load(Ordering::Relaxed), 1);
    }
}

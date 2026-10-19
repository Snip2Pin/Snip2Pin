//! One front-to-back metadata snapshot; pointer movement never queries the OS.
use super::Display;
use crate::geometry::{self, PixelRect};

#[derive(Clone, Copy, Debug)]
pub struct WindowBounds {
    /// Same desktop coordinate domain as Display: macOS points, pixels elsewhere.
    pub origin: [f64; 2],
    pub size: [f64; 2],
}

impl WindowBounds {
    pub(super) fn region(
        &self,
        display: &Display,
        pixels: [u32; 2],
        step: [u32; 2],
    ) -> Option<PixelRect> {
        if !self.origin.into_iter().chain(self.size).all(f64::is_finite)
            || self.size.into_iter().any(|side| side <= 0.0)
        {
            return None;
        }
        let start = std::array::from_fn(|axis| self.origin[axis] - f64::from(display.origin[axis]));
        let end = std::array::from_fn(|axis| start[axis] + self.size[axis]);
        geometry::selection_with_step(start, end, display.size.map(f64::from), pixels, step)
    }
}

#[cfg(all(not(target_arch = "wasm32"), target_os = "macos"))]
#[path = "windows/macos.rs"]
mod macos;

#[cfg(all(not(target_arch = "wasm32"), target_os = "macos"))]
pub(super) use macos::visible;

#[cfg(all(not(target_arch = "wasm32"), target_os = "linux"))]
#[path = "windows/linux.rs"]
mod linux;

#[cfg(all(not(target_arch = "wasm32"), target_os = "linux"))]
pub(super) use linux::visible;

#[cfg(all(not(target_arch = "wasm32"), target_os = "windows"))]
#[path = "windows/win32.rs"]
mod win32;

#[cfg(all(not(target_arch = "wasm32"), target_os = "windows"))]
pub(super) use win32::visible;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_regions_clip_in_native_coordinates_and_use_the_same_source_grid_as_dragging() {
        let display = Display {
            id: 1,
            origin: [-1000.0, 200.0],
            size: [1000.0, 800.0],
            scale: 2.0,
        };
        let window = WindowBounds {
            origin: [-1050.5, 250.5],
            size: [250.0, 100.0],
        };
        assert_eq!(
            window.region(&display, [2000, 1600], [2, 2]),
            Some(PixelRect {
                x: 0,
                y: 100,
                width: 400,
                height: 202,
            })
        );
        for window in [
            WindowBounds {
                origin: [0.0, 200.0],
                size: [100.0, 100.0],
            },
            WindowBounds {
                origin: [-900.0, 200.0],
                size: [0.0, 100.0],
            },
            WindowBounds {
                origin: [f64::NAN, 200.0],
                size: [100.0, 100.0],
            },
        ] {
            assert_eq!(window.region(&display, [2000, 1600], [2, 2]), None);
        }
    }
}

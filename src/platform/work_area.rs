//! Read the desktop's usable rectangle only when a capture creates pins.
use eframe::egui::{Pos2, Rect, Vec2};
use snip2pin::capture::Display;

fn on_display(display: &Display, area: Rect) -> Option<Rect> {
    let physical = Rect::from_min_size(display.origin.into(), display.size.into());
    let usable = physical.intersect(area);
    usable
        .is_positive()
        .then(|| Rect::from_min_max(usable.min / display.scale, usable.max / display.scale))
}

#[cfg(target_os = "windows")]
#[allow(unsafe_code)]
pub fn read(display: &Display) -> Option<Rect> {
    use windows_sys::Win32::{
        Foundation::{POINT, RECT},
        Graphics::Gdi::{GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromPoint},
    };
    let center = Pos2::from(display.origin) + Vec2::from(display.size) / 2.0;
    let empty = RECT {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    let mut info = MONITORINFO {
        cbSize: size_of::<MONITORINFO>() as u32,
        rcMonitor: empty,
        rcWork: empty,
        dwFlags: 0,
    };
    // SAFETY: POINT is passed by value. info is writable, correctly sized and
    // lives throughout the call; no monitor handle or borrowed pointer escapes.
    let valid = unsafe {
        let monitor = MonitorFromPoint(
            POINT {
                x: center.x as i32,
                y: center.y as i32,
            },
            MONITOR_DEFAULTTONEAREST,
        );
        !monitor.is_null() && GetMonitorInfoW(monitor, &mut info) != 0
    };
    valid
        .then(|| {
            on_display(
                display,
                Rect::from_min_max(
                    Pos2::new(info.rcWork.left as f32, info.rcWork.top as f32),
                    Pos2::new(info.rcWork.right as f32, info.rcWork.bottom as f32),
                ),
            )
        })
        .flatten()
}

#[cfg(target_os = "linux")]
pub fn read(display: &Display) -> Option<Rect> {
    use x11rb::{
        connection::Connection,
        protocol::xproto::{AtomEnum, ConnectionExt},
    };
    let (connection, screen) = x11rb::connect(None).ok()?;
    let root = connection.setup().roots.get(screen)?.root;
    let atom = |name: &[u8]| {
        connection
            .intern_atom(true, name)
            .ok()?
            .reply()
            .ok()
            .map(|reply| reply.atom)
    };
    let desktop = connection
        .get_property(
            false,
            root,
            atom(b"_NET_CURRENT_DESKTOP")?,
            AtomEnum::CARDINAL,
            0,
            1,
        )
        .ok()?
        .reply()
        .ok()?
        .value32()?
        .next()?;
    let offset = desktop.checked_mul(4)?;
    let work = connection
        .get_property(
            false,
            root,
            atom(b"_NET_WORKAREA")?,
            AtomEnum::CARDINAL,
            offset,
            4,
        )
        .ok()?
        .reply()
        .ok()?;
    let mut values = work.value32()?;
    let origin = Pos2::new(values.next()? as i32 as f32, values.next()? as i32 as f32);
    let size = Vec2::new(values.next()? as f32, values.next()? as f32);
    on_display(display, Rect::from_min_size(origin, size))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usable_area_is_intersected_before_dpi_conversion() {
        let display = Display {
            id: 1,
            origin: [-3840.0, 0.0],
            size: [3840.0, 2160.0],
            scale: 2.0,
        };
        let area = Rect::from_min_max(Pos2::new(-3840.0, 60.0), Pos2::new(3840.0, 2080.0));
        assert_eq!(
            on_display(&display, area),
            Some(Rect::from_min_max(
                Pos2::new(-1920.0, 30.0),
                Pos2::new(0.0, 1040.0)
            ))
        );
        assert!(
            on_display(
                &display,
                Rect::from_min_size(Pos2::new(100.0, 0.0), Vec2::splat(100.0))
            )
            .is_none()
        );
    }
}

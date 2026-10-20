//! EWMH stacking and frame extents, using one connection per capture.
use std::sync::atomic::{AtomicBool, Ordering};

use x11rb::{
    connection::Connection,
    protocol::xproto::{AtomEnum, ConnectionExt, MapState},
};

use super::WindowBounds;

pub(in crate::capture) fn visible(cancelled: &AtomicBool) -> Vec<WindowBounds> {
    // An XWayland-only list cannot tell when a native Wayland window covers it.
    if std::env::var_os("WAYLAND_DISPLAY").is_some()
        || std::env::var("XDG_SESSION_TYPE").is_ok_and(|session| session == "wayland")
    {
        return Vec::new();
    }
    enumerate(cancelled).unwrap_or_default()
}

fn enumerate(cancelled: &AtomicBool) -> Option<Vec<WindowBounds>> {
    let (connection, screen) = x11rb::connect(None).ok()?;
    let root = connection.setup().roots[screen].root;
    let atom = |name: &[u8]| {
        connection
            .intern_atom(true, name)
            .ok()?
            .reply()
            .ok()
            .map(|reply| reply.atom)
    };
    let stacking = atom(b"_NET_CLIENT_LIST_STACKING")?;
    if stacking == 0 {
        return None;
    }
    let desktop = atom(b"_NET_WM_DESKTOP")?;
    let current_desktop = atom(b"_NET_CURRENT_DESKTOP")?;
    let extents = atom(b"_NET_FRAME_EXTENTS")?;
    let pid = atom(b"_NET_WM_PID")?;
    let name = atom(b"_NET_WM_NAME")?;
    let utf8 = atom(b"UTF8_STRING")?;
    let window_type = atom(b"_NET_WM_WINDOW_TYPE")?;
    let dock = atom(b"_NET_WM_WINDOW_TYPE_DOCK")?;
    let background = atom(b"_NET_WM_WINDOW_TYPE_DESKTOP")?;
    let property = |window, atom| -> Option<Vec<u32>> {
        if atom == 0 {
            return None;
        }
        connection
            .get_property(false, window, atom, AtomEnum::ANY, 0, u32::MAX)
            .ok()?
            .reply()
            .ok()?
            .value32()
            .map(Iterator::collect)
    };
    let current = property(root, current_desktop).and_then(|values| values.first().copied());
    let windows = property(root, stacking)?;
    Some(
        windows
            .into_iter()
            .rev()
            .take_while(|_| !cancelled.load(Ordering::Acquire))
            .filter_map(|window| {
                let attributes = connection.get_window_attributes(window).ok()?;
                let geometry = connection.get_geometry(window).ok()?;
                let position = connection.translate_coordinates(window, root, 0, 0).ok()?;
                if attributes.reply().ok()?.map_state != MapState::VIEWABLE {
                    return None;
                }
                if property(window, pid).and_then(|values| values.first().copied())
                    == Some(std::process::id())
                    && name != 0
                    && utf8 != 0
                {
                    let title = connection
                        .get_property(
                            false,
                            window,
                            name,
                            utf8,
                            0,
                            crate::SELECTION_WINDOW_TITLE.len().div_ceil(4) as u32,
                        )
                        .ok()?
                        .reply()
                        .ok()?;
                    if title.bytes_after == 0
                        && title.value == crate::SELECTION_WINDOW_TITLE.as_bytes()
                    {
                        return None;
                    }
                }
                if let (Some(current), Some(workspace)) = (
                    current,
                    property(window, desktop).and_then(|values| values.first().copied()),
                ) && workspace != current
                    && workspace != u32::MAX
                {
                    return None;
                }
                if property(window, window_type).is_some_and(|types| {
                    types
                        .iter()
                        .any(|kind| *kind == dock || *kind == background)
                }) {
                    return None;
                }
                let geometry = geometry.reply().ok()?;
                let position = position.reply().ok()?;
                let frame = property(window, extents)
                    .filter(|values| values.len() == 4)
                    .unwrap_or_else(|| vec![0; 4]);
                Some(WindowBounds {
                    origin: [
                        f64::from(position.dst_x) - f64::from(frame[0]),
                        f64::from(position.dst_y) - f64::from(frame[2]),
                    ],
                    size: [
                        f64::from(geometry.width) + f64::from(frame[0]) + f64::from(frame[1]),
                        f64::from(geometry.height) + f64::from(frame[2]) + f64::from(frame[3]),
                    ],
                })
            })
            .collect(),
    )
}

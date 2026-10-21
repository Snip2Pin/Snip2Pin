//! Apply anchored content bounds in one native window transaction.
use snip2pin::{i18n::Message, pin::Zoom};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Applied,
    SizeOnly,
}

/// Own the lazy native binding for one pin, releasing it with that pin.
pub struct PinZoom {
    id: u64,
    window: Option<Window>,
}

impl PinZoom {
    pub fn new(id: u64) -> Self {
        Self { id, window: None }
    }

    pub fn apply(
        &mut self,
        pixels: [usize; 2],
        scale: f32,
        commands: &[Zoom],
    ) -> Result<Outcome, Message> {
        if !commands
            .iter()
            .any(|command| matches!(command, Zoom::At { .. }))
        {
            return Ok(Outcome::SizeOnly);
        }
        if self.window.is_none() {
            self.window = Window::find(self.id)?;
        }
        if let Some(window) = &self.window {
            window.apply(pixels, scale, commands)?;
            Ok(Outcome::Applied)
        } else {
            // Native Wayland has no global positioning request.
            Ok(Outcome::SizeOnly)
        }
    }
}

#[cfg(target_os = "macos")]
struct Window(objc2::rc::Retained<objc2_app_kit::NSWindow>);

#[cfg(target_os = "macos")]
impl Window {
    pub fn find(id: u64) -> Result<Option<Self>, Message> {
        Ok(Some(Self(super::pin_window::find(id)?)))
    }
    pub fn apply(&self, pixels: [usize; 2], _scale: f32, commands: &[Zoom]) -> Result<(), Message> {
        apply_macos(&self.0, pixels, commands);
        Ok(())
    }
}

#[cfg(target_os = "macos")]
fn apply_macos(window: &objc2_app_kit::NSWindow, pixels: [usize; 2], commands: &[Zoom]) {
    let content = window.contentRectForFrameRect(window.frame());
    let Some(top) = super::macos::desktop_top() else {
        return;
    };
    // Match the backend's top-left coordinates using the real primary screen.
    let current = super::macos::backend_rect(content, top);
    if let Some(target) = snip2pin::pin::zoom_rect(
        current,
        pixels,
        window.backingScaleFactor() as f32,
        commands,
    ) {
        let content = super::macos::native_rect(target, top);
        window.setFrame_display(window.frameRectForContentRect(content), false);
    }
}

#[cfg(target_os = "windows")]
struct Window(windows_sys::Win32::Foundation::HWND);

#[cfg(target_os = "windows")]
impl Window {
    pub fn find(id: u64) -> Result<Option<Self>, Message> {
        Ok(Some(Self(super::pin_window::find(id)?)))
    }
    #[allow(unsafe_code)]
    pub fn apply(&self, pixels: [usize; 2], scale: f32, commands: &[Zoom]) -> Result<(), Message> {
        use eframe::egui::{Rect, pos2, vec2};
        use windows_sys::Win32::{
            Foundation::{POINT, RECT},
            Graphics::Gdi::ClientToScreen,
            UI::WindowsAndMessaging::{
                GetClientRect, GetWindowRect, SWP_NOACTIVATE, SWP_NOOWNERZORDER, SWP_NOZORDER,
                SetWindowPos,
            },
        };
        let window = self.0;
        let mut client = RECT::default();
        let mut frame = RECT::default();
        let mut origin = POINT::default();
        // SAFETY: find returns this process's live pin; all output structures are
        // writable and used synchronously on its UI thread.
        if unsafe {
            GetClientRect(window, &mut client) == 0
                || GetWindowRect(window, &mut frame) == 0
                || ClientToScreen(window, &mut origin) == 0
        } {
            return Err(std::io::Error::last_os_error().to_string().into());
        }
        let current = Rect::from_min_size(
            pos2(origin.x as f32, origin.y as f32) / scale,
            vec2(
                (client.right - client.left) as f32,
                (client.bottom - client.top) as f32,
            ) / scale,
        );
        let Some(target) = snip2pin::pin::zoom_rect(current, pixels, scale, commands) else {
            return Ok(());
        };
        let inset = vec2(
            (origin.x - frame.left) as f32,
            (origin.y - frame.top) as f32,
        );
        let border = vec2(
            (frame.right - frame.left - client.right + client.left) as f32,
            (frame.bottom - frame.top - client.bottom + client.top) as f32,
        );
        let position = target.min.to_vec2() * scale - inset;
        let size = target.size() * scale + border;
        if !position.is_finite() || !size.is_finite() || size.min_elem() <= 0.0 {
            return Ok(());
        }
        // SAFETY: Same live UI-thread window, one bounds update; preserve focus,
        // stacking and ownership. No intermediate move/resize or corrective retry.
        if unsafe {
            SetWindowPos(
                window,
                std::ptr::null_mut(),
                position.x.round() as i32,
                position.y.round() as i32,
                size.x.round() as i32,
                size.y.round() as i32,
                SWP_NOACTIVATE | SWP_NOZORDER | SWP_NOOWNERZORDER,
            )
        } == 0
        {
            return Err(std::io::Error::last_os_error().to_string().into());
        }
        Ok(())
    }
}

#[cfg(target_os = "linux")]
struct Window {
    connection: x11rb::rust_connection::RustConnection,
    root: u32,
    window: u32,
    extents: u32,
}

#[cfg(target_os = "linux")]
impl Window {
    pub fn find(id: u64) -> Result<Option<Self>, Message> {
        use x11rb::{connection::Connection, protocol::xproto::ConnectionExt};
        if std::env::var_os("DISPLAY").is_none() {
            return Ok(None);
        }
        let (connection, screen) = x11rb::connect(None).map_err(|error| error.to_string())?;
        let root = connection.setup().roots[screen].root;
        let window =
            super::pin_window::named_x11_windows(&connection, root, &super::pin_window::title(id))?
                .into_iter()
                .next()
                .ok_or(snip2pin::i18n::Text::PinNotFound)?;
        let extents = connection
            .intern_atom(true, b"_NET_FRAME_EXTENTS")
            .map_err(|error| error.to_string())?
            .reply()
            .map_err(|error| error.to_string())?
            .atom;
        Ok(Some(Self {
            connection,
            root,
            window,
            extents,
        }))
    }
    pub fn apply(&self, pixels: [usize; 2], scale: f32, commands: &[Zoom]) -> Result<(), Message> {
        use eframe::egui::{Rect, pos2, vec2};
        use x11rb::{
            connection::Connection,
            protocol::xproto::{AtomEnum, ConfigureWindowAux, ConnectionExt},
        };
        let connection = &self.connection;
        let root = self.root;
        let window = self.window;
        let geometry = connection
            .get_geometry(window)
            .map_err(|error| error.to_string())?
            .reply()
            .map_err(|error| error.to_string())?;
        let origin = connection
            .translate_coordinates(window, root, 0, 0)
            .map_err(|error| error.to_string())?
            .reply()
            .map_err(|error| error.to_string())?;
        let current = Rect::from_min_size(
            pos2(f32::from(origin.dst_x), f32::from(origin.dst_y)) / scale,
            vec2(f32::from(geometry.width), f32::from(geometry.height)) / scale,
        );
        let Some(target) = snip2pin::pin::zoom_rect(current, pixels, scale, commands) else {
            return Ok(());
        };
        let atom = self.extents;
        // Undecorated clients normally have zero extents; honor the WM if present.
        let extents = if atom != 0 {
            connection
                .get_property(false, window, atom, AtomEnum::CARDINAL, 0, 4)
                .map_err(|error| error.to_string())?
                .reply()
                .map_err(|error| error.to_string())?
                .value32()
                .map(|values| values.collect::<Vec<_>>())
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        let inset = if extents.len() == 4 {
            vec2(extents[0] as f32, extents[2] as f32)
        } else {
            eframe::egui::Vec2::ZERO
        };
        let position = target.min.to_vec2() * scale - inset;
        let size = target.size() * scale;
        connection
            .configure_window(
                window,
                &ConfigureWindowAux::new()
                    .x(position.x.round() as i32)
                    .y(position.y.round() as i32)
                    .width(size.x.round() as u32)
                    .height(size.y.round() as u32),
            )
            .map_err(|error| error.to_string())?
            .check()
            .map_err(|error| error.to_string())?;
        connection.flush().map_err(|error| error.to_string())?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn size_only_zoom_does_not_acquire_a_native_window() {
        use super::{Outcome, PinZoom, Zoom};

        let mut zoom = PinZoom::new(u64::MAX);
        assert_eq!(
            zoom.apply([240, 160], 2.0, &[Zoom::In, Zoom::Out, Zoom::Original])
                .expect("ordinary zoom needs no native binding"),
            Outcome::SizeOnly
        );
        assert!(zoom.window.is_none());
    }
}

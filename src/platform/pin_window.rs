//! Focus this process's pins while keeping the originating Home reachable.
use snip2pin::i18n::{Message, Text};

const TITLE_PREFIX: &str = "Snip2Pin · 贴图 ";

/// Event-triggered focus checks reuse one connection for a fallback menu's lifetime.
#[derive(Default)]
pub struct AppFocus {
    #[cfg(target_os = "linux")]
    x11: Option<(x11rb::rust_connection::RustConnection, u32)>,
}

impl AppFocus {
    /// No focus query is made until a window explicitly reports losing focus.
    /// Unsupported protocols, missing ownership data and query errors are unknown.
    pub fn current(&mut self) -> Option<bool> {
        #[cfg(target_os = "linux")]
        {
            use x11rb::protocol::xproto::{AtomEnum, ConnectionExt};
            std::env::var_os("DISPLAY")?;
            if self.x11.is_none() {
                let (connection, _) = x11rb::connect(None).ok()?;
                let pid = connection
                    .intern_atom(true, b"_NET_WM_PID")
                    .ok()?
                    .reply()
                    .ok()?
                    .atom;
                if pid == 0 {
                    return None;
                }
                self.x11 = Some((connection, pid));
            }
            let (connection, pid) = self.x11.as_ref()?;
            let focus = connection.get_input_focus().ok()?.reply().ok()?.focus;
            focus_owned_by(focus, std::process::id(), |window| {
                let property = connection
                    .get_property(false, window, *pid, AtomEnum::CARDINAL, 0, 1)
                    .ok()?
                    .reply()
                    .ok()?;
                let process = if property.format == 0 {
                    None
                } else if property.format == 32 && property.bytes_after == 0 {
                    Some(property.value32()?.next()?)
                } else {
                    return None;
                };
                let parent = if process.is_some() {
                    0
                } else {
                    connection.query_tree(window).ok()?.reply().ok()?.parent
                };
                Some((process, parent))
            })
        }
        #[cfg(not(target_os = "linux"))]
        None
    }
}

#[cfg(any(target_os = "linux", test))]
fn focus_owned_by(
    mut window: u32,
    process: u32,
    mut inspect: impl FnMut(u32) -> Option<(Option<u32>, u32)>,
) -> Option<bool> {
    // None and PointerRoot are not concrete focus owners. A transient gap in
    // focus transfer must not be mistaken for another application's window.
    while window > 1 {
        let (owner, parent) = inspect(window)?;
        if let Some(owner) = owner.filter(|owner| *owner != 0) {
            return Some(owner == process);
        }
        if parent == window {
            return None;
        }
        window = parent;
    }
    None
}

pub(crate) fn title(id: u64) -> String {
    format!("{TITLE_PREFIX}{id}")
}

#[cfg(target_os = "macos")]
pub(super) fn id_from_title(title: &str) -> Option<u64> {
    title.strip_prefix(TITLE_PREFIX)?.parse().ok()
}

#[cfg(target_os = "macos")]
pub(super) fn find(id: u64) -> Result<objc2::rc::Retained<objc2_app_kit::NSWindow>, Message> {
    let thread = objc2::MainThreadMarker::new().ok_or(Text::UiThread)?;
    let app = objc2_app_kit::NSApplication::sharedApplication(thread);
    let title = title(id);
    app.windows()
        .iter()
        .find(|window| window.title().to_string() == title)
        .ok_or_else(|| Text::PinNotFound.into())
}

/// Called once on the UI thread for an explicit list selection.
#[cfg(target_os = "macos")]
pub fn activate(id: u64) -> Result<(), Message> {
    let window = find(id)?;
    let thread = objc2::MainThreadMarker::new().ok_or(Text::UiThread)?;
    let app = objc2_app_kit::NSApplication::sharedApplication(thread);
    let key = app.keyWindow();
    window.makeKeyWindow();
    if let Some(key) = key
        && key.windowNumber() != window.windowNumber()
        && key.level() == window.level()
    {
        // Keep the list receiving the second click when its pin overlaps Home.
        window.orderWindow_relativeTo(
            objc2_app_kit::NSWindowOrderingMode::Below,
            key.windowNumber(),
        );
    } else {
        window.orderFront(None);
    }
    Ok(())
}

#[cfg(target_os = "windows")]
pub(super) fn find(id: u64) -> Result<windows_sys::Win32::Foundation::HWND, Message> {
    windows_named(&title(id))?
        .into_iter()
        .next()
        .ok_or_else(|| Text::PinNotFound.into())
}

#[cfg(target_os = "windows")]
#[allow(unsafe_code)]
fn windows_named(title: &str) -> Result<Vec<windows_sys::Win32::Foundation::HWND>, Message> {
    use windows_sys::{
        Win32::{
            Foundation::{HWND, LPARAM},
            UI::WindowsAndMessaging::{
                EnumWindows, GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId,
            },
        },
        core::BOOL,
    };

    struct Search {
        title: Vec<u16>,
        windows: Vec<HWND>,
    }

    unsafe extern "system" fn visit(window: HWND, data: LPARAM) -> BOOL {
        // SAFETY: EnumWindows synchronously borrows the Search supplied below.
        let search = unsafe { &mut *(data as *mut Search) };
        let mut process = 0;
        // SAFETY: The enumeration supplies a valid HWND and process is writable.
        unsafe { GetWindowThreadProcessId(window, &mut process) };
        if process != std::process::id() {
            return 1;
        }
        // SAFETY: The HWND comes directly from the active enumeration.
        let length = unsafe { GetWindowTextLengthW(window) };
        if length <= 0 || length as usize != search.title.len() {
            return 1;
        }
        let mut title = vec![0; length as usize + 1];
        // SAFETY: title includes space for the terminator and lives for this call.
        let written = unsafe { GetWindowTextW(window, title.as_mut_ptr(), title.len() as i32) };
        if written as usize == search.title.len() && title[..written as usize] == search.title {
            search.windows.push(window);
        }
        1
    }

    let mut search = Search {
        title: title.encode_utf16().collect(),
        windows: Vec::new(),
    };
    // SAFETY: Search stays at a stable address during synchronous enumeration.
    // Only this process's handles are returned, for immediate UI-thread use.
    unsafe { EnumWindows(Some(visit), (&mut search as *mut Search) as LPARAM) };
    Ok(search.windows)
}

#[cfg(target_os = "windows")]
#[allow(unsafe_code)]
pub(super) fn raise_selection() -> Result<(), Message> {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOOWNERZORDER, SWP_NOSIZE, SetWindowPos,
    };
    for window in windows_named(snip2pin::SELECTION_WINDOW_TITLE)? {
        // SAFETY: Only this process's live selection windows are returned, used
        // synchronously on their UI thread. Preserve geometry and focus.
        if unsafe {
            SetWindowPos(
                window,
                HWND_TOPMOST,
                0,
                0,
                0,
                0,
                SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE | SWP_NOOWNERZORDER,
            )
        } == 0
        {
            return Err(std::io::Error::last_os_error().to_string().into());
        }
    }
    Ok(())
}

#[cfg(target_os = "windows")]
#[allow(unsafe_code)]
pub fn activate(id: u64) -> Result<(), Message> {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetFocus, SetFocus};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GWL_EXSTYLE, GetForegroundWindow, GetWindowLongW, GetWindowThreadProcessId, HWND_TOPMOST,
        SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_SHOWWINDOW, SetWindowPos, WS_EX_TOPMOST,
    };
    let window = find(id)?;
    // SAFETY: These synchronous queries retain no pointers. Only this process's
    // foreground topmost window can become the stacking sibling of its pin.
    let sibling = unsafe {
        let foreground = GetForegroundWindow();
        let mut process = 0;
        if !foreground.is_null() && foreground != window {
            GetWindowThreadProcessId(foreground, &mut process);
        }
        if process == std::process::id()
            && GetWindowLongW(foreground, GWL_EXSTYLE) as u32 & WS_EX_TOPMOST != 0
        {
            foreground
        } else {
            HWND_TOPMOST
        }
    };
    // SAFETY: Pins and Home share the calling UI thread's input queue. Let the
    // pin receive keys; only restore the relative stacking order below, not focus.
    unsafe { SetFocus(window) };
    // SAFETY: This reads the focus of the current UI thread without retaining data.
    if unsafe { GetFocus() } != window {
        return Err(std::io::Error::last_os_error().to_string().into());
    }
    // SAFETY: The live pin belongs to this process and this call runs on its UI
    // thread. Preserve its geometry, topmost level, and newly acquired focus.
    let raised = unsafe {
        SetWindowPos(
            window,
            sibling,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW,
        )
    };
    if raised == 0 {
        Err(std::io::Error::last_os_error().to_string().into())
    } else {
        Ok(())
    }
}

#[cfg(target_os = "linux")]
pub fn activate(id: u64) -> Result<(), Message> {
    use x11rb::{
        connection::Connection,
        protocol::xproto::{
            AtomEnum, ClientMessageEvent, ConfigureWindowAux, ConnectionExt, EventMask, StackMode,
        },
    };

    // The app chooses X11 whenever DISPLAY exists. Native Wayland provides no
    // equivalent request through this backend; native focus remains authoritative.
    if std::env::var_os("DISPLAY").is_none() {
        return Ok(());
    }
    let (connection, screen) = x11rb::connect(None).map_err(|error| error.to_string())?;
    let root = connection.setup().roots[screen].root;
    let atom = |name: &[u8]| -> Result<u32, Message> {
        Ok(connection
            .intern_atom(true, name)
            .map_err(|error| error.to_string())?
            .reply()
            .map_err(|error| error.to_string())?
            .atom)
    };
    let pid = atom(b"_NET_WM_PID")?;
    if pid == 0 {
        return Err(Text::PinNotFound.into());
    }
    let windows = named_x11_windows(&connection, root, &title(id))?;
    let property_value = |window, property, property_type| {
        connection
            .get_property(false, window, property, property_type, 0, 1)
            .ok()
            .and_then(|cookie| cookie.reply().ok())
            .and_then(|reply| reply.value32().and_then(|mut values| values.next()))
    };
    let active = atom(b"_NET_ACTIVE_WINDOW")?;
    let supported_atom = atom(b"_NET_SUPPORTED")?;
    if supported_atom == 0 {
        return Ok(());
    }
    let supported = connection
        .get_property(false, root, supported_atom, AtomEnum::ATOM, 0, u32::MAX)
        .map_err(|error| error.to_string())?
        .reply()
        .map_err(|error| error.to_string())?;
    let supports = |atom| {
        atom != 0
            && supported
                .value32()
                .is_some_and(|mut values| values.any(|value| value == atom))
    };
    if !supports(active) {
        return Ok(());
    }
    let sibling = property_value(root, active, AtomEnum::WINDOW).filter(|window| {
        property_value(*window, pid, AtomEnum::CARDINAL) == Some(std::process::id())
    });
    if let Some(window) = windows.into_iter().next() {
        let sibling = sibling.filter(|sibling| *sibling != window);
        let restack = atom(b"_NET_RESTACK_WINDOW")?;
        if sibling.is_some() && !supports(restack) {
            // Do not cover the initiating list when its WM cannot keep it above us.
            return Ok(());
        }
        let mask = EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY;
        connection
            .send_event(
                false,
                root,
                mask,
                ClientMessageEvent::new(
                    32,
                    window,
                    active,
                    [1, x11rb::CURRENT_TIME, sibling.unwrap_or(0), 0, 0],
                ),
            )
            .map_err(|error| error.to_string())?
            .check()
            .map_err(|error| error.to_string())?;
        // The WM decides activation and stacking. Send each request once, in
        // order, without polling or trying to override its focus policy.
        if let Some(sibling) = sibling {
            // Reparenting window managers make the client windows non-siblings.
            // Ask the WM to restack them instead of a failing ConfigureWindow.
            let event = ClientMessageEvent::new(
                32,
                window,
                restack,
                [1, sibling, u32::from(StackMode::BELOW), 0, 0],
            );
            connection.send_event(false, root, mask, event)
        } else {
            connection.configure_window(
                window,
                &ConfigureWindowAux::new().stack_mode(StackMode::ABOVE),
            )
        }
        .map_err(|error| error.to_string())?
        .check()
        .map_err(|error| error.to_string())?;
        connection.flush().map_err(|error| error.to_string())?;
        return Ok(());
    }
    Err(Text::PinNotFound.into())
}

#[cfg(target_os = "linux")]
pub(super) fn named_x11_windows(
    connection: &impl x11rb::connection::Connection,
    root: u32,
    title: &str,
) -> Result<Vec<u32>, Message> {
    use x11rb::protocol::xproto::{AtomEnum, ConnectionExt};
    let atom = |name: &[u8]| -> Result<u32, Message> {
        Ok(connection
            .intern_atom(true, name)
            .map_err(|error| error.to_string())?
            .reply()
            .map_err(|error| error.to_string())?
            .atom)
    };
    let clients = atom(b"_NET_CLIENT_LIST")?;
    let pid = atom(b"_NET_WM_PID")?;
    let name = atom(b"_NET_WM_NAME")?;
    let utf8 = atom(b"UTF8_STRING")?;
    if [clients, pid, name, utf8].contains(&0) {
        return Err(Text::PinNotFound.into());
    }
    let clients = connection
        .get_property(false, root, clients, AtomEnum::WINDOW, 0, u32::MAX)
        .map_err(|error| error.to_string())?
        .reply()
        .map_err(|error| error.to_string())?;
    let mut windows = Vec::new();
    for window in clients.value32().into_iter().flatten() {
        let own = connection
            .get_property(false, window, pid, AtomEnum::CARDINAL, 0, 1)
            .ok()
            .and_then(|cookie| cookie.reply().ok())
            .is_some_and(|reply| {
                reply.value32().and_then(|mut values| values.next()) == Some(std::process::id())
            });
        if !own {
            continue;
        }
        let named = connection
            .get_property(false, window, name, utf8, 0, title.len().div_ceil(4) as u32)
            .ok()
            .and_then(|cookie| cookie.reply().ok())
            .is_some_and(|reply| {
                reply.format == 8 && reply.bytes_after == 0 && reply.value == title.as_bytes()
            });
        if named {
            windows.push(window);
        }
    }
    Ok(windows)
}

#[cfg(target_os = "linux")]
pub(super) fn raise_selection() -> Result<(), Message> {
    use x11rb::{
        connection::Connection,
        protocol::xproto::{ConfigureWindowAux, ConnectionExt, StackMode},
    };
    if std::env::var_os("DISPLAY").is_none() {
        return Ok(());
    }
    let (connection, screen) = x11rb::connect(None).map_err(|error| error.to_string())?;
    let root = connection.setup().roots[screen].root;
    for window in named_x11_windows(&connection, root, snip2pin::SELECTION_WINDOW_TITLE)? {
        connection
            .configure_window(
                window,
                &ConfigureWindowAux::new().stack_mode(StackMode::ABOVE),
            )
            .map_err(|error| error.to_string())?
            .check()
            .map_err(|error| error.to_string())?;
    }
    connection.flush().map_err(|error| error.to_string())?;
    Ok(())
}

#[cfg(test)]
#[path = "pin_window_tests.rs"]
mod focus_tests;

//! Native context menus use the current pointer and the owning pin window.
#[cfg(any(target_os = "macos", target_os = "windows", test))]
use snip2pin::i18n::Text;
use snip2pin::i18n::{Locale, Message};

#[cfg(any(target_os = "macos", target_os = "windows"))]
use tray_icon::menu::{ContextMenu, Menu, MenuItem};

#[cfg(any(target_os = "macos", target_os = "windows", test))]
fn entries(id: u64, locale: Locale, locked: bool) -> [(String, &'static str, bool); 3] {
    [
        (format!("pin-copy:{id}"), locale.text(Text::Copy), true),
        (
            format!("pin-lock:{id}"),
            locale.text(if locked { Text::Unlock } else { Text::Lock }),
            true,
        ),
        (format!("pin-close:{id}"), locale.text(Text::Close), !locked),
    ]
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn menu(id: u64, locale: Locale, locked: bool) -> Result<Menu, Message> {
    let menu = Menu::new();
    for (id, label, enabled) in entries(id, locale, locked) {
        menu.append(&MenuItem::with_id(id, label, enabled, None))
            .map_err(|error| error.to_string())?;
    }
    Ok(menu)
}

/// Called on the UI thread in response to a pin's context-menu request.
/// `true` includes user cancellation; `false` means this backend is unsupported.
#[cfg(target_os = "macos")]
#[allow(unsafe_code)]
pub fn show(id: u64, locale: Locale, locked: bool) -> Result<bool, Message> {
    use objc2::rc::Retained;
    use objc2_app_kit::NSApplication;

    let thread = objc2::MainThreadMarker::new().ok_or(Text::UiThread)?;
    let application = NSApplication::sharedApplication(thread);
    let title = format!("Snip2Pin · 贴图 {id}");
    let windows = application.windows();
    let window = windows
        .iter()
        .find(|window| window.title().to_string() == title)
        .ok_or(Text::PinNotFound)?;
    let view = window.contentView().ok_or(Text::PinViewMissing)?;
    let menu = menu(id, locale, locked)?;
    // SAFETY: MainThreadMarker confirms AppKit's UI thread. The retained window,
    // content view and menu stay alive throughout this synchronous tracking call.
    // Only the borrowed NSView pointer is passed; no class or ownership changes.
    unsafe { menu.show_context_menu_for_nsview(Retained::as_ptr(&view).cast(), None) };
    Ok(true)
}

#[cfg(target_os = "windows")]
#[allow(unsafe_code)]
pub fn show(id: u64, locale: Locale, locked: bool) -> Result<bool, Message> {
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
        window: HWND,
    }

    unsafe extern "system" fn find(window: HWND, data: LPARAM) -> BOOL {
        // SAFETY: EnumWindows invokes this callback synchronously with the
        // exclusive Search borrow supplied below; the pointer never escapes.
        let search = unsafe { &mut *(data as *mut Search) };
        let mut process = 0;
        // SAFETY: EnumWindows supplies a valid HWND and process is writable.
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
        // SAFETY: title has room for the reported text and its null terminator;
        // GetWindowTextW writes synchronously and never retains its buffer.
        let written = unsafe { GetWindowTextW(window, title.as_mut_ptr(), title.len() as i32) };
        if written as usize == search.title.len() && title[..written as usize] == search.title {
            search.window = window;
            return 0;
        }
        1
    }

    let mut search = Search {
        title: format!("Snip2Pin · 贴图 {id}").encode_utf16().collect(),
        window: std::ptr::null_mut(),
    };
    // SAFETY: Search stays at a stable address during synchronous enumeration.
    // The callback only borrows it and retains a handle owned by this process.
    unsafe { EnumWindows(Some(find), (&mut search as *mut Search) as LPARAM) };
    if search.window.is_null() {
        return Err(Text::PinNotFound.into());
    }
    let menu = menu(id, locale, locked)?;
    // SAFETY: This UI-thread call uses the live pin HWND found in this process.
    // Menu events only enqueue app actions; the app cannot remove the owner
    // while this synchronous native menu tracking call is still running.
    unsafe { menu.show_context_menu_for_hwnd(search.window as isize, None) };
    Ok(true)
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub fn show(_id: u64, _locale: Locale, _locked: bool) -> Result<bool, Message> {
    // The existing Linux menu backend has no GTK integration or event loop.
    // Keep the lightweight app fallback without starting a second GUI runtime.
    Ok(false)
}

#[cfg(test)]
mod tests {
    #[test]
    fn native_menu_actions_preserve_copy_and_unlock_but_disable_locked_close() {
        assert_eq!(
            super::entries(42, super::Locale::Chinese, true),
            [
                ("pin-copy:42".into(), "复制", true),
                ("pin-lock:42".into(), "解锁", true),
                ("pin-close:42".into(), "关闭", false),
            ]
        );
        assert_eq!(
            super::entries(u64::MAX, super::Locale::English, false)[0].0,
            "pin-copy:18446744073709551615"
        );
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    #[test]
    fn unsupported_backend_requests_the_lightweight_fallback() {
        assert_eq!(super::show(42, super::Locale::English, true), Ok(false));
    }
}

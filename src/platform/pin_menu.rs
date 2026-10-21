//! Native context menus use the current pointer and the owning pin window.
#[cfg(target_os = "macos")]
use snip2pin::i18n::Text;
use snip2pin::i18n::{Locale, Message};
#[cfg(any(target_os = "macos", target_os = "windows", test))]
use snip2pin::pin::MenuCommand;

#[cfg(any(target_os = "macos", target_os = "windows"))]
use tray_icon::menu::{ContextMenu, Menu, MenuItem};

#[cfg(any(target_os = "macos", target_os = "windows", test))]
fn entries(
    id: u64,
    locale: Locale,
    locked: bool,
) -> [(String, &'static str, bool); MenuCommand::ALL.len()] {
    MenuCommand::ALL.map(|command| {
        (
            format!("{}:{id}", command.key()),
            locale.text(command.text(locked)),
            command.enabled(locked),
        )
    })
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

    let window = super::pin_window::find(id)?;
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
    let window = super::pin_window::find(id)?;
    let menu = menu(id, locale, locked)?;
    // SAFETY: This UI-thread call uses the live pin HWND found in this process.
    // Menu events only enqueue app actions; the app cannot remove the owner
    // while this synchronous native menu tracking call is still running.
    unsafe { menu.show_context_menu_for_hwnd(window as isize, None) };
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
                ("pin-restore:42".into(), "恢复原始尺寸", false),
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

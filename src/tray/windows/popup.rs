//! Cancel only Capture's native selection before Windows creates selection fade.
use snip2pin::i18n::Message;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use tray_icon::{
    TrayIcon,
    menu::{ContextMenu, Menu, MenuId},
};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::VK_RETURN;
use windows_sys::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, WPARAM},
    System::Threading::GetCurrentThreadId,
    UI::WindowsAndMessaging::{
        CallNextHookEx, EndMenu, GUI_INMENUMODE, GUI_POPUPMENUMODE, GUI_SYSTEMMENUMODE,
        GUITHREADINFO, GetCursorPos, GetGUIThreadInfo, GetMenuItemID, GetMenuState, HHOOK, HMENU,
        MF_BYPOSITION, MF_DISABLED, MF_GRAYED, MF_HILITE, MSG, MSGF_MENU, MenuItemFromPoint,
        PostMessageW, SetForegroundWindow, SetWindowsHookExW, TPM_BOTTOMALIGN, TPM_LEFTALIGN,
        TrackPopupMenu, UnhookWindowsHookEx, WH_MSGFILTER, WM_KEYDOWN, WM_LBUTTONUP, WM_NULL,
        WM_SYSKEYDOWN,
    },
};

struct Scope {
    owner: HWND,
    menu: HMENU,
    capture: u32,
    pending: Cell<bool>,
}

thread_local! {
    // Owned scope, never a borrowed stack pointer. Clone before any reentrant API.
    static ACTIVE: RefCell<Option<Rc<Scope>>> = const { RefCell::new(None) };
}

struct Hook {
    handle: HHOOK,
}

impl Hook {
    #[allow(unsafe_code)]
    fn install(scope: Rc<Scope>) -> Result<Self, Message> {
        if ACTIVE.with(|active| active.borrow().is_some())
            || menu_context().is_none_or(|info| {
                info.flags & (GUI_INMENUMODE | GUI_POPUPMENUMODE | GUI_SYSTEMMENUMODE) != 0
            })
        {
            return Err("A native menu is already tracking on this UI thread."
                .to_owned()
                .into());
        }
        ACTIVE.with(|active| *active.borrow_mut() = Some(scope));
        // SAFETY: The callback runs only on this UI thread. Owned TLS context is
        // installed before the hook and removed on failure or after unhooking.
        let handle = unsafe {
            SetWindowsHookExW(
                WH_MSGFILTER,
                Some(filter),
                std::ptr::null_mut(),
                GetCurrentThreadId(),
            )
        };
        if handle.is_null() {
            let error = std::io::Error::last_os_error();
            ACTIVE.with(|active| active.borrow_mut().take());
            return Err(error.to_string().into());
        }
        Ok(Self { handle })
    }

    #[allow(unsafe_code)]
    fn remove(&mut self) -> Result<(), Message> {
        if self.handle.is_null() {
            return Ok(());
        }
        let handle = std::mem::replace(&mut self.handle, std::ptr::null_mut());
        // SAFETY: Installed on this UI thread; the scope remains alive until the
        // hook is removed. On failure, clearing TLS still makes later callbacks
        // pass through safely; explicit removal reports the error to the caller.
        let removed = unsafe { UnhookWindowsHookEx(handle) };
        let error = (removed == 0).then(std::io::Error::last_os_error);
        ACTIVE.with(|active| active.borrow_mut().take());
        if let Some(error) = error {
            return Err(error.to_string().into());
        }
        Ok(())
    }
}

impl Drop for Hook {
    fn drop(&mut self) {
        let _ = self.remove();
    }
}

#[allow(unsafe_code)]
fn menu_context() -> Option<GUITHREADINFO> {
    let mut info = GUITHREADINFO {
        cbSize: std::mem::size_of::<GUITHREADINFO>() as u32,
        ..Default::default()
    };
    // SAFETY: Query this UI thread into a correctly sized writable structure.
    (unsafe { GetGUIThreadInfo(GetCurrentThreadId(), &mut info) } != 0).then_some(info)
}

#[allow(unsafe_code)]
fn capture_item(scope: &Scope, position: u32) -> bool {
    // SAFETY: The retained Menu owns this HMENU throughout native tracking.
    let (id, state) = unsafe {
        (
            GetMenuItemID(scope.menu, position as i32),
            GetMenuState(scope.menu, position, MF_BYPOSITION),
        )
    };
    id == scope.capture && state != u32::MAX && state & (MF_DISABLED | MF_GRAYED) == 0
}

#[allow(unsafe_code)]
unsafe extern "system" fn filter(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == MSGF_MENU as i32 && lparam != 0 {
        let scope = ACTIVE.with(|active| active.borrow().clone());
        if let Some(scope) = scope
            && let Some(info) = menu_context()
            && info.hwndMenuOwner == scope.owner
            && info.flags & GUI_POPUPMENUMODE != 0
        {
            // SAFETY: WH_MSGFILTER supplies a live MSG for this synchronous call.
            let message = unsafe { &*(lparam as *const MSG) };
            let capture = if message.message == WM_LBUTTONUP {
                // SAFETY: Only system hit-testing against this scope's own menu.
                let position = unsafe { MenuItemFromPoint(scope.owner, scope.menu, message.pt) };
                position >= 0 && capture_item(&scope, position as u32)
            } else if matches!(message.message, WM_KEYDOWN | WM_SYSKEYDOWN)
                && message.wParam == VK_RETURN as usize
            {
                // The public Menu supplied the exact Capture command ID. Its
                // by-command state must really be highlighted and enabled.
                // SAFETY: scope.menu is live; this reads only the known command.
                let state = unsafe { GetMenuState(scope.menu, scope.capture, 0) };
                state != u32::MAX
                    && state & MF_HILITE != 0
                    && state & (MF_DISABLED | MF_GRAYED) == 0
            } else {
                false
            };
            if capture {
                if scope.pending.replace(true) {
                    return 1;
                }
                // No RefCell or mutable MSG borrow crosses EndMenu's reentrancy.
                // SAFETY: Current-thread popup ownership was verified above.
                if unsafe { EndMenu() } != 0 {
                    return 1;
                }
                scope.pending.set(false);
            }
        }
    }
    // SAFETY: Every unmatched/failed operation continues the original hook chain.
    unsafe { CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam) }
}

#[allow(unsafe_code)]
pub(super) fn show(icon: &TrayIcon, menu: &Menu, capture: &MenuId) -> Result<bool, Message> {
    let position = menu
        .items()
        .iter()
        .position(|item| item.id() == capture)
        .ok_or_else(|| Message::from("Capture menu item is unavailable.".to_owned()))?;
    // Short library borrows finish here, before TrackPopupMenu's nested dispatch.
    let owner = icon.window_handle();
    let native_menu = menu.hpopupmenu() as HMENU;
    // SAFETY: The Menu/TrayIcon references keep both native handles alive.
    let capture = unsafe { GetMenuItemID(native_menu, position as i32) };
    if capture == u32::MAX {
        return Err("Capture native menu item is unavailable.".to_owned().into());
    }
    let scope = Rc::new(Scope {
        owner,
        menu: native_menu,
        capture,
        pending: Cell::new(false),
    });
    let mut point = windows_sys::Win32::Foundation::POINT::default();
    // SAFETY: Only a writable current-cursor query and current-thread hook.
    if unsafe { GetCursorPos(&mut point) } == 0 {
        return Err(std::io::Error::last_os_error().to_string().into());
    }
    let mut hook = Hook::install(scope.clone())?;
    // SAFETY: Match the notification-area menu protocol on our own owner. Muda's
    // existing subclass still routes every ordinary WM_COMMAND item unchanged.
    unsafe {
        SetForegroundWindow(owner);
        TrackPopupMenu(
            native_menu,
            TPM_BOTTOMALIGN | TPM_LEFTALIGN,
            point.x,
            point.y,
            0,
            owner,
            std::ptr::null(),
        );
        PostMessageW(owner, WM_NULL, 0, 0);
    }
    hook.remove()?;
    Ok(scope.pending.get())
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows_sys::{
        Win32::UI::WindowsAndMessaging::{
            AppendMenuW, CallMsgFilterW, CreatePopupMenu, CreateWindowExW, DestroyMenu,
            DestroyWindow, HWND_MESSAGE, WS_EX_NOACTIVATE,
        },
        core::w,
    };

    struct Fixture {
        owner: HWND,
        menu: HMENU,
    }
    impl Drop for Fixture {
        #[allow(unsafe_code)]
        fn drop(&mut self) {
            // SAFETY: This test owns both handles on their creating thread.
            unsafe {
                DestroyMenu(self.menu);
                DestroyWindow(self.owner);
            }
        }
    }

    #[test]
    #[allow(unsafe_code)]
    fn native_hook_rejects_other_contexts_nested_scopes_and_releases_its_thread() {
        // SAFETY: A private message-only owner cannot display or activate. It
        // creates the test thread's native GUI queue without opening a popup.
        let fixture = unsafe {
            Fixture {
                owner: CreateWindowExW(
                    WS_EX_NOACTIVATE,
                    w!("STATIC"),
                    w!("popup scope test"),
                    0,
                    0,
                    0,
                    0,
                    0,
                    HWND_MESSAGE,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null(),
                ),
                menu: CreatePopupMenu(),
            }
        };
        assert!(!fixture.owner.is_null() && !fixture.menu.is_null());
        // SAFETY: The fixture owns this native menu throughout the hook scope.
        assert_ne!(
            unsafe { AppendMenuW(fixture.menu, 0, 107, w!("Capture")) },
            0
        );
        let scope = Rc::new(Scope {
            owner: fixture.owner,
            menu: fixture.menu,
            capture: 107,
            pending: Cell::new(false),
        });
        let mut hook = Hook::install(scope.clone()).expect("install own-thread native hook");
        assert!(
            Hook::install(scope.clone()).is_err(),
            "nested scope is rejected"
        );
        assert!(
            std::thread::spawn(|| ACTIVE.with(|active| active.borrow().is_none()))
                .join()
                .expect("independent thread")
        );
        let message = MSG {
            hwnd: fixture.owner,
            message: WM_LBUTTONUP,
            ..Default::default()
        };
        // SAFETY: Exercise the real installed filter through User32. No owned
        // popup is tracking, so even a Capture-shaped message must pass through.
        assert_eq!(unsafe { CallMsgFilterW(&message, MSGF_MENU as i32) }, 0);
        assert!(!scope.pending.get());
        hook.remove()
            .expect("remove native hook before releasing handles");
        assert!(ACTIVE.with(|active| active.borrow().is_none()));
    }
}

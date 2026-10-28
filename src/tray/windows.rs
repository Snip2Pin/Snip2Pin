//! Select an embedded icon frame using the notification area's system metric.
use snip2pin::i18n::Message;

#[path = "windows/popup.rs"]
mod popup;

pub(super) fn requests_menu(id: &tray_icon::TrayIconId, event: &tray_icon::TrayIconEvent) -> bool {
    matches!(event, tray_icon::TrayIconEvent::Click {
        id: source,
        button: tray_icon::MouseButton::Left | tray_icon::MouseButton::Right,
        button_state: tray_icon::MouseButtonState::Up,
        ..
    } if source == id)
}

pub(super) fn show_menu(
    icon: &tray_icon::TrayIcon,
    menu: &tray_icon::menu::Menu,
    capture: &tray_icon::menu::MenuId,
) -> Result<bool, Message> {
    popup::show(icon, menu, capture)
}
use windows_sys::{
    Win32::UI::{
        HiDpi::{GetDpiForSystem, GetDpiForWindow, GetSystemMetricsForDpi},
        WindowsAndMessaging::{FindWindowW, SM_CXSMICON, SM_CYSMICON},
    },
    core::w,
};

pub(super) fn icon() -> Result<tray_icon::Icon, Message> {
    resource_icon(small_icon_size()?)
}

fn resource_icon(size: (u32, u32)) -> Result<tray_icon::Icon, Message> {
    // build.rs embeds the status symbol as group 2, preserving app icon group 1.
    // The library delegates frame selection/scaling to LoadImageW and owns HICON.
    tray_icon::Icon::from_resource(2, Some(size)).map_err(|error| error.to_string().into())
}

#[allow(unsafe_code)]
fn small_icon_size() -> Result<(u32, u32), Message> {
    // SAFETY: These calls only read system state. The taskbar owns its HWND;
    // GetDpiForWindow returns zero if it disappears during the query.
    let dpi = unsafe {
        let taskbar = FindWindowW(w!("Shell_TrayWnd"), std::ptr::null());
        let dpi = GetDpiForWindow(taskbar);
        if dpi == 0 { GetDpiForSystem() } else { dpi }
    };
    small_icon_size_for_dpi(dpi)
}

#[allow(unsafe_code)]
fn small_icon_size_for_dpi(dpi: u32) -> Result<(u32, u32), Message> {
    // SAFETY: Both calls only read the system's small-icon metrics at this DPI.
    let (width, height) = unsafe {
        (
            GetSystemMetricsForDpi(SM_CXSMICON, dpi),
            GetSystemMetricsForDpi(SM_CYSMICON, dpi),
        )
    };
    if width <= 0 || height <= 0 {
        return Err(std::io::Error::last_os_error().to_string().into());
    }
    Ok((width as u32, height as u32))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_own_icon_left_or_right_release_requests_the_popup() {
        use tray_icon::{MouseButton, MouseButtonState, TrayIconEvent, TrayIconId};
        let own = TrayIconId::from("own");
        for id in [own.clone(), TrayIconId::from("other")] {
            for button in [MouseButton::Left, MouseButton::Right, MouseButton::Middle] {
                for state in [MouseButtonState::Up, MouseButtonState::Down] {
                    let event = TrayIconEvent::Click {
                        id: id.clone(),
                        position: Default::default(),
                        rect: Default::default(),
                        button,
                        button_state: state,
                    };
                    assert_eq!(
                        requests_menu(&own, &event),
                        id == own && button != MouseButton::Middle && state == MouseButtonState::Up
                    );
                }
            }
        }
    }
    use windows_sys::Win32::{
        Graphics::Gdi::{BITMAP, DeleteObject, GetObjectW},
        System::LibraryLoader::GetModuleHandleW,
        UI::WindowsAndMessaging::{DestroyIcon, GetIconInfo, ICONINFO, IMAGE_ICON, LoadImageW},
    };

    #[test]
    fn embedded_resource_loads_at_system_small_icon_metrics() {
        // Exercise the production resource constructor from 100% through 400%.
        // The native bitmap probe verifies the same LoadImageW resource contract;
        // tray-icon keeps its own HICON private. This is not visual acceptance.
        for dpi in [96, 120, 144, 168, 192, 240, 288, 384] {
            let size = small_icon_size_for_dpi(dpi).expect("system small-icon metric");
            let _icon = resource_icon(size).expect("library loads embedded status icon group 2");
            assert_eq!(native_resource_size(size), size, "DPI {dpi}");
        }
        let _icon = icon().expect("current taskbar's DPI selects an embedded icon");
    }

    #[allow(unsafe_code)]
    fn native_resource_size(size: (u32, u32)) -> (u32, u32) {
        // SAFETY: This repeats the library's synchronous LoadImageW call against
        // this test executable's resource. Every returned GDI object and HICON
        // is released before asserting its metadata; no windows are created.
        unsafe {
            let handle = LoadImageW(
                GetModuleHandleW(std::ptr::null()),
                2usize as _,
                IMAGE_ICON,
                size.0 as i32,
                size.1 as i32,
                0,
            );
            assert!(!handle.is_null(), "load native icon resource");
            let mut info = ICONINFO::default();
            let found = GetIconInfo(handle, &mut info);
            DestroyIcon(handle);
            assert_ne!(found, 0, "read native icon bitmaps");
            let mut bitmap = BITMAP::default();
            let count = GetObjectW(
                info.hbmColor,
                std::mem::size_of::<BITMAP>() as i32,
                (&mut bitmap as *mut BITMAP).cast(),
            );
            DeleteObject(info.hbmColor);
            DeleteObject(info.hbmMask);
            assert_ne!(count, 0, "read native color bitmap dimensions");
            (bitmap.bmWidth as u32, bitmap.bmHeight as u32)
        }
    }
}

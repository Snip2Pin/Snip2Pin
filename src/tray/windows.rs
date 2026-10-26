//! Select an embedded icon frame using the notification area's system metric.
use snip2pin::i18n::Message;
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
    // build.rs embeds group 1 from the multi-size ICO. The library delegates
    // frame selection/scaling to LoadImageW and owns the resulting HICON.
    tray_icon::Icon::from_resource(1, Some(size)).map_err(|error| error.to_string().into())
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
    use windows_sys::Win32::{
        Graphics::Gdi::{BITMAP, DeleteObject, GetObjectW},
        System::LibraryLoader::GetModuleHandleW,
        UI::WindowsAndMessaging::{DestroyIcon, GetIconInfo, ICONINFO, IMAGE_ICON, LoadImageW},
    };

    #[test]
    fn embedded_resource_loads_at_system_small_icon_metrics() {
        // Exercise the production resource constructor at 100/125/150/200%.
        // The native bitmap probe verifies the same LoadImageW resource contract;
        // tray-icon keeps its own HICON private. This is not visual acceptance.
        for dpi in [96, 120, 144, 192] {
            let size = small_icon_size_for_dpi(dpi).expect("system small-icon metric");
            let _icon = resource_icon(size).expect("library loads embedded icon group 1");
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
                1usize as _,
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

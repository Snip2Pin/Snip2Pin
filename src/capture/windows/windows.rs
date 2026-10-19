//! One front-to-back EnumWindows pass, including this app's Home and pins.
#![allow(unsafe_code)]

use std::sync::atomic::{AtomicBool, Ordering};

use windows_sys::{
    Win32::{
        Foundation::{HWND, LPARAM, RECT},
        Graphics::Dwm::{DWMWA_CLOAKED, DWMWA_EXTENDED_FRAME_BOUNDS, DwmGetWindowAttribute},
        UI::WindowsAndMessaging::{
            EnumWindows, GWL_EXSTYLE, GetClassNameW, GetDesktopWindow, GetWindowLongPtrW,
            GetWindowRect, IsIconic, IsWindowVisible, WS_EX_TRANSPARENT,
        },
    },
    core::BOOL,
};

use super::WindowBounds;

struct Enumeration<'a> {
    cancelled: &'a AtomicBool,
    windows: Vec<WindowBounds>,
}

fn describe(window: HWND) -> Option<WindowBounds> {
    let mut rect = RECT::default();
    let mut cloaked = 0_u32;
    // Win32 window class names contain at most 256 characters, including NUL.
    const WINDOW_CLASS_CAPACITY: usize = 256;
    let mut class = [0; WINDOW_CLASS_CAPACITY];
    // SAFETY: EnumWindows provides a live HWND for this synchronous query. All
    // outputs are owned, correctly sized buffers. A disappearing window simply
    // fails its query; no native pointer or handle survives this function.
    unsafe {
        if IsWindowVisible(window) == 0
            || IsIconic(window) != 0
            || window == GetDesktopWindow()
            || GetWindowLongPtrW(window, GWL_EXSTYLE) as u32 & WS_EX_TRANSPARENT != 0
        {
            return None;
        }
        let length = GetClassNameW(window, class.as_mut_ptr(), class.len() as i32);
        let class = String::from_utf16_lossy(&class[..length.max(0) as usize]);
        if matches!(
            class.as_str(),
            "Progman" | "WorkerW" | "Shell_TrayWnd" | "Shell_SecondaryTrayWnd"
        ) {
            return None;
        }
        if DwmGetWindowAttribute(
            window,
            DWMWA_CLOAKED,
            (&mut cloaked as *mut u32).cast(),
            std::mem::size_of_val(&cloaked) as u32,
        ) >= 0
            && cloaked != 0
        {
            return None;
        }
        if DwmGetWindowAttribute(
            window,
            DWMWA_EXTENDED_FRAME_BOUNDS,
            (&mut rect as *mut RECT).cast(),
            std::mem::size_of_val(&rect) as u32,
        ) < 0
            && GetWindowRect(window, &mut rect) == 0
        {
            return None;
        }
    }
    Some(WindowBounds {
        origin: [f64::from(rect.left), f64::from(rect.top)],
        size: [
            f64::from(rect.right) - f64::from(rect.left),
            f64::from(rect.bottom) - f64::from(rect.top),
        ],
    })
}

unsafe extern "system" fn visit(window: HWND, state: LPARAM) -> BOOL {
    // SAFETY: visible supplies an exclusive stack borrow for this synchronous
    // enumeration. The callback cannot outlive it or retain any native handle.
    let state = unsafe { &mut *(state as *mut Enumeration<'_>) };
    if state.cancelled.load(Ordering::Acquire) {
        return 0;
    }
    if let Some(window) = describe(window) {
        state.windows.push(window);
    }
    1
}

pub(in crate::capture) fn visible(cancelled: &AtomicBool) -> Vec<WindowBounds> {
    let mut state = Enumeration {
        cancelled,
        windows: Vec::new(),
    };
    // SAFETY: state remains alive at this address until EnumWindows returns.
    // Cancellation may stop enumeration early; its partial result is discarded
    // by the capture worker and is never retried.
    unsafe { EnumWindows(Some(visit), (&mut state as *mut Enumeration<'_>) as LPARAM) };
    state.windows
}

//! Keep only our topmost pins below visible topmost taskbars.

#[derive(Clone, Copy)]
struct Entry {
    window: usize,
    taskbar: bool,
}

#[derive(Clone, Copy)]
enum Request {
    Raise,
    After(usize),
    Lower,
}

fn predecessor(order: &[Entry], request: Request) -> Option<usize> {
    let ceiling = order.iter().rposition(|entry| entry.taskbar)?;
    match request {
        Request::Raise => Some(order[ceiling].window),
        Request::After(window)
            if order.iter().position(|entry| entry.window == window)? < ceiling =>
        {
            Some(order[ceiling].window)
        }
        Request::After(_) | Request::Lower => None,
    }
}

#[cfg(target_os = "windows")]
pub(crate) use native::install;

#[cfg(target_os = "windows")]
#[allow(unsafe_code)]
mod native {
    use super::{Entry, Request, predecessor};
    use snip2pin::i18n::Message;
    use windows_sys::{
        Win32::{
            Foundation::{HWND, LPARAM, LRESULT, WPARAM},
            UI::{
                Shell::{
                    DefSubclassProc, GetWindowSubclass, RemoveWindowSubclass, SetWindowSubclass,
                },
                WindowsAndMessaging::{
                    EnumWindows, GW_HWNDNEXT, GWL_EXSTYLE, GetClassNameW, GetTopWindow, GetWindow,
                    GetWindowLongW, HWND_BOTTOM, HWND_NOTOPMOST, HWND_TOP, HWND_TOPMOST,
                    IsWindowVisible, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOOWNERZORDER, SWP_NOSIZE,
                    SWP_NOZORDER, SetWindowPos, WINDOWPOS, WM_NCDESTROY, WM_WINDOWPOSCHANGING,
                    WS_EX_TOPMOST,
                },
            },
        },
        core::BOOL,
    };

    const SUBCLASS: usize = 0x53325054;

    fn topmost(window: HWND) -> bool {
        // SAFETY: A transient query of a handle from the current native event/enumeration.
        unsafe { GetWindowLongW(window, GWL_EXSTYLE) as u32 & WS_EX_TOPMOST != 0 }
    }

    fn taskbar(window: HWND) -> bool {
        // SAFETY: Only reads the current visibility/style and a bounded class name.
        if unsafe { IsWindowVisible(window) } == 0 || !topmost(window) {
            return false;
        }
        let mut name = [0_u16; 32];
        // SAFETY: name is writable for the supplied capacity; the API terminates the name.
        let length = unsafe { GetClassNameW(window, name.as_mut_ptr(), name.len() as i32) };
        let name = &name[..length.max(0) as usize];
        name.iter().copied().eq("Shell_TrayWnd".encode_utf16())
            || name
                .iter()
                .copied()
                .eq("Shell_SecondaryTrayWnd".encode_utf16())
    }

    fn order() -> Option<Vec<Entry>> {
        unsafe extern "system" fn collect(window: HWND, data: LPARAM) -> BOOL {
            // SAFETY: EnumWindows synchronously borrows the local Vec below.
            unsafe { &mut *(data as *mut Vec<HWND>) }.push(window);
            1
        }
        let mut windows = Vec::<HWND>::new();
        // SAFETY: The local Vec stays at a stable address for synchronous enumeration.
        if unsafe { EnumWindows(Some(collect), (&mut windows as *mut Vec<HWND>) as LPARAM) } == 0 {
            return None;
        }
        let mut result = Vec::<Entry>::with_capacity(windows.len());
        // EnumWindows supplies membership/bounds, not an assumed sorting contract.
        // SAFETY: Null asks for the highest top-level window; no pointers are retained.
        let mut window = unsafe { GetTopWindow(std::ptr::null_mut()) };
        for _ in 0..windows.len() {
            if window.is_null() {
                return None;
            }
            if !windows.contains(&window)
                || result.iter().any(|entry| entry.window == window as usize)
            {
                return None;
            }
            result.push(Entry {
                window: window as usize,
                taskbar: taskbar(window),
            });
            // SAFETY: Read one current Z-order relationship; the bounded snapshot
            // and duplicate check prevent loops if the Shell changes concurrently.
            window = unsafe { GetWindow(window, GW_HWNDNEXT) };
        }
        window.is_null().then_some(result)
    }

    fn replacement(window: HWND, after: HWND) -> Option<HWND> {
        if !topmost(window) {
            return None;
        }
        let request = if after == HWND_BOTTOM || after == HWND_NOTOPMOST {
            Request::Lower
        } else if after == HWND_TOP || after == HWND_TOPMOST {
            Request::Raise
        } else {
            Request::After(after as usize)
        };
        if matches!(request, Request::Lower) {
            return None;
        }
        predecessor(&order()?, request).map(|window| window as HWND)
    }

    unsafe extern "system" fn subclass(
        window: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
        id: usize,
        _data: usize,
    ) -> LRESULT {
        if message == WM_WINDOWPOSCHANGING && lparam != 0 {
            // SAFETY: Windows owns this writable WINDOWPOS for this synchronous
            // message. Only its requested ordering sibling is changed; geometry,
            // focus and all flags retain the original operation's semantics.
            let position = unsafe { &mut *(lparam as *mut WINDOWPOS) };
            if position.flags & SWP_NOZORDER == 0
                && let Some(after) = replacement(window, position.hwndInsertAfter)
            {
                position.hwndInsertAfter = after;
            }
        } else if message == WM_NCDESTROY {
            // SAFETY: Remove only our callback/ID while this HWND still exists.
            unsafe { RemoveWindowSubclass(window, Some(subclass), id) };
        }
        // SAFETY: Continue the existing winit/menu subclass chain on every path.
        unsafe { DefSubclassProc(window, message, wparam, lparam) }
    }

    pub(crate) fn install(id: u64) -> Result<(), Message> {
        let window = super::super::find(id)?;
        let mut data = 0;
        // SAFETY: The caller is the application's UI thread and find returns
        // only its own live pin. No allocation or borrowed callback data escapes.
        if unsafe { GetWindowSubclass(window, Some(subclass), SUBCLASS, &mut data) } != 0 {
            return Ok(());
        }
        // SAFETY: The same-thread pin owns this callback until WM_NCDESTROY.
        if unsafe { SetWindowSubclass(window, Some(subclass), SUBCLASS, 0) } == 0 {
            return Err(std::io::Error::last_os_error().to_string().into());
        }
        // Evaluate the pin's current rank, rather than inventing a raise request.
        // An already legal pin must retain its order relative to other siblings.
        if let Some(after) = replacement(window, window) {
            // SAFETY: One initial placement of our own pin only. Future changes
            // are constrained in WINDOWPOS itself, never by corrective calls.
            if unsafe {
                SetWindowPos(
                    window,
                    after,
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_NOOWNERZORDER,
                )
            } == 0
            {
                return Err(std::io::Error::last_os_error().to_string().into());
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn order() -> [Entry; 5] {
        [
            Entry {
                window: 10,
                taskbar: false,
            },
            Entry {
                window: 11,
                taskbar: true,
            },
            Entry {
                window: 12,
                taskbar: false,
            },
            Entry {
                window: 13,
                taskbar: true,
            },
            Entry {
                window: 14,
                taskbar: false,
            },
        ]
    }

    #[test]
    fn raises_stop_below_every_taskbar() {
        assert_eq!(predecessor(&order(), Request::Raise), Some(13));
        assert_eq!(predecessor(&order(), Request::After(10)), Some(13));
        assert_eq!(predecessor(&order(), Request::After(11)), Some(13));
        assert_eq!(predecessor(&order(), Request::After(12)), Some(13));
    }

    #[test]
    fn legal_siblings_lowering_and_unknown_order_are_unchanged() {
        let mut initial = order().to_vec();
        initial.push(Entry {
            window: 15,
            taskbar: false,
        });
        // Initial setup of pin 15 must leave sibling 14 above it, below both bars.
        assert_eq!(predecessor(&initial, Request::After(15)), None);
        for request in [
            Request::After(13),
            Request::After(14),
            Request::After(99),
            Request::Lower,
        ] {
            assert_eq!(predecessor(&order(), request), None);
        }
        assert_eq!(predecessor(&[], Request::Raise), None);
        assert_eq!(
            predecessor(
                &[Entry {
                    window: 10,
                    taskbar: false
                }],
                Request::Raise
            ),
            None
        );
    }
}

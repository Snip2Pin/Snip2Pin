//! Native, asynchronous quit confirmation. Results return through the app queue.
use raw_window_handle::WindowHandle;
use snip2pin::i18n::{Locale, Message};

pub struct Dialog {
    #[cfg(target_os = "macos")]
    alert: objc2::rc::Retained<objc2_app_kit::NSAlert>,
}

impl Dialog {
    /// Windows owns its modal message box until the native result arrives.
    /// Keep the owner window alive and visible during that interval.
    pub fn can_dismiss(&self) -> bool {
        !cfg!(target_os = "windows")
    }
}

#[cfg(target_os = "macos")]
pub fn show(
    owner: WindowHandle<'_>,
    locale: Locale,
    result: impl FnOnce(Result<bool, Message>) + Send + 'static,
) -> Result<Option<Dialog>, Message> {
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSAlert, NSAlertSecondButtonReturn, NSView};
    use objc2_foundation::NSString;
    use raw_window_handle::RawWindowHandle;
    use snip2pin::i18n::Text;
    use std::cell::RefCell;

    let main = MainThreadMarker::new().ok_or(Text::UiThread)?;
    let RawWindowHandle::AppKit(handle) = owner.as_raw() else {
        return Ok(None);
    };
    // SAFETY: WindowHandle borrows the live host view. AppKit is accessed only
    // on the main thread; the sheet retains its owner while it is attached.
    #[allow(unsafe_code)]
    let view = unsafe { handle.ns_view.cast::<NSView>().as_ref() };
    let window = view.window().ok_or(Text::QuitDialogUnavailable)?;
    if !window.isVisible() {
        return Err(Text::QuitDialogUnavailable.into());
    }
    let alert = NSAlert::new(main);
    alert.setMessageText(&NSString::from_str(locale.text(Text::QuitLockedTitle)));
    alert.setInformativeText(&NSString::from_str(locale.text(Text::QuitLockedHelp)));
    let cancel = alert.addButtonWithTitle(&NSString::from_str(locale.text(Text::Cancel)));
    cancel.setKeyEquivalent(&NSString::from_str("\u{1b}"));
    let quit = alert.addButtonWithTitle(&NSString::from_str(locale.text(Text::QuitConfirm)));
    quit.setKeyEquivalent(&NSString::new());
    quit.setHasDestructiveAction(true);
    let result = RefCell::new(Some(result));
    let completion = block2::RcBlock::new(move |response| {
        // A dismissed or otherwise unrecognized response always means Cancel.
        let result = result.borrow_mut().take();
        if let Some(result) = result {
            result(Ok(response == NSAlertSecondButtonReturn));
        }
    });
    alert.beginSheetModalForWindow_completionHandler(&window, Some(&completion));
    Ok(Some(Dialog { alert }))
}

#[cfg(target_os = "macos")]
impl Drop for Dialog {
    fn drop(&mut self) {
        let sheet = self.alert.window();
        if let Some(parent) = sheet.sheetParent() {
            parent.endSheet(&sheet);
        }
    }
}

#[cfg(target_os = "windows")]
pub fn show(
    owner: WindowHandle<'_>,
    locale: Locale,
    result: impl FnOnce(Result<bool, Message>) + Send + 'static,
) -> Result<Option<Dialog>, Message> {
    use raw_window_handle::RawWindowHandle;
    use snip2pin::i18n::Text;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        IDOK, MB_DEFBUTTON2, MB_ICONWARNING, MB_OKCANCEL, MB_SETFOREGROUND, MB_TOPMOST, MessageBoxW,
    };

    let RawWindowHandle::Win32(handle) = owner.as_raw() else {
        return Ok(None);
    };
    let title: Vec<u16> = locale
        .text(Text::QuitLockedTitle)
        .encode_utf16()
        .chain([0])
        .collect();
    let body: Vec<u16> = locale
        .text(Text::QuitLockedHelp)
        .encode_utf16()
        .chain([0])
        .collect();
    let owner = handle.hwnd.get() as usize;
    std::thread::Builder::new()
        .name("quit-confirmation".into())
        .spawn(move || {
            // SAFETY: The app keeps this HWND alive and visible until the result is
            // consumed. Both UTF-16 buffers live for the whole call. A separate
            // thread avoids nesting a Windows message loop inside eframe's borrow.
            #[allow(unsafe_code)]
            let response = unsafe {
                MessageBoxW(
                    owner as _,
                    body.as_ptr(),
                    title.as_ptr(),
                    MB_OKCANCEL | MB_DEFBUTTON2 | MB_ICONWARNING | MB_TOPMOST | MB_SETFOREGROUND,
                )
            };
            result(if response == 0 {
                Err(Text::QuitDialogError.with(std::io::Error::last_os_error()))
            } else {
                Ok(response == IDOK)
            });
        })
        .map_err(|error| Text::QuitDialogError.with(error))?;
    Ok(Some(Dialog {}))
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub fn show(
    _owner: WindowHandle<'_>,
    _locale: Locale,
    _result: impl FnOnce(Result<bool, Message>) + Send + 'static,
) -> Result<Option<Dialog>, Message> {
    // Linux has no universal native confirmation API. Keep the existing UI
    // fallback rather than requiring another GUI runtime or external command.
    Ok(None)
}

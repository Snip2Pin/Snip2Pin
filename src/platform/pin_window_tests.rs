#[test]
fn current_focus_uses_process_ownership_including_child_windows() {
    for (name, focus, nodes, expected) in [
        ("own menu", 10, vec![(10, Some(42), 0)], Some(true)),
        (
            "menu child with PID on ancestor",
            20,
            vec![(20, None, 10), (10, Some(42), 0)],
            Some(true),
        ),
        (
            "external app child",
            20,
            vec![(20, None, 30), (30, Some(99), 0)],
            Some(false),
        ),
        ("missing PID", 20, vec![(20, None, 2), (2, None, 0)], None),
        ("query failure", 20, vec![], None),
        ("no focus", 0, vec![], None),
        ("pointer root", 1, vec![], None),
    ] {
        let mut inspected = Vec::new();
        let actual = super::focus_owned_by(focus, 42, |window| {
            inspected.push(window);
            nodes
                .iter()
                .find(|node| node.0 == window)
                .map(|node| (node.1, node.2))
        });
        assert_eq!(actual, expected, "{name}");
        if focus <= 1 {
            assert!(inspected.is_empty(), "special focus values have no window");
        }
    }
}

#[cfg(target_os = "windows")]
#[test]
#[allow(unsafe_code)]
fn passive_selection_initializes_layered_opacity_after_each_capture_transition() {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DestroyWindow, GWL_EXSTYLE, GetLayeredWindowAttributes, GetWindowLongW,
        IsWindowVisible, LWA_ALPHA, SetWindowLongW, WS_EX_LAYERED, WS_EX_NOACTIVATE,
        WS_EX_TRANSPARENT, WS_POPUP,
    };
    struct HiddenWindow(windows_sys::Win32::Foundation::HWND);
    impl Drop for HiddenWindow {
        fn drop(&mut self) {
            // SAFETY: The hidden test HWND is owned by this thread.
            unsafe { DestroyWindow(self.0) };
        }
    }
    let create = |title: &str| {
        let class: Vec<_> = "STATIC\0".encode_utf16().collect();
        let title: Vec<_> = title.encode_utf16().chain(Some(0)).collect();
        // SAFETY: Built-in class and terminated strings live through creation;
        // the test never shows or activates either window.
        let window = unsafe {
            CreateWindowExW(
                WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_NOACTIVATE,
                class.as_ptr(),
                title.as_ptr(),
                WS_POPUP,
                100,
                100,
                32,
                24,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null(),
            )
        };
        assert!(!window.is_null());
        HiddenWindow(window)
    };
    let selection = create(snip2pin::SELECTION_WINDOW_TITLE);
    let unrelated = create("Snip2Pin hidden non-selection fixture");
    for _ in 0..2 {
        let (mut color, mut alpha, mut flags) = (0, 0, 0);
        // SAFETY: Both fixture handles remain alive on their creating thread.
        assert_eq!(
            unsafe { GetLayeredWindowAttributes(selection.0, &mut color, &mut alpha, &mut flags) },
            0
        );
        super::raise_selection().unwrap();
        assert_ne!(
            unsafe { GetLayeredWindowAttributes(selection.0, &mut color, &mut alpha, &mut flags) },
            0
        );
        assert_eq!(alpha, u8::MAX);
        assert_eq!(flags, LWA_ALPHA);
        assert_eq!(unsafe { IsWindowVisible(selection.0) }, 0);
        assert_eq!(
            unsafe { GetLayeredWindowAttributes(unrelated.0, &mut color, &mut alpha, &mut flags) },
            0
        );
        let style = unsafe { GetWindowLongW(selection.0, GWL_EXSTYLE) };
        assert_eq!(
            style as u32 & (WS_EX_LAYERED | WS_EX_TRANSPARENT),
            WS_EX_LAYERED | WS_EX_TRANSPARENT
        );
        // Model winit leaving and re-entering passive mode between captures.
        unsafe {
            SetWindowLongW(
                selection.0,
                GWL_EXSTYLE,
                style & !(WS_EX_LAYERED | WS_EX_TRANSPARENT) as i32,
            );
            SetWindowLongW(selection.0, GWL_EXSTYLE, style);
        }
    }
}

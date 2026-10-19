#[cfg(target_os = "macos")]
#[path = "../src/platform/macos_dock.rs"]
#[allow(dead_code)]
mod macos_dock;
#[cfg(target_os = "macos")]
#[path = "../src/tray.rs"]
#[allow(dead_code, unused_imports)] // The harness-free target does not run tray's unit tests.
mod tray;

#[cfg(target_os = "macos")]
#[allow(unsafe_code)]
fn main() {
    use objc2::{MainThreadMarker, MainThreadOnly, sel};
    use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy, NSMenu, NSMenuItem};
    use objc2_foundation::{NSAppleEventDescriptor, NSAppleEventManager, NSString};

    let thread = MainThreadMarker::new().expect("native menu test runs on the main thread");
    let app = NSApplication::sharedApplication(thread);
    app.setActivationPolicy(NSApplicationActivationPolicy::Prohibited);
    let original_menu = app.mainMenu();
    let menu = NSMenu::initWithTitle(NSMenu::alloc(thread), &NSString::from_str("Generated test"));
    // SAFETY: A process-local menu uses NSApplication's documented Quit selector.
    // The test never invokes it until the adapter has replaced its target/action.
    let quit = unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(
            NSMenuItem::alloc(thread),
            &NSString::from_str("Generated quit"),
            Some(sel!(terminate:)),
            &NSString::from_str("q"),
        )
    };
    // SAFETY: NSApplication implements the selector above and outlives the item.
    unsafe { quit.setTarget(Some(&app)) };
    menu.addItem(&quit);
    app.setMainMenu(Some(&menu));
    let (sender, receiver) = std::sync::mpsc::channel();
    let mut dock = macos_dock::Dock::new(move |action| {
        sender.send(action).expect("live command receiver");
    })
    .expect("install system Quit route");

    // A raw test executable has no bundled NIB. Resource failure must leave the
    // Quit and Reopen bridges operational independently of the custom menu.
    assert!(dock.load_menu().is_err());
    assert_eq!(quit.title().to_string(), "Generated quit");
    assert_eq!(quit.keyEquivalent().to_string(), "q");
    let manager = NSAppleEventManager::sharedAppleEventManager();
    for _ in 0..2 {
        let event = NSAppleEventDescriptor::appleEventWithEventClass_eventID_targetDescriptor_returnID_transactionID(
            u32::from_be_bytes(*b"aevt"),
            u32::from_be_bytes(*b"rapp"),
            None,
            -1,
            0,
        );
        let reply = NSAppleEventDescriptor::nullDescriptor();
        // SAFETY: Both descriptors stay alive for synchronous in-process
        // dispatch. No event is sent to another process or to the WindowServer.
        let result = unsafe {
            manager.dispatchRawAppleEvent_withRawReply_handlerRefCon(
                std::ptr::NonNull::new(event.aeDesc().cast_mut()).expect("event descriptor"),
                std::ptr::NonNull::new(reply.aeDesc().cast_mut()).expect("reply descriptor"),
                std::ptr::null_mut(),
            )
        };
        assert_eq!(result, 0, "Dock reopen must reach the installed handler");
        assert_eq!(receiver.try_recv(), Ok(tray::Action::Home));
        assert!(!app.isActive());
        assert!(app.windows().is_empty());
    }
    for confirmed in [false, false, true] {
        menu.performActionForItemAtIndex(0);
        assert_eq!(receiver.try_recv(), Ok(tray::Action::Quit));
        dock.resolve_quit(confirmed);
        assert!(!app.isActive());
        assert!(app.windows().is_empty());
    }
    drop(dock);
    assert_eq!(quit.action(), Some(sel!(terminate:)));
    assert!(std::ptr::eq(
        &*quit.target().expect("restored application target"),
        &*app as &objc2::runtime::AnyObject,
    ));
    app.setMainMenu(original_menu.as_deref());
    assert!(!app.isActive());
    assert!(app.windows().is_empty());
    println!(
        "macOS native Reopen and Quit routes passed after missing NIB and repeated cancellation; no windows or activation"
    );
}

#[cfg(not(target_os = "macos"))]
fn main() {}

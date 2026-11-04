#[cfg(target_os = "macos")]
#[path = "../src/platform/macos_displays.rs"]
#[allow(dead_code)]
mod macos_displays;

#[cfg(target_os = "macos")]
#[allow(unsafe_code)]
fn main() {
    use objc2_app_kit::NSApplicationDidChangeScreenParametersNotification;
    use objc2_foundation::{NSNotificationCenter, NSString};
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    let _thread = objc2::MainThreadMarker::new().expect("main-thread notification test");
    // Use a private center: never notify a real application or alter displays.
    let center = NSNotificationCenter::new();
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = calls.clone();
    let observer = macos_displays::DisplayChanges::observe(center.clone(), move || {
        observed.fetch_add(1, Ordering::AcqRel);
    })
    .expect("display observer");
    // SAFETY: A private center, immutable notification names and no object.
    unsafe {
        center
            .postNotificationName_object(&NSString::from_str("UnrelatedFixtureNotification"), None);
        assert_eq!(calls.load(Ordering::Acquire), 0);
        for expected in 1..=2 {
            center.postNotificationName_object(
                NSApplicationDidChangeScreenParametersNotification,
                None,
            );
            assert_eq!(calls.load(Ordering::Acquire), expected);
        }
        drop(observer);
        center
            .postNotificationName_object(NSApplicationDidChangeScreenParametersNotification, None);
    }
    assert_eq!(
        calls.load(Ordering::Acquire),
        2,
        "dropping the observer removes its callback"
    );
    println!(
        "macOS display notifications and observer teardown passed on a private center; no windows or display changes"
    );
}

#[cfg(not(target_os = "macos"))]
fn main() {}

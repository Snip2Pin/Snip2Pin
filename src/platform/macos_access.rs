//! Public AppKit APIs only: no TCC edits, resets, activation, or cache hacks.
use snip2pin::i18n::{Message, Text};
use std::path::Path;

use objc2::MainThreadMarker;
use objc2_app_kit::NSWorkspace;
use objc2_foundation::{NSBundle, NSString, NSURL};

pub fn request() -> bool {
    MainThreadMarker::new().is_some() && objc2_core_graphics::CGRequestScreenCaptureAccess()
}

pub fn open_settings() -> Result<(), Message> {
    let _thread = MainThreadMarker::new().ok_or(Text::UiThread)?;
    let url = NSURL::URLWithString(&NSString::from_str(
        "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture",
    ))
    .ok_or(Text::SettingsUrl)?;
    NSWorkspace::sharedWorkspace()
        .openURL(&url)
        .then_some(())
        .ok_or(Text::OpenSettingsError.into())
}

pub fn check_installation() -> Result<(), Message> {
    let _thread = MainThreadMarker::new().ok_or(Text::UiThread)?;
    let bundle = NSBundle::mainBundle();
    if bundle
        .bundleIdentifier()
        .is_none_or(|id| id.to_string() != snip2pin::APP_ID)
    {
        return Err(Text::AppIdentity.into());
    }
    let expected = bundle.bundleURL().path().ok_or(Text::AppPath)?;
    let registered = NSWorkspace::sharedWorkspace()
        .URLForApplicationWithBundleIdentifier(&NSString::from_str(snip2pin::APP_ID))
        .and_then(|url| url.path())
        .ok_or(Text::AppUnregistered)?;
    let canonical = |path: &NSString| {
        std::fs::canonicalize(path.to_string()).map_err(|error| error.to_string())
    };
    if canonical(&expected)? != canonical(&registered)? {
        return Err(Text::AppRegistration.into());
    }
    Ok(())
}

pub fn check_identity_migration(target: &Path) -> Result<(), Message> {
    let _thread = MainThreadMarker::new().ok_or(Text::UiThread)?;
    let target = target.canonicalize().map_err(|error| error.to_string())?;
    for app in NSWorkspace::sharedWorkspace().runningApplications() {
        if app.isTerminated() {
            continue;
        }
        if app
            .bundleIdentifier()
            .is_some_and(|id| id.to_string() == "com.apple.systempreferences")
        {
            return Err(Text::QuitSettings.into());
        }
        if app
            .bundleURL()
            .and_then(|url| url.path())
            .is_some_and(|path| {
                std::fs::canonicalize(path.to_string()).is_ok_and(|path| path == target)
            })
        {
            return Err(Text::QuitApp.into());
        }
    }
    Ok(())
}

//! Public AppKit APIs only: no TCC edits, resets, activation, or cache hacks.
use std::path::Path;

use objc2::MainThreadMarker;
use objc2_app_kit::NSWorkspace;
use objc2_foundation::{NSBundle, NSString, NSURL};

pub fn request() -> bool {
    MainThreadMarker::new().is_some() && objc2_core_graphics::CGRequestScreenCaptureAccess()
}

pub fn open_settings() -> Result<(), String> {
    let _thread = MainThreadMarker::new().ok_or("授权设置必须从应用主线程打开。")?;
    let url = NSURL::URLWithString(&NSString::from_str(
        "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture",
    ))
    .ok_or("无法创建授权设置地址。")?;
    NSWorkspace::sharedWorkspace()
        .openURL(&url)
        .then_some(())
        .ok_or("无法打开系统授权设置。".into())
}

pub fn check_installation() -> Result<(), String> {
    let _thread = MainThreadMarker::new().ok_or("应用登记检查必须在主线程运行。")?;
    let bundle = NSBundle::mainBundle();
    if bundle
        .bundleIdentifier()
        .is_none_or(|id| id.to_string() != snip2pin::APP_ID)
    {
        return Err("应用元数据与编译标识不一致。".into());
    }
    let expected = bundle.bundleURL().path().ok_or("无法读取应用路径。")?;
    let registered = NSWorkspace::sharedWorkspace()
        .URLForApplicationWithBundleIdentifier(&NSString::from_str(snip2pin::APP_ID))
        .and_then(|url| url.path())
        .ok_or("系统未登记当前应用标识。".to_owned())?;
    let canonical = |path: &NSString| {
        std::fs::canonicalize(path.to_string()).map_err(|error| error.to_string())
    };
    if canonical(&expected)? != canonical(&registered)? {
        return Err("系统登记指向了另一份应用，安装未完成。".into());
    }
    Ok(())
}

pub fn check_identity_migration(target: &Path) -> Result<(), String> {
    let _thread = MainThreadMarker::new().ok_or("身份迁移检查必须在主线程运行。")?;
    let target = target.canonicalize().map_err(|error| error.to_string())?;
    for app in NSWorkspace::sharedWorkspace().runningApplications() {
        if app.isTerminated() {
            continue;
        }
        if app
            .bundleIdentifier()
            .is_some_and(|id| id.to_string() == "com.apple.systempreferences")
        {
            return Err("请先完全退出系统设置，再迁移应用标识，避免授权页缓存旧身份。日常更新不需要此步骤。".into());
        }
        if app
            .bundleURL()
            .and_then(|url| url.path())
            .is_some_and(|path| {
                std::fs::canonicalize(path.to_string()).is_ok_and(|path| path == target)
            })
        {
            return Err("请先从菜单栏退出 Snip2Pin，再迁移应用标识。".into());
        }
    }
    Ok(())
}

//! Capture, selection and pin geometry without native window lifecycle.
extern crate self as snip2pin;
pub const LOCAL_DEV: bool = cfg!(all(target_os = "macos", feature = "local-dev"));
pub const APP_NAME: &str = if LOCAL_DEV {
    "Snip2Pin Dev"
} else {
    "Snip2Pin"
};
pub const APP_ID: &str = if LOCAL_DEV {
    "io.github.Snip2Pin.Snip2Pin.dev"
} else {
    "io.github.Snip2Pin.Snip2Pin"
};
pub const HELP_URL: &str = concat!(env!("CARGO_PKG_REPOSITORY"), "/wiki");
/// Stable backend identity, independent of the UI language.
pub const SELECTION_WINDOW_TITLE: &str = "Snip2Pin · 框选";

pub mod capture;
pub mod geometry;
pub mod i18n;
pub mod icon;
pub mod permission;
pub mod pin;
pub mod session;
pub mod settings;
pub mod ui;

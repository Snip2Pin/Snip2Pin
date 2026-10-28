//! Capture, selection and pin geometry without native window lifecycle.
extern crate self as snip2pin;
pub const APP_NAME: &str = "Snip2Pin";
pub const APP_ID: &str = "io.github.Snip2Pin.Snip2Pin";
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

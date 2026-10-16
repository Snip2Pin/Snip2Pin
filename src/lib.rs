//! Capture, selection and pin geometry without native window lifecycle.
extern crate self as snip2pin;
pub const APP_NAME: &str = "Snip2Pin";
pub const APP_ID: &str = "io.github.Snip2Pin.Snip2Pin";

pub mod capture;
pub mod geometry;
pub mod i18n;
pub mod icon;
pub mod permission;
pub mod pin;
pub mod session;
pub mod settings;
pub mod ui;

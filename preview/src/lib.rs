//! Local, generated-image sandbox. Native services are never invoked.
#[cfg(any(target_arch = "wasm32", test))]
mod desktop;
#[cfg(any(target_arch = "wasm32", test))]
mod web;

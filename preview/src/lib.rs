//! Local, generated-image sandbox. Native services are never invoked.
#[cfg(any(target_arch = "wasm32", test))]
mod desktop;
#[cfg(target_arch = "wasm32")]
mod web;

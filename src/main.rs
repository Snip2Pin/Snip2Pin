#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use std::sync::{Arc, Mutex};

mod app;
#[cfg(target_os = "macos")]
#[path = "platform/macos_dock.rs"]
mod macos_dock;
mod platform;
mod startup;
mod tray;
use snip2pin::ui;

fn main() {
    if let Err(error) = run() {
        eprintln!("Snip2Pin: {error}");
        #[cfg(target_os = "windows")]
        platform::show_startup_error(&error.to_string());
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut arguments = std::env::args().skip(1);
    let action = arguments.next();
    if action.as_deref() == Some("--version") {
        println!("Snip2Pin {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    #[cfg(target_os = "macos")]
    if action.as_deref() == Some("--check-installation") {
        platform::macos_access::check_installation()?;
        println!(
            "Installation identity and registration verified: {}",
            snip2pin::APP_ID
        );
        return Ok(());
    }
    #[cfg(target_os = "macos")]
    if action.as_deref() == Some("--check-identity-migration") {
        let target = arguments
            .next()
            .ok_or("--check-identity-migration needs an application path")?;
        platform::macos_access::check_identity_migration(std::path::Path::new(&target))?;
        return Ok(());
    }
    #[cfg(target_os = "macos")]
    if action.as_deref() == Some("--selected-icon") {
        println!(
            "{}",
            match platform::macos_icon::selected_variant()? {
                snip2pin::icon::Variant::Line => "line",
                snip2pin::icon::Variant::Metal => "metal",
            }
        );
        return Ok(());
    }
    let background = action.as_deref() == Some("--background");
    let options = startup_options(background)?;
    let relaunch = Arc::new(Mutex::new(None));
    let relaunch_request = relaunch.clone();
    eframe::run_native(
        "Snip2Pin",
        platform::window_options(options),
        Box::new(|cc| Ok(Box::new(app::Snip2Pin::new(cc, relaunch_request)))),
    )?;
    if let Some(mut command) = relaunch
        .lock()
        .map_err(|_| snip2pin::i18n::Message::from(snip2pin::i18n::Text::RestartError))?
        .take()
    {
        command.spawn()?;
    }
    Ok(())
}

fn startup_options(background: bool) -> Result<eframe::NativeOptions, snip2pin::i18n::Message> {
    Ok(eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title(snip2pin::APP_NAME)
            .with_app_id(snip2pin::APP_ID)
            .with_inner_size(ui::style::LAUNCHER.window)
            .with_window_level(eframe::egui::WindowLevel::Normal)
            .with_transparent(true)
            // Bootstrap transparent capture viewports before hiding the root;
            // eframe cannot create child windows in a logic-only hidden pass.
            .with_decorations(!background)
            .with_has_shadow(!background)
            .with_active(!background)
            .with_mouse_passthrough(background)
            .with_icon(startup_window_icon()?)
            .with_resizable(true),
        centered: true,
        ..Default::default()
    })
}

#[cfg(target_os = "macos")]
fn startup_window_icon() -> Result<eframe::egui::IconData, snip2pin::i18n::Message> {
    // eframe treats Some(empty) as "keep the bundle icon". None would install
    // its own fallback. Runtime choices use our owned-buffer AppKit adapter.
    Ok(eframe::egui::IconData::default())
}

#[cfg(not(target_os = "macos"))]
fn startup_window_icon() -> Result<eframe::egui::IconData, snip2pin::i18n::Message> {
    Ok(eframe::egui::IconData {
        rgba: snip2pin::icon::render(snip2pin::icon::Variant::Line, snip2pin::icon::WINDOW_SIZE)?
            .into_raw(),
        width: snip2pin::icon::WINDOW_SIZE,
        height: snip2pin::icon::WINDOW_SIZE,
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn home_startup_uses_normal_window_level() {
        for background in [false, true] {
            let options = super::startup_options(background).expect("native options");
            assert_eq!(options.viewport.resizable, Some(true));
            assert_eq!(
                options.viewport.window_level,
                Some(eframe::egui::WindowLevel::Normal)
            );
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_startup_preserves_the_bundle_icon_until_the_native_preference_update() {
        let icon = super::startup_window_icon().expect("startup icon");
        assert_eq!(
            icon,
            eframe::egui::IconData::default(),
            "a nonempty startup icon schedules an eframe Dock override before preferences apply"
        );
    }
}

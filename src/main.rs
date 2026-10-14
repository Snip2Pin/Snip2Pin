#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod app;
mod platform;
mod ui;

fn main() {
    if let Err(error) = run() {
        eprintln!("Snip2Pin: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut arguments = std::env::args().skip(1);
    if arguments.next().as_deref() == Some("--write-icon") {
        let path = arguments
            .next()
            .ok_or("--write-icon needs an output path")?;
        let variant = match arguments.next().as_deref().unwrap_or("line") {
            "line" => snip2pin::icon::Variant::Line,
            "metal" => snip2pin::icon::Variant::Metal,
            _ => return Err("icon must be line or metal".into()),
        };
        snip2pin::icon::render_vector(variant, snip2pin::icon::EXPORT_SIZE)?.save(path)?;
        return Ok(());
    }
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("Snip2Pin · 小贴图")
            .with_app_id("io.github.rensilin.snipsprig")
            .with_inner_size(ui::style::LAUNCHER.window)
            .with_transparent(true)
            .with_icon(eframe::egui::IconData {
                rgba: snip2pin::icon::render(
                    snip2pin::icon::Variant::Line,
                    snip2pin::icon::WINDOW_SIZE,
                )?
                .into_raw(),
                width: snip2pin::icon::WINDOW_SIZE,
                height: snip2pin::icon::WINDOW_SIZE,
            })
            .with_resizable(false),
        centered: true,
        ..Default::default()
    };
    eframe::run_native(
        "Snip2Pin",
        platform::window_options(options),
        Box::new(|cc| Ok(Box::new(app::Snip2Pin::new(cc)))),
    )?;
    Ok(())
}

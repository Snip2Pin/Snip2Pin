#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod app;
mod platform;
mod ui;

fn main() {
    if let Err(error) = run() {
        eprintln!("SnipSprig: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut arguments = std::env::args().skip(1);
    if arguments.next().as_deref() == Some("--write-icon") {
        let path = arguments
            .next()
            .ok_or("--write-icon needs an output path")?;
        platform::icon(1024).save(path)?;
        return Ok(());
    }
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("SnipSprig · 小贴图")
            .with_app_id("io.github.rensilin.snipsprig")
            .with_inner_size([380.0, 240.0])
            .with_icon(eframe::egui::IconData {
                rgba: platform::icon(64).into_raw(),
                width: 64,
                height: 64,
            })
            .with_resizable(false),
        centered: true,
        ..Default::default()
    };
    eframe::run_native(
        "SnipSprig",
        options,
        Box::new(|cc| Ok(Box::new(app::SnipSprig::new(cc)))),
    )?;
    Ok(())
}

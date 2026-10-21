//! Local sandbox and an allowlisted, redistributable GitHub Pages bundle.
use crate::{Result, execute, output};
use std::{fs, path::Path, process::Command};

const TARGET: &str = "wasm32-unknown-unknown";
const SITE: &str = ".build/site";

fn build(font: &Path, profile: &str, destination: &Path) -> Result {
    if !font.is_file() {
        return Err("preview requires an existing CJK font file".into());
    }
    let lock: toml::Value = toml::from_str(&fs::read_to_string("Cargo.lock")?)?;
    let version = lock["package"]
        .as_array()
        .and_then(|packages| {
            packages
                .iter()
                .find(|package| package["name"].as_str() == Some("wasm-bindgen"))
        })
        .and_then(|package| package["version"].as_str())
        .ok_or("wasm-bindgen missing from lock file")?;
    if output(Command::new("wasm-bindgen").arg("--version"))? != format!("wasm-bindgen {version}") {
        return Err(format!(
            "install matching tool: cargo install wasm-bindgen-cli --version {version} --locked"
        )
        .into());
    }
    execute(Command::new("cargo").args([
        "build",
        "--locked",
        "--package",
        "snip2pin-preview",
        "--target",
        TARGET,
        "--profile",
        profile,
    ]))?;
    let metadata = output(Command::new("cargo").args([
        "metadata",
        "--locked",
        "--no-deps",
        "--format-version",
        "1",
    ]))?;
    let target = crate::package::target_directory(&metadata)?;
    let directory = if profile == "dev" { "debug" } else { profile };
    execute(
        Command::new("wasm-bindgen")
            .arg(
                target
                    .join(TARGET)
                    .join(directory)
                    .join("snip2pin_preview.wasm"),
            )
            .args(["--target", "web", "--no-typescript", "--out-dir"])
            .arg(destination),
    )?;
    let output_font = destination.join("font.ttf");
    if font != output_font {
        fs::copy(font, output_font)?;
    }
    Ok(())
}

pub fn local(font: &Path) -> Result {
    build(font, "dev", Path::new("preview/pkg"))?;
    println!(
        "Sandbox ready. Serve preview/ on 127.0.0.1; local fonts and generated files must not be published."
    );
    Ok(())
}

pub fn site() -> Result {
    let site = Path::new(SITE);
    if site.exists() {
        fs::remove_dir_all(site)?;
    }
    fs::create_dir_all(site.join("pkg"))?;
    let font = site.join("pkg/font.ttf");
    // Publication never accepts an arbitrary operating-system font.
    execute(
        Command::new("python3")
            .arg("preview/build_font.py")
            .arg(&font),
    )?;
    build(&font, "web", &site.join("pkg"))?;
    fs::create_dir_all(site.join("assets"))?;
    for (source, destination) in [
        ("preview/assets/wallpaper.webp", "assets/wallpaper.webp"),
        ("preview/fonts/OFL.txt", "FONT_LICENSE.txt"),
        ("LICENSE", "LICENSE.txt"),
    ] {
        fs::copy(source, site.join(destination))?;
    }
    let index = fs::read_to_string("preview/index.html")?.replace(
        "<!-- SITE_NOTICES -->",
        r#"<nav id="licenses"><a href="./LICENSE.txt" target="_blank" rel="noopener">MIT</a> · <a href="./FONT_LICENSE.txt" target="_blank" rel="noopener">Font / 字体</a> · <a href="./THIRD_PARTY_NOTICES.txt" target="_blank" rel="noopener">Licenses / 许可</a></nav>"#,
    );
    fs::write(site.join("index.html"), index)?;
    crate::notices::write_web(&site.join("THIRD_PARTY_NOTICES.txt"))?;
    for file in ["pkg/snip2pin_preview_bg.wasm", "pkg/font.ttf"] {
        println!("{file}: {} bytes", fs::metadata(site.join(file))?.len());
    }
    println!("Public demo ready in {SITE}; no local fonts or source files are included.");
    Ok(())
}

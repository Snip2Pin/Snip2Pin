fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=assets/icons/line.ico");
    println!("cargo:rerun-if-changed=assets/icons/status-windows.ico");
    println!("cargo:rerun-if-changed=packaging/windows/app.manifest");
    if std::env::var("CARGO_CFG_TARGET_OS")?.as_str() == "windows" {
        winresource::WindowsResource::new()
            .set_icon("assets/icons/line.ico")
            .set_icon_with_id("assets/icons/status-windows.ico", "2")
            .set_manifest_file("packaging/windows/app.manifest")
            .set("ProductName", "Snip2Pin")
            .set("FileDescription", "Snip2Pin")
            .set("OriginalFilename", "snip2pin.exe")
            .compile()?;
    }
    Ok(())
}

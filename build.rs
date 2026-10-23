fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=assets/icons/line.ico");
    if std::env::var("CARGO_CFG_TARGET_OS")?.as_str() == "windows" {
        winresource::WindowsResource::new()
            .set_icon("assets/icons/line.ico")
            .set("ProductName", "Snip2Pin")
            .set("FileDescription", "Snip2Pin")
            .set("OriginalFilename", "snip2pin.exe")
            .compile()?;
    }
    Ok(())
}

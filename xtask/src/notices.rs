//! Include registry-provided license texts without leaking local filesystem paths.
use crate::{Result, output};
use serde_json::Value;
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::process::Command;

pub fn write(destination: &Path) -> Result {
    let metadata: Value = serde_json::from_str(&output(Command::new("cargo").args([
        "metadata",
        "--locked",
        "--format-version",
        "1",
    ]))?)?;
    let packages = metadata["packages"].as_array().ok_or("missing packages")?;
    let app = packages
        .iter()
        .find(|p| p["name"] == "snip2pin")
        .ok_or("missing app")?;
    let mut pending = vec![app["id"].as_str().ok_or("app ID")?.to_owned()];
    let mut reachable = BTreeSet::new();
    while let Some(id) = pending.pop() {
        if !reachable.insert(id.clone()) {
            continue;
        }
        let node = metadata["resolve"]["nodes"]
            .as_array()
            .ok_or("dependency graph")?
            .iter()
            .find(|node| node["id"] == id)
            .ok_or("dependency node")?;
        for dependency in node["dependencies"].as_array().ok_or("dependencies")? {
            pending.push(dependency.as_str().ok_or("dependency ID")?.to_owned());
        }
    }
    let mut text = String::from(
        "Snip2Pin third-party notices\n\nThis includes dependencies for all supported platforms. License texts below are copied from registry packages. System-library licenses are supplied by their operating-system packages.\n",
    );
    let mut dependencies: Vec<_> = packages
        .iter()
        .filter(|p| {
            p["name"] != "snip2pin" && p["id"].as_str().is_some_and(|id| reachable.contains(id))
        })
        .collect();
    dependencies.sort_by_key(|p| p["id"].as_str().unwrap_or_default());
    for package in dependencies {
        let name = package["name"].as_str().ok_or("crate name")?;
        let version = package["version"].as_str().ok_or("crate version")?;
        text.push_str(&format!("\n\n=== {name} {version} ===\nLicense: {}\nSource: https://crates.io/crates/{name}/{version}\n", package["license"].as_str().unwrap_or("See package license-file")));
        let directory = Path::new(package["manifest_path"].as_str().ok_or("crate manifest")?)
            .parent()
            .ok_or("crate directory")?;
        append_texts(directory, 0, &mut text)?;
    }
    fs::write(destination, text)?;
    Ok(())
}

fn append_texts(directory: &Path, depth: usize, output: &mut String) -> Result {
    let mut entries: Vec<_> = fs::read_dir(directory)?.collect::<std::io::Result<_>>()?;
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        let name = entry.file_name().to_string_lossy().to_lowercase();
        let kind = entry.file_type()?;
        if kind.is_dir() && depth < 3 && ["licenses", "license", "fonts"].contains(&name.as_str()) {
            append_texts(&entry.path(), depth + 1, output)?;
        } else if kind.is_file()
            && ["license", "licence", "copying", "notice", "ofl"]
                .iter()
                .any(|prefix| name.starts_with(prefix))
            && let Ok(text) = fs::read_to_string(entry.path())
        {
            output.push('\n');
            output.push_str(&text);
        }
    }
    Ok(())
}

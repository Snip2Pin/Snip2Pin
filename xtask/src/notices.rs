//! Source-backed notices from the pinned cargo-about collector, plus font assets.
use crate::{Result, output};
use serde::Deserialize;
use serde_json::Value;
use std::{collections::BTreeSet, fs, path::Path, process::Command};

const VERSION: &str = "cargo-about 0.9.2";
const REVIEWED: &str = include_str!("../../packaging/licenses/reviewed.json");
const FONTS: &[(&str, &str)] = &[
    (
        "Hack-Regular.txt",
        "47c0cccbeec7e8614548cc485588b28149e7874188df5f41b36efebcee285c87",
    ),
    (
        "UFL.txt",
        "2f0015108d68627bd788d313f529c21ff4da2c2c42a5e1f3883acc83480f9002",
    ),
    (
        "OFL.txt",
        "6a73f9541c2de74158c0e7cf6b0a58ef774f5a780bf191f2d7ec9cc53efe2bf2",
    ),
    (
        "emoji-icon-font-mit-license.txt",
        "b9d2c1d909aa149996fd4c91dcb92b2362a04431640c1d200959da94caf8cde1",
    ),
];

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Reviewed {
    name: String,
    version: String,
    commit: String,
    manifest_sha256: String,
    statement: Option<String>,
    statement_sha256: Option<String>,
}

pub fn write(destination: &Path) -> Result {
    if output(
        Command::new("cargo-about")
            .arg("--version")
            .current_dir(crate::root()),
    )? != VERSION
    {
        return Err(format!("license generation requires {VERSION}").into());
    }
    let directory = tempfile::tempdir()?;
    let collected = directory.path().join("licenses.json");
    output(
        Command::new("cargo-about")
            .args([
                "generate",
                "--locked",
                "--manifest-path",
                "Cargo.toml",
                "--format",
                "json",
                "--fail",
            ])
            .arg("--output-file")
            .arg(&collected)
            .current_dir(crate::root()),
    )?;
    let data: Value = serde_json::from_slice(&fs::read(collected)?)?;
    let reviewed: Vec<Reviewed> = serde_json::from_str(REVIEWED)?;
    fs::write(destination, render(&data, &reviewed)?)?;
    Ok(())
}

fn checked_text(path: &Path, checksum: &str) -> Result<String> {
    let bytes = fs::read(path)?;
    if crate::package::digest(path)? != checksum {
        return Err(
            "reviewed license source changed; review its attribution before packaging".into(),
        );
    }
    let text = String::from_utf8(bytes)?;
    if text.trim().is_empty() {
        return Err("empty license source".into());
    }
    Ok(text)
}

fn package_name(package: &Value) -> Result<String> {
    Ok(format!(
        "{} {}",
        package["name"].as_str().ok_or("crate name")?,
        package["version"].as_str().ok_or("crate version")?
    ))
}

fn package_directory(package: &Value) -> Result<&Path> {
    Path::new(package["manifest_path"].as_str().ok_or("crate manifest")?)
        .parent()
        .ok_or_else(|| "crate directory".into())
}

fn append_declared_license(package: &Value, text: &mut String) -> Result {
    if let Some(file) = package["license_file"].as_str() {
        let directory = package_directory(package)?;
        let path = directory.join(file).canonicalize()?;
        if !path.starts_with(directory.canonicalize()?) {
            return Err("declared license-file escapes its published package".into());
        }
        let contents = fs::read_to_string(path)?;
        if contents.trim().is_empty() {
            return Err("empty declared license-file".into());
        }
        text.push_str(&format!(
            "\n\n=== {}: declared license-file ===\n{contents}\n",
            package_name(package)?
        ));
    }
    Ok(())
}

fn reviewed_declaration(package: &Value, reviewed: &[Reviewed]) -> Result<String> {
    let name = package_name(package)?;
    let entry = reviewed.iter().find(|entry| package["name"] == entry.name && package["version"] == entry.version)
        .ok_or_else(|| format!("missing source-backed license text for {name}; review the published upstream sources"))?;
    let directory = package_directory(package)?;
    let vcs: Value = serde_json::from_slice(&fs::read(directory.join(".cargo_vcs_info.json"))?)?;
    if vcs["git"]["sha1"] != entry.commit {
        return Err(format!("published source commit changed for {name}").into());
    }
    checked_text(&directory.join("Cargo.toml"), &entry.manifest_sha256)?;
    let mut text = format!(
        "\n\n=== {name}: reviewed upstream declaration ===\nSource: https://crates.io/crates/{}/{}\nRepository metadata: {}\nPublished source commit: {}\nLicense metadata: {}\nAuthors metadata (not a copyright claim): {}\n",
        entry.name,
        entry.version,
        package["repository"],
        entry.commit,
        package["license"],
        package["authors"]
    );
    if let Some(statement) = &entry.statement {
        text.push_str(&checked_text(
            &crate::root().join(statement),
            entry
                .statement_sha256
                .as_deref()
                .ok_or("statement checksum")?,
        )?);
    }
    Ok(text)
}

fn render(data: &Value, reviewed: &[Reviewed]) -> Result<String> {
    let mut text = String::from(
        "Snip2Pin third-party notices\n\nThe desktop app's normal dependencies for supported platforms are listed below. Build/development-only dependencies are excluded. Upstream texts and attributions are preserved; explicitly reviewed standard-license references are labelled separately. System libraries and fonts read from the installed operating system are not bundled here.\n",
    );
    let mut covered = BTreeSet::new();
    for license in data["licenses"].as_array().ok_or("collected licenses")? {
        let id = license["id"].as_str().ok_or("license ID")?;
        let contents = license["text"]
            .as_str()
            .filter(|text| !text.trim().is_empty())
            .ok_or("empty collected license")?;
        let users = license["used_by"].as_array().ok_or("license users")?;
        let mut names = Vec::new();
        for user in users {
            let package = &user["crate"];
            let name = package_name(package)?;
            covered.insert(name.clone());
            names.push(name);
            if license["source_path"].is_null() {
                text.push_str(&reviewed_declaration(package, reviewed)?);
            }
        }
        let label = if license["source_path"].is_null() {
            "SPDX canonical reference (not an upstream copyright notice)"
        } else {
            "upstream license text"
        };
        text.push_str(&format!(
            "\n\n=== {id}: {label} ===\nUsed by: {}\n\n{contents}\n",
            names.join(", ")
        ));
    }
    let packages = data["crates"].as_array().ok_or("collected crates")?;
    for entry in packages {
        if entry["license"] == "Ignore" {
            continue;
        }
        let package = &entry["package"];
        if !covered.contains(&package_name(package)?) {
            return Err(format!("no collected license for {}", package_name(package)?).into());
        }
        // Cargo's declared file is authoritative even with an unconventional name.
        append_declared_license(package, &mut text)?;
    }
    let fonts = packages
        .iter()
        .map(|entry| &entry["package"])
        .find(|package| package["name"] == "epaint_default_fonts")
        .ok_or("embedded font package missing")?;
    for (file, checksum) in FONTS {
        let contents = checked_text(
            &package_directory(fonts)?.join("fonts").join(file),
            checksum,
        )?;
        text.push_str(&format!(
            "\n\n=== Embedded font notice: {file} ===\n{contents}\n"
        ));
    }
    let nsis = checked_text(
        &crate::root().join("packaging/licenses/NSIS-3.13-COPYING.txt"),
        "9afab55b28917f993049218d52d5ed9ec56498bc6d53fe422aea5b60c867ee4b",
    )?;
    text.push_str(&format!("\n\n=== NSIS 3.13: Windows installer runtime only ===\nOfficial distribution copyright and license information:\n{nsis}\n"));
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_canonical_fallbacks_are_rejected() {
        let package = serde_json::json!({"name":"unknown", "version":"1.0.0"});
        assert!(reviewed_declaration(&package, &[]).is_err());
    }

    #[test]
    fn changed_reviewed_license_text_is_rejected() {
        let directory = tempfile::tempdir().expect("temporary source");
        let path = directory.path().join("source");
        fs::write(&path, b"changed source").expect("fixture");
        assert!(checked_text(&path, &"0".repeat(64)).is_err());
    }

    #[test]
    fn declared_license_file_does_not_depend_on_its_name() {
        let directory = tempfile::tempdir().expect("temporary crate");
        fs::create_dir(directory.path().join("terms")).expect("terms directory");
        let path = directory.path().join("terms/arbitrary.dat");
        let actual = include_str!("../../LICENSE");
        fs::write(&path, actual).expect("project's actual MIT license");
        let package = serde_json::json!({
            "name": "fixture", "version": "1.0.0",
            "manifest_path": directory.path().join("Cargo.toml"),
            "license_file": "terms/arbitrary.dat",
        });
        let mut text = String::new();
        append_declared_license(&package, &mut text).expect("declared notice");
        assert!(text.contains(actual));
        fs::write(path, b"").expect("empty source");
        assert!(append_declared_license(&package, &mut text).is_err());
    }

    #[test]
    #[ignore = "requires pinned cargo-about and upstream license downloads; packaging also runs this collection"]
    fn notices_keep_all_four_embedded_font_attributions() {
        let directory = tempfile::tempdir().expect("temporary notices");
        let path = directory.path().join("notices.txt");
        let result = write(&path);
        result.expect("source-backed notices");
        let text = fs::read_to_string(path).expect("generated notices");
        eprintln!("Generated source-backed notices: {} bytes", text.len());
        for marker in [
            "UBUNTU FONT LICENCE",
            "BITSTREAM VERA LICENSE",
            "SIL OPEN FONT LICENSE",
            "Copyright (c) 2014 John Slegers",
            "NSIS 3.13: Windows installer runtime only",
            "miow\nCopyright (c) 2014 Alex Crichton",
        ] {
            assert!(text.contains(marker), "missing {marker}");
        }
        assert!(!text.contains("/Users/"));
        assert!(!text.contains("registry/src/"));
    }
}

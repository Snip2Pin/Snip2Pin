use crate::{Result, execute, manifest, output, revision, version};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub const TARGETS: [&str; 4] = [
    "aarch64-apple-darwin",
    "x86_64-apple-darwin",
    "x86_64-pc-windows-msvc",
    "x86_64-unknown-linux-gnu",
];

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub schema: u8,
    pub version: String,
    pub revision: String,
    pub target: String,
    pub final_package: bool,
    pub file: String,
    pub sha256: String,
    pub candidate_sha256: Option<String>,
}

impl Manifest {
    pub fn validate(&self) -> Result {
        if self.schema != 1 || !TARGETS.contains(&self.target.as_str()) {
            return Err("unsupported package manifest".into());
        }
        crate::release::validate_inputs(&self.version, &self.revision)?;
        let expected = filename(&self.version, &self.target, self.final_package);
        if self.final_package != self.candidate_sha256.is_some()
            || self.candidate_sha256.as_ref().is_some_and(|hash| {
                hash.len() != 64 || !hash.bytes().all(|b| b.is_ascii_hexdigit())
            })
        {
            return Err("missing or invalid unsigned candidate provenance".into());
        }
        if self.file != expected
            || self.sha256.len() != 64
            || !self.sha256.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err("invalid package filename or checksum".into());
        }
        Ok(())
    }
}

pub fn filename(version: &str, target: &str, final_package: bool) -> String {
    let suffix = if final_package { "" } else { "-unsigned" };
    let extension = if target.contains("linux") {
        "tar.gz"
    } else {
        "zip"
    };
    format!("Snip2Pin-{version}-{target}{suffix}.{extension}")
}

pub fn digest(path: &Path) -> Result<String> {
    // Packages are small; the bound also rejects accidental large/non-package inputs.
    if fs::metadata(path)?.len() > 512 * 1024 * 1024 {
        return Err("package exceeds 512 MiB".into());
    }
    Ok(format!("{:x}", Sha256::digest(fs::read(path)?)))
}

fn write_manifest(directory: &Path, data: &Manifest) -> Result {
    data.validate()?;
    fs::write(
        directory.join(format!("{}.json", data.target)),
        serde_json::to_vec_pretty(data)?,
    )?;
    Ok(())
}

pub fn verify(directory: &Path) -> Result<Vec<Manifest>> {
    let mut manifests = Vec::new();
    for target in TARGETS {
        let path = directory.join(format!("{target}.json"));
        if !path.exists() {
            continue;
        }
        let data: Manifest = serde_json::from_slice(&fs::read(path)?)?;
        data.validate()?;
        if data.target != target
            || fs::symlink_metadata(directory.join(&data.file))?
                .file_type()
                .is_symlink()
            || digest(&directory.join(&data.file))? != data.sha256
        {
            return Err("package checksum, target, or file type mismatch".into());
        }
        manifests.push(data);
    }
    if manifests.is_empty() {
        return Err("no package manifests".into());
    }
    Ok(manifests)
}

pub fn build(target: &str, profile: &str) -> Result {
    if !TARGETS.contains(&target) || !["debug", "release"].contains(&profile) {
        return Err("unsupported target/profile".into());
    }
    let host = output(Command::new("rustc").arg("-vV"))?;
    if !host.contains(target) {
        return Err("package on a native runner for its target".into());
    }
    let version = version()?;
    crate::release::validate_inputs(&version, &revision()?)?;
    let mut build = Command::new("cargo");
    build.args([
        "build",
        "--locked",
        "--package",
        "snip2pin",
        "--target",
        target,
    ]);
    if profile == "release" {
        build.arg("--release");
    }
    execute(&mut build)?;
    let base = PathBuf::from(".build/packages").join(profile).join(target);
    if base.exists() {
        fs::remove_dir_all(&base)?;
    }
    let payload = base.join("payload");
    fs::create_dir_all(&payload)?;
    let executable = if target.contains("windows") {
        "snip2pin.exe"
    } else {
        "snip2pin"
    };
    let binary = PathBuf::from("target")
        .join(target)
        .join(profile)
        .join(executable);
    verify_version(&binary, &version)?;
    let archive = base.join(filename(&version, target, false));
    if target.contains("apple") {
        macos_bundle(&binary, &payload.join("Snip2Pin.app"), "line")?;
        crate::notices::write(
            &payload.join("Snip2Pin.app/Contents/Resources/THIRD_PARTY_NOTICES.txt"),
        )?;
        execute(
            Command::new("ditto")
                .args(["-c", "-k", "--sequesterRsrc", "--keepParent"])
                .arg(payload.join("Snip2Pin.app"))
                .arg(&archive),
        )?;
        execute(Command::new("unzip").arg("-tq").arg(&archive))?;
    } else {
        crate::notices::write(&payload.join("THIRD_PARTY_NOTICES.txt"))?;
        fs::copy(&binary, payload.join(executable))?;
        fs::copy("README.md", payload.join("README.md"))?;
        if let Some(license) = manifest()?["package"]
            .get("license-file")
            .and_then(|v| v.as_str())
        {
            fs::copy(license, payload.join("LICENSE"))?;
        } else if Path::new("LICENSE").is_file() {
            fs::copy("LICENSE", payload.join("LICENSE"))?;
        }
        if target.contains("linux") {
            fs::copy(
                "packaging/linux/io.github.Snip2Pin.Snip2Pin.desktop",
                payload.join("io.github.Snip2Pin.Snip2Pin.desktop"),
            )?;
            fs::copy(
                "assets/icons/line.png",
                payload.join("io.github.Snip2Pin.Snip2Pin.png"),
            )?;
            execute(Command::new("ldd").arg(&binary))?;
            execute(
                Command::new("tar")
                    .arg("-czf")
                    .arg(&archive)
                    .arg("-C")
                    .arg(&payload)
                    .arg("."),
            )?;
            execute(Command::new("tar").arg("-tzf").arg(&archive))?;
        } else {
            // PowerShell receives paths as arguments, never interpolated script fragments.
            execute(
                Command::new("pwsh")
                    .args(["-NoProfile", "-File", "packaging/windows/archive.ps1"])
                    .arg(&payload)
                    .arg(&archive),
            )?;
        }
    }
    write_manifest(
        &base,
        &Manifest {
            schema: 1,
            version,
            revision: revision()?,
            target: target.into(),
            final_package: false,
            file: archive
                .file_name()
                .ok_or("archive name")?
                .to_string_lossy()
                .into(),
            sha256: digest(&archive)?,
            candidate_sha256: None,
        },
    )?;
    verify(&base)?;
    println!("Candidate: {}", base.display());
    Ok(())
}

pub fn macos_bundle(binary: &Path, destination: &Path, variant: &str) -> Result {
    if !["line", "metal"].contains(&variant) {
        return Err("unknown icon".into());
    }
    let resources = destination.join("Contents/Resources");
    fs::create_dir_all(&resources)?;
    fs::create_dir_all(destination.join("Contents/MacOS"))?;
    fs::copy(binary, destination.join("Contents/MacOS/snip2pin"))?;
    let plist = destination.join("Contents/Info.plist");
    fs::copy("packaging/macos/Info.plist", &plist)?;
    fs::copy("LICENSE", resources.join("LICENSE"))?;
    let icon = if variant == "line" {
        "AppIcon-Line"
    } else {
        "AppIcon-Metal"
    };
    let build = output(Command::new("git").args(["rev-list", "--count", "HEAD"]))?;
    for (key, value) in [
        ("CFBundleShortVersionString", version()?),
        ("CFBundleVersion", build),
        ("CFBundleIconFile", icon.into()),
    ] {
        execute(
            Command::new("plutil")
                .args(["-replace", key, "-string", &value])
                .arg(&plist),
        )?;
    }
    let iconset = destination.join("AppIcon.iconset");
    fs::create_dir_all(&iconset)?;
    for size in [16_u32, 32, 128, 256, 512] {
        for (pixels, suffix) in [(size, ""), (size * 2, "@2x")] {
            execute(
                Command::new("sips")
                    .args(["-z", &pixels.to_string(), &pixels.to_string()])
                    .arg(format!("assets/icons/{variant}.png"))
                    .arg("--out")
                    .arg(iconset.join(format!("icon_{size}x{size}{suffix}.png"))),
            )?;
        }
    }
    execute(
        Command::new("iconutil")
            .args(["-c", "icns"])
            .arg(&iconset)
            .arg("-o")
            .arg(resources.join(format!("{icon}.icns"))),
    )?;
    fs::remove_dir_all(iconset)?;
    execute(Command::new("plutil").arg("-lint").arg(plist))?;
    Ok(())
}

pub fn finalize(candidate: &Path, archive: &Path) -> Result {
    let data = verify(candidate)?;
    if data.len() != 1 || data[0].final_package {
        return Err("expected one unsigned candidate".into());
    }
    let mut data = data.into_iter().next().ok_or("missing candidate")?;
    let expected = filename(&data.version, &data.target, true);
    if archive.file_name().and_then(|p| p.to_str()) != Some(&expected) {
        return Err("final archive name mismatch".into());
    }
    data.final_package = true;
    data.file = expected;
    data.candidate_sha256 = Some(data.sha256.clone());
    data.sha256 = digest(archive)?;
    write_manifest(archive.parent().ok_or("archive directory")?, &data)
}

pub fn verify_native(directory: &Path, target: &str) -> Result {
    let data = verify(directory)?
        .into_iter()
        .find(|d| d.target == target)
        .ok_or("missing native package")?;
    if !data.final_package {
        return Err("cannot publish an unsigned candidate".into());
    }
    let host = output(Command::new("rustc").arg("-vV"))?;
    if !host.contains(target) {
        return Err("signature checks require a native runner".into());
    }
    let unpacked = PathBuf::from(".build/verification").join(target);
    if unpacked.exists() {
        fs::remove_dir_all(&unpacked)?;
    }
    fs::create_dir_all(&unpacked)?;
    let archive = directory.join(&data.file).canonicalize()?;
    if target.contains("apple") {
        execute(
            Command::new("ditto")
                .args(["-x", "-k"])
                .arg(&archive)
                .arg(&unpacked),
        )?;
        let app = unpacked.join("Snip2Pin.app");
        execute(Command::new("codesign").args(["--verify", "--strict", "-R", "=anchor apple generic and certificate leaf[field.1.2.840.113635.100.6.1.13] exists"]).arg(&app))?;
        execute(
            Command::new("xcrun")
                .args(["stapler", "validate"])
                .arg(&app),
        )?;
        execute(
            Command::new("spctl")
                .args(["--assess", "--type", "execute"])
                .arg(&app),
        )?;
        let plist = app.join("Contents/Info.plist");
        let id = output(
            Command::new("plutil")
                .args(["-extract", "CFBundleIdentifier", "raw", "-o", "-"])
                .arg(&plist),
        )?;
        let version = output(
            Command::new("plutil")
                .args(["-extract", "CFBundleShortVersionString", "raw", "-o", "-"])
                .arg(&plist),
        )?;
        if id != "io.github.Snip2Pin.Snip2Pin" || version != data.version {
            return Err("bundle metadata mismatch".into());
        }
        let binary = app.join("Contents/MacOS/snip2pin");
        let architecture = if target.starts_with("aarch64") {
            "arm64"
        } else {
            "x86_64"
        };
        execute(
            Command::new("lipo")
                .args(["-verify_arch", architecture])
                .arg(&binary),
        )?;
        verify_version(&binary, &data.version)?;
    } else if target.contains("windows") {
        execute(
            Command::new("pwsh")
                .args(["-NoProfile", "-File", "packaging/windows/verify.ps1"])
                .arg(&archive)
                .arg(&unpacked),
        )?;
        verify_version(&unpacked.join("snip2pin.exe"), &data.version)?;
    } else {
        execute(
            Command::new("tar")
                .arg("-xzf")
                .arg(&archive)
                .arg("-C")
                .arg(&unpacked),
        )?;
        verify_version(&unpacked.join("snip2pin"), &data.version)?;
        let dependencies = output(Command::new("ldd").arg(unpacked.join("snip2pin")))?;
        if dependencies.contains("not found") {
            return Err("unresolved Linux runtime dependency".into());
        }
    }
    fs::remove_dir_all(unpacked)?;
    Ok(())
}

fn verify_version(binary: &Path, version: &str) -> Result {
    if output(Command::new(binary).arg("--version"))? != format!("Snip2Pin {version}") {
        return Err("packaged executable version mismatch".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_rejects_path_traversal_and_unknown_targets() {
        let mut data = Manifest {
            schema: 1,
            version: "0.2.0".into(),
            revision: "a".repeat(40),
            target: TARGETS[0].into(),
            final_package: true,
            file: filename("0.2.0", TARGETS[0], true),
            sha256: "b".repeat(64),
            candidate_sha256: Some("c".repeat(64)),
        };
        assert!(data.validate().is_ok());
        data.file = "../secret.key".into();
        assert!(data.validate().is_err());
        data.file = filename("0.2.0", TARGETS[0], true);
        data.target = "unknown".into();
        assert!(data.validate().is_err());
    }

    #[test]
    fn verify_detects_mutated_payload_and_finalize_preserves_original_digest() {
        let directory = tempfile::tempdir().expect("fixture");
        let candidate = directory.path().join("candidate");
        let final_dir = directory.path().join("final");
        fs::create_dir_all(&candidate).expect("candidate directory");
        fs::create_dir_all(&final_dir).expect("final directory");
        let data = Manifest {
            schema: 1,
            version: "0.2.0".into(),
            revision: "a".repeat(40),
            target: TARGETS[3].into(),
            final_package: false,
            file: filename("0.2.0", TARGETS[3], false),
            sha256: format!("{:x}", Sha256::digest(b"original")),
            candidate_sha256: None,
        };
        fs::write(candidate.join(&data.file), b"original").expect("payload");
        write_manifest(&candidate, &data).expect("manifest");
        assert_eq!(verify(&candidate).expect("valid").len(), 1);
        let archive = final_dir.join(filename("0.2.0", TARGETS[3], true));
        fs::write(&archive, b"signed bytes").expect("final payload");
        finalize(&candidate, &archive).expect("finalize");
        let final_data = verify(&final_dir).expect("final manifest");
        assert_eq!(
            final_data[0].candidate_sha256.as_deref(),
            Some(data.sha256.as_str())
        );
        assert_ne!(final_data[0].sha256, data.sha256);
        fs::write(candidate.join(&data.file), b"tampered").expect("tamper");
        assert!(verify(&candidate).is_err());
    }
}

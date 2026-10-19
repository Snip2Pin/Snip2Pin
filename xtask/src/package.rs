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

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Distribution {
    Signed,
    Unverified,
}

impl Distribution {
    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "signed" => Ok(Self::Signed),
            "unverified" => Ok(Self::Unverified),
            _ => Err("distribution must be signed or unverified".into()),
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub schema: u8,
    pub version: String,
    pub revision: String,
    pub target: String,
    pub final_package: bool,
    pub distribution: Option<Distribution>,
    pub file: String,
    pub sha256: String,
    pub candidate_sha256: Option<String>,
}

impl Manifest {
    pub fn validate(&self) -> Result {
        if self.schema != 2 || !TARGETS.contains(&self.target.as_str()) {
            return Err("unsupported package manifest".into());
        }
        crate::release::validate_inputs(&self.version, &self.revision)?;
        let expected = distribution_filename(&self.version, &self.target, self.distribution);
        if self.final_package != self.distribution.is_some() {
            return Err("candidate/final distribution state mismatch".into());
        }
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
    distribution_filename(
        version,
        target,
        final_package.then_some(Distribution::Signed),
    )
}

fn distribution_filename(
    version: &str,
    target: &str,
    distribution: Option<Distribution>,
) -> String {
    let suffix = match distribution {
        None => "-unsigned",
        Some(Distribution::Signed) => "",
        Some(Distribution::Unverified) => "-unverified",
    };
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
    Ok(Sha256::digest(fs::read(path)?)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
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
        fs::copy("packaging/README.txt", payload.join("README.txt"))?;
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
            schema: 2,
            version,
            revision: revision()?,
            target: target.into(),
            final_package: false,
            distribution: None,
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
    execute(
        Command::new("xcrun")
            .args(["ibtool", "--minimum-deployment-target", "13.0", "--compile"])
            .arg(resources.join("DockMenu.nib"))
            .arg("packaging/macos/DockMenu.xib"),
    )
    .map_err(|error| format!("Dock menu compilation requires Xcode's ibtool: {error}"))?;
    fs::create_dir_all(destination.join("Contents/MacOS"))?;
    fs::copy(binary, destination.join("Contents/MacOS/snip2pin"))?;
    let plist = destination.join("Contents/Info.plist");
    fs::copy("packaging/macos/Info.plist", &plist)?;
    fs::copy("LICENSE", resources.join("LICENSE"))?;
    fs::copy("packaging/README.txt", resources.join("README.txt"))?;
    for locale in ["en", "zh-Hans"] {
        let directory = resources.join(format!("{locale}.lproj"));
        fs::create_dir_all(&directory)?;
        fs::copy(
            format!("packaging/macos/{locale}.lproj/InfoPlist.strings"),
            directory.join("InfoPlist.strings"),
        )?;
    }
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
    finalize_as(candidate, archive, Distribution::Signed)
}

fn finalize_as(candidate: &Path, archive: &Path, distribution: Distribution) -> Result {
    let data = verify(candidate)?;
    if data.len() != 1 || data[0].final_package {
        return Err("expected one unsigned candidate".into());
    }
    let mut data = data.into_iter().next().ok_or("missing candidate")?;
    let expected = distribution_filename(&data.version, &data.target, Some(distribution));
    if archive.file_name().and_then(|p| p.to_str()) != Some(&expected) {
        return Err("final archive name mismatch".into());
    }
    data.final_package = true;
    data.distribution = Some(distribution);
    data.file = expected;
    data.candidate_sha256 = Some(data.sha256.clone());
    data.sha256 = digest(archive)?;
    write_manifest(archive.parent().ok_or("archive directory")?, &data)
}

pub fn prepare_unverified(candidate: &Path, destination: &Path, target: &str) -> Result {
    crate::release::verify_candidate(candidate, target)?;
    native_host(target)?;
    let data = verify(candidate)?.remove(0);
    fs::create_dir_all(destination)?;
    let archive = destination.join(distribution_filename(
        &data.version,
        target,
        Some(Distribution::Unverified),
    ));
    if archive.exists() {
        return Err("refusing to replace an existing package".into());
    }
    if target.contains("apple") {
        let unpacked = PathBuf::from(".build/unverified").join(target);
        if unpacked.exists() {
            fs::remove_dir_all(&unpacked)?;
        }
        fs::create_dir_all(&unpacked)?;
        execute(
            Command::new("ditto")
                .args(["-x", "-k"])
                .arg(candidate.join(&data.file))
                .arg(&unpacked),
        )?;
        let app = unpacked.join("Snip2Pin.app");
        // A bundle seal lets macOS verify integrity, without asserting publisher trust.
        execute(
            Command::new("codesign")
                .args([
                    "--force",
                    "--sign",
                    "-",
                    "--timestamp=none",
                    "--identifier",
                    "io.github.Snip2Pin.Snip2Pin",
                ])
                .arg(&app),
        )?;
        execute(
            Command::new("ditto")
                .args(["-c", "-k", "--sequesterRsrc", "--keepParent"])
                .arg(&app)
                .arg(&archive),
        )?;
        fs::remove_dir_all(unpacked)?;
    } else {
        fs::copy(candidate.join(&data.file), &archive)?;
    }
    finalize_as(candidate, &archive, Distribution::Unverified)
}

fn native_host(target: &str) -> Result {
    let host = output(Command::new("rustc").arg("-vV"))?;
    if !TARGETS.contains(&target) || !host.contains(target) {
        return Err("package checks require a supported native runner".into());
    }
    Ok(())
}

pub fn verify_native(directory: &Path, target: &str, distribution: Distribution) -> Result {
    let data = verify(directory)?
        .into_iter()
        .find(|d| d.target == target)
        .ok_or("missing native package")?;
    if !data.final_package || data.distribution != Some(distribution) {
        return Err("candidate or unexpected distribution mode".into());
    }
    if data.revision != revision()? || data.version != version()? {
        return Err("final package source differs from the selected commit".into());
    }
    native_host(target)?;
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
        execute(
            Command::new("codesign")
                .args(["--verify", "--strict"])
                .arg(&app),
        )?;
        if distribution == Distribution::Signed {
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
        } else {
            let signature = Command::new("codesign")
                .args(["--display", "--verbose=2"])
                .arg(&app)
                .output()?;
            if !signature.status.success()
                || !String::from_utf8_lossy(&signature.stderr)
                    .lines()
                    .any(|line| line == "Signature=adhoc")
            {
                return Err("unverified macOS package requires an ad-hoc bundle seal".into());
            }
        }
        let plist = app.join("Contents/Info.plist");
        if !app.join("Contents/Resources/DockMenu.nib").exists() {
            return Err("macOS package is missing its native Dock menu".into());
        }
        for locale in ["en", "zh-Hans"] {
            let strings = app.join(format!(
                "Contents/Resources/{locale}.lproj/InfoPlist.strings"
            ));
            execute(Command::new("plutil").arg("-lint").arg(strings))?;
        }
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
                .arg(&binary)
                .args(["-verify_arch", architecture]),
        )?;
        verify_version(&binary, &data.version)?;
    } else if target.contains("windows") {
        execute(
            Command::new("pwsh")
                .args(["-NoProfile", "-File", "packaging/windows/verify.ps1"])
                .arg(&archive)
                .arg(&unpacked)
                .arg(if distribution == Distribution::Signed {
                    "signed"
                } else {
                    "unverified"
                }),
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
    fn sha256_matches_known_vectors_with_lowercase_and_leading_zeroes() {
        let directory = tempfile::tempdir().expect("fixture");
        let file = directory.path().join("payload");
        for (input, expected) in [
            (
                "",
                "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
            ),
            (
                "abc",
                "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
            ),
        ] {
            fs::write(&file, input).expect("payload");
            assert_eq!(digest(&file).expect("SHA-256"), expected);
        }
    }

    #[test]
    fn manifest_rejects_path_traversal_and_unknown_targets() {
        let mut data = Manifest {
            schema: 2,
            version: "0.2.0".into(),
            revision: "a".repeat(40),
            target: TARGETS[0].into(),
            final_package: true,
            distribution: Some(Distribution::Signed),
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
    fn distribution_state_and_filename_cannot_be_silently_relabelled() {
        let mut data = Manifest {
            schema: 2,
            version: "0.2.0".into(),
            revision: "a".repeat(40),
            target: TARGETS[0].into(),
            final_package: true,
            distribution: Some(Distribution::Unverified),
            file: distribution_filename("0.2.0", TARGETS[0], Some(Distribution::Unverified)),
            sha256: "b".repeat(64),
            candidate_sha256: Some("c".repeat(64)),
        };
        assert!(data.validate().is_ok());
        data.distribution = Some(Distribution::Signed);
        assert!(data.validate().is_err());
        data.distribution = None;
        assert!(data.validate().is_err());
        assert!(Distribution::parse("automatic-fallback").is_err());
        data.distribution = Some(Distribution::Unverified);
        data.schema = 1;
        assert!(data.validate().is_err());
    }

    #[test]
    fn verify_detects_mutated_payload_and_finalize_preserves_original_digest() {
        let directory = tempfile::tempdir().expect("fixture");
        let candidate = directory.path().join("candidate");
        let final_dir = directory.path().join("final");
        fs::create_dir_all(&candidate).expect("candidate directory");
        fs::create_dir_all(&final_dir).expect("final directory");
        let file = filename("0.2.0", TARGETS[3], false);
        fs::write(candidate.join(&file), b"original").expect("payload");
        let data = Manifest {
            schema: 2,
            version: "0.2.0".into(),
            revision: "a".repeat(40),
            target: TARGETS[3].into(),
            final_package: false,
            distribution: None,
            sha256: digest(&candidate.join(&file)).expect("candidate digest"),
            file,
            candidate_sha256: None,
        };
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

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
pub struct Artifact {
    pub format: Format,
    pub file: String,
    pub sha256: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Format {
    Zip,
    Dmg,
    Nsis,
    TarGz,
    Deb,
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
    pub artifacts: Vec<Artifact>,
    pub candidate_sha256: Option<String>,
}

impl Manifest {
    pub fn validate(&self) -> Result {
        if self.schema != 3 || !TARGETS.contains(&self.target.as_str()) {
            return Err("unsupported package manifest".into());
        }
        crate::release::validate_inputs(&self.version, &self.revision)?;
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
        let formats = formats(&self.target, self.final_package);
        if self.artifacts.len() != formats.len() {
            return Err("incomplete or duplicate package formats".into());
        }
        for format in formats {
            let matches: Vec<_> = self
                .artifacts
                .iter()
                .filter(|a| a.format == *format)
                .collect();
            if matches.len() != 1 {
                return Err("incomplete or duplicate package formats".into());
            }
            let artifact = matches[0];
            if artifact.file
                != artifact_filename(&self.version, &self.target, self.distribution, *format)
                || artifact.sha256.len() != 64
                || !artifact.sha256.bytes().all(|b| b.is_ascii_hexdigit())
            {
                return Err("invalid package filename or checksum".into());
            }
        }
        Ok(())
    }

    pub fn artifact(&self, format: Format) -> Result<&Artifact> {
        self.artifacts
            .iter()
            .find(|a| a.format == format)
            .ok_or_else(|| "missing package format".into())
    }
}

pub fn formats(target: &str, final_package: bool) -> &'static [Format] {
    if !final_package {
        if target.contains("linux") {
            &[Format::TarGz]
        } else {
            &[Format::Zip]
        }
    } else if target.contains("apple") {
        &[Format::Dmg]
    } else if target.contains("windows") {
        &[Format::Zip, Format::Nsis]
    } else {
        &[Format::TarGz, Format::Deb]
    }
}

pub fn candidate_filename(version: &str, target: &str) -> String {
    artifact_filename(
        version,
        target,
        None,
        if target.contains("linux") {
            Format::TarGz
        } else {
            Format::Zip
        },
    )
}

pub fn artifact_filename(
    version: &str,
    target: &str,
    distribution: Option<Distribution>,
    format: Format,
) -> String {
    let suffix = match distribution {
        None => "-unsigned",
        Some(Distribution::Signed) => "",
        Some(Distribution::Unverified) => "-unverified",
    };
    let extension = match format {
        Format::Zip => "zip",
        Format::Dmg => "dmg",
        Format::Nsis => "exe",
        Format::TarGz => "tar.gz",
        Format::Deb => "deb",
    };
    let installer = if format == Format::Nsis { "-Setup" } else { "" };
    format!("Snip2Pin-{version}-{target}{suffix}{installer}.{extension}")
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
        if !fs::symlink_metadata(&path)?.file_type().is_file() {
            return Err("package manifest must be a regular file".into());
        }
        let data: Manifest = serde_json::from_slice(&fs::read(path)?)?;
        data.validate()?;
        if data.target != target {
            return Err("package target mismatch".into());
        }
        for artifact in &data.artifacts {
            if !fs::symlink_metadata(directory.join(&artifact.file))?
                .file_type()
                .is_file()
                || digest(&directory.join(&artifact.file))? != artifact.sha256
            {
                return Err("package checksum or file type mismatch".into());
            }
        }
        manifests.push(data);
    }
    if manifests.is_empty() {
        return Err("no package manifests".into());
    }
    Ok(manifests)
}

fn binary_build_command(target: &str, profile: &str) -> Result<Command> {
    if !TARGETS.contains(&target) || !["debug", "release"].contains(&profile) {
        return Err("unsupported target/profile".into());
    }
    let mut build = Command::new("cargo");
    build.args([
        "build",
        "--locked",
        "--package",
        "snip2pin",
        "--bin",
        "snip2pin",
        "--target",
        target,
    ]);
    if profile == "release" {
        build.arg("--release");
    }
    Ok(build)
}

/// Build and verify only the native application executable, without packaging.
pub fn build_binary(target: &str, profile: &str) -> Result<PathBuf> {
    let mut build = binary_build_command(target, profile)?;
    native_host(target)?;
    execute(&mut build)?;
    let executable = if target.contains("windows") {
        "snip2pin.exe"
    } else {
        "snip2pin"
    };
    let binary = PathBuf::from("target")
        .join(target)
        .join(profile)
        .join(executable);
    verify_version(&binary, &version()?)?;
    Ok(binary)
}

/// Construct the installed files without archiving, signing, or publishing them.
pub fn prepare_payload(binary: &Path, target: &str, payload: &Path) -> Result {
    if !TARGETS.contains(&target) {
        return Err("unsupported payload target".into());
    }
    fs::create_dir_all(payload)?;
    let executable = binary.file_name().ok_or("missing executable filename")?;
    if target.contains("apple") {
        macos_bundle(binary, &payload.join("Snip2Pin.app"), "line")?;
    } else {
        crate::notices::write(&payload.join("THIRD_PARTY_NOTICES.txt"))?;
        fs::copy(binary, payload.join(executable))?;
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
        }
    }
    Ok(())
}

pub fn build(target: &str, profile: &str) -> Result {
    let version = version()?;
    crate::release::validate_inputs(&version, &revision()?)?;
    let binary = build_binary(target, profile)?;
    let base = PathBuf::from(".build/packages").join(profile).join(target);
    if base.exists() {
        fs::remove_dir_all(&base)?;
    }
    let payload = base.join("payload");
    prepare_payload(&binary, target, &payload)?;
    let archive = base.join(candidate_filename(&version, target));
    if target.contains("apple") {
        execute(
            Command::new("ditto")
                .args(["-c", "-k", "--sequesterRsrc", "--keepParent"])
                .arg(payload.join("Snip2Pin.app"))
                .arg(&archive),
        )?;
        execute(Command::new("unzip").arg("-tq").arg(&archive))?;
    } else if target.contains("linux") {
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
    crate::sizes::candidate(target, profile, &binary, &payload, &archive)?;
    write_manifest(
        &base,
        &Manifest {
            schema: 3,
            version,
            revision: revision()?,
            target: target.into(),
            final_package: false,
            distribution: None,
            artifacts: vec![Artifact {
                format: formats(target, false)[0],
                file: archive
                    .file_name()
                    .ok_or("archive name")?
                    .to_string_lossy()
                    .into(),
                sha256: digest(&archive)?,
            }],
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
    crate::notices::write(&resources.join("THIRD_PARTY_NOTICES.txt"))?;
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

pub fn finalize(candidate: &Path, destination: &Path, distribution: Distribution) -> Result {
    let data = verify(candidate)?;
    if data.len() != 1 || data[0].final_package {
        return Err("expected one unsigned candidate".into());
    }
    let mut data = data.into_iter().next().ok_or("missing candidate")?;
    let candidate_hash = data
        .artifacts
        .first()
        .ok_or("missing candidate archive")?
        .sha256
        .clone();
    let artifacts = formats(&data.target, true)
        .iter()
        .map(|format| {
            let file = artifact_filename(&data.version, &data.target, Some(distribution), *format);
            let path = destination.join(&file);
            if !fs::symlink_metadata(&path)?.file_type().is_file() {
                return Err("final artifact must be a regular file".into());
            }
            crate::sizes::final_artifact(&data.target, &file, &path)?;
            Ok(Artifact {
                format: *format,
                sha256: digest(&path)?,
                file,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    data.final_package = true;
    data.distribution = Some(distribution);
    data.artifacts = artifacts;
    data.candidate_sha256 = Some(candidate_hash);
    write_manifest(destination, &data)
}

pub fn prepare_unverified(candidate: &Path, destination: &Path, target: &str) -> Result {
    let data = crate::release::verify_candidate(candidate, target)?;
    native_host(target)?;
    fs::create_dir_all(destination)?;
    for format in formats(target, true) {
        if destination
            .join(artifact_filename(
                &data.version,
                target,
                Some(Distribution::Unverified),
                *format,
            ))
            .exists()
        {
            return Err("refusing to replace an existing package".into());
        }
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
                .arg(candidate.join(&data.artifact(Format::Zip)?.file))
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
        let archive = destination.join(artifact_filename(
            &data.version,
            target,
            Some(Distribution::Unverified),
            Format::Dmg,
        ));
        create_dmg(&app, &archive)?;
        fs::remove_dir_all(unpacked)?;
    } else if target.contains("windows") {
        execute(
            Command::new("pwsh")
                .args([
                    "-NoProfile",
                    "-File",
                    "packaging/windows/distribute.ps1",
                    "-Candidate",
                ])
                .arg(candidate)
                .arg("-OutputDirectory")
                .arg(destination)
                .args(["-Distribution", "unverified"]),
        )?;
        return Ok(());
    } else {
        prepare_linux(&data, candidate, destination, Distribution::Unverified)?;
    }
    finalize(candidate, destination, Distribution::Unverified)
}

pub fn create_dmg(app: &Path, destination: &Path) -> Result {
    if destination.exists() {
        return Err("refusing to replace a disk image".into());
    }
    let staging = tempfile::tempdir()?;
    execute(
        Command::new("ditto")
            .arg(app)
            .arg(staging.path().join("Snip2Pin.app")),
    )?;
    #[cfg(unix)]
    std::os::unix::fs::symlink("/Applications", staging.path().join("Applications"))?;
    // A transport image must not inherit the host filesystem. APFS image
    // auto-sizing can exhaust the image while copying a real application.
    execute(
        Command::new("hdiutil")
            .args([
                "create",
                "-fs",
                "HFS+",
                "-format",
                "UDZO",
                "-volname",
                "Snip2Pin",
                "-srcfolder",
            ])
            .arg(staging.path())
            .arg(destination),
    )?;
    Ok(())
}

pub fn prepare_linux(
    data: &Manifest,
    candidate: &Path,
    destination: &Path,
    distribution: Distribution,
) -> Result {
    native_host(&data.target)?;
    let archive = destination.join(artifact_filename(
        &data.version,
        &data.target,
        Some(distribution),
        Format::TarGz,
    ));
    if archive.exists() {
        return Err("refusing to replace an archive".into());
    }
    fs::copy(
        candidate.join(&data.artifact(Format::TarGz)?.file),
        &archive,
    )?;
    let unpacked = PathBuf::from(".build/debian").join(&data.target);
    if unpacked.exists() {
        fs::remove_dir_all(&unpacked)?;
    }
    fs::create_dir_all(&unpacked)?;
    execute(
        Command::new("tar")
            .arg("-xzf")
            .arg(&archive)
            .arg("-C")
            .arg(&unpacked),
    )?;
    crate::installers::debian(&unpacked, destination, data, distribution)?;
    fs::remove_dir_all(unpacked)?;
    Ok(())
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
    if !target.contains("windows") {
        fs::create_dir_all(&unpacked)?;
    }
    let archive = directory
        .join(
            &data
                .artifact(if target.contains("apple") {
                    Format::Dmg
                } else if target.contains("windows") {
                    Format::Zip
                } else {
                    Format::TarGz
                })?
                .file,
        )
        .canonicalize()?;
    let mut mounted = None;
    if target.contains("apple") {
        if distribution == Distribution::Signed {
            execute(Command::new("codesign").args(["--verify", "--strict", "-R", "=anchor apple generic and certificate leaf[field.1.2.840.113635.100.6.1.13] exists"]).arg(&archive))?;
            execute(
                Command::new("xcrun")
                    .args(["stapler", "validate"])
                    .arg(&archive),
            )?;
            execute(
                Command::new("spctl")
                    .args([
                        "--assess",
                        "--type",
                        "open",
                        "--context",
                        "context:primary-signature",
                    ])
                    .arg(&archive),
            )?;
        }
        execute(
            Command::new("hdiutil")
                .args(["attach", "-readonly", "-nobrowse", "-mountpoint"])
                .arg(&unpacked)
                .arg(&archive),
        )?;
        mounted = Some(MountedImage(unpacked.clone()));
        let app = unpacked.join("Snip2Pin.app");
        if fs::read_link(unpacked.join("Applications"))? != Path::new("/Applications") {
            return Err("disk image Applications link mismatch".into());
        }
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
        for file in ["LICENSE", "THIRD_PARTY_NOTICES.txt"] {
            let metadata = fs::symlink_metadata(app.join("Contents/Resources").join(file))?;
            if !metadata.is_file() || metadata.len() == 0 {
                return Err("macOS package is missing its license notices".into());
            }
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
                .arg("-Installer")
                .arg(
                    directory
                        .join(&data.artifact(Format::Nsis)?.file)
                        .canonicalize()?,
                )
                .arg("-Destination")
                .arg(&unpacked)
                .arg("-InstallSmoke")
                .arg("-Distribution")
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
        crate::installers::verify_debian(directory, &unpacked, &data)?;
    }
    if let Some(image) = mounted {
        image.detach()?;
    }
    fs::remove_dir_all(unpacked)?;
    Ok(())
}

struct MountedImage(PathBuf);

impl MountedImage {
    fn detach(mut self) -> Result {
        execute(Command::new("hdiutil").arg("detach").arg(&self.0))?;
        self.0.clear();
        Ok(())
    }
}

impl Drop for MountedImage {
    fn drop(&mut self) {
        if !self.0.as_os_str().is_empty() {
            let _ = Command::new("hdiutil").arg("detach").arg(&self.0).status();
        }
    }
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
    fn release_size_builds_only_the_locked_application_binary() {
        for target in TARGETS {
            let command = binary_build_command(target, "release").expect("native build command");
            assert_eq!(command.get_program(), "cargo");
            assert_eq!(
                command.get_args().collect::<Vec<_>>(),
                [
                    "build",
                    "--locked",
                    "--package",
                    "snip2pin",
                    "--bin",
                    "snip2pin",
                    "--target",
                    target,
                    "--release"
                ]
            );
        }
        assert!(binary_build_command("unknown", "release").is_err());
        assert!(binary_build_command(TARGETS[0], "custom").is_err());
    }

    fn final_fixture(target: &str) -> Manifest {
        Manifest {
            schema: 3,
            version: "0.2.0".into(),
            revision: "a".repeat(40),
            target: target.into(),
            final_package: true,
            distribution: Some(Distribution::Unverified),
            candidate_sha256: Some("c".repeat(64)),
            artifacts: formats(target, true)
                .iter()
                .map(|format| Artifact {
                    format: *format,
                    file: artifact_filename(
                        "0.2.0",
                        target,
                        Some(Distribution::Unverified),
                        *format,
                    ),
                    sha256: "b".repeat(64),
                })
                .collect(),
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn disk_image_contains_generated_app_and_applications_link_without_opening_finder() {
        let directory = tempfile::tempdir().expect("disk image fixture");
        let app = directory.path().join("fixture.app");
        fs::create_dir_all(app.join("Contents")).expect("fixture app");
        fs::write(app.join("Contents/payload"), b"inert fixture").expect("payload");
        let image = directory.path().join("fixture.dmg");
        create_dmg(&app, &image).expect("create generated disk image");
        assert!(create_dmg(&app, &image).is_err());
        let mount = directory.path().join("mount");
        fs::create_dir(&mount).expect("mount directory");
        execute(
            Command::new("hdiutil")
                .args(["attach", "-readonly", "-nobrowse", "-mountpoint"])
                .arg(&mount)
                .arg(&image),
        )
        .expect("read-only fixture mount");
        let mounted = MountedImage(mount.clone());
        assert_eq!(
            fs::read(mount.join("Snip2Pin.app/Contents/payload")).expect("mounted payload"),
            b"inert fixture"
        );
        assert_eq!(
            fs::read_link(mount.join("Applications")).expect("Applications link"),
            Path::new("/Applications")
        );
        mounted.detach().expect("detach fixture image");
    }

    #[test]
    fn final_manifests_require_each_native_format_exactly_once() {
        for target in TARGETS {
            let valid = final_fixture(target);
            valid.validate().expect("complete native formats");
            let mut missing = final_fixture(target);
            missing.artifacts.pop();
            assert!(missing.validate().is_err());
            let mut extra = final_fixture(target);
            extra.artifacts.push(Artifact {
                format: Format::Dmg,
                file: "extra.dmg".into(),
                sha256: "b".repeat(64),
            });
            assert!(extra.validate().is_err());
            let mut wrong = final_fixture(target);
            wrong.artifacts[0].format = Format::Nsis;
            assert!(wrong.validate().is_err());
            let mut bad_hash = final_fixture(target);
            bad_hash.artifacts[0].sha256 = "z".repeat(64);
            assert!(bad_hash.validate().is_err());
            let mut bad_source = final_fixture(target);
            bad_source.revision = "main".into();
            assert!(bad_source.validate().is_err());
            if valid.artifacts.len() > 1 {
                let mut duplicate = final_fixture(target);
                duplicate.artifacts[1].format = duplicate.artifacts[0].format;
                assert!(duplicate.validate().is_err());
            }
        }
    }

    #[test]
    fn verification_checks_every_final_artifact() {
        let directory = tempfile::tempdir().expect("fixture");
        let mut data = final_fixture(TARGETS[2]);
        for artifact in &mut data.artifacts {
            let path = directory.path().join(&artifact.file);
            fs::write(&path, artifact.file.as_bytes()).expect("artifact");
            artifact.sha256 = digest(&path).expect("digest");
        }
        write_manifest(directory.path(), &data).expect("manifest");
        verify(directory.path()).expect("all valid");
        let installer = directory
            .path()
            .join(&data.artifact(Format::Nsis).expect("installer").file);
        fs::write(&installer, b"tampered").expect("tamper");
        assert!(verify(directory.path()).is_err());
        fs::remove_file(&installer).expect("remove installer");
        assert!(verify(directory.path()).is_err());
        #[cfg(unix)]
        {
            let target = directory.path().join("outside.exe");
            fs::write(&target, b"payload").expect("external artifact");
            std::os::unix::fs::symlink(target, installer).expect("symlink artifact");
            assert!(verify(directory.path()).is_err());
        }
    }

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
            schema: 3,
            version: "0.2.0".into(),
            revision: "a".repeat(40),
            target: TARGETS[0].into(),
            final_package: true,
            distribution: Some(Distribution::Signed),
            artifacts: vec![Artifact {
                format: Format::Dmg,
                file: artifact_filename(
                    "0.2.0",
                    TARGETS[0],
                    Some(Distribution::Signed),
                    Format::Dmg,
                ),
                sha256: "b".repeat(64),
            }],
            candidate_sha256: Some("c".repeat(64)),
        };
        assert!(data.validate().is_ok());
        data.artifacts[0].file = "../secret.key".into();
        assert!(data.validate().is_err());
        data.artifacts[0].file =
            artifact_filename("0.2.0", TARGETS[0], Some(Distribution::Signed), Format::Dmg);
        data.target = "unknown".into();
        assert!(data.validate().is_err());
    }

    #[test]
    fn distribution_state_and_filename_cannot_be_silently_relabelled() {
        let mut data = Manifest {
            schema: 3,
            version: "0.2.0".into(),
            revision: "a".repeat(40),
            target: TARGETS[0].into(),
            final_package: true,
            distribution: Some(Distribution::Unverified),
            artifacts: vec![Artifact {
                format: Format::Dmg,
                file: artifact_filename(
                    "0.2.0",
                    TARGETS[0],
                    Some(Distribution::Unverified),
                    Format::Dmg,
                ),
                sha256: "b".repeat(64),
            }],
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
        let file = candidate_filename("0.2.0", TARGETS[3]);
        fs::write(candidate.join(&file), b"original").expect("payload");
        let data = Manifest {
            schema: 3,
            version: "0.2.0".into(),
            revision: "a".repeat(40),
            target: TARGETS[3].into(),
            final_package: false,
            distribution: None,
            artifacts: vec![Artifact {
                format: Format::TarGz,
                sha256: digest(&candidate.join(&file)).expect("candidate digest"),
                file,
            }],
            candidate_sha256: None,
        };
        write_manifest(&candidate, &data).expect("manifest");
        assert_eq!(verify(&candidate).expect("valid").len(), 1);
        assert!(finalize(&candidate, &final_dir, Distribution::Signed).is_err());
        assert!(!final_dir.join(format!("{}.json", TARGETS[3])).exists());
        for format in formats(TARGETS[3], true) {
            let archive = final_dir.join(artifact_filename(
                "0.2.0",
                TARGETS[3],
                Some(Distribution::Signed),
                *format,
            ));
            fs::write(&archive, b"signed bytes").expect("final payload");
        }
        finalize(&candidate, &final_dir, Distribution::Signed).expect("finalize");
        let final_data = verify(&final_dir).expect("final manifest");
        assert_eq!(
            final_data[0].candidate_sha256.as_deref(),
            Some(data.artifacts[0].sha256.as_str())
        );
        assert_ne!(final_data[0].artifacts[0].sha256, data.artifacts[0].sha256);
        fs::write(candidate.join(&data.artifacts[0].file), b"tampered").expect("tamper");
        assert!(verify(&candidate).is_err());
    }
}

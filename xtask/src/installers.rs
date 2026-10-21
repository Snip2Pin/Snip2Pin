//! Small native format adapters; all files derive from the verified candidate.
use crate::package::{self, Distribution, Format, Manifest};
use crate::{Result, execute, output};
use std::{fs, path::Path, process::Command};

pub fn debian(
    payload: &Path,
    destination: &Path,
    data: &Manifest,
    distribution: Distribution,
) -> Result {
    let root = payload.join("deb-root");
    for directory in [
        "DEBIAN",
        "usr/bin",
        "usr/share/applications",
        "usr/share/icons/hicolor/1024x1024/apps",
        "usr/share/doc/snip2pin",
    ] {
        fs::create_dir_all(root.join(directory))?;
    }
    fs::copy(payload.join("snip2pin"), root.join("usr/bin/snip2pin"))?;
    fs::copy(
        payload.join("io.github.Snip2Pin.Snip2Pin.desktop"),
        root.join("usr/share/applications/io.github.Snip2Pin.Snip2Pin.desktop"),
    )?;
    fs::copy(
        payload.join("io.github.Snip2Pin.Snip2Pin.png"),
        root.join("usr/share/icons/hicolor/1024x1024/apps/io.github.Snip2Pin.Snip2Pin.png"),
    )?;
    for file in ["LICENSE", "README.txt", "THIRD_PARTY_NOTICES.txt"] {
        fs::copy(
            payload.join(file),
            root.join("usr/share/doc/snip2pin").join(file),
        )?;
    }
    // dpkg-shlibdeps consults this source-package description when mapping ELF
    // dependencies to the native Ubuntu 24.04 package database.
    fs::create_dir_all(payload.join("debian"))?;
    fs::write(
        payload.join("debian/control"),
        "Source: snip2pin\nSection: graphics\nPriority: optional\nMaintainer: Snip2Pin contributors <noreply@github.com>\n\nPackage: snip2pin\nArchitecture: amd64\nDescription: Capture a region and keep it floating on screen\n",
    )?;
    let dependencies = output(
        Command::new("dpkg-shlibdeps")
            .current_dir(payload)
            .arg("-O")
            .arg("-e")
            .arg(payload.join("snip2pin").canonicalize()?),
    )?;
    let dependencies = dependencies
        .strip_prefix("shlibs:Depends=")
        .ok_or("missing Debian ELF dependencies")?;
    // These libraries are loaded at runtime and cannot be inferred from ELF NEEDED.
    let depends = format!(
        "{dependencies}, libegl1, libgl1, libxkbcommon0, libxkbcommon-x11-0, libwayland-client0, libxcb1, libx11-6, libxcursor1, libxi6, libxrandr2, libdbus-1-3, libfontconfig1"
    );
    fs::write(
        root.join("DEBIAN/control"),
        format!(
            "Package: snip2pin\nVersion: {}\nArchitecture: amd64\nMaintainer: Snip2Pin contributors <noreply@github.com>\nSection: graphics\nPriority: optional\nDepends: {depends}\nRecommends: fonts-wqy-microhei | fonts-noto-cjk\nDescription: Capture a region and keep it floating on screen\n Built on Ubuntu 24.04; other Debian derivatives require native acceptance.\n",
            data.version
        ),
    )?;
    let file = destination.join(package::artifact_filename(
        &data.version,
        &data.target,
        Some(distribution),
        Format::Deb,
    ));
    if file.exists() {
        return Err("refusing to replace a Debian package".into());
    }
    execute(
        Command::new("dpkg-deb")
            .args(["--build", "--root-owner-group"])
            .arg(root)
            .arg(file),
    )
}

pub fn verify_debian(directory: &Path, unpacked: &Path, data: &Manifest) -> Result {
    let deb = directory.join(&data.artifact(Format::Deb)?.file);
    for (field, expected) in [
        ("Package", "snip2pin"),
        ("Version", data.version.as_str()),
        ("Architecture", "amd64"),
    ] {
        if output(Command::new("dpkg-deb").arg("--field").arg(&deb).arg(field))? != expected {
            return Err("Debian package metadata mismatch".into());
        }
    }
    if output(
        Command::new("dpkg-deb")
            .arg("--field")
            .arg(&deb)
            .arg("Depends"),
    )?
    .is_empty()
    {
        return Err("Debian package has no runtime dependencies".into());
    }
    let extracted = unpacked.join("deb-verified");
    execute(
        Command::new("dpkg-deb")
            .arg("--extract")
            .arg(deb)
            .arg(&extracted),
    )?;
    for (portable, installed) in [
        ("snip2pin", "usr/bin/snip2pin"),
        (
            "io.github.Snip2Pin.Snip2Pin.desktop",
            "usr/share/applications/io.github.Snip2Pin.Snip2Pin.desktop",
        ),
        (
            "io.github.Snip2Pin.Snip2Pin.png",
            "usr/share/icons/hicolor/1024x1024/apps/io.github.Snip2Pin.Snip2Pin.png",
        ),
        ("LICENSE", "usr/share/doc/snip2pin/LICENSE"),
        ("README.txt", "usr/share/doc/snip2pin/README.txt"),
        (
            "THIRD_PARTY_NOTICES.txt",
            "usr/share/doc/snip2pin/THIRD_PARTY_NOTICES.txt",
        ),
    ] {
        if package::digest(&unpacked.join(portable))?
            != package::digest(&extracted.join(installed))?
        {
            return Err("Debian package differs from the verified portable payload".into());
        }
    }
    Ok(())
}

pub fn aur(destination: &Path, version: &str, url: &str, checksum: &str) -> Result {
    fs::create_dir(destination)?;
    fs::write(
        destination.join("PKGBUILD"),
        aur_build(version, url, checksum),
    )?;
    let info = output(
        Command::new("makepkg")
            .arg("--printsrcinfo")
            .current_dir(destination),
    )?;
    fs::write(destination.join(".SRCINFO"), format!("{info}\n"))?;
    Ok(())
}

fn aur_build(version: &str, url: &str, checksum: &str) -> String {
    format!(
        r#"# Maintainer: Snip2Pin contributors
pkgname=snip2pin-bin
pkgver={version}
pkgrel=1
pkgdesc='Capture a region and keep it floating on screen'
arch=('x86_64')
url='https://github.com/Snip2Pin/Snip2Pin'
license=('MIT')
depends=('glibc' 'gcc-libs' 'libxcb' 'libx11' 'libxcursor' 'libxi' 'libxrandr' 'dbus' 'fontconfig' 'libxkbcommon' 'libxkbcommon-x11' 'wayland' 'libglvnd')
optdepends=('xdg-desktop-portal: screen capture under Wayland; install a portal backend for your desktop'
            'wqy-microhei: Chinese UI font when no Noto CJK font is installed')
provides=('snip2pin')
conflicts=('snip2pin')
source=('snip2pin.tar.gz::{url}')
sha256sums=('{checksum}')

package() {{
  install -Dm755 "$srcdir/snip2pin" "$pkgdir/usr/bin/snip2pin"
  install -Dm644 "$srcdir/io.github.Snip2Pin.Snip2Pin.desktop" "$pkgdir/usr/share/applications/io.github.Snip2Pin.Snip2Pin.desktop"
  install -Dm644 "$srcdir/io.github.Snip2Pin.Snip2Pin.png" "$pkgdir/usr/share/icons/hicolor/1024x1024/apps/io.github.Snip2Pin.Snip2Pin.png"
  install -Dm644 "$srcdir/LICENSE" "$pkgdir/usr/share/licenses/$pkgname/LICENSE"
  install -Dm644 "$srcdir/THIRD_PARTY_NOTICES.txt" "$pkgdir/usr/share/doc/$pkgname/THIRD_PARTY_NOTICES.txt"
}}
"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "linux")]
    #[test]
    #[ignore = "requires native Debian packaging tools; run explicitly in Ubuntu CI"]
    fn debian_fixture_preserves_payload_and_rejects_content_changes() {
        let directory = tempfile::tempdir().expect("Debian fixture");
        let payload = directory.path().join("payload");
        let destination = directory.path().join("final");
        fs::create_dir(&payload).expect("payload directory");
        fs::create_dir(&destination).expect("final directory");
        fs::copy("/bin/true", payload.join("snip2pin")).expect("inert native ELF fixture");
        for name in [
            "LICENSE",
            "README.txt",
            "THIRD_PARTY_NOTICES.txt",
            "io.github.Snip2Pin.Snip2Pin.desktop",
            "io.github.Snip2Pin.Snip2Pin.png",
        ] {
            fs::write(payload.join(name), format!("fixture {name}")).expect("fixture resource");
        }
        let data = Manifest {
            schema: 3,
            version: "0.2.0".into(),
            revision: "a".repeat(40),
            target: "x86_64-unknown-linux-gnu".into(),
            final_package: true,
            distribution: Some(Distribution::Unverified),
            candidate_sha256: Some("b".repeat(64)),
            artifacts: vec![package::Artifact {
                format: Format::Deb,
                file: package::artifact_filename(
                    "0.2.0",
                    "x86_64-unknown-linux-gnu",
                    Some(Distribution::Unverified),
                    Format::Deb,
                ),
                sha256: String::new(),
            }],
        };
        debian(&payload, &destination, &data, Distribution::Unverified)
            .expect("build fixture Debian package");
        verify_debian(&destination, &payload, &data).expect("verify fixture metadata and content");
        fs::remove_dir_all(payload.join("deb-verified")).expect("clear extraction");
        fs::write(payload.join("LICENSE"), b"changed").expect("mutate comparison payload");
        assert!(verify_debian(&destination, &payload, &data).is_err());
    }

    #[test]
    fn arch_recipe_binds_version_url_and_checksum_without_install_hooks() {
        let recipe = aur_build(
            "0.2.0",
            "https://github.com/Snip2Pin/Snip2Pin/releases/download/v0.2.0/package.tar.gz",
            &"a".repeat(64),
        );
        assert!(recipe.contains("pkgver=0.2.0"));
        assert!(recipe.contains("/releases/download/v0.2.0/package.tar.gz"));
        assert!(recipe.contains(&format!("sha256sums=('{}')", "a".repeat(64))));
        assert!(!recipe.contains("SKIP"));
        assert!(!recipe.contains("install="));
        assert!(recipe.contains("provides=('snip2pin')"));
    }
}

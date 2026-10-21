//! Measured package sizes, kept separate from signed provenance metadata.
use crate::Result;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

const MIB: u64 = 1024 * 1024;
const MAC_ARM_BINARY_LIMIT: u64 = 10 * MIB;
const MAC_ARM_PAYLOAD_LIMIT: u64 = 12 * MIB;

pub fn candidate(
    target: &str,
    profile: &str,
    binary: &Path,
    payload: &Path,
    archive: &Path,
) -> Result {
    let binary_bytes = regular_file_bytes(binary)?;
    let payload_bytes = payload_bytes(payload)?;
    report(target, &format!("{profile} executable"), binary_bytes)?;
    report(
        target,
        &format!("{profile} payload (regular files)"),
        payload_bytes,
    )?;
    report(
        target,
        &format!("{profile} candidate archive"),
        regular_file_bytes(archive)?,
    )?;
    check_budget(target, profile, binary_bytes, payload_bytes)
}

pub fn final_artifact(target: &str, file: &str, path: &Path) -> Result {
    report(target, file, regular_file_bytes(path)?)
}

fn regular_file_bytes(path: &Path) -> Result<u64> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file() {
        return Err("size measurement requires a regular file".into());
    }
    Ok(metadata.len())
}

fn payload_bytes(path: &Path) -> Result<u64> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.is_file() {
        return Ok(metadata.len());
    }
    if !metadata.is_dir() {
        return Ok(0);
    }
    let mut bytes = 0_u64;
    for entry in fs::read_dir(path)? {
        bytes = bytes
            .checked_add(payload_bytes(&entry?.path())?)
            .ok_or("payload size overflow")?;
    }
    Ok(bytes)
}

fn check_budget(target: &str, profile: &str, binary: u64, payload: u64) -> Result {
    // Native release baselines are required before adding other target budgets.
    if target == "aarch64-apple-darwin" && profile == "release" {
        for (name, bytes, limit) in [
            ("executable", binary, MAC_ARM_BINARY_LIMIT),
            ("payload", payload, MAC_ARM_PAYLOAD_LIMIT),
        ] {
            if bytes > limit {
                return Err(format!(
                    "{target} release {name} size {bytes} bytes exceeds budget {limit} bytes"
                )
                .into());
            }
        }
    }
    Ok(())
}

fn report(target: &str, name: &str, bytes: u64) -> Result {
    println!(
        "Size: {target} {name}: {bytes} bytes ({:.2} MiB)",
        bytes as f64 / MIB as f64
    );
    if let Some(path) = std::env::var_os("GITHUB_STEP_SUMMARY") {
        let mut summary = OpenOptions::new().create(true).append(true).open(path)?;
        writeln!(
            summary,
            "\n- `{target}` {name}: **{bytes} bytes** ({:.2} MiB)",
            bytes as f64 / MIB as f64
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payload_sums_nested_regular_files() {
        let directory = tempfile::tempdir().expect("temporary payload");
        fs::create_dir(directory.path().join("nested")).expect("directory");
        fs::write(directory.path().join("first"), b"123").expect("file");
        fs::write(directory.path().join("nested/second"), b"45678").expect("file");
        assert_eq!(payload_bytes(directory.path()).expect("size"), 8);
    }

    #[cfg(unix)]
    #[test]
    fn payload_does_not_follow_file_directory_or_broken_symlinks() {
        let directory = tempfile::tempdir().expect("temporary payload");
        let external = tempfile::tempdir().expect("external files");
        fs::write(external.path().join("outside"), b"not payload").expect("file");
        for (source, name) in [
            (external.path().to_path_buf(), "directory"),
            (external.path().join("outside"), "file"),
            (external.path().join("missing"), "broken"),
            (directory.path().to_path_buf(), "cycle"),
        ] {
            std::os::unix::fs::symlink(source, directory.path().join(name)).expect("symlink");
        }
        assert_eq!(payload_bytes(directory.path()).expect("size"), 0);
        assert!(regular_file_bytes(&directory.path().join("file")).is_err());
    }

    #[test]
    fn release_budget_accepts_exact_limits_and_rejects_each_excess() {
        let target = "aarch64-apple-darwin";
        assert!(
            check_budget(
                target,
                "release",
                MAC_ARM_BINARY_LIMIT,
                MAC_ARM_PAYLOAD_LIMIT
            )
            .is_ok()
        );
        assert!(check_budget(target, "release", MAC_ARM_BINARY_LIMIT + 1, 0).is_err());
        assert!(check_budget(target, "release", 0, MAC_ARM_PAYLOAD_LIMIT + 1).is_err());
    }

    #[test]
    fn debug_and_targets_without_native_baselines_only_report() {
        assert!(check_budget("aarch64-apple-darwin", "debug", u64::MAX, u64::MAX).is_ok());
        for target in [
            "x86_64-apple-darwin",
            "x86_64-pc-windows-msvc",
            "x86_64-unknown-linux-gnu",
        ] {
            assert!(check_budget(target, "release", u64::MAX, u64::MAX).is_ok());
        }
    }
}

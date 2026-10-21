//! Measured package sizes, kept separate from signed provenance metadata.
use crate::Result;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

const MIB: u64 = 1024 * 1024;
// Native release baselines are recorded in docs/workflow.md. Limits include
// room for notices and platform metadata without hiding dependency growth.
const RELEASE_LIMITS: &[(&str, u64, u64)] = &[
    ("aarch64-apple-darwin", 10 * MIB, 12 * MIB),
    ("x86_64-apple-darwin", 12 * MIB, 14 * MIB),
    ("x86_64-pc-windows-msvc", 12 * MIB, 14 * MIB),
    ("x86_64-unknown-linux-gnu", 20 * MIB, 22 * MIB),
];

pub fn check_release(target: &str) -> Result {
    let binary = crate::package::build_binary(target, "release")?;
    let payload = tempfile::tempdir()?;
    crate::package::prepare_payload(&binary, target, payload.path())?;
    let binary_bytes = regular_file_bytes(&binary)?;
    let payload_bytes = payload_bytes(payload.path())?;
    report(target, "release executable", binary_bytes)?;
    report(target, "release payload (regular files)", payload_bytes)?;
    check_budget(target, "release", binary_bytes, payload_bytes)
}

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
    if profile != "release" {
        return Ok(());
    }
    let (_, binary_limit, payload_limit) = RELEASE_LIMITS
        .iter()
        .find(|(supported, _, _)| *supported == target)
        .ok_or("release target has no reviewed size budget")?;
    for (name, bytes, limit) in [
        ("executable", binary, binary_limit),
        ("payload", payload, payload_limit),
    ] {
        if bytes > *limit {
            return Err(format!(
                "{target} release {name} size {bytes} bytes exceeds budget {limit} bytes"
            )
            .into());
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
        for &(target, binary, payload) in RELEASE_LIMITS {
            assert!(check_budget(target, "release", binary, payload).is_ok());
            assert!(check_budget(target, "release", binary + 1, 0).is_err());
            assert!(check_budget(target, "release", 0, payload + 1).is_err());
        }
    }

    #[test]
    fn debug_is_ungated_and_new_release_targets_require_a_budget() {
        for &(target, _, _) in RELEASE_LIMITS {
            assert!(check_budget(target, "debug", u64::MAX, u64::MAX).is_ok());
        }
        assert!(check_budget("unreviewed-target", "release", 0, 0).is_err());
        assert_eq!(RELEASE_LIMITS.len(), crate::package::TARGETS.len());
        for target in crate::package::TARGETS {
            assert!(check_budget(target, "release", 0, 0).is_ok());
        }
    }
}

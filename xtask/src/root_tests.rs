use std::{fs, path::Path, process::Command};

fn workspace(directory: &Path) {
    fs::create_dir_all(directory.join("src")).expect("root source directory");
    fs::create_dir_all(directory.join("tools/checker/src")).expect("member source directory");
    fs::write(
        directory.join("Cargo.toml"),
        "[package]\nname = \"snip2pin\"\nversion = \"0.1.0\"\n[workspace]\nmembers = [\"tools/checker\"]\n",
    )
    .expect("workspace manifest");
    fs::write(directory.join("src/lib.rs"), "").expect("root source");
    fs::write(
        directory.join("tools/checker/Cargo.toml"),
        "[package]\nname = \"xtask\"\nversion = \"0.1.0\"\n",
    )
    .expect("member manifest");
    fs::write(directory.join("tools/checker/src/main.rs"), "fn main() {}").expect("member source");
}

#[test]
fn reused_binary_resolves_the_runtime_workspace_from_root_member_or_cargo_manifest() {
    const EXPECTED: &str = "SNIP2PIN_XTASK_TEST_EXPECTED_ROOT";
    if let Some(expected) = std::env::var_os(EXPECTED) {
        if expected.is_empty() {
            assert!(
                crate::root().is_err(),
                "invalid invocation must fail closed"
            );
        } else {
            let root = crate::root().expect("runtime workspace");
            assert_eq!(
                root.canonicalize().expect("resolved workspace identity"),
                Path::new(&expected)
            );
            std::env::set_current_dir(&root).expect("run command in workspace");
            assert_eq!(crate::root().expect("subsequent resource lookup"), root);
        }
        return;
    }
    let directory = tempfile::tempdir().expect("temporary workspaces");
    let first = directory.path().join("first tree");
    let second = directory.path().join("second tree");
    workspace(&first);
    workspace(&second);
    let unrelated = directory.path().join("unrelated");
    fs::create_dir(&unrelated).expect("unrelated workspace");
    fs::write(unrelated.join("Cargo.toml"), "[workspace]\nmembers = []\n")
        .expect("unrelated manifest");
    for (cwd, manifest, valid) in [
        (second.clone(), None, true),
        (second.join("tools/checker/src"), None, true),
        (first.clone(), Some(second.join("tools/checker")), true),
        (directory.path().to_owned(), None, false),
        (first.clone(), Some(second.join("missing")), false),
        (unrelated, None, false),
    ] {
        let mut command = Command::new(std::env::current_exe().expect("reused compiled test"));
        command
            .args([
                "--exact",
                "root_tests::reused_binary_resolves_the_runtime_workspace_from_root_member_or_cargo_manifest",
                "--nocapture",
            ])
            .current_dir(&cwd)
            .env(
                EXPECTED,
                if valid {
                    second.canonicalize().expect("runtime root")
                } else {
                    "".into()
                },
            )
            .env_remove("CARGO_MANIFEST_DIR");
        if let Some(manifest) = manifest {
            command.env("CARGO_MANIFEST_DIR", manifest);
        }
        let result = command.output().expect("run the same compiled artifact");
        assert!(
            result.status.success(),
            "cwd={cwd:?}: {}{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
    }
}

#[cfg(target_os = "windows")]
#[test]
fn workspace_root_allows_the_real_powershell_archive_adapter() {
    let root = crate::root().expect("runtime workspace");
    assert!(root.is_absolute());
    let directory = tempfile::tempdir().expect("isolated archive fixture");
    let payload = directory.path().join("payload with spaces");
    fs::create_dir(&payload).expect("fixture payload");
    fs::write(payload.join("snip2pin.exe"), b"archive fixture").expect("fixture executable");
    let archive = directory.path().join("fixture.zip");
    let result = Command::new("pwsh")
        .current_dir(&root)
        .args(["-NoProfile", "-File", "packaging/windows/archive.ps1"])
        .arg(&payload)
        .arg(&archive)
        .output()
        .expect("run the production archive adapter without policy overrides");
    assert!(
        result.status.success(),
        "archive adapter failed from {root:?}: {}{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(fs::metadata(archive).expect("archive output").len() > 0);
}

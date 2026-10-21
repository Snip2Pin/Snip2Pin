//! Credential lifecycle tests use dummy data and command stubs, never real stores.
#[cfg(target_os = "macos")]
#[test]
fn temporary_macos_credentials_are_cleaned_after_success_and_failure() {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::process::Command;
    let repository = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap();
    for failure in ["none", "import", "notarize", "distribute"] {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        for name in ["bin", "runner", "packaging/macos"] {
            fs::create_dir_all(root.join(name)).unwrap();
        }
        for (name, script) in [
            (
                "security",
                r#"#!/bin/bash
printf '%s\n' "$1" >> "$TEST_ROOT/actions"
case "$1" in
  create-keychain) touch "$4" ;;
  import) [[ "$FAILURE" != import ]] || exit 1 ;;
  find-identity) printf '%s\n' "$MACOS_CERTIFICATE_SHA1" ;;
esac
"#,
            ),
            (
                "xcrun",
                r#"#!/bin/bash
[[ "$FAILURE" != notarize ]]
"#,
            ),
            ("openssl", "#!/bin/bash\nprintf 'dummy-random-password'\n"),
        ] {
            let file = root.join("bin").join(name);
            fs::write(&file, script).unwrap();
            fs::set_permissions(file, fs::Permissions::from_mode(0o755)).unwrap();
        }
        fs::write(
            root.join("packaging/macos/distribute.sh"),
            r#"#!/bin/bash
test -f "$SNIP2PIN_SIGN_KEYCHAIN"
test -f "$(dirname "$SNIP2PIN_SIGN_KEYCHAIN")/notary.p8"
[[ "$FAILURE" != distribute ]]
"#,
        )
        .unwrap();
        let result = Command::new("bash")
            .arg(repository.join("packaging/macos/ci-sign.sh"))
            .args(["candidate", "output"])
            .current_dir(root)
            .env(
                "PATH",
                format!("{}:/usr/bin:/bin", root.join("bin").display()),
            )
            .env("TEST_ROOT", root)
            .env("RUNNER_TEMP", root.join("runner"))
            .env("FAILURE", failure)
            .env("GITHUB_ACTIONS", "true")
            .env("RUNNER_ENVIRONMENT", "github-hosted")
            .env("MACOS_CERTIFICATE_SHA1", "a".repeat(40))
            .env("MACOS_CERTIFICATE_P12_BASE64", "ZHVtbXk=")
            .env("MACOS_CERTIFICATE_PASSWORD", "dummy-p12-secret")
            .env("MACOS_NOTARY_KEY_P8", "dummy-notary-secret")
            .env("MACOS_NOTARY_KEY_ID", "dummy-id")
            .env("MACOS_NOTARY_ISSUER_ID", "dummy-issuer")
            .output()
            .unwrap();
        assert_eq!(
            result.status.success(),
            failure == "none",
            "{failure}: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(
            fs::read_dir(root.join("runner")).unwrap().count(),
            0,
            "{failure}"
        );
        assert!(
            fs::read_to_string(root.join("actions"))
                .unwrap()
                .contains("delete-keychain")
        );
        for stream in [result.stdout, result.stderr] {
            let log = String::from_utf8_lossy(&stream);
            assert!(!log.contains("dummy-p12-secret"));
            assert!(!log.contains("dummy-notary-secret"));
        }
    }
}

#[cfg(target_os = "windows")]
#[test]
fn windows_signing_scripts_parse_and_refuse_local_use() {
    use std::process::Command;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap();
    let result = Command::new("pwsh").args(["-NoProfile", "-Command", r#"
$ErrorActionPreference = 'Stop'
Get-ChildItem packaging/windows/*.ps1 | ForEach-Object {
    $errors = $null; $tokens = $null
    [System.Management.Automation.Language.Parser]::ParseFile($_.FullName, [ref]$tokens, [ref]$errors) | Out-Null
    if ($errors.Count -gt 0) { throw 'PowerShell syntax error' }
}
./packaging/windows/pure-tests.ps1
$env:GITHUB_ACTIONS = 'false'
try { ./packaging/windows/ci-sign.ps1 -Candidate unused -OutputDirectory unused }
catch { if ($_.Exception.Message -eq 'CI signing requires a GitHub-hosted runner') { exit 0 }; throw }
throw 'Signing unexpectedly accepted a local invocation'
"#]).current_dir(root).status().unwrap();
    assert!(result.success());
}

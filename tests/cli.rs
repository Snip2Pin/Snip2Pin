#[test]
fn version_query_exits_without_gui_or_desktop_access() {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_snip2pin"))
        .arg("--version")
        .output()
        .expect("run version query");
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).expect("UTF-8"),
        format!("{} {}\n", snip2pin::APP_NAME, env!("CARGO_PKG_VERSION"))
    );
    assert!(output.stderr.is_empty());
}

#[test]
fn identity_query_exits_without_gui_or_desktop_access() {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_snip2pin"))
        .args(["--version", "--app-id"])
        .output()
        .expect("run identity query");
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).expect("UTF-8"),
        format!("{}\n", snip2pin::APP_ID)
    );
    assert!(output.stderr.is_empty());
}

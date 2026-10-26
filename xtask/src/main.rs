//! Repository checks and packaging; no GUI, desktop capture, or credentials.
mod icons;
mod installers;
mod messages;
mod notices;
mod package;
mod preview;
mod release;
mod sizes;
// Keep xtask's tests inside the test binary. Integration-test targets make Cargo
// rebuild the normal executable, which Windows locks while `xtask check` runs.
#[cfg(test)]
mod ci_signing_tests;
#[cfg(test)]
mod root_tests;

use std::error::Error;
use std::path::{Path, PathBuf};
use std::process::Command;

type Result<T = ()> = std::result::Result<T, Box<dyn Error>>;

fn main() {
    if let Err(error) = run() {
        eprintln!("xtask: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result {
    std::env::set_current_dir(root()?)?;
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {
        ["check"] => check(),
        ["lint"] => lint(),
        ["check-release-size", target] => sizes::check_release(target),
        ["icons"] => icons::generate(),
        ["preview", font] => preview::local(Path::new(font)),
        ["preview-site"] => preview::site(),
        ["check-messages", base, head, title] => messages::check(base, head, title),
        ["candidate-check", version, revision, run_id] => release::check_candidates(version, revision, run_id),
        ["verify-candidate", directory, target] => release::verify_candidate(Path::new(directory), target).map(|_| ()),
        ["check-msrv"] => {
            let minimum = manifest()?["package"]["rust-version"].as_str().ok_or("missing rust-version")?.to_owned();
            execute(Command::new("rustup").args(["toolchain", "install", &minimum, "--profile", "minimal"]))?;
            execute(Command::new("cargo").args([&format!("+{minimum}"), "check", "--locked", "--workspace", "--all-targets"]))
        }
        ["package", target, profile] => package::build(target, profile),
        ["bundle-macos", binary, destination, variant] => {
            package::macos_bundle(Path::new(binary), Path::new(destination), variant)
        }
        ["verify", directory] => package::verify(Path::new(directory)).map(|_| ()),
        ["verify-native", directory, target, distribution] => package::verify_native(Path::new(directory), target, package::Distribution::parse(distribution)?),
        ["prepare-unverified", candidate, destination, target] => package::prepare_unverified(Path::new(candidate), Path::new(destination), target),
        ["finalize", candidate, destination, distribution] => package::finalize(Path::new(candidate), Path::new(destination), package::Distribution::parse(distribution)?),
        ["dmg", app, destination] => package::create_dmg(Path::new(app), Path::new(destination)),
        ["prepare-linux", candidate, destination, distribution] => {
            let distribution = package::Distribution::parse(distribution)?;
            let data = release::verify_candidate(Path::new(candidate), "x86_64-unknown-linux-gnu")?;
            std::fs::create_dir_all(destination)?;
            package::prepare_linux(&data, Path::new(candidate), Path::new(destination), distribution)?;
            package::finalize(Path::new(candidate), Path::new(destination), distribution)
        }
        ["aur-package", directory, destination] => release::aur_package(Path::new(directory), Path::new(destination)),
        ["release-check", version, revision] => release::preflight(version, revision),
        ["stage", version, revision, directory, notes, distribution, "--confirm"] => {
            release::stage(version, revision, Path::new(directory), Path::new(notes), package::Distribution::parse(distribution)?)
        }
        ["publish", version, revision, run_id, directory, distribution, "--confirm"] => {
            release::publish(version, revision, run_id, Path::new(directory), package::Distribution::parse(distribution)?)
        }
        _ => Err("usage: cargo xtask check | lint | icons | preview FONT | preview-site | check-release-size TARGET | check-msrv | check-messages BASE_SHA HEAD_SHA PR_TITLE | candidate-check VERSION SHA RUN | verify-candidate DIR TARGET | package TARGET debug|release | bundle-macos BINARY DEST line|metal | verify DIR | verify-native DIR TARGET signed|unverified | prepare-unverified CANDIDATE_DIR OUTPUT_DIR TARGET | prepare-linux CANDIDATE_DIR OUTPUT_DIR signed|unverified | dmg APP OUTPUT_DMG | finalize CANDIDATE_DIR OUTPUT_DIR signed|unverified | aur-package FINAL_DIR NEW_OUTPUT_DIR | release-check VERSION SHA | stage VERSION SHA DIR NOTES signed|unverified --confirm | publish VERSION SHA BUILD_RUN DIR signed|unverified --confirm".into()),
    }
}

fn root() -> Result<PathBuf> {
    // Cargo supplies this at runtime, including when a shared target directory
    // reuses an executable compiled in another worktree. Direct invocations use
    // their current directory instead. Never fall back to the build directory.
    let mut command = Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()));
    command.args(["locate-project", "--workspace", "--message-format", "plain"]);
    if let Some(directory) = std::env::var_os("CARGO_MANIFEST_DIR") {
        command
            .arg("--manifest-path")
            .arg(Path::new(&directory).join("Cargo.toml"));
    }
    let manifest = PathBuf::from(output(&mut command)?);
    let data: toml::Value = toml::from_str(&std::fs::read_to_string(&manifest)?)?;
    if !data.get("workspace").is_some_and(toml::Value::is_table)
        || data
            .get("package")
            .and_then(|package| package.get("name"))
            .and_then(toml::Value::as_str)
            != Some("snip2pin")
    {
        return Err("xtask must run in the Snip2Pin Cargo workspace".into());
    }
    Ok(manifest
        .parent()
        .ok_or("Cargo workspace manifest has no parent directory")?
        .canonicalize()?)
}

fn output(command: &mut Command) -> Result<String> {
    let result = command.output()?;
    if !result.status.success() {
        return Err(format!(
            "{command:?} failed: {}",
            String::from_utf8_lossy(&result.stderr)
        )
        .into());
    }
    Ok(String::from_utf8(result.stdout)?.trim().into())
}

fn execute(command: &mut Command) -> Result {
    if !command.status()?.success() {
        return Err(format!("{command:?} failed").into());
    }
    Ok(())
}

fn manifest() -> Result<toml::Value> {
    Ok(toml::from_str(&std::fs::read_to_string("Cargo.toml")?)?)
}

fn version() -> Result<String> {
    Ok(manifest()?["package"]["version"]
        .as_str()
        .ok_or("missing version")?
        .into())
}

fn revision() -> Result<String> {
    output(Command::new("git").args(["rev-parse", "HEAD"]))
}

fn check() -> Result {
    lint()?;
    execute(Command::new("cargo").args(["test", "--locked", "--workspace"]))?;
    // Windows cannot replace the running xtask executable.
    execute(Command::new("cargo").args([
        "build",
        "--locked",
        "--workspace",
        "--exclude",
        "xtask",
    ]))?;
    execute(Command::new("git").args(["lfs", "fsck"]))?;
    Ok(())
}

fn lint() -> Result {
    for args in [
        vec!["fmt", "--all", "--", "--check"],
        vec![
            "clippy",
            "--locked",
            "--workspace",
            "--all-targets",
            "--",
            "-D",
            "warnings",
        ],
    ] {
        execute(Command::new("cargo").args(args))?;
    }
    execute(Command::new("git").args(["diff", "--check"]))?;
    execute(Command::new("git").args(["diff", "--cached", "--check"]))?;
    if cfg!(target_os = "macos") {
        execute(Command::new("bash").args([
            "-n",
            "packaging/macos/build.sh",
            "packaging/macos/signing.sh",
            "packaging/macos/distribute.sh",
            "packaging/macos/ci-sign.sh",
        ]))?;
        execute(Command::new("plutil").args(["-lint", "packaging/macos/Info.plist"]))?;
        execute(Command::new("xmllint").args(["--noout", "packaging/macos/DockMenu.xib"]))?;
        for locale in ["en", "zh-Hans"] {
            execute(Command::new("plutil").args([
                "-lint",
                &format!("packaging/macos/{locale}.lproj/InfoPlist.strings"),
            ]))?;
        }
        execute(Command::new("rustfmt").args(["--check", "packaging/macos/tests/signing.rs"]))?;
    }
    Ok(())
}

#[cfg(test)]
mod desktop_features_tests {
    #[test]
    fn hyperlink_commands_have_a_native_browser_backend() {
        let graph = crate::output(
            std::process::Command::new("cargo")
                .current_dir(crate::root().expect("workspace"))
                .args([
                    "tree",
                    "--locked",
                    "--package",
                    "snip2pin",
                    "--edges",
                    "features",
                    "--invert",
                    "egui-winit",
                    "--prefix",
                    "none",
                ]),
        )
        .expect("resolved application features");
        for feature in ["links", "webbrowser"] {
            assert!(
                graph
                    .lines()
                    .any(|line| line == format!("egui-winit feature {feature:?}")),
                "OpenUrl output needs the native {feature} adapter enabled"
            );
        }
    }
}

//! Repository checks and packaging; no GUI, desktop capture, or credentials.
mod installers;
mod messages;
mod notices;
mod package;
mod release;
mod sizes;
// Keep xtask's tests inside the test binary. Integration-test targets make Cargo
// rebuild the normal executable, which Windows locks while `xtask check` runs.
#[cfg(test)]
mod ci_signing_tests;

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
    std::env::set_current_dir(root())?;
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {
        ["check"] => check(),
        ["lint"] => lint(),
        ["check-release-size", target] => sizes::check_release(target),
        ["preview", font] => preview(Path::new(font)),
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
        _ => Err("usage: cargo xtask check | lint | check-release-size TARGET | check-msrv | check-messages BASE_SHA HEAD_SHA PR_TITLE | candidate-check VERSION SHA RUN | verify-candidate DIR TARGET | package TARGET debug|release | bundle-macos BINARY DEST line|metal | verify DIR | verify-native DIR TARGET signed|unverified | prepare-unverified CANDIDATE_DIR OUTPUT_DIR TARGET | prepare-linux CANDIDATE_DIR OUTPUT_DIR signed|unverified | dmg APP OUTPUT_DMG | finalize CANDIDATE_DIR OUTPUT_DIR signed|unverified | aur-package FINAL_DIR NEW_OUTPUT_DIR | release-check VERSION SHA | stage VERSION SHA DIR NOTES signed|unverified --confirm | publish VERSION SHA BUILD_RUN DIR signed|unverified --confirm".into()),
    }
}

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .to_owned()
}

fn preview(font: &Path) -> Result {
    if !font.is_file() {
        return Err("preview requires an existing local CJK font file".into());
    }
    let lock: toml::Value = toml::from_str(&std::fs::read_to_string("Cargo.lock")?)?;
    let version = lock["package"]
        .as_array()
        .and_then(|packages| {
            packages
                .iter()
                .find(|package| package["name"].as_str() == Some("wasm-bindgen"))
        })
        .and_then(|package| package["version"].as_str())
        .ok_or("wasm-bindgen missing from lock file")?;
    let installed = output(Command::new("wasm-bindgen").arg("--version"))?;
    if installed != format!("wasm-bindgen {version}") {
        return Err(format!(
            "install matching tool: cargo install wasm-bindgen-cli --version {version} --locked"
        )
        .into());
    }
    execute(Command::new("cargo").args([
        "build",
        "--locked",
        "--package",
        "snip2pin-preview",
        "--target",
        "wasm32-unknown-unknown",
    ]))?;
    let target = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target"));
    execute(
        Command::new("wasm-bindgen")
            .arg(target.join("wasm32-unknown-unknown/debug/snip2pin_preview.wasm"))
            .args([
                "--target",
                "web",
                "--out-dir",
                "preview/pkg",
                "--no-typescript",
            ]),
    )?;
    std::fs::copy(font, "preview/pkg/font.ttf")?;
    println!(
        "Sandbox ready. Serve preview/ on 127.0.0.1; generated files and local font are ignored by Git."
    );
    Ok(())
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

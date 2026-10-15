use crate::package::{self, Manifest, TARGETS};
use crate::{Result, execute, manifest, output, revision};
use serde_json::Value;
use std::path::Path;
use std::process::Command;

const REPOSITORY: &str = "Snip2Pin/Snip2Pin";

pub fn validate_inputs(version: &str, revision: &str) -> Result {
    let parts: Vec<_> = version.split('.').collect();
    if parts.len() != 3
        || parts.iter().any(|p| {
            p.is_empty()
                || p.len() > 4
                || !p.bytes().all(|b| b.is_ascii_digit())
                || (p.len() > 1 && p.starts_with('0'))
        })
    {
        return Err("version must be stable MAJOR.MINOR.PATCH, without v".into());
    }
    if revision.len() != 40
        || !revision
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err("revision must be a full lowercase commit SHA".into());
    }
    Ok(())
}

fn api(path: &str) -> Result<Value> {
    Ok(serde_json::from_str(&output(
        Command::new("gh").args(["api", &format!("repos/{REPOSITORY}/{path}")]),
    )?)?)
}

pub fn preflight(version: &str, expected_revision: &str) -> Result {
    validate_inputs(version, expected_revision)?;
    if revision()? != expected_revision || crate::version()? != version {
        return Err("checked-out commit/version differs from release inputs".into());
    }
    if !output(Command::new("git").args(["status", "--porcelain"]))?.is_empty() {
        return Err("release requires a clean checkout".into());
    }
    execute(Command::new("git").args([
        "merge-base",
        "--is-ancestor",
        expected_revision,
        "origin/main",
    ]))?;
    let package = manifest()?;
    let fields = &package["package"];
    if fields
        .get("license")
        .and_then(toml::Value::as_str)
        .is_none()
        && fields
            .get("license-file")
            .and_then(toml::Value::as_str)
            .is_none()
    {
        return Err("select and document the project license before release".into());
    }
    if !Path::new("LICENSE").is_file() {
        return Err("missing LICENSE".into());
    }
    let runs = api(&format!(
        "actions/workflows/ci.yml/runs?head_sha={expected_revision}&event=push&per_page=100"
    ))?;
    if !runs["workflow_runs"]
        .as_array()
        .ok_or("missing CI runs")?
        .iter()
        .any(|r| {
            r["head_branch"] == "main"
                && r["head_sha"] == expected_revision
                && r["conclusion"] == "success"
        })
    {
        return Err("no successful main CI run for this exact commit".into());
    }
    Ok(())
}

pub fn validate_set(data: &[Manifest], version: &str, revision: &str) -> Result {
    if data.len() != TARGETS.len() {
        return Err("all four native packages are required".into());
    }
    for target in TARGETS {
        if data
            .iter()
            .filter(|d| {
                d.target == target
                    && d.version == version
                    && d.revision == revision
                    && d.final_package
            })
            .count()
            != 1
        {
            return Err("incomplete, unsigned, duplicate, or mixed-source packages".into());
        }
    }
    Ok(())
}

pub fn stage(version: &str, revision: &str, directory: &Path, notes: &Path) -> Result {
    preflight(version, revision)?;
    let data = package::verify(directory)?;
    validate_set(&data, version, revision)?;
    if !notes.is_file() || std::fs::read_to_string(notes)?.trim().is_empty() {
        return Err("release notes are required".into());
    }
    let tag = format!("v{version}");
    // Refuse replacement, including an existing unpublished draft/tag.
    if !output(Command::new("git").args([
        "ls-remote",
        "--tags",
        "origin",
        &format!("refs/tags/{tag}"),
    ]))?
    .is_empty()
    {
        return Err("version tag already exists; never replace a release".into());
    }
    let existing = api("releases?per_page=100")?;
    if existing
        .as_array()
        .ok_or("release list")?
        .iter()
        .any(|r| r["tag_name"] == tag)
    {
        return Err("release/draft already exists".into());
    }
    let mut command = Command::new("gh");
    command
        .args([
            "release",
            "create",
            &tag,
            "--repo",
            REPOSITORY,
            "--draft",
            "--target",
            revision,
            "--title",
            &format!("Snip2Pin {version}"),
            "--notes-file",
        ])
        .arg(notes);
    for data in data {
        command.arg(directory.join(&data.file));
        command.arg(directory.join(format!("{}.json", data.target)));
    }
    execute(&mut command)
}

pub fn publish(version: &str, revision: &str, run_id: &str, directory: &Path) -> Result {
    if std::env::var("GITHUB_ACTIONS").as_deref() != Ok("true")
        || std::env::var("GITHUB_WORKFLOW").as_deref() != Ok("Publish verified release (manual)")
        || std::env::var("GITHUB_REF").as_deref() != Ok("refs/heads/main")
    {
        return Err("publication is restricted to the manually approved GitHub workflow".into());
    }
    preflight(version, revision)?;
    check_run(run_id)?;
    let data = package::verify(directory)?;
    validate_set(&data, version, revision)?;
    let tag = format!("v{version}");
    let candidates = Path::new(".build/candidate-provenance").join(run_id);
    if candidates.exists() {
        std::fs::remove_dir_all(&candidates)?;
    }
    execute(
        Command::new("gh")
            .args([
                "run",
                "download",
                run_id,
                "--repo",
                REPOSITORY,
                "--pattern",
                "candidate-*",
                "--dir",
            ])
            .arg(&candidates),
    )?;
    for final_data in &data {
        let original =
            package::verify(&candidates.join(format!("candidate-{}", final_data.target)))?;
        if original.len() != 1
            || original[0].final_package
            || original[0].revision != revision
            || original[0].version != version
            || final_data.candidate_sha256.as_deref() != Some(original[0].sha256.as_str())
        {
            return Err("final package is not linked to this candidate run".into());
        }
    }
    let refs = output(Command::new("git").args([
        "ls-remote",
        "--tags",
        "origin",
        &format!("refs/tags/{tag}"),
        &format!("refs/tags/{tag}^{{}}"),
    ]))?;
    if let Some(line) = refs.lines().last()
        && line.split_whitespace().next() != Some(revision)
    {
        return Err("existing version tag points to another commit".into());
    }
    let draft = api(&format!("releases/tags/{tag}"))?;
    if draft["draft"] != true || draft["target_commitish"] != revision {
        return Err("draft target/draft state mismatch".into());
    }
    let assets = draft["assets"].as_array().ok_or("draft assets")?;
    if assets.len() != TARGETS.len() * 2 {
        return Err("unexpected draft assets".into());
    }
    for data in data {
        for file in [&data.file, &format!("{}.json", data.target)] {
            if assets.iter().filter(|a| a["name"] == *file).count() != 1 {
                return Err("draft asset set mismatch".into());
            }
        }
    }
    // The workflow first checks signatures on native OSes, then re-downloads
    // the draft and compares it with the frozen validated artifact before this call.
    execute(Command::new("gh").args([
        "release",
        "edit",
        &tag,
        "--repo",
        REPOSITORY,
        "--draft=false",
        "--latest",
    ]))
}

fn check_run(run_id: &str) -> Result {
    if run_id.is_empty() || !run_id.bytes().all(|b| b.is_ascii_digit()) {
        return Err("invalid candidate run ID".into());
    }
    validate_candidate_run(&api(&format!("actions/runs/{run_id}"))?)
}

pub fn verify_candidate(directory: &Path, target: &str) -> Result {
    let data = package::verify(directory)?;
    validate_unsigned(&data, target, &crate::version()?, &crate::revision()?)
}

fn validate_unsigned(data: &[Manifest], target: &str, version: &str, revision: &str) -> Result {
    if !TARGETS.contains(&target)
        || data.len() != 1
        || data[0].target != target
        || data[0].final_package
        || data[0].version != version
        || data[0].revision != revision
        || data[0].candidate_sha256.is_some()
    {
        return Err("candidate target/source/version or unsigned state mismatch".into());
    }
    Ok(())
}

pub fn check_candidates(version: &str, revision: &str, run_id: &str) -> Result {
    preflight(version, revision)?;
    check_run(run_id)?;
    let directory = Path::new(".build/signing-candidates");
    if directory.exists() {
        return Err("candidate validation directory already exists".into());
    }
    execute(
        Command::new("gh")
            .args([
                "run",
                "download",
                run_id,
                "--repo",
                REPOSITORY,
                "--pattern",
                "candidate-*",
                "--dir",
            ])
            .arg(directory),
    )?;
    for target in TARGETS {
        let data = package::verify(&directory.join(format!("candidate-{target}")))?;
        validate_unsigned(&data, target, version, revision)?;
    }
    Ok(())
}

fn validate_candidate_run(run: &Value) -> Result {
    if run["head_branch"] != "main"
        || run["event"] != "workflow_dispatch"
        || run["path"] != ".github/workflows/candidates.yml"
        || run["conclusion"] != "success"
    {
        return Err("candidate run is not a successful manual main workflow".into());
    }
    // head_sha identifies the workflow definition, not necessarily its selected
    // source revision. Each original artifact's revision and digest are checked
    // against the final manifest below; main may have advanced in the meantime.
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signing_rejects_wrong_source_target_version_or_already_signed_candidates() {
        let sha = "a".repeat(40);
        let target = TARGETS[0];
        let mut data = vec![Manifest {
            schema: 1,
            version: "0.2.0".into(),
            revision: sha.clone(),
            target: target.into(),
            final_package: false,
            file: package::filename("0.2.0", target, false),
            sha256: "b".repeat(64),
            candidate_sha256: None,
        }];
        assert!(validate_unsigned(&data, target, "0.2.0", &sha).is_ok());
        assert!(validate_unsigned(&data, TARGETS[1], "0.2.0", &sha).is_err());
        assert!(validate_unsigned(&data, target, "0.2.1", &sha).is_err());
        assert!(validate_unsigned(&data, target, "0.2.0", &"c".repeat(40)).is_err());
        data[0].final_package = true;
        assert!(validate_unsigned(&data, target, "0.2.0", &sha).is_err());
        assert!(validate_unsigned(&[], target, "0.2.0", &sha).is_err());
    }

    #[test]
    fn candidate_run_can_build_a_main_ancestor_but_cannot_come_from_a_pr() {
        let mut run = serde_json::json!({"head_branch": "main", "event": "workflow_dispatch", "path": ".github/workflows/candidates.yml", "conclusion": "success", "head_sha": "b".repeat(40)});
        assert!(validate_candidate_run(&run).is_ok());
        run["event"] = "pull_request".into();
        assert!(validate_candidate_run(&run).is_err());
        run["event"] = "workflow_dispatch".into();
        run["head_branch"] = "feature".into();
        assert!(validate_candidate_run(&run).is_err());
    }

    #[test]
    fn release_inputs_reject_shell_and_ref_injection() {
        let sha = "a".repeat(40);
        assert!(validate_inputs("0.2.0", &sha).is_ok());
        for version in [
            "v0.2.0",
            "0.2.0-rc.1",
            "01.2.3",
            "1.2.3;touch x",
            "../1.2.3",
        ] {
            assert!(validate_inputs(version, &sha).is_err());
        }
        assert!(validate_inputs("0.2.0", "main").is_err());
        assert!(validate_inputs("0.2.0", &"A".repeat(40)).is_err());
    }

    #[test]
    fn publication_requires_every_target_and_one_source_revision() {
        let sha = "a".repeat(40);
        let mut data: Vec<_> = TARGETS
            .iter()
            .map(|target| Manifest {
                schema: 1,
                version: "0.2.0".into(),
                revision: sha.clone(),
                target: (*target).into(),
                final_package: true,
                file: package::filename("0.2.0", target, true),
                sha256: "b".repeat(64),
                candidate_sha256: Some("c".repeat(64)),
            })
            .collect();
        assert!(validate_set(&data, "0.2.0", &sha).is_ok());
        data[0].revision = "b".repeat(40);
        assert!(validate_set(&data, "0.2.0", &sha).is_err());
        data[0].revision = sha.clone();
        data[0].final_package = false;
        assert!(validate_set(&data, "0.2.0", &sha).is_err());
        assert!(validate_set(&data[1..], "0.2.0", &sha).is_err());
    }
}

use crate::package::{self, Distribution, Manifest, TARGETS};
use crate::{Result, execute, manifest, output, revision};
use serde_json::Value;
use std::path::Path;
use std::process::Command;

const REPOSITORY: &str = "Snip2Pin/Snip2Pin";
const CI_RUNS_PER_PAGE: usize = 100;

const UNVERIFIED_NOTICE: &str = "## 未验证发布者 / Unverified publisher\n\n这些安装包没有受系统信任的发布者签名；macOS 包仅有 ad-hoc 完整性签名，未经 Apple 公证，Windows 包未签名。首次打开可能需要用户通过系统提供的选项手动允许；系统或组织策略可能禁止运行。macOS 更新可能需要重新授予截屏权限。\n\nThese packages have no trusted publisher signature. macOS bundles use an ad-hoc integrity seal and are not notarized by Apple; Windows executables are unsigned. Opening may require explicit approval using OS-provided controls and may be blocked by device policy. macOS updates may require screen capture permission again.";

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
    if !successful_main_ci(expected_revision, api)? {
        return Err("no successful main CI run for this exact commit".into());
    }
    Ok(())
}

fn successful_main_ci(
    revision: &str,
    mut fetch: impl FnMut(&str) -> Result<Value>,
) -> Result<bool> {
    // Filter exact source locally: the SHA search index can omit a completed run.
    let path =
        |page| format!("actions/workflows/ci.yml/runs?per_page={CI_RUNS_PER_PAGE}&page={page}");
    let mut response = fetch(&path(1))?;
    // Bound the scan to the initial history snapshot, even if new runs arrive.
    let pages = response["total_count"]
        .as_u64()
        .ok_or("missing CI run count")?
        .div_ceil(CI_RUNS_PER_PAGE as u64)
        .max(1);
    for page in 1..=pages {
        if page > 1 {
            response = fetch(&path(page))?;
        }
        let runs = response["workflow_runs"]
            .as_array()
            .ok_or("missing CI runs")?;
        if runs.iter().any(|run| valid_main_ci(run, revision)) {
            return Ok(true);
        }
        if runs.len() < CI_RUNS_PER_PAGE {
            break;
        }
    }
    Ok(false)
}

fn valid_main_ci(run: &Value, revision: &str) -> bool {
    run["head_branch"] == "main"
        && run["head_sha"] == revision
        && run["path"] == ".github/workflows/ci.yml"
        && (run["event"] == "push" || run["event"] == "workflow_dispatch")
        && run["conclusion"] == "success"
}

pub fn validate_set(
    data: &[Manifest],
    version: &str,
    revision: &str,
    distribution: Distribution,
) -> Result {
    for manifest in data {
        manifest.validate()?;
    }
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
                    && d.distribution == Some(distribution)
            })
            .count()
            != 1
        {
            return Err("incomplete, unsigned, duplicate, or mixed-source packages".into());
        }
    }
    Ok(())
}

pub fn stage(
    version: &str,
    revision: &str,
    directory: &Path,
    notes: &Path,
    distribution: Distribution,
) -> Result {
    preflight(version, revision)?;
    let data = package::verify(directory)?;
    validate_set(&data, version, revision, distribution)?;
    if !notes.is_file() || std::fs::read_to_string(notes)?.trim().is_empty() {
        return Err("release notes are required".into());
    }
    let generated_notes = Path::new(".build/release-notes.md");
    let notes_path = if distribution == Distribution::Unverified {
        std::fs::create_dir_all(".build")?;
        std::fs::write(
            generated_notes,
            format!("{UNVERIFIED_NOTICE}\n\n{}", std::fs::read_to_string(notes)?),
        )?;
        generated_notes
    } else {
        notes
    };
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
        .arg(notes_path);
    for data in data {
        for artifact in &data.artifacts {
            command.arg(directory.join(&artifact.file));
        }
        command.arg(directory.join(format!("{}.json", data.target)));
    }
    execute(&mut command)
}

pub fn publish(
    version: &str,
    revision: &str,
    run_id: &str,
    directory: &Path,
    distribution: Distribution,
) -> Result {
    if std::env::var("GITHUB_ACTIONS").as_deref() != Ok("true")
        || std::env::var("GITHUB_WORKFLOW").as_deref() != Ok("Manage release packages (manual)")
        || std::env::var("GITHUB_REF").as_deref() != Ok("refs/heads/main")
        || std::env::var("GITHUB_JOB").as_deref() != Ok("publish")
        || std::env::var("GITHUB_EVENT_NAME").as_deref() != Ok("workflow_dispatch")
    {
        return Err("publication is restricted to the manually approved GitHub workflow".into());
    }
    preflight(version, revision)?;
    check_run(run_id)?;
    let data = package::verify(directory)?;
    validate_set(&data, version, revision, distribution)?;
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
            || final_data.candidate_sha256.as_deref()
                != original[0].artifacts.first().map(|a| a.sha256.as_str())
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
    // The REST tag endpoint returns published releases only. The CLI resolves
    // unpublished drafts too, including ones whose tag does not exist yet.
    let draft: Value = serde_json::from_str(&output(Command::new("gh").args([
        "release",
        "view",
        &tag,
        "--repo",
        REPOSITORY,
        "--json",
        "isDraft,targetCommitish,body,assets",
    ]))?)?;
    if draft["isDraft"] != true || draft["targetCommitish"] != revision {
        return Err("draft target/draft state mismatch".into());
    }
    if distribution == Distribution::Unverified
        && !draft["body"]
            .as_str()
            .is_some_and(|body| body.contains(UNVERIFIED_NOTICE))
    {
        return Err("unverified release must retain its bilingual trust/permission notice".into());
    }
    let assets = draft["assets"].as_array().ok_or("draft assets")?;
    let expected_assets = data.iter().map(|d| d.artifacts.len() + 1).sum::<usize>();
    if assets.len() != expected_assets {
        return Err("unexpected draft assets".into());
    }
    for data in data {
        for file in data
            .artifacts
            .iter()
            .map(|a| a.file.clone())
            .chain(std::iter::once(format!("{}.json", data.target)))
        {
            if assets.iter().filter(|a| a["name"] == file).count() != 1 {
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

/// Generate local Arch packaging only after the exact assets are public.
pub fn aur_package(directory: &Path, destination: &Path) -> Result {
    let data = package::verify(directory)?
        .into_iter()
        .find(|data| data.target == "x86_64-unknown-linux-gnu")
        .ok_or("missing Linux package")?;
    if !data.final_package {
        return Err("AUR requires a final Linux package".into());
    }
    let artifact = data.artifact(package::Format::TarGz)?;
    let tag = format!("v{}", data.version);
    let release = api(&format!("releases/tags/{tag}"))?;
    let url = format!(
        "https://github.com/{REPOSITORY}/releases/download/{tag}/{}",
        artifact.file
    );
    validate_public_asset(&release, &tag, &artifact.file, &url)?;
    let refs = output(Command::new("git").args([
        "ls-remote",
        "--tags",
        "https://github.com/Snip2Pin/Snip2Pin.git",
        &format!("refs/tags/{tag}"),
        &format!("refs/tags/{tag}^{{}}"),
    ]))?;
    if refs
        .lines()
        .last()
        .and_then(|line| line.split_whitespace().next())
        != Some(data.revision.as_str())
    {
        return Err("public version tag source differs from final manifest".into());
    }
    if destination.exists() {
        return Err("AUR output directory already exists".into());
    }
    fs_download_check(&tag, &artifact.file, &artifact.sha256)?;
    crate::installers::aur(destination, &data.version, &url, &artifact.sha256)
}

fn validate_public_asset(release: &Value, tag: &str, file: &str, url: &str) -> Result {
    if release["draft"] != false || release["prerelease"] != false || release["tag_name"] != tag {
        return Err("AUR requires a public stable release".into());
    }
    let assets = release["assets"].as_array().ok_or("release assets")?;
    if assets
        .iter()
        .filter(|asset| asset["name"] == file && asset["browser_download_url"] == url)
        .count()
        != 1
    {
        return Err("public Linux asset name or URL mismatch".into());
    }
    Ok(())
}

fn fs_download_check(tag: &str, file: &str, checksum: &str) -> Result {
    let directory = Path::new(".build/aur-public-asset");
    if directory.exists() {
        return Err("AUR verification directory already exists".into());
    }
    std::fs::create_dir_all(directory)?;
    let result: Result = (|| {
        execute(
            Command::new("gh")
                .args([
                    "release",
                    "download",
                    tag,
                    "--repo",
                    REPOSITORY,
                    "--pattern",
                    file,
                    "--dir",
                ])
                .arg(directory),
        )?;
        if package::digest(&directory.join(file))? != checksum {
            return Err("public Linux asset differs from the verified final manifest".into());
        }
        Ok(())
    })();
    let cleanup = std::fs::remove_dir_all(directory);
    result?;
    cleanup?;
    Ok(())
}

pub fn verify_candidate(directory: &Path, target: &str) -> Result<Manifest> {
    let data = package::verify(directory)?;
    validate_unsigned(&data, target, &crate::version()?, &crate::revision()?)?;
    data.into_iter()
        .next()
        .ok_or_else(|| "missing candidate".into())
}

fn validate_unsigned(data: &[Manifest], target: &str, version: &str, revision: &str) -> Result {
    if !TARGETS.contains(&target)
        || data.len() != 1
        || data[0].target != target
        || data[0].final_package
        || data[0].distribution.is_some()
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

    fn ci_run(sha: &str) -> Value {
        serde_json::json!({"head_branch":"main", "head_sha":sha,
            "path":".github/workflows/ci.yml", "event":"push", "conclusion":"success"})
    }

    fn ci_page(runs: Vec<Value>, total: u64) -> Value {
        serde_json::json!({"workflow_runs": runs, "total_count": total})
    }

    #[test]
    fn main_ci_search_uses_history_when_the_sha_search_index_omits_a_completed_run() {
        let sha = "a".repeat(40);
        let found = successful_main_ci(&sha, |path| {
            // Reproduce the API discrepancy: the ordinary history includes the
            // successful run while a head_sha search returns an empty result.
            Ok(if path.contains("head_sha=") {
                ci_page(vec![], 0)
            } else {
                ci_page(vec![ci_run(&sha)], 1)
            })
        })
        .expect("CI API response");
        assert!(
            found,
            "an indexed SHA search must not hide the successful CI history"
        );
    }

    fn ci_page_number(path: &str) -> u64 {
        path.split(['?', '&'])
            .find_map(|field| field.strip_prefix("page="))
            .expect("page parameter")
            .parse()
            .expect("numeric page")
    }

    #[test]
    fn main_ci_search_finds_an_exact_manual_run_after_a_full_page_of_other_commits() {
        let sha = "a".repeat(40);
        let mut calls = Vec::new();
        let found = successful_main_ci(&sha, |path| {
            let page = ci_page_number(path);
            calls.push(page);
            Ok(match page {
                1 => ci_page(vec![ci_run(&"b".repeat(40)); CI_RUNS_PER_PAGE], 101),
                2 => {
                    let mut run = ci_run(&sha);
                    run["event"] = "workflow_dispatch".into();
                    ci_page(vec![run], 101)
                }
                _ => panic!("unexpected history page"),
            })
        })
        .expect("history");
        assert!(found);
        assert_eq!(calls, [1, 2]);
    }

    #[test]
    fn main_ci_search_stops_at_the_first_match_or_the_initial_history_boundary() {
        let sha = "a".repeat(40);
        for has_match in [false, true] {
            let mut calls = 0;
            let found = successful_main_ci(&sha, |_| {
                calls += 1;
                assert!(
                    calls <= 2,
                    "history growth must not make the scan unbounded"
                );
                let mut runs = vec![ci_run(&"b".repeat(40)); CI_RUNS_PER_PAGE];
                if has_match {
                    runs[0] = ci_run(&sha);
                }
                // Later responses report growth; keep the initial two-page bound.
                Ok(ci_page(runs, if calls == 1 { 200 } else { 10_000 }))
            })
            .expect("history");
            assert_eq!(found, has_match);
            assert_eq!(calls, if has_match { 1 } else { 2 });
        }
    }

    #[test]
    fn main_ci_search_rejects_other_sources_workflows_events_and_unsuccessful_runs() {
        let sha = "a".repeat(40);
        for (key, value) in [
            ("head_sha", "b".repeat(40)),
            ("head_branch", "feature".into()),
            ("path", ".github/workflows/other.yml".into()),
            ("event", "pull_request".into()),
            ("conclusion", "failure".into()),
            ("conclusion", "in_progress".into()),
        ] {
            let mut run = ci_run(&sha);
            run[key] = value.into();
            assert!(
                !successful_main_ci(&sha, |_| Ok(ci_page(vec![run.clone()], 1)))
                    .expect("nonmatching history")
            );
        }
        assert!(!successful_main_ci(&sha, |_| Ok(ci_page(vec![], 0))).expect("empty history"));
    }

    #[test]
    fn main_ci_search_fails_closed_on_malformed_responses_and_api_errors() {
        let sha = "a".repeat(40);
        for response in [
            serde_json::json!({"total_count": 1}),
            serde_json::json!({"workflow_runs": [ci_run(&sha)]}),
        ] {
            assert!(successful_main_ci(&sha, |_| Ok(response.clone())).is_err());
        }
        assert!(successful_main_ci(&sha, |_| Err("API unavailable".into())).is_err());
    }

    #[test]
    fn aur_requires_the_exact_public_stable_asset() {
        let url = "https://github.com/Snip2Pin/Snip2Pin/releases/download/v0.2.0/linux.tar.gz";
        let release = serde_json::json!({ "tag_name": "v0.2.0", "draft": false,
            "prerelease": false, "assets": [{ "name": "linux.tar.gz", "browser_download_url": url }] });
        validate_public_asset(&release, "v0.2.0", "linux.tar.gz", url).expect("public asset");
        for key in ["draft", "prerelease"] {
            let mut invalid = release.clone();
            invalid[key] = true.into();
            assert!(validate_public_asset(&invalid, "v0.2.0", "linux.tar.gz", url).is_err());
        }
        assert!(validate_public_asset(&release, "v0.2.1", "linux.tar.gz", url).is_err());
        assert!(validate_public_asset(&release, "v0.2.0", "other.tar.gz", url).is_err());
        assert!(
            validate_public_asset(
                &release,
                "v0.2.0",
                "linux.tar.gz",
                "https://example.com/other"
            )
            .is_err()
        );
        let mut duplicate = release.clone();
        duplicate["assets"]
            .as_array_mut()
            .expect("assets")
            .push(release["assets"][0].clone());
        assert!(validate_public_asset(&duplicate, "v0.2.0", "linux.tar.gz", url).is_err());
    }

    fn final_artifacts(target: &str, distribution: Distribution) -> Vec<package::Artifact> {
        package::formats(target, true)
            .iter()
            .map(|format| package::Artifact {
                format: *format,
                file: package::artifact_filename("0.2.0", target, Some(distribution), *format),
                sha256: "b".repeat(64),
            })
            .collect()
    }

    #[test]
    fn verified_candidate_returns_metadata_and_rejects_tampering_or_wrong_target() {
        let directory = tempfile::tempdir().expect("candidate fixture");
        let version = crate::version().expect("workspace version");
        let revision = crate::revision().expect("source revision");
        let target = TARGETS[0];
        let file = package::candidate_filename(&version, target);
        let payload = directory.path().join(&file);
        std::fs::write(&payload, b"candidate bytes").expect("candidate payload");
        let checksum = package::digest(&payload).expect("candidate checksum");
        let data = Manifest {
            schema: 3,
            version: version.clone(),
            revision: revision.clone(),
            target: target.into(),
            final_package: false,
            distribution: None,
            artifacts: vec![package::Artifact {
                format: package::Format::Zip,
                file: file.clone(),
                sha256: checksum.clone(),
            }],
            candidate_sha256: None,
        };
        std::fs::write(
            directory.path().join(format!("{target}.json")),
            serde_json::to_vec(&data).expect("manifest JSON"),
        )
        .expect("candidate manifest");

        let verified = verify_candidate(directory.path(), target).expect("valid candidate");
        assert_eq!(verified.version, version);
        assert_eq!(verified.revision, revision);
        assert_eq!(verified.target, target);
        let artifact = verified
            .artifact(package::Format::Zip)
            .expect("zip candidate");
        assert_eq!(artifact.file, file);
        assert_eq!(artifact.sha256, checksum);
        assert!(!verified.final_package);
        assert_eq!(verified.distribution, None);
        assert_eq!(verified.candidate_sha256, None);
        assert!(verify_candidate(directory.path(), TARGETS[1]).is_err());

        std::fs::write(payload, b"changed bytes").expect("tampered payload");
        assert!(verify_candidate(directory.path(), target).is_err());
    }

    #[test]
    fn exact_main_ci_can_be_automatic_or_manual_but_never_a_pr() {
        let sha = "a".repeat(40);
        let mut run = serde_json::json!({"head_branch":"main", "head_sha":sha,
            "path":".github/workflows/ci.yml", "event":"push", "conclusion":"success"});
        assert!(valid_main_ci(&run, &sha));
        run["event"] = "workflow_dispatch".into();
        assert!(valid_main_ci(&run, &sha));
        assert!(!valid_main_ci(&run, &"b".repeat(40)));
        run["event"] = "pull_request".into();
        assert!(!valid_main_ci(&run, &sha));
        run["event"] = "workflow_dispatch".into();
        run["head_branch"] = "feature".into();
        assert!(!valid_main_ci(&run, &sha));
    }

    #[test]
    fn unverified_publication_requires_a_complete_unmixed_explicit_set() {
        let sha = "a".repeat(40);
        let mut data: Vec<_> = TARGETS
            .iter()
            .map(|target| Manifest {
                schema: 3,
                version: "0.2.0".into(),
                revision: sha.clone(),
                target: (*target).into(),
                final_package: true,
                distribution: Some(Distribution::Unverified),
                artifacts: final_artifacts(target, Distribution::Unverified),
                candidate_sha256: Some("c".repeat(64)),
            })
            .collect();
        assert!(validate_set(&data, "0.2.0", &sha, Distribution::Unverified).is_ok());
        assert!(validate_set(&data, "0.2.0", &sha, Distribution::Signed).is_err());
        data[0].distribution = Some(Distribution::Signed);
        assert!(validate_set(&data, "0.2.0", &sha, Distribution::Unverified).is_err());
    }

    #[test]
    fn signing_rejects_wrong_source_target_version_or_already_signed_candidates() {
        let sha = "a".repeat(40);
        let target = TARGETS[0];
        let mut data = vec![Manifest {
            schema: 3,
            version: "0.2.0".into(),
            revision: sha.clone(),
            target: target.into(),
            final_package: false,
            distribution: None,
            artifacts: vec![package::Artifact {
                format: package::Format::Zip,
                file: package::candidate_filename("0.2.0", target),
                sha256: "b".repeat(64),
            }],
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
                schema: 3,
                version: "0.2.0".into(),
                revision: sha.clone(),
                target: (*target).into(),
                final_package: true,
                distribution: Some(Distribution::Signed),
                artifacts: final_artifacts(target, Distribution::Signed),
                candidate_sha256: Some("c".repeat(64)),
            })
            .collect();
        assert!(validate_set(&data, "0.2.0", &sha, Distribution::Signed).is_ok());
        assert!(validate_set(&data, "0.2.0", &sha, Distribution::Unverified).is_err());
        data[0].revision = "b".repeat(40);
        assert!(validate_set(&data, "0.2.0", &sha, Distribution::Signed).is_err());
        data[0].revision = sha.clone();
        data[0].final_package = false;
        assert!(validate_set(&data, "0.2.0", &sha, Distribution::Signed).is_err());
        assert!(validate_set(&data[1..], "0.2.0", &sha, Distribution::Signed).is_err());
    }
}

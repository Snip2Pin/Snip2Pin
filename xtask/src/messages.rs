use crate::{Result, execute, output};
use std::process::Command;

const TYPES: &[&str] = &[
    "feat", "fix", "docs", "refactor", "perf", "test", "build", "ci", "chore", "revert",
];

pub fn validate(subject: &str) -> Result {
    let (prefix, description) = subject
        .split_once(": ")
        .ok_or("expected type(scope): description; scope and ! are optional")?;
    let prefix = prefix.strip_suffix('!').unwrap_or(prefix);
    let kind = if let Some((kind, scope)) = prefix.split_once('(') {
        let scope = scope.strip_suffix(')').ok_or("unclosed scope")?;
        if scope.is_empty()
            || !scope
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        {
            return Err("scope must use lowercase letters, digits or hyphens".into());
        }
        kind
    } else {
        prefix
    };
    if !TYPES.contains(&kind)
        || description.trim().is_empty()
        || description != description.trim()
        || subject.chars().any(char::is_control)
        || subject.chars().count() > 100
    {
        return Err(
            "invalid type/description, control character, or subject longer than 100 characters"
                .into(),
        );
    }
    Ok(())
}

fn validate_sha(sha: &str) -> Result {
    if sha.len() != 40 || !sha.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("message checks require full base/head commit SHAs".into());
    }
    Ok(())
}

pub fn check(base: &str, head: &str, title: &str) -> Result {
    validate_sha(base)?;
    validate_sha(head)?;
    validate(title).map_err(|e| format!("PR title: {e}"))?;
    let commits =
        output(Command::new("git").args(["log", "--format=%H", &format!("{base}..{head}")]))?;
    if commits.is_empty() {
        return Err("PR has no commits".into());
    }
    for sha in commits.lines() {
        let subject = output(Command::new("git").args(["show", "-s", "--format=%s", sha]))?;
        validate(&subject).map_err(|e| format!("commit {sha}: {e}"))?;
    }
    Ok(())
}

pub fn check_pr(number: &str) -> Result {
    let number: u64 = number.parse()?;
    if number == 0 {
        return Err("PR number must be positive".into());
    }
    let pr: serde_json::Value = serde_json::from_str(&output(Command::new("gh").args([
        "api",
        &format!("repos/{}/pulls/{number}", crate::REPOSITORY),
    ]))?)?;
    let head = pr["head"]["sha"].as_str().ok_or("missing PR head")?;
    if pr["state"] != "open" || pr["base"]["ref"] != "main" || head != crate::revision()? {
        return Err("manual message checks require the open PR's exact head on its source branch, targeting main".into());
    }
    let base = pr["base"]["sha"].as_str().ok_or("missing PR base")?;
    validate_sha(base)?;
    // Resolve the live base's commit graph without changing the selected source.
    execute(Command::new("git").args(["fetch", "--no-tags", "origin", base]))?;
    check(base, head, pr["title"].as_str().ok_or("missing PR title")?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manual_pr_numbers_are_validated_before_any_api_call() {
        for value in ["", "0", "../9", "9; echo unsafe"] {
            assert!(check_pr(value).is_err());
        }
    }

    #[test]
    fn conventional_subjects_support_scopes_breaking_changes_and_chinese() {
        for subject in [
            "fix(pin): 修复顶部按钮位置",
            "feat!: change settings format",
            "ci: automate signing",
        ] {
            assert!(validate(subject).is_ok(), "{subject}");
        }
        for subject in [
            "update stuff",
            "Fix: typo",
            "fix(): empty scope",
            "fix(Pin): uppercase",
            "fix: ",
            "fix: trailing ",
            "fix: a\nb",
            "fix: a\rb",
            "fix: a\u{1b}b",
            "fix: a\tb",
        ] {
            assert!(validate(subject).is_err(), "{subject:?}");
        }
        assert!(validate(&format!("fix: {}", "a".repeat(96))).is_err());
    }
}

use crate::{Result, output};
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

pub fn check(base: &str, head: &str, title: &str) -> Result {
    for sha in [base, head] {
        if sha.len() != 40 || !sha.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err("message checks require full base/head commit SHAs".into());
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;

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

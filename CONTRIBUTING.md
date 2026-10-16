# Contributing to Snip2Pin

Keep the product focused on screenshot + pin. Read [AGENTS.md](AGENTS.md) for
implementation constraints and [the workflow](docs/workflow.md) for development,
validation, packaging and manual releases.

1. Describe the problem and acceptance criteria; use an Issue for larger changes.
2. Start a short `feat/…`, `fix/…` or `chore/…` branch from current `main`.
3. Keep one concern per PR. Add behavior regressions for bugs, update documentation
   when behavior changes, and remove unused code.
4. Install the commit hooks described below. Run `cargo xtask check` before a PR.
5. Open a PR with automatic results and explicit native interaction coverage.
   Tests use generated images, never private desktop or clipboard content.
   Keep PR descriptions and comments focused on the problem, design, implementation,
   fixes and necessary validation. Do not publish chat transcripts, personal details,
   approval conversations, local activity or internal coordination/model settings.
6. Confirm the original symptom is gone before merging, in addition to passing
   required checks. Keep unverified fixes in a draft PR; visible native interaction
   issues need actual interaction acceptance. Do not enable auto-merge while
   acceptance is pending. Merge by squash; do not push directly to `main`.

日常开发使用短分支和 PR，一个 PR 解决一个问题。先写清验收标准，再实现和验证；
涉及窗口、权限或安装时，单独记录真实交互结果。CI 通过不能代替人工验收。
原始现象尚未确认消失时保持 Draft；用户仍可复现则继续修复，不能先合入。
提交、合并和标签推送都不会自动发版；只有用户明确要求时才运行发版流程。

Contributions are licensed under the project's MIT license. No CLA is required.

Install [pre-commit](https://pre-commit.com/) and cargo-deny once, with Rust and
Git LFS available on `PATH`:

```sh
uv tool install pre-commit==4.7.0
cargo install cargo-deny --version 0.20.2 --locked
pre-commit install
pre-commit run --all-files
```

Each commit checks Rust formatting, Clippy, staged whitespace, YAML/TOML syntax,
dependency advisories, licenses and sources. Checks fail without rewriting files;
fix the reported issue, stage the changes and commit again. Dependency advisories
require network access. Full tests, builds and native acceptance remain PR checks.
If another tool manages `core.hooksPath`, preserve it and use its supported
pre-commit integration instead of replacing existing hooks.

Use Conventional Commits for PR titles and every commit subject:
`type(scope)!: description` (scope and `!` optional, maximum 100 characters).
For example, `fix(pin): keep controls visible` or `docs: 精简使用说明`.
Allowed types and body guidance are in AGENTS.md; the required `messages` workflow
enforces the format. Squash merges use the PR title/body.

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

## Local builds

Fetch Git LFS assets before building, as shown in the README. The development
toolchain is pinned in `rust-toolchain.toml`; `cargo xtask check-msrv` separately
checks the minimum supported version declared in `Cargo.toml`.

On Ubuntu/Debian, install the native build dependencies:

```sh
sudo apt-get install pkg-config libclang-dev libxcb1-dev libxrandr-dev libdbus-1-dev libpipewire-0.3-dev libwayland-dev libegl-dev libgbm-dev libxkbcommon-dev libfontconfig1-dev fonts-noto-cjk
cargo xtask check
cargo xtask check-msrv
```

Bundle, package and full-payload size checks need the pinned license collector:

```sh
cargo install cargo-about --version 0.9.2 --locked --features cli
```

Dependency updates may require reviewing [license sources](packaging/licenses/README.md).

macOS bundle builds require full Xcode and `ibtool`. For a local development
installation, create the app-specific signing identity once, follow the system
certificate confirmation, and reuse that identity for later updates:

```sh
bash packaging/macos/signing.sh --setup
security add-trusted-cert -r trustRoot -p codeSign \
  -k "$HOME/Library/Application Support/Snip2Pin/signing/identity.keychain-db" \
  "$HOME/Library/Application Support/Snip2Pin/signing/certificate.pem"
bash packaging/macos/test-signing.sh
bash packaging/macos/build.sh --install
```

Keep the private key and password outside the repository. Trust the certificate
only for code signing in your user account. Local development signing is not
Developer ID notarization; see [the workflow](docs/workflow.md) for distribution.
The installer preserves signing identity and settings and does not stop the
running app. Quit it before installing, then reopen it yourself.

Preferences live in `~/Library/Application Support/Snip2Pin` on macOS,
`%APPDATA%\Snip2Pin` on Windows, and `$XDG_CONFIG_HOME/Snip2Pin`
(`~/.config/Snip2Pin` by default) on Linux. The bundle identifier is
`io.github.Snip2Pin.Snip2Pin`. Only an explicitly requested identifier change
uses `--install --migrate-identity`; ordinary updates use `--install`.

Regenerate all icon PNGs from their editable SVG sources with `cargo xtask icons`.
Runtime icon loading uses the committed PNGs; SVG rendering belongs to the
developer tool. Commit binary resources through Git LFS.

`cargo run --locked -- --demo` uses generated images without reading the desktop.
For isolated browser validation, use the [shared UI sandbox](docs/workflow.md#本地-gui-沙箱).
Native and web hosts share interaction rules as well as widgets. The workspace
tests exercise the web host with generated input; keep host-level regressions
alongside shared rule tests when changing cross-platform behavior.

## Checks and contributions

Install [pre-commit](https://pre-commit.com/) and cargo-deny once, with Rust and
Git LFS available on `PATH`:

```sh
uv tool install pre-commit==4.7.0
cargo install cargo-deny --version 0.20.2 --locked
pre-commit install
pre-commit run --all-files
```

Each commit checks Rust formatting, Clippy, YAML/TOML syntax, merge conflicts,
large files, dependency advisories, licenses and sources. Official hooks also fix
trailing whitespace and normalize the final newline, blocking the commit when
they change a file; review and stage those changes before committing again.
Files above 500 KiB must use Git LFS; the size hook skips LFS-managed files.
The local Rust hook calls `cargo xtask lint`, keeping format, Clippy and repository
metadata checks identical to CI rather than duplicating them in hook wrappers.
Dependency advisories require network access. Full tests, builds and native
acceptance remain PR checks.
macOS packaging needs the full Xcode installation and its `ibtool`, as described
in [the workflow](docs/workflow.md#本地-macos-菜单资源).
If another tool manages `core.hooksPath`, preserve it and use its supported
pre-commit integration instead of replacing existing hooks.

Use Conventional Commits for PR titles and every commit subject:
`type(scope)!: description` (scope and `!` optional, maximum 100 characters).
For example, `fix(pin): keep controls visible` or `docs: 精简使用说明`.
Allowed types and body guidance are in AGENTS.md; the required `messages` workflow
enforces the format. Squash merges use the PR title/body.

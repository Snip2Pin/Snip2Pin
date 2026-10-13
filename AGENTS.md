# SnipSprig

## 项目原则

- 保持项目简洁，只做框选截图、置顶贴图、拖动和关闭。
- 未使用的代码、注释、文档和旧实现要及时清理，不保留备用代码或过时说明。
- 已有 Git commit 历史的内容可以直接清理，不用担心误删；需要时从历史恢复。
- 代码使用 Rust，统一支持 Windows、macOS 和 Linux，平台差异集中在适配层。
- 没有请求时，不增加 OCR、标注、录屏、账号、云同步、历史记录或开机启动。
- 只有用户主动操作时才截图，不在启动或后台自动截图。
- 截图留在本机内存中，关闭贴图后释放，不上传、不写入日志。
- 系统截图后端若生成临时文件，应在读取后立即清理，不保留截图历史。
- 尊重系统权限和桌面限制，不绕过屏幕录制、Portal 或安全设置。

## Structure

- `src/capture.rs`: monitor enumeration and in-memory screen capture.
- `src/geometry.rs`: validated pixel selection, cropping, and fit calculations.
- `src/app.rs`: capture state machine, events, and window lifecycle.
- `src/ui/`: launcher, selection overlay, and floating image views.
- `src/platform.rs`: keyboard shortcut and local system-font adapters.
- `tests/`: deterministic checks using generated images.
- `packaging/`: platform packaging metadata and scripts.
- `.github/workflows/ci.yml`: format, Clippy, tests, and builds on all three OSes.

## Development

Use the pinned Rust toolchain and commit `Cargo.lock`. Keep dependencies small.
Use published crates instead of unpinned Git branches. Avoid unsafe code,
production unwrap, hidden background work, and blocking the UI.
Capture runs on a worker; only the UI thread creates or changes windows.
Cancellation and closing a pin must release image data and textures.
Use events and state transitions instead of idle polling.

Use dedicated file patch tools for text edits. Programmatic transformation is
appropriate only when the transformation itself is complex; state the reason and
show the resulting diff. Keep code, comments, README, and this file consistent.

## Validation

```sh
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo build --locked --release
git diff --check
```

Cover reverse drag, DPI conversion, empty and out-of-bounds selections, image
release, and capture/cancel transitions. Never make CI capture a real desktop or
require privacy permission. Use generated demo images for visual checks.
Distinguish compile/test coverage from interactive checks on Windows, macOS,
Linux X11, and Wayland. Document compositor-controlled pinning and shortcuts.

Do not commit screenshots, credentials, toolchains, caches, or build output.
Keep macOS bundle ID `io.github.rensilin.snipsprig` stable. Ad-hoc signing does
not mean an app is notarized for distribution.

The initial task authorizes creation and pushing of this GitHub repository.
Do not infer permission to publish releases or change repository visibility.

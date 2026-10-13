# SnipSprig · 小贴图

A small Rust desktop app for Windows, macOS, and Linux. Capture a region,
keep it above other windows, drag to move, and double-click to close.

## 使用

打开应用，点击「截图并置顶」，拖动框选区域，松手后生成贴图。
拖动贴图可移动位置，点击右上角 × 或双击图片可关闭。
框选时按 Esc 取消。主窗口可以最小化；关闭主窗口会退出应用。

快捷键：macOS 为 **⌘⇧2**，Windows / Linux X11 为 **Ctrl+Shift+2**。
快捷键被占用或桌面不支持时，从窗口点击截图即可。
多显示器时，可以在主窗口选择要截图的屏幕；该屏幕的画面会在
当前窗口所在的屏幕上全屏显示，供你框选。

应用读取后的截图数据留在本机内存里，取消框选或关闭贴图后释放。
不保存历史、不上传截图，没有 OCR、标注或录屏。

## Platform support

| Platform | Support |
| --- | --- |
| Windows 10/11 | Capture, floating pins, global shortcut |
| macOS 13+ | Capture, floating pins, global shortcut; screen-recording permission required |
| Linux X11 | Capture, floating pins, global shortcut; a desktop/window manager is required |
| Linux Wayland | Uses XWayland for window-level requests when available; capture and global shortcuts depend on the desktop |

On macOS, allow SnipSprig in System Settings → Privacy & Security → Screen
Recording (called Screen & System Audio Recording on newer macOS), then relaunch.
An app built locally is ad-hoc signed; it is not Apple-notarized.

On Linux with an X11 display (including XWayland), the app uses the X11 window
backend so floating pins can request always-on-top through the window manager.
Pure Wayland does not expose a universal always-on-top or global-shortcut API.
Use your compositor's window rules/shortcut settings, or an X11 session, when
these features are required. Wayland capture also depends on the desktop's
screenshot/Portal support. Do not assume all compositors behave identically.
Some Wayland capture backends use a temporary screenshot file, which xcap
removes after reading. The app itself does not retain files or a capture history.

## Development

Install [Rust](https://rustup.rs/). The toolchain is pinned to 1.99.0.
All application code is Rust; egui/eframe renders native windows and xcap captures
screens. No webview, JavaScript runtime, network service, or external screenshot
command is required.

Ubuntu/Debian build dependencies:

```sh
sudo apt-get install pkg-config libclang-dev libxcb1-dev libxrandr-dev libdbus-1-dev libpipewire-0.3-dev libwayland-dev libegl-dev libgbm-dev libxkbcommon-dev libfontconfig1-dev fonts-noto-cjk
```

```sh
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo run --locked
cargo build --locked --release
```

Binary output: `target/release/snipsprig` (`snipsprig.exe` on Windows).
Package a Mac app with `bash packaging/macos/build.sh`.

For a safe GUI check using generated pixels instead of desktop content:

```sh
cargo run --locked -- --demo
```

In demo mode, click the capture button and select part of the generated grid.
This exercises capture transitions, cropping, and pin windows without requesting
screen access.

## Structure

```text
src/
  capture.rs       Monitor adapter, in-memory capture, validated cropping
  geometry.rs      Viewport-to-pixel conversion and aspect-preserving sizing
  session.rs       Single-capture gate and stale-result rejection
  app.rs           Event-driven capture/window lifecycle
  platform.rs      Global shortcut and local font/icon adapters
  ui/              Launcher, selection overlay, floating pin
tests/             Generated-image and state-transition tests
packaging/         macOS bundle script and Linux desktop entry
AGENTS.md          Scope, cleanup rules, and validation requirements
```

Capture happens on a worker after app windows are hidden. The selected image is
uploaded to a texture; full-screen capture buffers are released after selection.
Pins are independent borderless windows. Removing a pin releases its texture.
Platform permission errors restore the launcher with a visible message.

CI runs format, Clippy, tests, and release builds on Windows, macOS, and Linux.
CI artifacts contain the corresponding platform binary/app. CI uses generated
test data and does not verify real desktop permission prompts or compositor rules.

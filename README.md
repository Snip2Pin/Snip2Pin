# SnipSprig · 小贴图

<img src="assets/icon.svg" width="96" alt="SnipSprig icon" />

A small Rust desktop app for Windows, macOS, and Linux. Capture a region,
keep it above other windows, drag to move, and double-click to close.

## 使用 / Usage

### 中文

打开应用，点击「截图并置顶」，在任意显示器上拖动框选区域，
松手后在原位生成贴图，保持框选时的大小。
拖动贴图可移动位置，点击右上角 × 或双击图片可关闭。
框选时按 Esc 取消。主窗口可以最小化；关闭主窗口会退出应用。

默认快捷键：macOS 为 **⌘⇧2**，Windows / Linux X11 为 **Ctrl+Shift+2**。
点击「修改快捷键」，按下新的组合键即可保存，Esc 取消修改。
组合键需要包含 Ctrl、Alt，或 macOS 的 Command 键；注册失败时保留旧快捷键。
设置保存在本机，重启后继续使用。
快捷键被占用或桌面不支持时，从窗口点击截图即可。
应用自动覆盖所有显示器，每次截图都会刷新屏幕布局，无需选择屏幕。
一次框选位于一个显示器内；可以把鼠标移到任意屏幕后开始框选。
框选使用独立的无边框选区层，不进入系统全屏或切换桌面。
截图保留当时屏幕上可见的内容，包括主窗口和已有贴图。
可以先最小化主窗口，再用快捷键截图。

macOS 首次截图时需要屏幕录制权限。在「系统设置 → 隐私与安全性 →
屏幕录制」（较新系统称为「屏幕与系统音频录制」）中允许 SnipSprig，
然后重新启动应用。

应用读取后的截图数据留在本机内存里，取消框选或关闭贴图后释放。
不保存历史、不上传截图，没有 OCR、标注或录屏。

### English

Open the app and click **“截图并置顶” (Capture and pin)**. Drag to select a region
on any monitor, then release to create a floating pin at the original position
and displayed size. Drag the pin to move it; click **×** in
the top-right corner or double-click the image to close it. Press **Esc** while
selecting to cancel. You can minimize the main window; closing it quits the app.

Default shortcut: **Command+Shift+2** on macOS, **Ctrl+Shift+2** on Windows / Linux X11.
Click **“修改快捷键” (Change shortcut)** and press a new combination to save it;
press **Esc** to cancel. Include Ctrl, Alt, or Command on macOS. If registration
fails, the previous shortcut stays active. Settings are stored locally and
loaded when the app restarts.
If the shortcut is unavailable or already in use, click the capture button.
All monitors are covered automatically and the layout is refreshed on each
capture. Each selection stays within one monitor; move the pointer to any
monitor before starting your drag. Separate borderless selection layers avoid
native fullscreen and desktop transitions. Captures include everything visible
at that moment, including the main window and existing pins. Minimize the main
window before using the shortcut if you want to keep it out of the capture.

On macOS, allow SnipSprig in System Settings → Privacy & Security → Screen
Recording (called Screen & System Audio Recording on newer macOS), then relaunch.
Screen-recording permission is required for the first capture.

Once read by the app, screenshot data stays in local memory and is released when
you cancel a selection or close its pin. The app does not retain capture history
or upload screenshots, and has no OCR, annotation, or recording features.

## Platform support

| Platform | Support |
| --- | --- |
| Windows 10/11 | Capture, floating pins, global shortcut |
| macOS 13+ | Capture, floating pins, global shortcut; screen-recording permission required |
| Linux X11 | Capture, floating pins, global shortcut; a desktop/window manager is required |
| Linux Wayland | Uses XWayland for window-level requests when available; capture and global shortcuts depend on the desktop |

A macOS app built locally is ad-hoc signed; it is not Apple-notarized.

On Linux with an X11 display (including XWayland), the app uses the X11 window
backend so floating pins can request always-on-top through the window manager.
Pure Wayland does not expose a universal always-on-top or global-shortcut API.
It also leaves absolute window placement to the compositor, so selection-layer
placement and original-position pinning require X11/XWayland.
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
  geometry.rs      Viewport-to-pixel conversion and original-position sizing
  session.rs       Single-capture gate and stale-result rejection
  app.rs           Event-driven capture/window lifecycle
  platform.rs      Global shortcut and local font/icon adapters
  settings.rs      Shortcut validation, recording, and local preferences
  icon.rs          Rendering of the shared vector icon
  ui/              Launcher, selection overlay, floating pin
assets/icon.svg    Editable vector artwork for all platforms
tests/             Generated-image and state-transition tests
packaging/         macOS bundle script and Linux desktop entry
AGENTS.md          Scope, cleanup rules, and validation requirements
```

Capture happens on a worker while the desktop stays visible. Transparent,
mouse-pass-through selection windows are prepared for all monitors. A capture
fills these windows without moving or resizing the launcher, entering native
fullscreen, or hiding existing pins. After selection, the overlay remains until
the new pin has painted its first frame. Full-display buffers are then released.
Pins are independent borderless windows. Removing a pin releases its texture.
Platform permission errors appear in the launcher.

CI runs format, Clippy, tests, and release builds on Windows, macOS, and Linux.
CI artifacts contain the corresponding platform binary/app. CI uses generated
test data and does not verify real desktop permission prompts or compositor rules.

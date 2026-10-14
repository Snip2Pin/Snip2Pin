# Snip2Pin · 小贴图

<p>
  <img src="assets/icons/line.svg" width="96" alt="Light icon: minimal lines" />
  <img src="assets/icons/metal.svg" width="96" alt="Dark icon: dark metal" />
</p>

A small Rust desktop app for Windows, macOS, and Linux. Capture a region,
keep it above other windows, drag to move, and double-click to close.

**截图，原位置顶。 / Snip a region. Pin it in place.**

## 使用 / Usage

### 中文

- 点击「截图并置顶」或按快捷键，在任意显示器上拖动框选，松手后原位置顶，并自动复制到剪贴板。
- 蓝框标识鼠标所在屏幕。框选时按 Esc 取消。
- 拖动贴图可移动，双击关闭；鼠标移入时显示复制图标和 ×，上方放不下时显示在下方。顶部贴图会下移，过高时等比例缩小。

默认快捷键：macOS **⌘⇧2**，Windows / Linux X11 **Ctrl+Shift+2**。
点击「修改快捷键」可更换。

主题默认跟随系统，图标默认跟随主题；均可在主窗口修改，设置自动保存。

macOS 首次使用：在「系统设置 → 隐私与安全性 → 屏幕录制」
（较新系统称为「屏幕与系统音频录制」）中允许 Snip2Pin，再重新打开应用。

### English

- Click **“截图并置顶” (Capture and pin)** or use the shortcut. Drag to select a
  region on any monitor; release to pin it in place and copy it to the clipboard.
- A blue border marks the screen under the pointer. Press **Esc** to cancel.
- Drag a pin to move it; double-click to close. Hover to show **Copy** and **×**
  icons above the image, or below when space is tight. Top-edge pins move down
  and scale proportionally only when too tall to fit.

Default shortcut: **Command+Shift+2** on macOS, **Ctrl+Shift+2** on Windows / Linux X11.
Click **“修改快捷键” (Change shortcut)** to change it.

Theme follows the system and icons follow the theme by default. Change either
in the main window; settings are saved automatically.

On first use on macOS, allow Snip2Pin in System Settings → Privacy & Security →
Screen Recording (Screen & System Audio Recording on newer macOS), then reopen it.

## Platform support

| Platform | Support |
| --- | --- |
| Windows 10/11 | Capture, floating pins, global shortcut |
| macOS 13+ | Capture, floating pins, global shortcut; screen-recording permission required |
| Linux X11 | Capture, floating pins, global shortcut; a desktop/window manager is required |
| Linux Wayland | Uses XWayland for window-level requests when available; capture and global shortcuts depend on the desktop |

Local macOS builds use a fixed development signing certificate; they are not
Apple-notarized.

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
Install [Git LFS](https://git-lfs.com/), then fetch the binary icon resources
before building:

```sh
git lfs install --local
git lfs pull
```

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
cargo build --locked
git lfs fsck
```

Development builds use `target/debug/snip2pin` (`snip2pin.exe` on Windows).
Version changes, tags, distribution packages, and published releases require an
explicit release request. CI validates ordinary pushes and pull requests without
packaging or publishing an application.

For local macOS installation, create a development identity once. Its key and
password stay in a private, app-specific keychain directory outside the repository;
the private keychain is added to the search list only while signing, then the
original list is restored. Setup adds a copy of the public
certificate to the default keychain so macOS can discover it during verification.
Trust this certificate only for code signing in your user account, using the
macOS confirmation when requested. Private-key access remains restricted to
signing tools; this trust does not cover HTTPS or other certificate purposes:

```sh
bash packaging/macos/signing.sh --setup
security add-trusted-cert -r trustRoot -p codeSign \
  -k "$HOME/Library/Application Support/io.github.rensilin.SnipSprig/signing/identity.keychain-db" \
  "$HOME/Library/Application Support/io.github.rensilin.SnipSprig/signing/certificate.pem"
bash packaging/macos/test-signing.sh
bash packaging/macos/build.sh --install
```

Keep this identity for future updates. The installer rejects a different signing
identity, keeps the app's bundle ID and version, and refreshes only this app's
Finder/Launchpad metadata. It does not stop the running application; reopen it
yourself to use the update. `SNIP2PIN_SIGN_IDENTITY` and optional
`SNIP2PIN_SIGN_KEYCHAIN` can select an existing signing identity instead.
The historical `SnipSprig` directory and bundle ID are intentionally retained
so renaming the app does not create a new settings or signing identity.
The default build is a development bundle; only `--release` creates a distribution
archive, and it requires a Developer ID signing identity.

SVG files are editable sources; PNG/ICO/ICNS and other binary image assets use
Git LFS. After editing an icon, regenerate its PNG and run the resource tests:

```sh
cargo run --locked -- --write-icon assets/icons/line.png line
cargo run --locked -- --write-icon assets/icons/metal.png metal
cargo test --locked --test settings
```

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
  settings.rs      Shortcut validation, theme/icon settings, local preferences
  icon.rs          Icon selection, PNG loading, and SVG resource generation
  ui/              Launcher, selection overlay, floating pin
assets/icons/      Line/metal SVG sources and Git LFS PNG resources
tests/             Generated-image and state-transition tests
packaging/         macOS bundle script and Linux desktop entry
AGENTS.md          Scope, cleanup rules, and validation requirements
```

Capture happens on a worker while the desktop stays visible. Transparent,
mouse-pass-through selection windows are prepared for all monitors. A capture
fills these windows without moving or resizing the launcher, entering native
fullscreen, or hiding existing pins. After selection, the overlay remains until
the new pin has painted its first frame. Full-display buffers are then released.
Pins are independent borderless windows. Removing a pin releases its texture and
in-memory copy buffer; the system manages the image already on the clipboard.
Platform permission errors appear in the launcher.

CI runs LFS validation, format, Clippy, tests, and development builds on Windows,
macOS, and Linux. Image tests use generated pixels; macOS also checks selection
window geometry using invisible windows with application activation prohibited.
CI does not verify real desktop permission prompts or compositor rules.

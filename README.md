# Snip2Pin · 小贴图

[源码 / Source](https://github.com/Snip2Pin/Snip2Pin)

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
- 拖动贴图移动，拖动边缘缩放，双击关闭；缩放遵循系统惯例。悬停显示复制、关闭按钮，窄图竖排，极小图使用右键菜单；小图保持原始尺寸，复制始终使用原图。
- 菜单栏／托盘提供打开主页、截图、关闭全部贴图和退出。关闭主页会留在后台；主窗口可开启「开机自启」，默认关闭。

默认快捷键：macOS **⌘⇧2**，Windows / Linux X11 **Ctrl+Shift+2**。
点击「修改快捷键」可更换。

界面默认按系统首选语言选择中英文，无匹配时使用英文；可在「语言」中手动选择。主题默认跟随系统，图标默认跟随主题；设置自动保存。macOS 的 Dock 图标即时更新，Finder／启动台图标在本地更新安装时应用已保存的选择。

macOS 首次截图会请求屏幕录制权限。未授权时，点击应用内「打开授权设置」，
允许 Snip2Pin 后点击「授权后继续截图」；若仍未生效，点击「重新打开应用」。

采用 GitHub Releases 安装包分发，不上 App Store。标有 `-unverified` 的 macOS 包未经公证、Windows 包未签名，系统可能拦截。macOS 请先将应用放入「应用程序」，首次打开被阻止时，可按[Apple 指引](https://support.apple.com/zh-cn/102445)在「隐私与安全性」中确认。此类包更新后可能需要重新授权截屏。

### English

- Click **Capture & pin** or use the shortcut. Drag to select a
  region on any monitor; release to pin it in place and copy it to the clipboard.
- A blue border marks the screen under the pointer. Press **Esc** to cancel.
- Drag a pin to move it and its edges to resize; proportions, **Shift** and anchors
  follow the operating system. Double-click to close. Copy always uses the original
  screenshot. Hover to show copy/close controls; they move within the visible image.
  Narrow pins stack the controls; tiny pins use the right-click menu without enlarging.
- The menu bar / tray offers Home, Capture, Close All and Quit. Closing Home keeps
  the app running. Enable login startup in Home if wanted; it is **off by default**.

Default shortcut: **Command+Shift+2** on macOS, **Ctrl+Shift+2** on Windows / Linux X11.
Click **Change shortcut** to change it.

The interface follows your system's preferred languages (Chinese or English),
falling back to English. Override it under **Language**. Theme follows the system
and icons follow the theme; settings are saved automatically. On macOS, Dock icons update
immediately; local update installs apply the saved icon to Finder and Launchpad.

On first capture on macOS, allow screen recording. If access is missing, use
**Open permission settings**, allow Snip2Pin, then click
**Allow access & continue**. If needed, use **Reopen app**.

Distribution uses GitHub Releases, outside the App Store. Packages marked
`-unverified` are not notarized on macOS and are unsigned on Windows; the OS may
block them. Move the macOS app to Applications, then follow [Apple's instructions](https://support.apple.com/en-us/102445)
if opening is blocked. Updates to these packages may require screen capture permission again.

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
The Linux tray requires a StatusNotifier-compatible desktop tray host.
Pure Wayland does not expose a universal always-on-top or global-shortcut API.
It also leaves absolute window placement to the compositor, so selection-layer
placement and original-position pinning require X11/XWayland.
Use your compositor's window rules/shortcut settings, or an X11 session, when
these features are required. Wayland capture also depends on the desktop's
screenshot/Portal support. Do not assume all compositors behave identically.
Some Wayland capture backends use a temporary screenshot file, which xcap
removes after reading. The app itself does not retain files or a capture history.

## Development

See [Contributing](CONTRIBUTING.md) and [开发与发版流程 / Workflow](docs/workflow.md).
Development uses short branches, PRs and native CI. Releases require an explicit
request, a manual build, approved package preparation and a separate manual publication.

Install [Rust](https://rustup.rs/). The toolchain is pinned to 1.99.0.
Install [Git LFS](https://git-lfs.com/), then fetch the binary icon resources
before building:

```sh
git clone https://github.com/Snip2Pin/Snip2Pin.git
cd Snip2Pin
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
cargo xtask check
cargo xtask check-msrv
cargo deny check
cargo run --locked
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
  -k "$HOME/Library/Application Support/Snip2Pin/signing/identity.keychain-db" \
  "$HOME/Library/Application Support/Snip2Pin/signing/certificate.pem"
bash packaging/macos/test-signing.sh
bash packaging/macos/build.sh --install
```

Keep this identity for future updates. The installer rejects a different signing
identity, keeps the app's bundle ID and version, and refreshes only this app's
Finder/Launchpad metadata. It does not stop the running application; reopen it
yourself to use the update. `SNIP2PIN_SIGN_IDENTITY` and optional
`SNIP2PIN_SIGN_KEYCHAIN` can select an existing signing identity instead.
Configuration directories use only `Snip2Pin`: `~/Library/Application Support/Snip2Pin`
on macOS, `%APPDATA%\Snip2Pin` on Windows, and `$XDG_CONFIG_HOME/Snip2Pin`
(`~/.config/Snip2Pin` by default) on Linux. The app identifier is `io.github.Snip2Pin.Snip2Pin`.
Explicit identifier migrations use `--install --migrate-identity`, preserving the
same signing certificate; macOS may require screen permission again after that
one-time identity change. Migration checks require Snip2Pin and System Settings
to be fully closed, preventing cached identities from sending permission grants
to the old app ID. Subsequent updates use ordinary `--install`. Installation
verifies the app's bundle identity and registered path, restoring the previous
app if validation fails.
This script builds and optionally installs a local development bundle. Distribution
uses the manual workflows in [the release guide](docs/workflow.md). The default
`unverified` mode needs no paid certificate; `signed` requires production credentials
and Developer ID notarization on macOS.

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
  pin.rs           Initial fitting, coordinates and adaptive controls placement
  session.rs       Single-capture gate and stale-result rejection
  permission.rs    User-driven permission state; no native dialogs or polling
  app.rs           Event-driven capture/window lifecycle
  platform.rs      Global shortcut and local font/icon adapters
  settings.rs      Shortcut validation, theme/icon settings, local preferences
  icon.rs          Icon selection, PNG loading, and SVG resource generation
  i18n.rs          Typed Chinese/English catalogue and system language matching
  tray.rs          Native menu bar/tray commands and symbolic status icon
  startup.rs       Opt-in current-user login startup registration
  app/tests.rs     Capture/window lifecycle regressions
  ui/              Views, style profiles, copy artwork and view regressions
assets/icons/      Line/metal SVG sources and Git LFS PNG resources
tests/             Generated-image and state-transition tests
packaging/         macOS bundle script and Linux desktop entry
xtask/             Shared Rust checks, package manifests and manual release tooling
docs/workflow.md   Development, native acceptance and release procedures
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

CI runs LFS validation, format, Clippy, workspace tests, dependency/MSRV checks,
development builds and package smoke checks on Windows x64, macOS ARM64/Intel,
and Linux x64. Image tests use generated pixels; macOS also checks selection
window geometry using invisible windows with application activation prohibited.
CI does not verify real desktop permission prompts or compositor rules.

Licensed under [MIT](LICENSE).

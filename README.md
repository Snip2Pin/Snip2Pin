# Snip2Pin · 截图置顶

[![CI](https://img.shields.io/github/actions/workflow/status/Snip2Pin/Snip2Pin/ci.yml?branch=main&label=CI)](https://github.com/Snip2Pin/Snip2Pin/actions/workflows/ci.yml) [![License: MIT](https://img.shields.io/github/license/Snip2Pin/Snip2Pin)](LICENSE) [![Rust 1.95+](https://img.shields.io/badge/Rust-1.95%2B-b7410e?logo=rust)](Cargo.toml) [![Platforms](https://img.shields.io/badge/platform-macOS%20%7C%20Windows%20%7C%20Linux-2563eb)](#支持平台--platforms)

<p>
  <img src="assets/icons/line.svg" width="64" alt="极简线条图标 / Line icon" />
  <img src="assets/icons/metal.svg" width="64" alt="深色金属图标 / Metal icon" />
</p>

**截图，原位置顶。 / Snip a region. Pin it in place.**

用 Rust 编写的轻量截图置顶工具，支持 Windows、macOS 和 Linux。
A small Rust app that keeps screenshots above other windows.

![主页、贴图列表与悬停操作按钮 / Home, pin list and hover controls](assets/readme/shared-ui.png)

<sub>共享界面的浏览器演示，使用生成图片；不展示原生窗口外观。 / Shared UI preview with generated images; native window chrome is not shown.</sub>

## 安装 / Install

安装包通过 [GitHub Releases](https://github.com/Snip2Pin/Snip2Pin/releases) 分发。
项目仍在开发，安装包会在首次发版后提供。
Installers will be available on GitHub Releases after the first release.

| 系统 / System | 安装包 / Package |
| --- | --- |
| macOS 13+ · Apple silicon / Intel | DMG，将应用拖入 Applications / Drag the app to Applications |
| Windows 10/11 · x64 | Setup.exe，或免安装 ZIP / Installer or portable ZIP |
| Linux · x64 | tar.gz；Ubuntu 24.04 提供 .deb / tar.gz; .deb for Ubuntu 24.04 |
| Arch Linux | 首次发版后提供 AUR 配方；尚未提交 / AUR recipe planned after the first release; not yet submitted |

`-unverified` 表示 macOS 未经公证、Windows 未签名，系统可能拦截。
macOS 首次打开被阻止时，按 [Apple 指引](https://support.apple.com/zh-cn/102445)确认；
更新此类包可能需要重新授权截屏。
Packages marked `-unverified` are not notarized on macOS or signed on Windows.
Follow [Apple's instructions](https://support.apple.com/en-us/102445) if opening is blocked;
updates may require screen-capture permission again.

## 使用

- 点击「截图并置顶」或按快捷键：悬停后单击截取窗口，或在任意显示器拖动框选。完成后自动复制，并原位置顶；Esc 取消。
- 拖动移动，拖动边缘缩放；鼠标滚轮或触摸板捏合围绕指针缩放。悬停显示复制、锁定、关闭按钮，右键菜单提供全部操作。复制始终使用原图。
- 主页列表可定位贴图、双击名称编辑、锁定或关闭。锁定后不能移动、缩放或关闭；退出前会确认。
- 聚焦贴图后，方向键微调位置，长按逐步加速，Shift＋方向键快速移动；`+`／`-` 缩放，`0` 恢复原始尺寸。× 或 Esc 关闭。
- 语言、主题和图标默认跟随系统，可手动选择；开机自启默认关闭。菜单栏／托盘与 macOS Dock 提供截图和主页入口。

| 快捷键 | macOS | Windows / Linux X11 |
| --- | --- | --- |
| 截图（可修改） | ⌘⇧2 | Ctrl+Shift+2 |
| 解锁全部贴图 | ⌘⌥⇧L | Ctrl+Alt+Shift+L |

macOS 首次截图需允许屏幕录制权限，按应用中的授权提示操作即可。
截图和贴图仅保存在内存中，不上传、不保留历史；系统剪贴板由系统管理。

## Usage

- Click **Capture & pin** or use the shortcut. Hover and click a window, or drag a region on any monitor. The result is copied and pinned in place. **Esc** cancels.
- Drag to move; drag edges to resize. Use the wheel or trackpad pinch to zoom around the pointer. Hover for copy, lock and close; right-click for every action. Copy always uses the original image.
- Use Home to locate pins, double-click names to edit, lock or close. Locked pins cannot move, resize or close; quitting asks for confirmation.
- Focus a pin to move with arrows. Holding gradually increases the step; **Shift+arrows** moves quickly. Use `+` / `-` to zoom, `0` to restore the original size, and **×** or **Esc** to close.
- Language, theme and icon follow the system with manual overrides. Login startup is off by default. The menu bar / tray and macOS Dock offer Capture and Home.

| Shortcut | macOS | Windows / Linux X11 |
| --- | --- | --- |
| Capture, customizable | Command+Shift+2 | Ctrl+Shift+2 |
| Unlock all pins | Command+Option+Shift+L | Ctrl+Alt+Shift+L |

On macOS, follow the in-app screen-recording permission prompts on first capture.
Images stay in memory; there is no upload or capture history. The OS manages the clipboard.

## 支持平台 / Platforms

Windows 10/11、macOS 13+、Linux X11 提供截图、置顶和全局快捷键。
Windows 10/11, macOS 13+ and Linux X11 support capture, floating pins and global shortcuts.

<details>
<summary>平台差异 / Platform differences</summary>

- 窗口截图只截预览框内的当前可见内容，包括遮挡物；跨屏窗口只截当前显示器内的部分。未识别到窗口时默认选中整屏，仍可拖动框选。窗口内框选极小区域时，先长按再拖动。
- Native edge resizing follows each OS's conventions. Window capture matches the visible preview, including overlaps, and is clipped to the selected monitor. Hold before dragging a tiny region inside a window.
- 小图不会为放下按钮而自动放大；窄图隐藏左侧按钮，右键菜单保留全部操作。触摸屏单指框选或拖动；触摸板双指滑动不移动贴图。
- Small pins retain their size; narrow pins hide leftmost buttons and keep all actions in the context menu. Touchscreens support one-finger selection/dragging; two-finger precision scrolling does not move pins.
- Linux 有 X11（包括 XWayland）时使用 X11 窗口后端。纯 Wayland 的置顶、全局快捷键和绝对定位由合成器决定，不能保证原位置顶；可使用合成器窗口规则或 X11 会话。托盘需要兼容 StatusNotifier 的桌面。
- With X11/XWayland available, Linux uses the X11 window backend. Pure Wayland controls pinning, global shortcuts and absolute placement; original-position pinning is not guaranteed. Use compositor rules or X11 when needed. The tray needs a StatusNotifier host.
- Linux 截图取决于桌面的截图／Portal 支持；部分后端读取后会清理临时文件。发行包基于 Ubuntu 24.04 / glibc 2.39，不保证旧版发行版兼容。
- Linux capture depends on desktop screenshot/Portal support. Some backends use a temporary file, removed after reading. Packages target Ubuntu 24.04 / glibc 2.39; older distributions are not guaranteed.
- 中文界面需要系统中文字体；.deb 推荐安装文泉驿微米黑，已有 Noto CJK 也可使用。tar.gz／Arch 用户可安装 `wqy-microhei` 对应的发行版字体包。
- Chinese UI needs a system CJK font. The .deb recommends WenQuanYi Micro Hei; existing Noto CJK works too. For tar.gz / Arch, install the distribution's Micro Hei font package if needed.
- macOS 的 Dock 图标即时更新；Finder／启动台图标在本地更新安装时应用保存的选择。
- macOS Dock icons update immediately; local update installs apply the saved icon to Finder and Launchpad.

</details>

## 开发 / Development

需要 [Rust](https://rustup.rs/)、[Git LFS](https://git-lfs.com/)；开发工具链由 `rust-toolchain.toml` 固定。
Requires Rust and Git LFS; the development toolchain is pinned in `rust-toolchain.toml`.

```sh
git clone https://github.com/Snip2Pin/Snip2Pin.git
cd Snip2Pin
git lfs install --local
git lfs pull
cargo xtask check
cargo run --locked
```

构建依赖、提交规范和本地安装见 [Contributing](CONTRIBUTING.md)，
打包、许可与人工发版流程见 [开发与发版流程 / Workflow](docs/workflow.md)。
See those guides for native build dependencies, local installation, validation and manual releases.

原生界面使用 egui/eframe，无 WebView；浏览器演示仅用于安全验证。
The app uses egui/eframe native windows, without a WebView; the browser preview is a test sandbox.

## 许可证 / License

[MIT](LICENSE) © Snip2Pin contributors。依赖与字体保留各自许可证，安装包附带 `THIRD_PARTY_NOTICES.txt`。
Dependencies and fonts retain their own licenses; packages include `THIRD_PARTY_NOTICES.txt`.

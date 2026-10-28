# Snip2Pin · 截图置顶

[![CI](https://img.shields.io/github/actions/workflow/status/Snip2Pin/Snip2Pin/ci.yml?branch=main&label=CI)](https://github.com/Snip2Pin/Snip2Pin/actions/workflows/ci.yml) [![License: MIT](https://img.shields.io/github/license/Snip2Pin/Snip2Pin)](LICENSE) [![Rust 1.95+](https://img.shields.io/badge/Rust-1.95%2B-b7410e?logo=rust)](Cargo.toml) [![Platforms](https://img.shields.io/badge/platform-macOS%20%7C%20Windows%20%7C%20Linux-2563eb)](#安装)

<p>
  <img src="assets/icons/line.svg" width="64" alt="极简线条图标 / Line icon" />
  <img src="assets/icons/metal.svg" width="64" alt="深色金属图标 / Metal icon" />
</p>

**截图，原位置顶。 / Snip a region. Pin it in place.**

用 Rust 编写的轻量截图置顶工具，支持 Windows、macOS 和 Linux。

A small Rust app that keeps screenshots above other windows.

[在线体验 / Try the demo](https://snip2pin.github.io/Snip2Pin/) · [简体中文](#简体中文) · [English](#english) · [安装](#安装) · [使用](#使用)

左右滚动查看场景，点击图片可放大。 / Scroll sideways for more scenes; click an image to enlarge.

<table>
<tr>
<td nowrap="nowrap" valign="top" width="720">
<a href="assets/readme/data-comparison.webp"><img src="assets/readme/data-comparison.webp" width="720" alt="钉住上周指标，对照本周报表 / Last week's metrics pinned beside this week's report" /></a><br />
<strong>数据对比 / Compare data</strong><br />
<sub>左侧钉住上周指标，右侧查看本周报表。 / Pin last week's metrics on the left; read this week's report on the right.</sub>
</td>
<td nowrap="nowrap" valign="top" width="720">
<a href="assets/readme/code-reference.webp"><img src="assets/readme/code-reference.webp" width="720" alt="代码定义与调用处对照 / A function definition pinned beside its caller" /></a><br />
<strong>代码阅读 / Read code</strong><br />
<sub>左侧保留函数定义，右侧继续阅读调用代码。 / Keep the definition on the left while reading its caller on the right.</sub>
</td>
<td nowrap="nowrap" valign="top" width="720">
<a href="assets/readme/task-reference.webp"><img src="assets/readme/task-reference.webp" width="720" alt="聊天要求与待办核对 / Chat requirements pinned beside a checklist" /></a><br />
<strong>待办与沟通 / Follow up</strong><br />
<sub>左侧钉住聊天要求，右侧逐项完成待办。 / Pin chat requirements on the left; work through the checklist on the right.</sub>
</td>
</tr>
</table>

<sub>Rust/WASM 界面实际运行截图，仅用虚构内容；图中标注用于说明。 / Actual Rust/WASM UI screenshots with fictional content and explanatory labels.</sub>

在线演示只操作模拟桌面，不读取真实屏幕。 / The demo uses a simulated desktop without reading your screen.

[本地运行演示 / Run the demo locally](docs/workflow.md#本地-gui-沙箱)

## 简体中文

### 安装

从 [GitHub Releases](https://github.com/Snip2Pin/Snip2Pin/releases) 下载对应系统的安装包。

| 系统 | 安装方式 |
| --- | --- |
| macOS 13+，Apple silicon / Intel | 打开 DMG，将应用拖入 Applications |
| Windows 10/11，x64 | 运行 Setup.exe，或解压便携 ZIP |
| Linux，x64 | 解压 tar.gz；Ubuntu 24.04 可使用 .deb |

Windows 安装版提供开始菜单入口，可选择创建桌面快捷方式，并可从系统设置卸载。便携 ZIP 解压后运行 `snip2pin.exe`，无需安装。

Arch Linux 的 AUR 配方计划在首次发版后提供，目前尚未提交。

macOS 首次截图需允许屏幕录制权限，按应用中的提示操作即可。文件名带 `-unverified` 的包在 macOS 上未经公证，在 Windows 上未签名，系统可能阻止打开。macOS 用户可按 [Apple 指引](https://support.apple.com/zh-cn/102445)确认；更新此类包后，可能需要重新授权截屏。

### 使用

钉住一份参考，继续手头的工作：代码阅读时对照函数定义，分析数据时并排查看两期指标，或把聊天里的待办留在眼前。

点击「截图并置顶」或按截图快捷键，进入截图模式。在任意显示器拖动框选，或悬停到窗口后单击截取。按 Esc 取消。

截图完成后，图片会自动复制到剪贴板，并在原位置顶。

| 操作 | 方式 |
| --- | --- |
| 移动 | 拖动图片 |
| 缩放 | 拖动边缘，或用滚轮、触摸板捏合围绕指针缩放 |
| 再次复制 | 悬停后点击复制按钮，始终复制原图 |
| 锁定 | 悬停后点击锁定按钮，锁定后无法移动、缩放或关闭 |
| 定位、命名与排序 | 在主页列表中定位贴图，双击名称编辑，拖动左侧手柄排序 |
| 关闭 | 点击 ×，或聚焦贴图后按 Esc |

右键菜单也可复制、锁定或关闭贴图。有锁定贴图时，退出应用会先询问。

#### 快捷键

| 操作 | macOS | Windows / Linux X11 |
| --- | --- | --- |
| 截图，可修改 | ⌘⇧2 | Ctrl+Shift+2 |
| 解锁全部贴图 | ⌘⌥⇧L | Ctrl+Alt+Shift+L |

贴图获得焦点后，方向键微调位置，长按逐步加速；Shift＋方向键快速移动。`+`／`-` 缩放，`0` 恢复原始尺寸。

语言、主题和图标默认跟随系统，可在主页手动选择；开机自启默认关闭。菜单栏／托盘与 macOS Dock 也提供截图和主页入口。

主页及菜单中的「帮助」可打开 [Wiki 使用指南](https://github.com/Snip2Pin/Snip2Pin/wiki)。

截图和贴图保存在内存中，不上传，也不保留历史记录。剪贴板由系统管理。Linux 部分截图后端会使用临时文件，读取后清理。

<details>
<summary>使用须知与平台差异</summary>

- **截图范围**：窗口截图与预览一致，只截当前显示器内的可见画面，包括遮挡物。未识别到窗口时默认选中整屏，仍可拖动框选。在窗口内框选极小区域时，先长按再拖动。
- **小图操作**：小图不会为容纳按钮而放大。窄图会从左侧逐个隐藏按钮，复制、锁定和关闭仍可通过右键菜单完成。
- **系统交互**：边缘缩放遵循各系统惯例。触摸屏支持单指框选或拖动；触摸板双指滑动不会移动贴图。
- **图形驱动**：需要支持 OpenGL 2.0 或更高版本的驱动；部分虚拟机或远程桌面的基础显示驱动可能不满足要求。
- **Linux 桌面**：有 X11（包括 XWayland）时使用 X11 窗口后端，支持置顶和全局快捷键。纯 Wayland 的置顶、全局快捷键和绝对定位由合成器决定，不能保证原位置顶；可使用合成器窗口规则或 X11 会话。截图取决于桌面的截图／Portal 支持，托盘需要兼容 StatusNotifier 的桌面。
- **Linux 安装与字体**：发行包基于 Ubuntu 24.04 / glibc 2.39，不保证旧版发行版兼容。中文界面需要系统中文字体；.deb 推荐安装文泉驿微米黑，已有 Noto CJK 也可使用。tar.gz／Arch 用户可安装 `wqy-microhei` 对应的发行版字体包。
- **macOS 图标**：Dock 图标即时更新；Finder／启动台图标在本地更新安装时应用保存的选择。

</details>

## English

### Install

Download the package for your system from [GitHub Releases](https://github.com/Snip2Pin/Snip2Pin/releases).

| System | Installation |
| --- | --- |
| macOS 13+, Apple silicon / Intel | Open the DMG and drag the app to Applications |
| Windows 10/11, x64 | Run Setup.exe or extract the portable ZIP |
| Linux, x64 | Extract tar.gz; use .deb on Ubuntu 24.04 |

The Windows installer adds a Start menu entry and offers a desktop shortcut; uninstall it through Windows Settings. For the portable ZIP, extract it and run `snip2pin.exe` without installing.

An AUR recipe is planned after the first release. It has not been submitted yet.

On macOS, follow the in-app screen-recording permission prompts on first capture. Packages marked `-unverified` are not notarized on macOS or signed on Windows, so the OS may block them. Follow [Apple's instructions](https://support.apple.com/en-us/102445) if opening is blocked; updates may require screen-capture permission again.

### Usage

Keep a reference in view while you work: a function definition beside its callers, last week's metrics beside this week's, or a task from a chat while you complete it.

Click **Capture & pin** or use the capture shortcut. Drag a region on any monitor, or hover over a window and click to capture it. Press Esc to cancel.

The result is copied to the clipboard and pinned in place.

| Action | How |
| --- | --- |
| Move | Drag the image |
| Resize | Drag an edge, or zoom around the pointer with the wheel or a trackpad pinch |
| Copy again | Hover and click the copy icon; copying always uses the original image |
| Lock | Hover and click the lock icon to prevent moving, resizing and closing |
| Locate, rename and reorder | Locate a pin in the Home list; double-click its name to edit; drag the left handle to reorder |
| Close | Click × or press Esc while the pin has focus |

The context menu also offers Copy, Lock and Close. Quitting asks for confirmation when there are locked pins.

#### Shortcuts

| Action | macOS | Windows / Linux X11 |
| --- | --- | --- |
| Capture, customizable | Command+Shift+2 | Ctrl+Shift+2 |
| Unlock all pins | Command+Option+Shift+L | Ctrl+Alt+Shift+L |

With a pin focused, use arrows for fine movement. Holding an arrow key gradually increases its step size; Shift+arrows moves quickly. Use `+` / `-` to zoom and `0` to restore the original size.

Language, theme and icon follow the system with manual overrides on Home. Login startup is off by default. The menu bar / tray and macOS Dock also offer Capture and Home.

Choose **Help** on Home or in the menu to open the [Wiki guide](https://github.com/Snip2Pin/Snip2Pin/wiki).

Images stay in memory; there is no upload or capture history. The OS manages the clipboard. Some Linux capture backends use temporary files, removed after reading.

<details>
<summary>Usage notes and platform differences</summary>

- **Capture area:** Window capture matches the visible preview, including overlaps, and is clipped to the current monitor. Unrecognized areas select the whole display; dragging still selects a region. Hold before dragging a tiny region inside a window.
- **Small pins:** Small images are not enlarged to fit controls. Narrow pins hide leftmost buttons, while Copy, Lock and Close remain in the context menu.
- **Native interaction:** Edge resizing follows each OS's conventions. Touchscreens support one-finger selection and dragging; two-finger precision scrolling on a trackpad does not move pins.
- **Graphics driver:** OpenGL 2.0 or newer is required. Basic display drivers in some virtual machines or remote desktop sessions may not provide it.
- **Linux desktops:** With X11/XWayland available, the app uses X11 for floating pins and global shortcuts. Pure Wayland controls pinning, global shortcuts and absolute placement; original-position pinning is not guaranteed. Use compositor rules or X11 when needed. Capture depends on desktop screenshot/Portal support, and the tray needs a StatusNotifier host.
- **Linux packages and fonts:** Packages target Ubuntu 24.04 / glibc 2.39; older distributions are not guaranteed. Chinese UI needs a system CJK font. The .deb recommends WenQuanYi Micro Hei; existing Noto CJK works too. For tar.gz / Arch, install the distribution's Micro Hei font package if needed.
- **macOS icons:** Dock icons update immediately. Local update installs apply the saved icon to Finder and Launchpad.

</details>

## 开发 / Development

准备好 [Rust](https://rustup.rs/) 和 [Git LFS](https://git-lfs.com/)，并按 [贡献指南](CONTRIBUTING.md#local-builds)安装平台构建依赖。开发工具链由 `rust-toolchain.toml` 固定。

Install Rust, Git LFS and the [native build dependencies](CONTRIBUTING.md#local-builds) first. The development toolchain is pinned in `rust-toolchain.toml`.

```sh
git clone https://github.com/Snip2Pin/Snip2Pin.git
cd Snip2Pin
git lfs install --local
git lfs pull
cargo xtask check
cargo run --locked
```

提交规范和本地安装见 [贡献指南](CONTRIBUTING.md)，打包、许可与手动发版见 [开发与发版流程](docs/workflow.md)。

See [Contributing](CONTRIBUTING.md) for contribution rules and local installation, and [Workflow](docs/workflow.md) for packaging, licensing and manual releases.

原生界面使用 egui/eframe，无 WebView；浏览器演示用于验证共享界面。

The app uses egui/eframe native windows, without a WebView. The browser demo tests the shared UI.

## 许可证 / License

[MIT](LICENSE) © Snip2Pin contributors。依赖与字体保留各自许可证，安装包附带 `THIRD_PARTY_NOTICES.txt`。

Dependencies and fonts retain their own licenses; packages include `THIRD_PARTY_NOTICES.txt`.

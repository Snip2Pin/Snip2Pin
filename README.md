# SnipSprig · 小贴图

A tiny native macOS menu-bar app: capture a region and keep it floating above
other windows. Built with Swift 6 and AppKit, without third-party dependencies.

## 使用

1. 打开 `SnipSprig.app`，点击「开始截图」。
2. 首次使用时，在系统设置里允许 SnipSprig 的屏幕录制权限，然后重新打开应用。
3. 按 **⌘⇧2**，拖动框选区域，松手后截图自动置顶。

拖动贴图可移动位置。点击右上角 **×** 关闭，也可以直接双击图片。
框选时按 **Esc** 取消。菜单栏里的图钉可以开始截图、关闭全部贴图或退出。
关闭说明窗口后，应用仍在菜单栏运行。

截图临时保存在本机私有临时目录，读取后立即删除；贴图关闭后释放图片。
应用不会上传截图，也不保存截图历史。全局快捷键不需要辅助功能权限。
如果快捷键被其他应用占用，请从菜单栏开始截图。

## Build

Requirements: macOS 13+, Swift 6+ (Xcode or Command Line Tools).

```sh
bash scripts/test.sh
bash scripts/build.sh
open dist/SnipSprig.app
```

The test runner uses Swift Testing and handles missing macro search paths in
some Command Line Tools releases using that toolchain's own plugin.
The build script builds for the host Mac's architecture. The generated app is ad-hoc
signed for local use; a Developer ID signature and notarization are needed for
broader distribution. Build output is in `dist/` and is not committed.

To inspect a floating demo without capturing any screen content:

```sh
open -n dist/SnipSprig.app --args --demo
```

## Project structure

```text
Sources/
  SnipSprig/        App lifecycle, menu bar, global shortcut, floating UI
  SnipSprigKit/     Capture process lifecycle and screenshot placement
Tests/             Geometry, cancellation, cleanup, and concurrency tests
Resources/         App bundle metadata
scripts/           Build, package, and generate the app icon
AGENTS.md          Development rules and validation expectations
```

`CaptureRunner` runs macOS's native `screencapture` selection UI asynchronously,
then returns image data and removes the private temporary directory. AppKit
creates a non-activating floating panel for each image. Each panel is released
when closed. All UI state is isolated to the main actor.

No OCR, annotations, recording, cloud upload, account system, or auto-launch.

# Snip2Pin

## 项目原则

- 产品定位是截图 + Pin：框选截图、自动复制、原位置顶、拖动和关闭；快捷键、主题和图标仅作为基础设置，保持项目简洁。
- README 的使用介绍保持简洁，中英文同步，只保留基本操作、快捷键、基础设置和必要的权限说明。
- 主题默认跟随系统，用户可选择跟随系统、浅色或深色，设置需要持久保存。
- 图标默认跟随主题：浅色使用极简线条，深色使用深色金属；用户也可手动固定任一图标。
- PNG、ICO、ICNS 等二进制资源使用 Git LFS；SVG 源文件使用普通 Git，并保持源文件与 PNG 一致。
- 只有用户明确、显式要求发版时，才进行发版操作。日常修改不自动升版本、打标签、创建或发布 GitHub Release、发布 crate 或上传分发包。
- CI 只运行验证，不因普通提交或合并自动发版；本地开发构建不代表已经发版。
- 自动覆盖所有显示器，每次截图刷新屏幕布局；所有贴图保持截图原始尺寸，不得自动缩小、放大或拉伸。顶部空间不足时只调整贴图和按钮的位置。
- 截图模式用蓝框标识鼠标所在屏幕，其他屏幕略微变暗；随鼠标切换高亮，不在后台轮询鼠标。
- 贴图边框和阴影画在图片外侧；鼠标移入时显示复制和关闭图标，复制在左，关闭在右，不能遮挡图片。初次顶部截图和拖回顶部时均放在下方，向下移出足够空间后再放回上方。复制使用双层照片图标。
- 成功框选后自动复制图片到系统剪贴板，复制按钮可再次复制；取消、无效或过期选区不能改动剪贴板。测试只检查复制命令，不写入真实系统剪贴板。
- 窗口位置和尺寸只在初始化时设置一次，不因原生坐标或尺寸不匹配而重试。普通贴图通过窗口内偏移补偿小幅坐标差值，顶部接受系统放置位置；初始化期间仍须响应 Esc 和关闭，完成后不得干预用户拖动。
- 拖动贴图时，根据原生窗口顶边与所在显示器可用区顶边的距离即时切换按钮上下位置：上方够放就放上方，否则放下方。不得将预留的上方透明空间算作可用距离。切换不能移动或缩放图片，也不能重设窗口位置和尺寸。
- macOS 窗口按整数原生点取整，图片须在窗口内单独定位和绘制，不能随窗口尺寸取整而偏移或拉伸。
- 使用预先准备的透明无边框选区层，不切换系统全屏、不隐藏主窗口或已有贴图。
- 新贴图绘制完成后才清除选区画面，避免截图结束时露出空白帧。
- 未使用的代码、注释、文档和旧实现要及时清理，不保留备用代码或过时说明。
- 已有 Git commit 历史的内容可以直接清理，不用担心误删；需要时从历史恢复。
- 代码使用 Rust，统一支持 Windows、macOS 和 Linux，平台差异集中在适配层。
- macOS 本地安装必须复用固定的代码签名证书，不能用每次随构建变化的 ad-hoc 签名。
- 更新前验证新包符合已安装应用的签名要求；证书缺失时停止，不自动换签名身份。
- 签名私钥放在独立的本地钥匙串，不进入仓库；不得修改 TCC 数据库、自动重置截屏授权，或放宽签名要求来保留权限。
- 签名证书及其私钥、钥匙串、密码、令牌等本机凭据必须保存在仓库之外，不提交到 Git、不上传到 GitHub，也不写入日志或 CI 产物；提交前检查暂存内容，不能只依赖 `.gitignore`。
- 更新 Finder／启动台图标时，更新应用自身的资源引用并刷新注册信息，不重启 Finder／Dock 或清空全局缓存。
- Linux 有 X11 显示服务时优先使用 X11/XWayland 窗口后端，保留置顶能力。
- 没有请求时，不增加 OCR、标注、录屏、账号、云同步、历史记录或开机启动。
- 只有用户主动操作时才截图，不在启动或后台自动截图。
- 截图和复制用的图片保存在本机内存中，关闭贴图后释放；系统剪贴板的内容由系统管理。不上传、不写入日志。
- 系统截图后端若生成临时文件，应在读取后立即清理，不保留截图历史。
- 尊重系统权限和桌面限制，不绕过屏幕录制、Portal 或安全设置。

## Structure

- `src/capture.rs`: monitor enumeration and in-memory screen capture.
- `src/geometry.rs`: validated pixel selection and original-position pin placement.
- `src/app.rs`: capture state machine, events, and window lifecycle.
- `src/ui/`: launcher, selection overlay, and floating image views.
- `src/platform.rs`: keyboard shortcut and local system-font adapters.
- `src/settings.rs`: shortcut validation, theme/icon preferences, and local storage.
- `assets/icons/`, `src/icon.rs`: vector sources, LFS PNG assets, and icon selection.
- `tests/`: deterministic checks using generated images.
- `packaging/`: platform packaging metadata and scripts.
- `.github/workflows/ci.yml`: format, Clippy, tests, and builds on all three OSes.

## Development

Use the pinned Rust toolchain and commit `Cargo.lock`. Keep dependencies small.
Use published crates instead of unpinned Git branches. Avoid unsafe code,
production unwrap, hidden background work, and blocking the UI.
Capture runs on a worker; only the UI thread creates or changes windows.
Cancellation and closing a pin must release image data and textures.
Use events and state transitions instead of idle polling. Keep empty selection
layers transparent and mouse-pass-through outside capture mode. Never use native
fullscreen for macOS selection; it triggers desktop/Space animations.
Shortcut changes must retain the previous binding if registration fails. Demo
mode must not register a global shortcut, save settings, or read the desktop.
macOS selection windows must join Spaces without creating a fullscreen Space,
cover the menu bar, and release keyboard focus when capture finishes.
After raising macOS selection windows above system UI, restore `NSScreen.frame`
instead of the work area. Warm inactive layers too, and defer image painting
until the viewport reports the complete display size; never stretch a desktop
capture into a menu-bar-constrained window. Native geometry tests must keep
windows invisible and prohibit application activation.
Use egui's theme preference to follow system appearance without polling. Theme
and icon changes must preserve unrelated settings and existing shortcut files.
Keep unsafe code denied by default. A platform API that requires unsafe may have
one narrowly scoped exception, with documented preconditions and owned memory.
macOS Dock icons must decode PNG in Rust, avoiding AppKit PNG loading and borrowed
pixel buffers. Updating an icon must not modify the signed application bundle.

Use dedicated file patch tools for text edits. Programmatic transformation is
appropriate only when the transformation itself is complex; state the reason and
show the resulting diff. Keep code, comments, README, and this file consistent.

## Validation

```sh
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo build --locked
git lfs fsck
git diff --check
```

Cover reverse drag, DPI conversion, empty and out-of-bounds selections, image
release, and capture/cancel transitions. Never make CI capture a real desktop or
require privacy permission. Use generated demo images for visual checks.
Distinguish compile/test coverage from interactive checks on Windows, macOS,
Linux X11, and Wayland. Document compositor-controlled pinning and shortcuts.

Do not commit screenshots, credentials, toolchains, caches, or build output.
Keep macOS bundle ID `io.github.rensilin.snipsprig` stable. Local certificate
signing is for development; it does not mean an app is notarized for distribution.
The Snip2Pin rename preserves the historical app ID, preference/signing directory,
and existing certificate. Do not rename these identities or regenerate credentials.
`build.sh --install` performs a local development update. Distribution packaging
requires `--release` and a Developer ID identity, only on an explicit release request.

The initial task authorizes creation and pushing of this GitHub repository.
Do not infer permission to publish releases or change repository visibility.

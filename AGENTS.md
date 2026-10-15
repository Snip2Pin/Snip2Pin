# Snip2Pin

Canonical repository: `https://github.com/Snip2Pin/Snip2Pin`.

## 项目原则

- 产品定位是截图 + Pin：框选截图、自动复制、原位置顶、拖动、手动缩放和关闭；菜单栏入口、开机自启、快捷键、主题和图标仅作为基础设置，保持项目简洁。
- README 的使用介绍保持简洁，中英文同步，只保留基本操作、快捷键、基础设置和必要的权限说明。
- 代码、配置路径和打包元数据不得包含个人姓名或账号，也不保留旧产品名称。配置目录只使用 `Snip2Pin`，应用标识统一为 `io.github.Snip2Pin.Snip2Pin`（保持用户选定的大小写）；产品身份与仓库所有者无关。
- 项目网站后续使用 GitHub Pages；建立并验证页面之前，不添加尚不存在的主页链接。
- 主题默认跟随系统，用户可选择跟随系统、浅色或深色，设置需要持久保存。
- 图标默认跟随主题：浅色使用极简线条，深色使用深色金属；用户也可手动固定任一图标。
- PNG、ICO、ICNS 等二进制资源使用 Git LFS；SVG 源文件使用普通 Git，并保持源文件与 PNG 一致。
- 只有用户明确、显式要求发版时，才进行发版操作。日常修改不自动升版本、打标签、创建或发布 GitHub Release、发布 crate 或上传分发包。
- CI 只运行验证，不因普通提交或合并自动发版；本地开发构建不代表已经发版。
- 当前采用 GitHub Releases 安装包分发，不上 App Store，暂不开通付费签名服务。默认明确选择 `unverified`：macOS 仅使用 ad-hoc 完整性签名、未经公证，Windows 可执行文件未签名；文件名和发布说明必须如实标识。`signed` 为未来取得正式凭据后的选项，签名失败不得自动降级。不清除 quarantine、不关闭 Gatekeeper/SmartScreen、不自动信任自签证书；用户使用系统允许的方式自行确认。不得承诺未公证包更新可无感保留 macOS 截屏授权。
- 自动覆盖所有显示器，每次截图刷新屏幕布局；贴图初始保持截图原始尺寸，禁止自动缩放或拉伸。用户可拖动图片边角自由缩放，按住 Shift 保持本次缩放开始时的比例。顶部空间不足时只调整位置和留白，不能借此缩放图片。
- 手动缩放只改变显示尺寸，复制始终使用原始截图；原始像素不能被覆盖或反复重采样。缩放锚点、图片位置和尺寸以原生坐标保存，适配 DPI 和缩放变化；复制、关闭按钮仍须在可用区内。
- 菜单栏／托盘提供打开主页、截图、关闭全部贴图和退出；关闭主页时保留托盘和贴图，通过菜单退出才关闭应用。菜单事件唤醒 UI，不使用后台轮询；无托盘后端时保留主页和正常关闭行为。
- 开机自启默认关闭，只在用户明确勾选后写入当前用户的系统配置；关闭时仅删除本应用入口。启动和安装不得自动启用自启，测试只操作临时目录，不修改真实登录项或注册表。自启后台启动，不弹主页、不抢焦点、不截图；源码运行的 macOS 应用须先安装后才能启用。
- 截图模式用蓝框标识鼠标所在屏幕，其他屏幕略微变暗；随鼠标切换高亮，不在后台轮询鼠标。
- 贴图边框和阴影画在图片外侧；鼠标移入时显示复制和关闭图标，复制在左，关闭在右，不能遮挡图片。复制使用双层照片图标。
- 成功框选后自动复制图片到系统剪贴板，复制按钮可再次复制；取消、无效或过期选区不能改动剪贴板。测试只检查复制命令，不写入真实系统剪贴板。
- 窗口初始化只设置一次，不因原生坐标或尺寸不匹配而重试。普通贴图通过窗口内偏移补偿小幅坐标差值，初始化期间仍须响应 Esc 和关闭。随后仅由用户拖动或缩放事件更新几何；移动只调整透明留白，缩放只按用户操作调整显示尺寸。静止时不得轮询鼠标或重复定位。
- 贴图可向左右和下方移出屏幕；操作图标必须保持在显示器可用区内、可见且可点击，避开 Dock、菜单栏等系统界面。macOS 顶部遵守系统约束：图片上边缘可紧贴菜单栏下缘，不继续移到菜单栏外，不能用裁切图片模拟越界。其他方向的移动边界由可达的操作图标决定。拖动锚点以图片内的抓取点和原生坐标保存，正确处理 DPI 和缩放变化。无法读取全局鼠标坐标或指定位置的平台使用系统拖动。
- 位置根据图片、窗口和显示器的实际边界计算，禁止硬编码屏幕坐标。尺寸、间距、颜色、时间阈值等使用有明确用途的配置或常量，避免散落数值；数学常量、格式规范、图标路径和测试样例无需机械抽象。
- 操作图标的位置根据图片、显示器可用区和实际空间在四边动态计算，优先在图片上方；图片移出右侧时放到左侧，移出左侧时放到右侧。上方按钮的完整区域一旦被菜单栏遮挡，就应放到下方；图片下移后上方空间足够时恢复上方。角落处综合选择可达位置，始终不遮挡图片。不能将透明留白误当作屏幕内可用空间。用户拖动时可重新分配透明留白，让完整图片贴紧菜单栏；不得因此改变图片原始尺寸或抓取点。悬停和按钮操作本身不能重新定位或调整窗口。
- 普通贴图必须置于 Dock 和菜单栏之下，不得靠提高到系统界面之上的窗口层级解决移动限制。macOS 使用原生移动接口，保留窗口类型和系统监听的生命周期；不得修改窗口运行时类型或系统共享类。框选层仅在用户主动截图时覆盖系统界面。
- macOS 窗口按整数原生点取整，图片须在窗口内单独定位和绘制，不能随窗口尺寸取整而偏移或拉伸。
- 使用预先准备的透明无边框选区层，不切换系统全屏、不隐藏主窗口或已有贴图。
- 新贴图绘制完成后才清除选区画面，避免截图结束时露出空白帧。
- 未使用的代码、注释、文档和旧实现要及时清理，不保留备用代码或过时说明。
- 已有 Git commit 历史的内容可以直接清理，不用担心误删；需要时从历史恢复。
- 代码使用 Rust，统一支持 Windows、macOS 和 Linux，平台差异集中在适配层。
- macOS 本机开发构建安装（`build.sh --install`）必须复用固定的代码签名证书，不能用每次随构建变化的 ad-hoc 签名；GitHub 分发包按明确选择的分发模式处理。
- 本机开发更新前验证新包符合已安装应用的签名要求；证书缺失时停止，不自动换签名身份。只有用户明确要求更换应用标识时，才使用显式迁移选项，仅替换签名要求中的 identifier，完整保留原证书和信任约束。
- 本地开发签名私钥保留在独立的本地钥匙串，不导出到 CI；不得修改 TCC 数据库、自动重置截屏授权，或放宽签名要求来保留权限。
- 正式发行凭据可保存在受审批保护的 GitHub `release` Environment Secrets；只允许手动、main 来源的签名任务读取。私钥、钥匙串、密码和令牌不得进入 Git、日志、缓存或 CI 产物；提交前检查暂存内容，不能只依赖 `.gitignore`。Secrets 并非不可导出的硬件密钥，授权任务可以读取明文，需审阅工作流和依赖并及时轮换泄露凭据。
- 更新 Finder／启动台图标时，更新应用自身的资源引用并刷新注册信息，不重启 Finder／Dock 或清空全局缓存。
- Linux 有 X11 显示服务时优先使用 X11/XWayland 窗口后端，保留置顶能力。
- 没有请求时，不增加 OCR、标注、录屏、账号、云同步、历史记录或开机启动。
- 只有用户主动操作时才截图，不在启动或后台自动截图。
- 截图和复制用的图片保存在本机内存中，关闭贴图后释放；系统剪贴板的内容由系统管理。不上传、不写入日志。
- 系统截图后端若生成临时文件，应在读取后立即清理，不保留截图历史。
- 尊重系统权限和桌面限制，不绕过屏幕录制、Portal 或安全设置。

## 需求理解与实现

- 实现前先弄清用户要解决什么问题、为什么需要这个行为，以及怎样才算使用顺畅。区分用户的目标与用户提出的具体做法，不能把具体做法当成需求的全部；实现和验证都要围绕目标展开。
- 方案必须考虑通用性和可维护性，覆盖不同平台、显示器布局、DPI、缩放比例、图片尺寸和边缘位置。把共同规则封装在合适的模块中，平台差异留在适配层；保持项目简洁，不为假设中的需求增加抽象或功能。
- 禁止通过硬编码位置、偏移量或针对某个场景的特殊数值修补现象。坐标和布局应从实际边界及统一的坐标换算推导，样式参数和阈值应有明确含义并集中管理。仅把散落数字改成常量，不能代替对模型和计算方式的改进。
- 出现偏移或位置异常时，先定位坐标单位、DPI、缩放、窗口边框、原生系统约束及生命周期中的根因。不得用反复移动、缩放、重试或轮询来掩盖错误；之前用定位重试补偿偏移既未解决根因，又引入了死循环问题，不能再采用这种做法。
- 图标放到图片下方的目的是让用户操作更顺畅：图片移动时复制和关闭图标仍应留在屏幕内并可操作。“顶部时把图标放下面”只是实现手段；macOS 顶部应让完整图片贴紧菜单栏下缘，不能被旧的透明上边距阻挡，也不能用裁切实现。其他方向应保留图片越界移动能力。
- 验证用户最终能否完成操作，而不只验证中间数值或某个分支。涉及窗口行为时，检查真实的平台约束和完整交互路径；布局计算正确或隐藏窗口能设置位置，不代表用户拖动时一定正确。明确区分自动化验证与用户实际交互验证，发现缺口时补足测试。

## Structure

- `src/capture.rs`: monitor enumeration and in-memory screen capture.
- `src/geometry.rs`: validated pixel selection and original-position pin placement.
- `src/pin.rs`: pin layout metrics, coordinate conversion, one-time preparation, and drag anchoring.
- `src/app.rs`: capture state machine, events, and window lifecycle.
- `src/ui/`: launcher, selection overlay, floating image views, shared style profiles, and copy artwork.
- `src/app/tests.rs`, `src/ui/pin/tests.rs`: lifecycle and view regressions, separate from production logic.
- `src/platform.rs`: keyboard shortcut and local system-font adapters.
- `src/platform/macos.rs`: pin stacking adapter, preserving the backend's window class and observation lifecycle.
- `src/settings.rs`: shortcut validation, theme/icon preferences, and local storage.
- `src/tray.rs`: event-driven native menu bar/tray commands and status artwork.
- `src/startup.rs`: explicit current-user login registration with atomic, escaped file entries or Windows Run registry values.
- `assets/icons/`, `src/icon.rs`: vector sources, LFS PNG assets, and icon selection.
- `tests/`: deterministic checks using generated images.
- `packaging/`: platform packaging metadata and scripts.
- `.github/workflows/ci.yml`: format, Clippy, tests, and builds on all three OSes.
- `xtask/`: shared Rust checks, package/manifest validation, license notices and manual release commands.
- `docs/workflow.md`: contributor flow, native acceptance, approved CI signing and manual publication.
- `.github/rulesets/`: versioned main/tag protection definitions, applied and verified through GitHub.

## Development

Follow [docs/workflow.md](docs/workflow.md) and [CONTRIBUTING.md](CONTRIBUTING.md).
Use short branches and focused PRs; `main` requires all native CI checks and
dependency checks, resolved discussions, an up-to-date base, and squash merge.
Do not bypass protections or push directly to `main`. Single-maintainer PRs do
not require a second reviewer. Describe user-visible acceptance criteria before
implementation; separate automatic coverage from actual native interaction.
Keep repository rules in `.github/rulesets/` consistent with GitHub settings.

PR titles and every branch commit subject must use `type(scope)!: description`;
scope and `!` are optional. Types: feat, fix, docs, refactor, perf, test, build,
ci, chore, revert. Use lowercase ASCII types/scopes (scope may contain digits
and hyphens), a nonempty Chinese or English description, and at most 100 characters.
Examples: `fix(pin): 修复顶部按钮位置`, `ci: automate release signing`.
Use `!` and a `BREAKING CHANGE:` body/footer when changing incompatible behavior.
Explain why in the body when useful; avoid vague subjects such as `update` or `fix`.
PR descriptions state the problem/result, scope, validation and unverified native
coverage; link issues when applicable. `messages.yml` checks titles on edits and
all branch commits on updates; fix/reword invalid commits before merging.
Squash commits use the validated PR title and PR body. Do not bypass the required
`messages` check. Conventional formatting never authorizes a version bump/release.

Use `cargo xtask check` locally and in CI. Shared development/packaging logic
belongs in the Rust `xtask` workspace member; native scripts only adapt platform
tools. Read the development toolchain from rust-toolchain.toml and verify the
minimum Rust version declared in Cargo.toml with `cargo xtask check-msrv`.
Keep xtask's own tests in `#[cfg(test)]` modules: Cargo builds the normal binary
for integration-test targets, which cannot replace a running `xtask.exe` on Windows.
Dependency updates go through PRs and `cargo deny check`; do not auto-merge or
globally suppress advisories/licenses to obtain a green check. Any scoped
exception needs an advisory ID, impact analysis and a dated follow-up.
The project and contributions use MIT; include LICENSE in application packages
and preserve required third-party license notices.

CI uses pinned Action commit SHAs, least-privilege tokens, finite timeouts and
platform-specific caches. PRs cannot access signing credentials. Daily checks
may build unsigned debug packages and verify final unverified package layout
without GUI activation, installing, uploading distribution files, using signing
credentials, tagging or creating Releases. macOS debug bundle integrity checks
may use an ad-hoc seal; never use the local development identity in CI.
Only explicit manual candidate builds upload unsigned release-profile artifacts.
Formal publication requires a complete set of four native targets, matching
version/source SHA, checksums, native checks for the explicitly selected
distribution mode, recorded
installation/upgrade acceptance and approval of the `release` Environment.
Do not run candidate, signing, stage or publish commands without an explicit
release request. Implementing/testing these workflows is not such a request.
Production signing credentials may be provisioned once through the protected
`release` Environment Secrets, using the names documented in docs/workflow.md.
Do not export/upload the local development identity. Use GitHub-hosted ephemeral
runners, single-step secret exposure, temporary keychains/certificate imports and
cleanup on failure. No credentials in caches/artifacts; no signing on PRs or
automatic push/merge events. Do not attach a public-PR self-hosted runner to the
user's everyday computer. Missing/invalid production credentials must fail closed;
never fall back to ad-hoc/development signatures in `signed` mode. No automatic
version bump/release. `publish.yml` defaults to the `prepare` operation and explicit `unverified` distribution,
without secrets; it validates candidate run/source/digests, prepares packages
without rebuilding, checks native integrity and creates only a draft. Manifests
record the distribution mode. Never mix modes or publish unverified files using
signed names/checks. Preserve the bilingual trust notice in unverified Releases.
Actual publication remains a separate manual request with native acceptance and
Environment approval. Windows PFX support requires a provider-approved exportable
code-signing certificate; nonexportable/hardware identities need a cloud signing
adapter instead of attempts to export them.
Use Cargo.toml as the single version source; generate package metadata and bind
manifests to a full commit SHA. Publish the verified files without rebuilding;
never overwrite published tags/assets. Fix a release with a new patch version.

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
macOS pins stay at floating window level, below the Dock and menu bar. Never
replace the runtime class of an observed window: AppKit owns KVO subclasses and
their observation bookkeeping. Test creation, movement, appearance changes,
and destruction using real winit windows, hidden with activation prohibited;
plain NSWindow geometry tests alone cannot cover backend observation teardown.
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
Select Finder/Launchpad bundle resources before signing during local installation.
Do not set a custom file icon on a signed app: its Finder metadata/resource fork
fails strict signature verification. Runtime icon choices update the Dock; document
the bundle-icon installation requirement rather than silently invalidating signing.

Use dedicated file patch tools for text edits. Programmatic transformation is
appropriate only when the transformation itself is complex; state the reason and
show the resulting diff. Keep code, comments, README, and this file consistent.

## Validation

```sh
cargo xtask check
cargo xtask check-msrv
cargo deny check
```

Cover reverse drag, DPI conversion, empty and out-of-bounds selections, image
release, and capture/cancel transitions. Never make CI capture a real desktop or
require privacy permission. Use generated demo images for visual checks.
Distinguish compile/test coverage from interactive checks on Windows, macOS,
Linux X11, and Wayland. Document compositor-controlled pinning and shortcuts.

Do not commit screenshots, credentials, toolchains, caches, or build output.
Keep macOS bundle ID `io.github.Snip2Pin.Snip2Pin` stable. Local certificate
signing is for development; it does not mean an app is notarized for distribution.
Settings use the platform configuration root plus `Snip2Pin`; local macOS signing
material uses `~/Library/Application Support/Snip2Pin/signing`, outside the repo.
Reuse the existing certificate; do not regenerate credentials during a rename.
An explicitly requested identifier change uses `--install --migrate-identity`,
which preserves the certificate requirement; later updates use ordinary `--install`.
`build.sh --install` performs a local development update. Formal distribution
uses the manual workflows documented in docs/workflow.md, production signing
and notarization only when `signed` is explicitly selected; default `unverified`
packages are not Apple-notarized. Never use the local development certificate to release.

The initial task authorizes creation and pushing of this GitHub repository.
Do not infer permission to publish releases or change repository visibility.

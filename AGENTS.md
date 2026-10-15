# Snip2Pin

Canonical repository: `https://github.com/Snip2Pin/Snip2Pin`.

## 项目原则

- 产品定位是截图 + Pin：框选截图、自动复制、原位置顶、拖动、手动缩放和关闭；菜单栏入口、开机自启、快捷键、主题和图标仅作为基础设置，保持项目简洁。
- README 的使用介绍保持简洁，中英文同步，只保留基本操作、快捷键、基础设置和必要的权限说明。
- 主页采用紧凑直达布局：一个截图主按钮，快捷键与主题、图标、开机自启直接可见；版本与 GitHub Releases 入口固定在底部。长快捷键可换行，小窗口内容可滚动，不把设置拆成多页，不增加常驻操作说明。
- 代码、配置路径和打包元数据不得包含个人姓名或账号，也不保留旧产品名称。配置目录只使用 `Snip2Pin`，应用标识统一为 `io.github.Snip2Pin.Snip2Pin`（保持用户选定的大小写）；产品身份与仓库所有者无关。
- 项目网站后续使用 GitHub Pages；建立并验证页面之前，不添加尚不存在的主页链接。
- 主题默认跟随系统，用户可选择跟随系统、浅色或深色，设置需要持久保存。
- 图标默认跟随主题：浅色使用极简线条，深色使用深色金属；用户也可手动固定任一图标。
- PNG、ICO、ICNS 等二进制资源使用 Git LFS；SVG 源文件使用普通 Git，并保持源文件与 PNG 一致。
- 只有用户明确、显式要求发版时，才进行发版操作。日常修改不自动升版本、打标签、创建或发布 GitHub Release、发布 crate 或上传分发包。
- CI 只运行验证，不因普通提交或合并自动发版；本地开发构建不代表已经发版。
- 当前采用 GitHub Releases 安装包分发，不上 App Store，暂不开通付费签名服务。默认明确选择 `unverified`：macOS 仅使用 ad-hoc 完整性签名、未经公证，Windows 可执行文件未签名；文件名和发布说明必须如实标识。`signed` 为未来取得正式凭据后的选项，签名失败不得自动降级。不清除 quarantine、不关闭 Gatekeeper/SmartScreen、不自动信任自签证书；用户使用系统允许的方式自行确认。不得承诺未公证包更新可无感保留 macOS 截屏授权。
- 自动覆盖所有显示器，每次截图刷新屏幕布局；初始置顶保持原始显示尺寸，仅超过可用区时等比缩小。按钮不得决定图片尺寸，不为容纳按钮放大小图或拉伸短边，不添加应用层最小窗口尺寸；系统自身的尺寸约束须单独验证。复制始终使用原始像素。
- 移动和缩放优先交给系统，比例、Shift 和边缘锚点遵循各平台原生惯例，不再强制统一手写手势。图片内容铺满原生内容区，以系统实际位置和尺寸为准；UI 不计算下一帧窗口位置或尺寸，不在绘制中回写主窗口几何。
- 缩放和初始适配只改变显示尺寸，复制始终使用原始截图；原始像素不能被覆盖或反复重采样。显示器、DPI 和应用缩放按统一单位换算；复制、关闭按钮仍须在可用区内。
- 菜单栏／托盘提供打开主页、截图、关闭全部贴图和退出；关闭主页时保留托盘和贴图，通过菜单退出才关闭应用。菜单事件唤醒 UI，不使用后台轮询；无托盘后端时保留主页和正常关闭行为。
- 开机自启默认关闭，只在用户明确勾选后写入当前用户的系统配置；关闭时仅删除本应用入口。启动和安装不得自动启用自启，测试只操作临时目录，不修改真实登录项或注册表。自启后台启动，不弹主页、不抢焦点、不截图；源码运行的 macOS 应用须先安装后才能启用。
- 截图模式用蓝框标识鼠标所在屏幕，其他屏幕略微变暗；随鼠标切换高亮，不在后台轮询鼠标。
- 完成或取消截图时恢复普通光标；演示和自动化测试不得修改系统光标或抢占用户焦点。
- 贴图采用系统阴影，不为旧边框样式增加透明边距或坐标补偿。复制、关闭在图片内部悬停显示：空间足够时横排（左复制、右关闭），窄图改竖排（上关闭、下复制），仍放不下时通过右键菜单操作；所有贴图均支持右键菜单。复制使用双层照片图标。按钮点击不得触发窗口拖动或缩放。视觉样式可为更流畅的交互调整。
- 成功框选后自动复制图片到系统剪贴板，复制按钮可再次复制；取消、无效或过期选区不能改动剪贴板。测试只检查复制命令，不写入真实系统剪贴板。
- 初始位置与尺寸在创建贴图时确定并保持构造参数稳定，界面缩放变化不能把用户移动或缩放后的窗口重置回原位。系统实际结果作为后续绘制依据，不因不匹配而重试或补偿。初始化期间仍须响应 Esc 和关闭。原生移动和缩放事件驱动重绘与按钮布局；静止时不得轮询鼠标或重复定位。
- 图片移动遵守系统原生窗口约束，不借裁切模拟越界或与系统争夺位置。操作按钮根据图片与显示器可用区的交集布局，避开 Dock、菜单栏等系统界面；macOS 图片上边缘应能紧贴菜单栏下缘。不用透明留白实现换边。
- 位置根据图片、窗口和显示器的实际边界计算，禁止硬编码屏幕坐标。尺寸、间距、颜色、时间阈值等使用有明确用途的配置或常量，避免散落数值；数学常量、格式规范、图标路径和测试样例无需机械抽象。
- 操作按钮默认图内右上角，右侧被遮挡时向左移动，顶部被遮挡时向下移动；空间恢复后回到右上角。按钮限制在图内可见交集，不改变图片位置或尺寸，不新增常驻按钮窗口、全局鼠标监听或轮询。右键菜单仅在用户主动操作时创建临时窗口，独立于小图尺寸，操作完成、Esc 或失焦后关闭；不能把菜单裁切在极小的图片窗口里。极端越界使可见面积不足时，不通过定位重试争夺系统窗口控制。
- 后端不提供全局窗口位置时（例如部分 Wayland 桌面），按钮依据实际本地内容区布局，不能用初始截图位置或尺寸猜测当前窗口边界；系统负责窗口可达性。
- 普通贴图必须置于 Dock 和菜单栏之下，不得靠提高到系统界面之上的窗口层级解决移动限制。macOS 使用原生移动接口，保留窗口类型和系统监听的生命周期；不得修改窗口运行时类型或系统共享类。框选层仅在用户主动截图时覆盖系统界面。
- 原生内容区就是图片区域；按实际内容区绘制，接受系统原生尺寸与缩放惯例，不保留期望宿主与实际宿主相减的图片偏移。
- 框选预览边界与最终裁剪共用同一个像素矩形。平台窗口只能表达更粗坐标时，在框选阶段按实际像素密度统一边界；不得在松手后单独取整窗口原点、重新裁剪或补偿位置。非整数像素密度不能假装存在精确的整数网格。
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
- macOS 截屏授权由主线程在用户截图时检查和申请，截图工作线程只读取权限，不能弹授权框。未授权时保留空闲状态，在主页提供打开授权设置、授权后继续和重新打开应用；不后台轮询权限，不反复弹请求，不要求用户猜应用路径或手动修复数据库。
- 重新打开应用必须由用户明确点击，先关闭窗口并释放快捷键，再启动新实例；演示模式不得申请权限、打开系统设置或重启真实应用。
- 应用标识迁移前须检查应用及系统设置已完全退出，避免同一路径的新旧身份被长驻进程缓存。安装后验证实际应用元数据与系统登记路径一致；验证失败恢复原包。备份不使用可登记的 `.app` 后缀，成功后清理，日常更新不得清除权限或重启其他应用。
- 截图和复制用的图片保存在本机内存中，关闭贴图后释放；系统剪贴板的内容由系统管理。不上传、不写入日志。
- 系统截图后端若生成临时文件，应在读取后立即清理，不保留截图历史。
- 尊重系统权限和桌面限制，不绕过屏幕录制、Portal 或安全设置。

## 需求理解与实现

- 实现前先弄清用户要解决什么问题、为什么需要这个行为，以及怎样才算使用顺畅。区分用户的目标与用户提出的具体做法，不能把具体做法当成需求的全部；实现和验证都要围绕目标展开。
- 新方案或评审建议与用户先前指令、已确认需求冲突时，说明冲突、依据与推荐方案，向用户确认；确认前不实施冲突部分，其他独立工作继续。不能擅自覆盖旧要求，也不能仅因旧要求而默认否决新方案。
- PR 正文和评论面向不了解内部沟通的贡献者，只保留问题背景、设计依据、实现、修复的 bug，以及必要的验证结论和限制。技术评审发现放在对应 PR 评论中；仓库文档只保留当前使用方法、开发命令、架构与长期维护规范。
- 公开引用 commit、PR、Issue、CI 运行或代码位置时使用可点击链接，便于追溯；commit 用短 SHA 作链接文字并指向完整 SHA，PR／Issue 使用编号作链接文字。发布前确认链接目标真实存在。
- 不得将聊天内容、指令原文、个人信息、偏好与授权经过、本地操作／安装记录、内部协作过程、agent 角色或模型配置写入公开 PR、Issue、提交说明等。公开内容须先提炼为必要的项目技术信息，不直接复制对话或内部评审报告；验收仅记录提交、平台、场景和技术结论。
- 方案必须考虑通用性和可维护性，覆盖不同平台、显示器布局、DPI、缩放比例、图片尺寸和边缘位置。把共同规则封装在合适的模块中，平台差异留在适配层；保持项目简洁，不为假设中的需求增加抽象或功能。
- 禁止通过硬编码位置、偏移量或针对某个场景的特殊数值修补现象。坐标和布局应从实际边界及统一的坐标换算推导，样式参数和阈值应有明确含义并集中管理。仅把散落数字改成常量，不能代替对模型和计算方式的改进。
- 出现偏移或位置异常时，先定位坐标单位、DPI、缩放、窗口边框、原生系统约束及生命周期中的根因。不得用反复移动、缩放、重试或轮询来掩盖错误；之前用定位重试补偿偏移既未解决根因，又引入了死循环问题，不能再采用这种做法。
- 图标放到图片下方的目的是让用户操作更顺畅：图片移动时复制和关闭图标仍应留在屏幕内并可操作。“顶部时把图标放下面”只是实现手段；macOS 顶部应让完整图片贴紧菜单栏下缘，不能被旧的透明上边距阻挡，也不能用裁切实现。其他方向应保留图片越界移动能力。
- 验证用户最终能否完成操作，而不只验证中间数值或某个分支。涉及窗口行为时，检查真实的平台约束和完整交互路径；布局计算正确或隐藏窗口能设置位置，不代表用户拖动时一定正确。明确区分自动化验证与用户实际交互验证，发现缺口时补足测试。
- 确认原始问题真正消失前不得合并或启用自动合并，证据不足的修复保持 Draft PR。闪烁、跳动、瞬时偏移须验证可见原生窗口连续操作的中间帧；自动检查、网页或隐藏缓冲区测试通过不能视为真实交互通过。用户不允许操控屏幕时先安装验证构建，等待用户明确确认；仍能复现就继续修复，不先合入或宣称完成。验收记录关联具体提交，后续影响该行为的改动须重新验证。
- 原生窗口是移动、缩放和当前几何的唯一权威；Rust 只保留不可变原图、初始放置与图内按钮布局。删除应用层的拖动锚点、缩放求解、重复期望 Rect 和反馈补偿；跨 DPI 先统一坐标单位。
- 窗口与渲染时序按实际后端验证，包括 egui 多 pass 重绘和连续原生事件。图片绘制不能产生主窗口几何副作用，不能依赖某个可能被丢弃的 UI pass 提交 drawable 补丁。不得用巨大透明窗口、手动更新 GL context 或定位重试掩盖问题。
- 截图线程使用有界队列，取消立即恢复 UI、丢弃晚到结果并阻止尚未开始的后续屏幕采集，不承诺强停正在执行的系统调用。像素转换在工作线程完成，截图数据与纹理上传共享不可变图片；上传前验证 GPU 纹理边长上限。多屏并行不跨线程搬运非 Send 原生句柄，Linux Portal 不并发请求。
- GUI 沙箱复用实际 Rust 视图和几何，不复制一套 HTML 产品界面；仅使用生成图片，禁止真实截图、系统剪贴板、权限、自启和全局快捷键操作。多 agent 评审使用各自独立的 headless 浏览器上下文；浏览器交互和隐藏原生几何验证都不能替代合成器视觉验收。

## Structure

- `src/capture.rs`, `src/capture/worker.rs`: monitor enumeration, shared in-memory pixels and bounded cancellable capture work.
- `src/geometry.rs`: validated pixel selection and original-position pin placement.
- `src/pin.rs`: initial proportional fitting, display coordinate conversion and adaptive visible image controls placement.
- `src/app.rs`: capture state machine, events, and window lifecycle.
- `src/ui/`: launcher, selection overlay, floating image views, shared style profiles, and copy artwork.
- `preview/`: local WebAssembly sandbox using the same Rust UI and generated images; no native services.
- `src/app/tests.rs`, `src/ui/pin/tests.rs`, `src/ui/launcher/tests.rs`: lifecycle and view regressions, separate from production logic.
- `src/platform.rs`: keyboard shortcut and local system-font adapters.
- `src/platform/macos.rs`: floating level only, preserving backend classes and observation lifecycle.
- `src/settings.rs`: shortcut validation, theme/icon preferences, and local storage.
- `src/permission.rs`: user-driven authorization state without native calls or polling.
- `src/platform/macos_access.rs`: main-thread permission requests, settings navigation, and installation/identity migration checks.
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

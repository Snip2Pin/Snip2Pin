# 开发与发版流程

Snip2Pin 使用短分支、PR 和三平台检查，只有用户明确要求时才进入发版流程。
开发构建、未签名候选包和最终安装包分开管理。目前只计划通过 GitHub 分发安装包，
不上 App Store，暂不开通付费签名服务。默认 `unverified`，不需要生产凭据；未来取得
证书后可显式选择 `signed`，由受审批保护的 Actions 签名。本地开发身份只留在本机。
项目采用 MIT 许可证。产品范围和实现约束见 [AGENTS.md](../AGENTS.md)。

## 日常开发

1. 明确用户要完成的操作及验收标准。较大功能先开 Issue，小修复直接在 PR 描述。
2. 从最新 `main` 建立短分支，一次处理一个问题，不设长期 `develop` 分支。
3. 用可复现的测试定位根因，验证外部行为；不要用重试、轮询或硬编码偏移掩盖问题。
4. 运行 `cargo xtask check`，提交有意义的 Git 历史，创建 PR。
5. 按原始复现场景验收，确认问题真正消失，记录自动检查和真实交互的不同覆盖范围；满足下述合并门槛并解决审阅讨论后 squash 合并。

### 修复验收与合并门槛

修复必须验证用户最初遇到的现象已消失，不能用 CI 通过、几何数值正确、网页测试
或隐藏窗口缓冲区读回通过，代替可见原生窗口的交互结果。涉及闪烁、跳动、瞬时偏移
或窗口生命周期时，验收须覆盖连续操作的中间帧，不能只看操作结束后的静态画面。

涉及输入到窗口命令的改动，自动回归须经过生产贴图回调、Action/Event 分派及根逻辑，
覆盖过期的根窗口几何、DPI/界面缩放、跨 viewport 续传及消费时锁定检查；保留底层原生隔离测试，
不能只用各模块分别通过的测试代替实际调用链。

证据不足或原始场景尚未验收时，PR 保持 Draft，评论注明待验证场景，不合并、
不启用自动合并、不宣称修复完成。先提供本地验证构建；用户不允许操控屏幕时，
由用户验证并明确确认原问题消失，不能以无法自行操作为理由降低验收要求。
用户仍能复现时继续在该 PR 排查，保留失败证据，不先合入再处理。
验收结论对应具体提交；后续改动影响已验收行为时重新验证。完整记录放 PR 评论。

PR 正文和评论只保留问题背景、设计依据、实现、修复内容及必要的验证结论和限制。
技术评审发现放在对应 PR 评论；不得公开聊天内容、个人信息、偏好与授权经过、
本地操作记录或内部协作与模型配置，也不直接粘贴内部评审报告。验收仅记录提交、
平台、场景和技术结论；仓库只保留当前使用、开发、架构和维护文档。
新方案与先前确认的需求冲突时，先说明原因和取舍并询问用户，
确认前继续其他独立工作，不擅自更改或默认否决方案。

`main` 禁止直接推送、强推和删除，要求四个原生目标及依赖检查通过，且分支包含
最新主分支。单人维护不要求另一人审批；增加协作者后再启用至少一人审阅。
规则定义保存在 `.github/rulesets/`；修改文件后仍需通过 GitHub API 应用并读回核对。
仓库只允许 squash 合并，合并后删除远端功能分支。

PR 标题及分支内每个 commit 的标题使用 Conventional Commits：
`type(scope)!: description`，scope 和 `!` 可省略。type 只允许 `feat`、`fix`、
`docs`、`refactor`、`perf`、`test`、`build`、`ci`、`chore`、`revert`；scope
使用小写字母、数字和连字符。描述可用中文或英文，整行不超过 100 字符且不能空白。
例如 `fix(pin): 修复顶部按钮位置`、`ci: automate signing`。正文按需解释原因；
不兼容变更使用 `!` 并在正文注明 `BREAKING CHANGE:`。PR 正文说明问题、结果、范围、
关联 Issue 和验证缺口。独立 `messages.yml` 在 PR 更新和标题修改时检查，失败阻止合并；
修改不合规提交后使用安全的分支更新，不强推 main。squash commit 沿用 PR 标题和正文。

## 统一验证

首次开发安装提交钩子（先确保 Rust 工具链、Git LFS 在 `PATH` 中）：

```sh
uv tool install pre-commit==4.7.0
cargo install cargo-deny --version 0.20.2 --locked
pre-commit install
pre-commit run --all-files
```

`.pre-commit-config.yaml` 固定外部钩子版本。每次提交检查 YAML/TOML、Rust 格式、
Clippy、合并冲突标记、大文件，以及 cargo-deny 的安全公告、许可证、来源策略。
通用检查复用官方 pre-commit-hooks；尾随空白和文件末尾换行由官方钩子自动修正，
修改文件后阻止当次提交，需检查并重新暂存。大文件阈值为 500 KiB，使用 Git LFS
管理的文件由大小检查跳过；`--enforce-all` 使本地与 CI 全量检查均有效。
Rust 的 local hook 只调用 `cargo xtask lint`，复用 CI 的格式、Clippy、diff 和打包元数据检查，
不在钩子配置里重复维护检查逻辑。其他检查失败也会阻止提交。
钩子运行时缺少 Rust/cargo-deny 或网络检查失败时
如实报错，不静默跳过。依赖审核与 CI 使用 cargo-deny 0.20.2。

已有工具管理 `core.hooksPath` 时保留它，通过该工具支持的方式调用项目配置，
不能为安装新钩子覆盖现有安全或 LFS 钩子。安装后实际提交一次，确认检查被触发。
本地钩子属于提前反馈，不能替代 CI 和原生交互验收，也不启用发布操作。

```sh
cargo xtask lint
cargo xtask check
cargo xtask check-msrv
cargo deny check
```

`lint` 运行格式、Clippy、diff 与 macOS 脚本/模板静态检查；`check` 复用 `lint`，
再运行工作区测试、开发构建和 LFS 检查。工具自身的回归测试也属于工作区测试。
CI 同时检查 pre-commit 配置并全量运行官方通用钩子，避免本地钩子漏装或跳过时遗漏。
`check-msrv` 从 Cargo.toml 读取
最低 Rust 版本并检查所有目标；开发工具链只在 rust-toolchain.toml 固定。

CI 对 PR、`main` 推送和手动验证运行，取消同一分支的过期检查。所有 Actions 固定
完整提交 SHA，普通任务只读仓库，不运行真实截图、不申请权限、不安装应用、不使用证书签名、
不上传安装包。缓存按平台、架构、工具链和锁文件隔离，不能依赖缓存正确性。
CI 在四种原生 runner 上验证开发包及 unverified 最终包结构、完整性和 `--version`，
macOS 测试可使用不带证书的 ad-hoc 完整性签名，不访问本机或生产身份，不显示 GUI。

WebAssembly 检查独立编译共享 UI，避免只在网页入口使用的代码漏检。GUI 沙箱复用
Rust 视图，使用生成图片，不提供真实截图、系统剪贴板、自启或全局快捷键注册。
网页宿主也在 `cargo test --workspace` 中运行：直接驱动同一 Rust 界面，验证焦点、菜单、
快捷键录制、晚到截图结果、锁定与主题变化。跨端修改同时检查共享规则和宿主事件流，
不能仅以共用绘制函数或 WASM 编译通过作为行为一致的证据。
本地 `packaging/macos/build.sh --install` 使用优化构建；这属于开发安装，不是发版。

### 本地 macOS 菜单资源

Dock 菜单使用 `packaging/macos/DockMenu.xib`，打包时由 Xcode 的 `ibtool` 编译，
通过 NSApplication 的官方 `dockMenu` outlet 连接，不替换窗口后端的 delegate。
macOS 打包需要安装完整 Xcode，Command Line Tools 本身不包含 `ibtool`。
完成 Xcode 的首次初始化后确认工具可用，再构建：

```sh
xcrun --find ibtool
bash packaging/macos/build.sh --install
```

打包时从 XIB 源码编译菜单，不提交二进制 NIB，不需要额外的资源下载流程。

### 本地 GUI 沙箱

安装锁文件匹配的 wasm-bindgen-cli，传入本机 CJK 字体后构建，仅在回环地址访问：

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.129 --locked
cargo xtask preview '/System/Library/Fonts/STHeiti Light.ttc'
python3 -m http.server 8765 --bind 127.0.0.1 --directory preview
```

Windows/Linux 使用本机 `.ttf`、`.ttc` 或 `.otf` 字体路径。生成文件与字体位于
Git 忽略的 `preview/pkg/`，不提交或对外发布。自动化使用独立 headless 浏览器上下文，
不连接用户的现有浏览器；网页与隐藏窗口检查不能替代原生合成器交互验收。

沙箱使用原创壁纸和虚构窗口模拟桌面，主页、框选与贴图复用实际 Rust 视图。
截图仅冻结网页画布，不读取真实桌面；复制、自启与快捷键设置只在沙箱内模拟。
壁纸由浏览器解码，不进入原生应用资源或依赖。
顶部菜单可切换数据、代码和待办场景，示例窗口使用标准 egui 控件，可拖动或切换页签。
调整画布尺寸会取消正在进行的框选，避免使用旧窗口坐标。

### GitHub Pages 交互演示

公开网页使用独立构建入口，不上传本地沙箱目录或系统字体。需要 Python 3.12、
上文的 WASM 工具链及 `cargo-about 0.9.2`：

```sh
python3 -m pip install --require-hashes -r preview/requirements.txt
cargo xtask preview-site
python3 -m http.server 8766 --bind 127.0.0.1 --directory .build/site
```

构建使用 `web` 优化配置，从固定上游提交下载 Noto Sans SC 并校验 SHA-256。
只保留常用简体字、界面文案及示例所需字形，固定为 400 字重并重命名为
`Snip2Pin Preview Sans`，保留原字体版权与 OFL 许可。原生应用不打包这份字体。
输出 `.build/site/` 包含网页、WASM、壁纸、字体、项目许可证及实际 Web 依赖声明，
不包含本地字体、设置或源码；第三方声明由现有许可收集器按 WASM 依赖图生成。

`pages.yml` 在 PR 中构建验证，在 `main` 更新或手动运行时部署。仓库 Pages 来源设置为
GitHub Actions，`github-pages` 环境仅允许 `main`；只有部署任务持有 Pages 写权限与
OIDC 权限。它只发布交互演示，与桌面安装包、签名和 GitHub Releases 流程独立。
首次部署成功并验证实际页面后，再将演示链接加入 README。

Dependabot 每周提交 Cargo 和 Actions 更新 PR，不自动合并；Cargo.lock 必须提交。
`deny.toml` 检查 RustSec、依赖许可证及来源，重复版本作为提示；不得为通过检查
整体关闭策略。确需忽略公告时注明公告 ID、影响分析、上游修复和复查日期。
依赖许可证允许列表不改变项目 MIT 许可证，也不免除发行时保留第三方声明的要求。

## 支持的发行包

| 原生目标 | 包格式 | 共有校验；signed 模式额外校验 |
| --- | --- | --- |
| aarch64-apple-darwin | DMG，含 `.app` 与 Applications 链接 | 只读挂载、完整性、架构、标识、版本；signed 同时校验应用及 DMG 的签名与公证票据 |
| x86_64-apple-darwin | DMG | 同上 |
| x86_64-pc-windows-msvc | 便携 ZIP、NSIS `-Setup.exe` | 结构、版本、安装/卸载；signed 校验应用、嵌入卸载程序及安装程序的 Authenticode 与时间戳 |
| x86_64-unknown-linux-gnu | tar.gz、`.deb` | 动态库依赖、版本、Debian 元数据及两种格式的有效载荷一致性 |

macOS 最低部署目标为 13.0；最低系统仍须真实机器验收。Linux 构建基线为 Ubuntu
24.04（glibc 2.39），不承诺可在更旧系统直接运行；Wayland 的置顶、快捷键和定位
能力取决于合成器。`.deb` 在同一 Ubuntu 24.04 runner 上由 `dpkg-shlibdeps` 推导 ELF
依赖，并声明运行时加载的桌面库；不承诺所有 Debian 版本兼容。Windows 安装到当前
用户目录，不要求管理员权限，不自动启动应用或启用自启；便携版移动路径后需重新设置自启。
Windows MSVC 目标静态链接 C 运行库；CI 读取优化 EXE 的真实 PE 导入表，拒绝依赖
需另装的 Visual C++ 运行库 DLL，避免开发机自带的运行库掩盖安装依赖。该部分计入体积预算。
NSIS 3.13 从官方归档按固定 SHA-256 安装，编译器版本也会检查。日常 CI 检验临时
安装、卸载及无关文件保留，不显示安装界面；Windows 安装验收仅在 GitHub 托管 runner
执行，本机打包不自动安装。Linux 另在固定 digest 的干净 Ubuntu 24.04 容器中解析
依赖、安装、重复安装并卸载 `.deb`，只运行 `--version`，不启动桌面。
暂不增加 MSI、AppImage 或自动更新。

`unverified` 是明确选择的分发方式，文件名保留 `-unverified`，清单记录相同模式。
macOS 只有 ad-hoc bundle seal，能够校验完整性但没有受信任的发布者身份，且未经
Apple 公证；Windows 必须确认为未签名。发布说明自动加入中英文信任及权限说明，
发布前检查该说明未被移除。不清除 quarantine、不禁用系统保护或自动信任自签证书。
macOS 首次打开被阻止时由用户按[Apple 官方指引](https://support.apple.com/102445)
在系统设置里确认，设备管理策略可能禁止放行。未公证包更新可能需要重新授权截屏；
本地固定证书的开发更新不受此流程影响。`signed` 失败必须停止，不自动降级。

版本唯一来源为 Cargo.toml，macOS 模板版本在打包时生成，构建号为完整 Git 历史中
该提交的计数。首期发版入口只接受稳定版 `MAJOR.MINOR.PATCH`，不带 `v`；Git tag
为 `vMAJOR.MINOR.PATCH`。不在普通提交中自动升级版本，也不发布 crate。

### 体积检查

PR CI 在四个原生目标上运行 `cargo xtask check-release-size TARGET`，只构建
发行优化配置的可执行文件和临时安装内容并记录字节数，不创建候选包、上传文件或执行发版。
完整 payload 递归统计普通文件、不跟随符号链接；打包时另行记录候选归档及
每一种最终安装包的字节数，结果写入日志和 GitHub job summary。
压缩下载大小与安装后的 payload 大小分别验收。

预算以 `xtask/src/sizes.rs` 为准，根据四个原生 runner 的发行构建实测设置，
不以调试包推算。系统提供的动态库和字体不属于 payload，包管理器的额外下载与
占用仍需在安装验收中核对。

| 目标 | 可执行文件上限 | payload 上限 |
| --- | --- | --- |
| macOS ARM | 10 MiB | 12 MiB |
| macOS Intel | 12 MiB | 14 MiB |
| Windows x64 | 12 MiB | 14 MiB |
| Linux x64 | 20 MiB | 22 MiB |

新增依赖、内嵌资源或预算调整须说明体积变化和理由，不使用
UPX 等压缩壳掩盖体积，也不牺牲启动速度、可访问性或必要的平台支持。

## 发版前准备

仅在用户明确要求发版后执行本节及后续步骤。以下命令是操作手册，不是授权。

1. 建立版本 PR，更新 Cargo.toml、Cargo.lock 及简短中英文发布说明，记录破坏性变化。
2. 合并 PR，等待该 main 提交的全部 CI 通过（自动 push 或 main 上手动触发的 CI 均可，
   PR 检查不能代替），记下完整 SHA。发布说明放在已提交的
   文件中，例如本次发版专用 `docs/releases/VERSION.md`；不保留空白模板或虚构版本。
3. 按许可证核查依赖和字体/图标的第三方声明，确保发行物包含所需许可文本。
4. 默认选择 `unverified`，无需签名 Secrets；只有选择 `signed` 时，才按下一节配置
   正式凭据，缺少或无效则停止。GitHub 站外分发的 Developer ID/公证同样需要 Apple
   Developer Program 会员，不上 App Store 并不免除这项要求。

## 可选：配置 signed 模式的发行凭据

在仓库 Settings → Environments → `release` 配置下列 Secrets 和 Variables。
该环境只允许 main，要求维护者批准且管理员也不能跳过；单人维护允许本人审批。
Secrets 加密存储但获准运行的代码能够读取明文，因此审批时应核对工作流及所选源码。
不使用仓库级或组织级共享签名 Secrets，不将秘密写入 Issue、PR、聊天、日志或文件附件。

| 类型 | 名称 | 内容 |
| --- | --- | --- |
| Secret | `MACOS_CERTIFICATE_P12_BASE64` | 含私钥和证书链的正式 Developer ID Application `.p12` 的 Base64 |
| Secret | `MACOS_CERTIFICATE_PASSWORD` | 上述加密 P12 的密码 |
| Secret | `MACOS_NOTARY_KEY_P8` | App Store Connect 团队 API Key 的 P8 原文 |
| Secret | `MACOS_NOTARY_KEY_ID` | API Key ID |
| Secret | `MACOS_NOTARY_ISSUER_ID` | 团队 Issuer ID |
| Variable | `MACOS_CERTIFICATE_SHA1` | 所选正式证书的 40 位 SHA-1 指纹 |
| Secret | `WINDOWS_CERTIFICATE_PFX_BASE64` | 证书提供方允许导出的正式代码签名 PFX 的 Base64 |
| Secret | `WINDOWS_CERTIFICATE_PASSWORD` | PFX 密码 |
| Variable | `WINDOWS_CERTIFICATE_SHA1` | 所选正式代码签名证书的 40 位 SHA-1 指纹 |
| Variable | `WINDOWS_TIMESTAMP_URL` | 提供方的 HTTPS RFC 3161 时间戳服务 URL |

使用 GitHub 环境设置或 `gh secret set NAME --env release` 从安全输入配置，禁止把
秘密作为命令行字面值。Base64 只是编码，不能代替 Secrets 加密。API Key 权限仅满足
公证；证书轮换时同步更新密码和指纹，验证成功后撤销旧凭据。不得在仓库内暂存导出文件。

本机现有自签证书不能用于 Apple 正式公证，也不应上传；Developer ID 需另行取得。
Windows 此入口仅支持合法可导出的代码签名证书。硬件/HSM 或云端不可导出证书不能
转换成 PFX；使用这类服务时需接入对应签名 API（优先 OIDC），当前入口不会冒充支持。
未配置齐两端凭据前不能使用 `signed`；不影响显式选择 `unverified` 分发。

参考：[GitHub 证书导入](https://docs.github.com/en/actions/how-tos/deploy/deploy-to-third-party-platforms/sign-xcode-applications)、
[环境审批和秘密保护](https://docs.github.com/en/actions/reference/security/secure-use)、
[Apple Developer ID](https://developer.apple.com/developer-id/)。

## 构建候选包

在 GitHub Actions 手动运行 **Build release candidates (manual)**，选择 `main`，
填写版本和已包含在 `main` 中的完整源提交 SHA。工作流检查版本、干净检出、项目许可证、来源
及这个提交的成功 CI，并重新拉取公告库审计依赖；允许 `main` 已经前进，构建始终检出指定提交。四个原生 runner
运行相同的 Rust 打包工具：

```sh
cargo xtask package TARGET release
```

候选包在 Actions artifacts 保留 14 天，不创建 tag 或 Release。文件名标明
`-unsigned`，旁边的目标 JSON 记录版本、提交、目标和 SHA-256。过期后重新手动构建，
不要改用不同提交的包。日常 CI 用 `debug` 检验同一条打包路径，并且不上传这些包。

## Actions 准备安装包与草稿

手动运行已有的 **Manage release packages (manual)**（`publish.yml`），选择 `main`，
`operation` 选择 `prepare`（默认），填写版本、完整源 SHA、
上一节成功的候选构建 run ID 和 `distribution`（默认 `unverified`）。所选源提交必须已包含本流程及对应的
`docs/releases/VERSION.md` 发布说明。校验任务不读取签名秘密，先检查来源、确切提交
的成功 CI、四个原始候选包的版本和哈希，并重新审计依赖。

维护者在 `release` 环境批准四个平台任务后，`unverified` 模式不读取任何 Secrets，
macOS 对候选应用加 ad-hoc 完整性签名并创建 DMG，Windows/Linux 从原候选有效载荷生成全部格式，
生成明确标识为 unverified 的最终包、SHA-256 及来源清单。

只有 `signed` 模式中，macOS 将 P12 导入临时钥匙串，用
临时 API Key 文件配置公证凭据，完成 Developer ID 签名、时间戳、公证和票据附加；
Windows 临时导入 PFX，按指定指纹用 SDK SignTool 签名应用、NSIS 嵌入卸载程序和最终安装程序，验证时间戳；Linux 只生成
最终包和清单。不会重新编译应用。秘密仅传给各自的签名步骤，不传给下载/上传步骤。
无论成功或失败都清理临时钥匙串、导入的证书私钥和凭据文件；任务取消/强制结束时
依赖 GitHub 托管临时虚拟机销毁兜底，不在日常电脑部署自托管 runner。

四个平台按所选模式完成原生校验后，上传仅含安装包和 JSON 的 `final-*` artifacts
（保留 14 天），随后自动创建 Draft Release。任一平台失败均不创建草稿，也不发布。
包清单保留原候选哈希，正式发布时再次和原候选 run 比对。现有草稿/标签不会被覆盖。
签名任务可读取秘密，因此批准前应检查所选源码和工作流，不能批准不可信改动。
此入口只创建草稿，不能代替后续实际安装验收和手动发布确认。JSON 使用 schema 3，
每目标清单包含共享版本、源 SHA、目标、分发模式、原候选哈希及 `artifacts` 列表。
每个 artifact 包含类型、固定文件名及 SHA-256；候选仅一个归档，最终严格要求表中全部
格式，拒绝重复、缺失、混合模式、路径穿越及符号链接。四目标共六个最终文件、四个清单。
候选清单的 distribution 为 null，最终清单为 signed 或 unverified；旧清单需重新构建。

## 可选：本机 signed 签名备用入口

将四个 artifact 分别解压到不同目录。使用同一提交的干净源码运行下面的工具。
本机使用的凭据保存在仓库之外的钥匙串、证书存储或硬件中，不传入命令行密码，
不在公共 PR runner 或你的日常电脑上安装自托管 runner。

macOS 在本机钥匙串中配置 Developer ID 与 notarytool profile，然后对两个架构分别执行：

```sh
# 环境变量只包含身份名/profile 名，不是密码。
bash packaging/macos/distribute.sh CANDIDATE_DIR dist/final
```

设置 `SNIP2PIN_SIGN_IDENTITY`、`SNIP2PIN_NOTARY_PROFILE`；脚本检查候选哈希，
先完成应用签名、公证及票据，再生成 DMG，对外层 DMG 签名、公证及附加票据后生成清单；此后不再修改内容。
公证将已签名应用交给 Apple 检查，不上传私钥或钥匙串。当前开发自签身份继续用于
本地更新；第一次切换正式发行身份可能需要重新授权，不能承诺无感保留 TCC 权限。

Windows 在有 Windows SDK SignTool 的本机 PowerShell 7 中执行：

```powershell
./packaging/windows/distribute.ps1 -Candidate CANDIDATE_DIR -OutputDirectory dist/final -CertificateThumbprint THUMBPRINT -TimestampUrl RFC3161_URL
```

私钥留在当前用户证书存储或硬件提供方，工具不接收 PFX 或密码。签名和时间戳验证
失败会停止。打包时校验三个可执行文件的签名，不自动安装；`verify-native` 的 Windows
安装/卸载冒烟检查只在 GitHub 托管 runner 上执行。有效签名不代表已建立 SmartScreen 信誉。

Linux 候选包无需平台证书；在安装好 `dpkg-dev` 的 Ubuntu 24.04 上生成 tar.gz 与 `.deb`：

```sh
cargo xtask prepare-linux CANDIDATE_DIR dist/final signed
```

签名完成后不得改动应用内容；修改需回到构建步骤。最终清单与安装包一起保留，
包含原候选包的 SHA-256；正式发布时还会对照候选构建 run 的原始 artifact 核对来源。
第三方声明使用固定版本 cargo-about 按应用的四平台普通依赖生成，排除开发／构建工具。
`about.toml` 校验上游许可来源；少数仅声明许可的上游版本使用经核对的版本、源码提交和
manifest 哈希，保留原始声明并明确标注标准 SPDX 参考文本。未知回退会阻止打包。
内置字体的四份声明及 Windows 安装器 NSIS 运行时声明单独保留；来源和更新方法见
[许可收集说明](../packaging/licenses/README.md)。自动收集不能代替发版前的许可核对。

## 可选：公开发版后的 AUR 配方

在已安装 `makepkg`、`gh` 和本项目 Rust 工具链的 Arch Linux 环境中，手动执行：

```sh
cargo xtask aur-package VERIFIED_FINAL_DIR NEW_AUR_DIR
```

命令先校验本地 schema 3 Linux 最终清单和全部文件，再检查同版本 GitHub Release
已公开、非草稿且非预发布，下载确切名称和 URL 的 tar.gz 并比较 SHA-256。随后生成
`snip2pin-bin` 的 `PKGBUILD`，由 `makepkg --printsrcinfo` 生成 `.SRCINFO`。配方固定
源 URL 和真实校验和，不使用 `SKIP`，保留 MIT 许可及第三方声明。该命令需要网络，
只写新目录，不上传 AUR、不推送仓库、不新增自动化。维护者须在 Arch 上检查依赖并
实际 `makepkg` 安装、升级和卸载后自行提交 AUR；生成配方不代表已完成兼容性验收。

格式依据：[Apple 分发打包](https://developer.apple.com/documentation/xcode/packaging-mac-software-for-distribution)、
[NSIS 文档](https://nsis.sourceforge.io/Docs/Chapter5.html)、
[dpkg-deb](https://manpages.debian.org/bookworm/dpkg/dpkg-deb.1.en.html)、
[dpkg-shlibdeps](https://manpages.debian.org/bookworm/dpkg-dev/dpkg-shlibdeps.1.en.html)、
[Arch PKGBUILD](https://wiki.archlinux.org/title/PKGBUILD)。

## 安装与交互验收

在各平台从实际包安装或解压运行，记录系统版本、架构、包哈希和结果：

- 首次安装、覆盖上一发行版、退出后重开；配置、主题、快捷键和自启选择保留。
- 首次截图、拒绝后再次授权、撤销权限后的恢复；普通更新不反复丢失授权。
- 多显示器和不同 DPI 下框选、初始放置、原图复制、系统拖动与边缘缩放、修饰键原生行为；图内按钮悬停显示、遮挡时移动、点击不触发拖动，以及顶部贴菜单栏。
- 新建贴图后的首次拖动、应用失焦后的首次点击拖动和后续拖动都应保持鼠标抓取点，不能把图片左上角跳到鼠标处；快速按下松开不启动过期拖动。顶部框选须验证首次显示时的位置，不能以手动移到菜单栏后的结果代替初始放置验收。
- 使用带文字或网格的生成图片验证普通、窄、极小和极端长宽比选区：小图不得放大，文字密度和初始尺寸须与框选一致；超出可用区时仅等比缩小。分别检查横排、窄图逐个隐藏按钮和极小图右键菜单的复制、关闭、Esc 与失焦。仅验证按钮放得下不能代表截图显示正确。
- GUI 依赖升级后确认原生剪贴板功能仍启用；实际验证自动复制和手动复制得到的图片保持原始像素。输出命令的单元测试不能证明平台剪贴板后端可用，自动检查不得读取或覆盖真实剪贴板。
- Retina 奇数像素边界、反向框选、不同界面缩放：拖动预览、松手裁剪和初始原生内容区应使用相同边界。记录原生后端的坐标精度及尚未验证的平台差异，不能用隐藏窗口结果宣称合成器视觉验收通过。
- macOS 菜单栏/Dock 不被普通贴图遮挡，图标更新符合已说明的安装行为。
- 托盘、全局快捷键、取消、关闭全部贴图和退出；自启默认关闭。
- Linux 分别记录 X11 与实际验证的 Wayland 桌面，明确限制，不笼统宣称兼容。

CI 的隐藏窗口和包检查不能替代这些交互。测试记录不包含真实桌面截图或剪贴板内容。

## 草稿与正式发布

准备工作流已创建草稿时直接进行实际安装验收，然后运行下述发布工作流。
仅使用本机备用签名入口时，将四个最终包及四份 JSON 收集到 `dist/final`，运行：

```sh
cargo xtask verify dist/final
cargo xtask stage VERSION FULL_SHA dist/final docs/releases/VERSION.md signed --confirm
```

只有这个显式命令才创建 Draft Release；它要求四个目标齐全、版本和提交一致、哈希
正确、CI 通过且版本未使用。不存在完整产物或真实验收结果时，不运行它。

再次手动运行 **Manage release packages (manual)**，`operation` 选择 `publish`，填写版本、完整 SHA、成功候选构建
run ID、与草稿一致的 `distribution`，以及 `publish VERSION` 验收确认。工作流先对源提交重新审计依赖，
再按明确选择的模式重验原生完整性、版本、架构、依赖及签名状态；signed 还须验证
时间戳和公证，模式不一致或混用即失败。随后冻结通过验证的文件。`release` Environment
等待维护者批准，发布任务重新下载 Draft 并逐文件比对，确认没有中途更换后才发布。
本次通过后不重新构建；tag 指向同一个源提交，发布后不能重写或删除版本 tag。

`release` Environment 只允许 main 手动任务，要求维护者批准；单人维护允许本人批准
自己触发的任务，但不能省略这一步。普通任务只读；读取 Draft 的签名校验任务因
GitHub 的草稿访问要求使用写 token，仅在下载步骤传入，其他步骤不持有该环境变量。
不储存长期 PAT；正式签名 Secrets 仅在签名工作流的原生签名步骤显式注入。

## 失败与恢复

任何平台失败则不发布，已成功的平台包仍是候选产物。验收未通过时删除本机临时产物
并修复，不能把 signed 任务临时改成 unverified 来忽略失败，也不能替换已发布文件。
版本已发布后发现问题，保留原 tag/安装包与问题说明，另发修复版本；必要时把旧的
可用版本重新标为 latest。旧版本运行前先核对配置兼容性，不清理用户配置或权限。

## 自动化边界

- `ci.yml`：日常开发检查和开发包结构测试，无分发上传。
- `messages.yml`：PR 标题和分支提交格式，标题编辑也会重查，无秘密和写权限。
- `candidates.yml`：仅手动生成指定提交的未签名候选包，无发布权限。
- `publish.yml`：已有的手动发行入口，prepare 操作校验来源并在审批后准备安装包和草稿；
  publish 操作另行验收并发布已有草稿，再经过 Environment 批准。两项操作不能在一次运行中连发。
- `xtask/`：共用检查、版本元数据、包结构、校验清单和发行输入验证。
- `packaging/`：各平台必须原生执行的签名/公证接口；不得复制业务逻辑。

流程文件可以实现并测试，但创建版本、构建正式候选、签名公证、创建草稿和发布
始终需要用户的明确发版要求。实现流程本身不构成发版授权。

# 开发与发版流程

Snip2Pin 使用短分支、PR 和三平台检查，只有用户明确要求时才进入发版流程。
开发构建、未签名候选包和正式发行包分开管理，签名私钥及密码始终保留在本机。
项目采用 MIT 许可证。产品范围和实现约束见 [AGENTS.md](../AGENTS.md)。

## 日常开发

1. 明确用户要完成的操作及验收标准。较大功能先开 Issue，小修复直接在 PR 描述。
2. 从最新 `main` 建立短分支，一次处理一个问题，不设长期 `develop` 分支。
3. 用可复现的测试定位根因，验证外部行为；不要用重试、轮询或硬编码偏移掩盖问题。
4. 运行 `cargo xtask check`，提交有意义的 Git 历史，创建 PR。
5. 记录自动检查和真实交互的不同覆盖范围，解决审阅讨论后 squash 合并。

`main` 禁止直接推送、强推和删除，要求四个原生目标及依赖检查通过，且分支包含
最新主分支。单人维护不要求另一人审批；增加协作者后再启用至少一人审阅。
规则定义保存在 `.github/rulesets/`；修改文件后仍需通过 GitHub API 应用并读回核对。
仓库只允许 squash 合并，合并后删除远端功能分支。

## 统一验证

```sh
cargo xtask check
cargo xtask check-msrv
cargo deny check
```

`check` 顺序运行格式、Clippy、工作区测试和开发构建，再检查 LFS、diff 与 macOS
脚本/模板。工具自身的回归测试也属于工作区测试。`check-msrv` 从 Cargo.toml 读取
最低 Rust 版本并检查所有目标；开发工具链只在 rust-toolchain.toml 固定。

CI 对 PR、`main` 推送和手动验证运行，取消同一分支的过期检查。所有 Actions 固定
完整提交 SHA，普通任务只读仓库，不运行真实截图、不申请权限、不安装应用、不签名、
不上传安装包。缓存按平台、架构、工具链和锁文件隔离，不能依赖缓存正确性。
CI 在四种原生 runner 上验证开发包结构和 `--version`，不会显示 GUI。

Dependabot 每周提交 Cargo 和 Actions 更新 PR，不自动合并；Cargo.lock 必须提交。
`deny.toml` 检查 RustSec、依赖许可证及来源，重复版本作为提示；不得为通过检查
整体关闭策略。确需忽略公告时注明公告 ID、影响分析、上游修复和复查日期。
依赖许可证允许列表不改变项目 MIT 许可证，也不免除发行时保留第三方声明的要求。

## 支持的发行包

| 原生目标 | 包格式 | 正式发布前的校验 |
| --- | --- | --- |
| aarch64-apple-darwin | `.app.zip` | Developer ID、时间戳、Hardened Runtime、公证票据、架构与版本 |
| x86_64-apple-darwin | `.app.zip` | 同上 |
| x86_64-pc-windows-msvc | 便携 ZIP | Authenticode、时间戳、版本 |
| x86_64-unknown-linux-gnu | tar.gz | 动态库依赖、版本、X11/Wayland 声明 |

macOS 最低部署目标为 13.0；最低系统仍须真实机器验收。Linux 构建基线为 Ubuntu
24.04（glibc 2.39），不承诺可在更旧系统直接运行；Wayland 的置顶、快捷键和定位
能力取决于合成器。Windows 包先提供便携版，移动程序路径后需重新设置自启。
暂不增加 MSI、AppImage、自动更新或多个 Linux 包管理器入口。

版本唯一来源为 Cargo.toml，macOS 模板版本在打包时生成，构建号为完整 Git 历史中
该提交的计数。首期发版入口只接受稳定版 `MAJOR.MINOR.PATCH`，不带 `v`；Git tag
为 `vMAJOR.MINOR.PATCH`。不在普通提交中自动升级版本，也不发布 crate。

## 发版前准备

仅在用户明确要求发版后执行本节及后续步骤。以下命令是操作手册，不是授权。

1. 建立版本 PR，更新 Cargo.toml、Cargo.lock 及简短中英文发布说明，记录破坏性变化。
2. 合并 PR，等待该 main 提交的全部 CI 通过，记下完整 SHA。发布说明放在已提交的
   文件中，例如本次发版专用 `docs/releases/VERSION.md`；不保留空白模板或虚构版本。
3. 按许可证核查依赖和字体/图标的第三方声明，确保发行物包含所需许可文本。
4. 确认本机可用正式发行签名身份；缺少凭据就停止，不退回开发自签或 ad-hoc 签名。

## 构建候选包

在 GitHub Actions 手动运行 **Build release candidates (manual)**，选择 `main`，
填写版本和当时 `main` 的完整 SHA。工作流检查版本、干净检出、项目许可证、来源
及这个提交的成功 CI；四个原生 runner 运行相同的 Rust 打包工具：

```sh
cargo xtask package TARGET release
```

候选包在 Actions artifacts 保留 14 天，不创建 tag 或 Release。文件名标明
`-unsigned`，旁边的目标 JSON 记录版本、提交、目标和 SHA-256。过期后重新手动构建，
不要改用不同提交的包。日常 CI 用 `debug` 检验同一条打包路径，并且不上传这些包。

## 本机签名

将四个 artifact 分别解压到不同目录。使用同一提交的干净源码运行下面的工具。
凭据保存在仓库之外的钥匙串、证书存储或硬件中，不传入命令行密码，不上传 Secrets，
不在公共 PR runner 或你的日常电脑上安装自托管 runner。

macOS 在本机钥匙串中配置 Developer ID 与 notarytool profile，然后对两个架构分别执行：

```sh
# 环境变量只包含身份名/profile 名，不是密码。
bash packaging/macos/distribute.sh CANDIDATE_DIR dist/final
```

设置 `SNIP2PIN_SIGN_IDENTITY`、`SNIP2PIN_NOTARY_PROFILE`；脚本检查候选哈希，
签名、公证、附加票据并验证 Gatekeeper 后才生成最终 ZIP 和新清单。
公证将已签名应用交给 Apple 检查，不上传私钥或钥匙串。当前开发自签身份继续用于
本地更新；第一次切换正式发行身份可能需要重新授权，不能承诺无感保留 TCC 权限。

Windows 在有 Windows SDK SignTool 的本机 PowerShell 7 中执行：

```powershell
./packaging/windows/distribute.ps1 -Candidate CANDIDATE_DIR -OutputDirectory dist/final -CertificateThumbprint THUMBPRINT -TimestampUrl RFC3161_URL
```

私钥留在当前用户证书存储或硬件提供方，工具不接收 PFX 或密码。签名和时间戳验证
失败会停止。有效签名不代表已建立 SmartScreen 信誉。

Linux 候选包无需平台证书；验证后复制为去掉 `-unsigned` 的最终文件名，再执行：

```sh
cargo xtask verify CANDIDATE_DIR
cargo xtask finalize CANDIDATE_DIR dist/final/Snip2Pin-VERSION-x86_64-unknown-linux-gnu.tar.gz
```

签名完成后不得改动应用内容；修改需回到构建步骤。最终清单与安装包一起保留，
包含原候选包的 SHA-256；正式发布时还会对照候选构建 run 的原始 artifact 核对来源。
第三方声明从实际依赖的 registry 许可文本生成并随包分发；发行前仍需核对字体及
系统库要求，不能把自动收集当成法律审查。

## 安装与交互验收

在各平台从实际包安装或解压运行，记录系统版本、架构、包哈希和结果：

- 首次安装、覆盖上一发行版、退出后重开；配置、主题、快捷键和自启选择保留。
- 首次截图、拒绝后再次授权、撤销权限后的恢复；普通更新不反复丢失授权。
- 多显示器和不同 DPI 下框选、原位置顶、复制、拖动、Shift 缩放、四边操作图标。
- macOS 菜单栏/Dock 不被普通贴图遮挡，图标更新符合已说明的安装行为。
- 托盘、全局快捷键、取消、关闭全部贴图和退出；自启默认关闭。
- Linux 分别记录 X11 与实际验证的 Wayland 桌面，明确限制，不笼统宣称兼容。

CI 的隐藏窗口和包检查不能替代这些交互。测试记录不包含真实桌面截图或剪贴板内容。

## 草稿与正式发布

将四个最终包及四份 JSON 收集到 `dist/final`，本机先运行：

```sh
cargo xtask verify dist/final
cargo xtask stage VERSION FULL_SHA dist/final docs/releases/VERSION.md --confirm
```

只有这个显式命令才创建 Draft Release；它要求四个目标齐全、版本和提交一致、哈希
正确、CI 通过且版本未使用。不存在完整产物或真实验收结果时，不运行它。

手动运行 **Publish verified release (manual)**，填写版本、完整 SHA、成功候选构建
run ID，以及 `publish VERSION` 验收确认。工作流对下载的最终包在原生平台重验签名、
时间戳、公证、版本、架构和依赖，再冻结通过验证的文件。`release` Environment
等待维护者批准，发布任务重新下载 Draft 并逐文件比对，确认没有中途更换后才发布。
本次通过后不重新构建；tag 指向同一个源提交，发布后不能重写或删除版本 tag。

`release` Environment 只允许 main 手动任务，要求维护者批准；单人维护允许本人批准
自己触发的任务，但不能省略这一步。普通任务只读；读取 Draft 的签名校验任务因
GitHub 的草稿访问要求使用写 token，仅在下载步骤传入，其他步骤不持有该环境变量。
不储存长期 PAT 或任何签名秘密。

## 失败与恢复

任何平台失败则不发布，已成功的平台包仍是候选产物。验收未通过时删除本机临时产物
并修复，不能通过忽略签名、降低检查或替换已发布文件继续。
版本已发布后发现问题，保留原 tag/安装包与问题说明，另发修复版本；必要时把旧的
可用版本重新标为 latest。旧版本运行前先核对配置兼容性，不清理用户配置或权限。

## 自动化边界

- `ci.yml`：日常开发检查和开发包结构测试，无分发上传。
- `candidates.yml`：仅手动生成指定提交的未签名候选包，无发布权限。
- `publish.yml`：仅手动验收并发布已有草稿，经过 Environment 批准。
- `xtask/`：共用检查、版本元数据、包结构、校验清单和发行输入验证。
- `packaging/`：各平台必须原生执行的签名/公证接口；不得复制业务逻辑。

流程文件可以实现并测试，但创建版本、构建正式候选、签名公证、创建草稿和发布
始终需要用户的明确发版要求。实现流程本身不构成发版授权。

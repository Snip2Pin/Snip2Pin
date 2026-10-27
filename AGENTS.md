# Snip2Pin

Canonical repository: `https://github.com/Snip2Pin/Snip2Pin`.

## 项目原则

- 中文显示名为 `Snip2Pin · 截图置顶`，英文显示名为 `Snip2Pin`；界面、打包本地化资源及文档保持一致。
- 产品定位是截图 + Pin：框选截图、自动复制、原位置顶、拖动、手动缩放和关闭；菜单栏入口、开机自启、快捷键、主题和图标仅作为基础设置，保持项目简洁。
- README 的使用介绍保持简洁，中英文同步，只保留基本操作、快捷键、基础设置和必要的权限说明。
- 主页采用紧凑直达布局，使用系统原生边缘拖拽调整窗口大小：一个截图主按钮，快捷键与主题、图标、开机自启直接可见；版本与 GitHub Releases 入口固定在底部。长快捷键可换行，小窗口内容可滚动，不把设置拆成多页，不增加常驻操作说明。
- 代码、配置路径和打包元数据不得包含个人姓名或账号，也不保留旧产品名称。配置目录只使用 `Snip2Pin`，应用标识统一为 `io.github.Snip2Pin.Snip2Pin`（保持用户选定的大小写）；产品身份与仓库所有者无关。
- 网页交互演示使用 GitHub Pages，复用实际 Rust/WASM 界面；建立并验证页面之前，不添加尚不存在的演示链接。
- 公开演示使用 `cargo xtask preview-site` 构建，只打包生成的网页资源、开源字体及许可声明，不上传本机系统字体、源码目录、配置或凭据。字体来源固定版本并校验 SHA-256，子集化后保留许可和版权、使用独立字体名称。网页字体不进入原生应用资源。
- `pages.yml` 对 PR 只构建检查，仅从 `main` 部署到 `github-pages` 环境；网页资源可随主分支更新，不触发应用发版、签名、标签或 GitHub Release。网站部署权限仅授予部署任务，不使用 `release` 环境或本机身份。
- 主题默认跟随系统，用户可选择跟随系统、浅色或深色，设置需要持久保存。
- 界面支持中文、英文，默认在启动时按系统首选语言顺序匹配，无匹配或读取失败时回退英文；用户可选择跟随系统、简体中文或 English，手动选择立即生效并持久保存。文案集中在类型化翻译目录，新增文案同时提供中英文，原生界面与 WebAssembly 共用。消息保留语义直到显示时翻译，系统或第三方诊断信息保留原文；翻译不得参与事件路由或原生窗口身份识别。覆盖长英文、小窗口、动态参数及旧设置兼容，不轮询系统语言。
- 图标默认跟随主题：浅色使用极简线条，深色使用深色金属；用户也可手动固定任一图标。
- PNG、ICO、ICNS 等二进制资源使用 Git LFS；SVG 源文件使用普通 Git，并保持源文件与 PNG 一致。
- 运行时只读取预生成图标；SVG 渲染依赖仅放在开发工具中，用 `cargo xtask icons` 再生资源并校验像素一致性，避免把开发工具的代码带进应用。
- 应用和安装包体积也是验收指标：分别报告可执行文件、解包内容及各发行格式的实际字节数，不用压缩包大小代替安装体积。按实测平台基线设置发版构建预算，变更预算须说明原因；调试包不套用发行包上限。不使用可执行压缩壳，不为缩小体积牺牲可访问性、画质、必要的平台能力或启动速度。
- Windows MSVC 构建使用 Rust 内置的静态 C 运行库链接选项，避免要求用户另装 Visual C++ 运行库；CI 核对实际发行优化 EXE 的导入表并计入体积预算，不能只以带开发工具的 runner 上能启动作为依赖验收。
- Windows EXE 在编译时嵌入版本元数据和从同一 SVG 生成的多尺寸 ICO；ICO 使用 Git LFS，资源工具仅作构建依赖。运行时窗口图标不能代替资源管理器和快捷方式的文件图标，不在签名后修改 EXE。安装版使用当前用户目录，提供开始菜单入口和可选桌面快捷方式；卸载清理应用创建的快捷方式与指向本安装目录的自启入口，保留设置、无关文件及其他便携副本的自启入口。便携 ZIP 解压即用，保留许可证与第三方声明。CI 检查实际 EXE 图标资源、快捷方式目标及安装／升级／卸载边界，原生交互验收单独记录。
- 只有用户明确、显式要求发版时，才进行发版操作。日常修改不自动升版本、打标签、创建或发布 GitHub Release、发布 crate 或上传分发包。
- CI 只运行验证，不因普通提交或合并自动发版；本地开发构建不代表已经发版。
- 当前采用 GitHub Releases 安装包分发，不上 App Store，暂不开通付费签名服务。默认明确选择 `unverified`：macOS 仅使用 ad-hoc 完整性签名、未经公证，Windows 可执行文件未签名；文件名和发布说明必须如实标识。`signed` 为未来取得正式凭据后的选项，签名失败不得自动降级。不清除 quarantine、不关闭 Gatekeeper/SmartScreen、不自动信任自签证书；用户使用系统允许的方式自行确认。不得承诺未公证包更新可无感保留 macOS 截屏授权。
- 自动覆盖所有显示器，每次截图刷新屏幕布局；初始置顶保持原始显示尺寸，仅超过可用区时等比缩小。截图只校验非空和源图边界，不设置额外的 2 像素或 2 点下限，原生窗口和宿主决定实际最小尺寸。高亮边框为 1 个物理像素。按钮不得决定图片尺寸，不为容纳按钮放大小图或拉伸短边；不增加 DPI 下限换算、几何反馈或定位重试。复制始终使用原始像素。
- 鼠标／触摸屏单指拖动和边缘缩放交给系统，比例、Shift 和边缘锚点遵循各平台原生惯例，不再强制统一手写手势。图片内容铺满原生内容区，以系统实际位置和尺寸为准；UI 只分派语义输入，不计算窗口几何。键盘缩放及恢复原始尺寸单次请求尺寸；滚轮和捏合缩放固定手势开始时鼠标指向的图片点，由平台适配器从实际内容区计算，使用一次原生调用同时更新位置和尺寸。遵守系统约束，不重试或补偿；Wayland 缺少全局位置时保留尺寸调整，不猜测锚点位置。
- 缩放和初始适配只改变显示尺寸，复制始终使用原始截图；原始像素不能被覆盖或反复重采样。显示器、DPI 和应用缩放按统一单位换算；复制、关闭按钮仍须在可用区内。
- 菜单栏／托盘及 macOS Dock 提供打开主页、截图和关闭未锁定贴图等操作；关闭主页时保留托盘和贴图，通过菜单退出才关闭应用。明确打开主页或点击 macOS Dock 图标时恢复隐藏／最小化并激活主页，使其显示到其他普通应用之前，保持普通窗口层级；不能用 key-window 状态代替应用前台状态，也不靠永久置顶、定时或反复聚焦补偿。菜单事件唤醒 UI，不使用后台轮询；无托盘后端时保留主页，关闭主页走统一退出确认。macOS Dock 菜单通过官方 NIB outlet 接入，退出和重新打开的 AppleEvent 路由独立于菜单资源加载，不替换 winit 的应用 delegate 或窗口类。
- 主页列出当前贴图的缩略图、定位、锁定和关闭操作；定位将键盘焦点交给对应贴图。列表与图片高亮只依据真实窗口焦点，直接点击图片同样生效，失焦即取消；不保留额外选中状态或高亮计时，不新增截图历史。选中且未锁定的贴图实际显示尺寸不足以容纳一个操作按钮时，列表显示橙色提示“贴图太小？按 + 放大”；使用已观测的内容区尺寸，不以原图像素或初始尺寸猜测，未知时不提示，不恢复图片最小尺寸限制。尺寸跨提示阈值时通过事件重绘主页，不轮询或形成重绘循环，贴图关闭时释放提示状态。列表可折叠并限高滚动，默认至少显示 3.5 行（行高随字体推导），可独立纵向拖长，少量贴图按内容收缩；复用已有纹理。
- 列表采用轻量行布局，保留左右内距；缩略图在带底色、细边框与内留白的固定区域居中，完整展示，不裁切原图。名称起点对齐，长名称截断并可查看全文，重名须可区分；操作按钮弱化，不能紧贴边缘或挤压名称。行高随字体增长，覆盖窄窗、大字体、极端图片比例、空列表和长列表滚动。单击行内非操作按钮区域选中并定位，双击名称进入标准文本编辑器，清空后显示随语言变化的默认编号。主页采用普通窗口层级，不能为定位或管理贴图而永久置顶；只有等待退出确认时可临时提升，取消须恢复。定位不改变几何，不用焦点往返、定时或层级降升补偿；名称编辑不触发定位或截图，锁定不妨碍命名。名称及编辑撤销状态随贴图关闭释放，不写入设置、日志或磁盘，不改变原生窗口身份及原图。
- 列表顺序独立于贴图的前后层级与存储顺序；点击定位、聚焦、锁定、改名均不重排。左侧留白内提供独立拖动手柄，用 egui 的拖放机制及插入标记调整列表顺序。拖动时原行只留空位，复用同一行绘制函数与 `dnd_drag_source` 浮层显示带底色和阴影的卡片，保持鼠标的原始抓取点；浮层不接受操作，不额外创建系统窗口，不手写指针捕获或拖动事件循环。验证拖动开始及中间帧，而不只验证松手后的顺序。长列表在边缘拖动时滚动；仅成功放下时排序，Esc、失焦或列表外松开取消，不触发定位、改名或关闭。新截图放在列表顶部，既有项目保留用户顺序；关闭时清理对应顺序及拖放状态。锁定不妨碍列表排序；排序只保存在当前会话的共享界面状态中，不改变窗口位置、层级或原图，不新增持久历史。网页版和原生版复用同一实现。
- 聚焦且未锁定的贴图支持方向键微调位置、`+`／`=` 放大、`-` 缩小、`0` 恢复原始显示尺寸；右键菜单同样提供恢复操作。移动步长使用系统可表达的最小单位，只从实际外框位置发出一次移动请求，不猜测 Wayland 缺失的全局坐标；边界由系统处理。键盘缩放保持当前比例，最短边放大时须实际增长，尺寸边界遵循原生系统和宿主；恢复原始尺寸不施加应用下限。文本编辑或 Ctrl／Command／Alt 组合不触发键盘移动与缩放。保持窗口与图片同尺寸，不为点击热区自动放大或增加透明外边框；非 macOS 聚焦时可加宽图内缩放热区，同时保留中心拖动区域。
- 普通方向键每次轻按只移动一个原生最小步长，长按仅在系统重复按键事件到达时步进；随有效长按时长逐步增加每次步长并封顶，不按帧平滑移动，也不在无输入帧中继续移动。Shift＋方向键使用更大的固定步长，长按沿用相同的系统重复节拍；松开 Shift 回到微调，无须松开方向键。步长增长参数集中在 `src/ui/pin/keyboard.rs`，快速步长复用 `src/pin.rs`。UI 只保留按键会话与时长，不缓存预计窗口坐标，不自行请求持续重绘、计时器或轮询；每帧合并一次移动请求。松手、失焦、锁定、菜单、文本编辑、截图或关闭即清理；中断后须重新按下，不能靠重复事件恢复。普通方向键和 Shift 的步进节奏均由系统负责，不追赶无输入期间的位移。
- 锁定贴图不可移动、缩放、通过按钮／Esc／菜单关闭，关闭全部跳过锁定项；复制、定位与解锁仍可用。所有关闭和手势入口在消费事件时检查锁定，不能只禁用按钮。有锁定贴图时主动退出须确认，取消保留全部图片且取消待执行重启；确认后才释放图片并退出。
- macOS 锁定保留原生窗口样式、位置、内容尺寸和跨屏呈现；以当前内容尺寸的原生 min/max 约束及 movable 状态阻止移动缩放，解锁恢复原配置。不通过切换 resizable 样式、重建窗口、定位或 DPI 补偿实现锁定。原生跨屏显示须实际验收，隐藏窗口的几何测试不能证明合成器完整显示。
- 全局解锁全部贴图使用 macOS Command+Option+Shift+L、Windows/Linux X11 Ctrl+Alt+Shift+L；同一快捷键管理器分派截图与恢复命令，截图配置不能覆盖恢复组合键，注册失败须明确提示。菜单保留解锁入口；Wayland 保留窗口内回退，不宣称全局可用。恢复只解锁，不关闭图片、移动、缩放或抢焦点，重复操作幂等；锁定图尝试拖动时提示组合键。
- 快捷键录制实时显示当前按下的组合，纯修饰键不提交、不报错；校验、注册及保存反馈紧邻快捷键控件，不依赖长列表后的通用消息。重新录入当前快捷键也按成功处理，不重复注册或写设置。原生全局回调与普通按键共用录制完成路径；回调入队时保留录制输入归属，取消或完成后延迟消费不得触发截图或解锁。取消或失败保留旧快捷键，原生版和网页版共用录制状态与视图规则，覆盖真实按键顺序和按住修饰键的中间态；录制输入不得触发截图或透传贴图操作。
- 解锁提醒复用截图的屏幕提示绘制与已准备的透明屏幕层，显示在鼠标所在屏幕可用区域的中上方，显示 1 秒；提示框按文字和屏幕可用宽度自适应，宽度足够时单行，仅空间不足才换行。不画在图片窗口里，不改变图片大小，不抢焦点或拦截鼠标，不增加独立提示窗口。必要时只调整本进程透明层的前后顺序，每次提示只执行一次，不反复争夺层级。只在触发时读取指针位置，不监听全局鼠标或轮询；位置不可用时在已知屏幕均显示，不猜测旧截图所在屏幕。解锁、开始截图或到期后清除提示，并以单次重绘清空屏幕层。
- 隐藏提示层先通过框架的隐藏绘制请求准备内容，再显示并开始计时；根窗口的完整 UI pass 必须先发布新子窗口回调，不能让 logic-only pass 请求旧回调绘制。首帧确认绑定当前提示和显示器，过期或重复确认不能重启计时。Windows 被动提示复用同一原生窗口，按目标显示器实际 DPI 在隐藏帧中测量提示框，显示时窗口仅占提示框边界；可用工作区只用于排版，不能用整块透明工作区窗口承载提示。收缩后仍按原工作区宽度排版，避免再次换行或增加顶部间距；隐藏时保留小窗几何，截图模式才恢复完整屏幕。验证首次、重复提示及截图模式切换的实际可见帧，并比较系统栏亮度，不能只检查是否消失。
- Windows 提示层准备时原生透明度设为 0；映射后等待所有目标屏幕完成可见帧绘制，再一次性恢复透明度并开始 1 秒计时，避免隐藏缓冲区未保留时闪出窗口背景。macOS/Linux 保留首个准备帧确认后计时的行为。准备或显示失败只清除对应代际。
- 普通滚轮在指向未锁定贴图且未按鼠标按钮时按比例缩放，无须先聚焦；捏合按后端倍率等比缩放。不实现触摸板双指滚动平移窗口，普通 Point 精细滚动不移动或缩放贴图，不能改用全局监听、透明覆盖层或抢占其他应用的滚动事件续传。复用框架原生 Zoom 事件，兼容 Ctrl+滚轮模拟的捏合；同帧存在原生 Zoom 时忽略模拟滚动，避免重复处理。精细滚动与普通滚轮的区分以输入单位为准，不猜测设备；仅模拟普通滚轮的触摸板保留滚轮行为。操作按钮、弹出菜单、文本编辑和 Command/Alt 组合不触发滚动调整。键盘方向键默认微调，Shift 快速移动；`=`/`-` 默认约 1% 缩放，Shift 约 25% 放大或其倒数缩小，`+` 同样可放大，`0` 恢复原始尺寸。滚轮按输入单位、UI 缩放统一归一化，同帧缩放增量合并；锁定检查仍在事件消费处执行，不影响原图或原生拖缩行为。触屏单指沿用框架的指针映射与系统拖动；取消触摸只清除当前框选，不生成截图或复制。
- 滚轮及捏合缩放只在开始时命中贴图；系统实际送达本应用的续传保持原目标，每个输入只处理一次，不承诺收到指针移出应用后的事件。原生开始／结束／取消优先；无阶段信号时沿用框架滚动回退的 150 毫秒静默间隔，仅在下一次输入时判断，不启动计时器或轮询。锁定、菜单、文本编辑、失焦、截图和关闭释放手势状态。缩放锚点保留原图比例及起始屏幕坐标，先按原生网格取整尺寸再计算位置，避免逐帧累积取整误差。原生句柄和 X11 连接按贴图生命周期复用，几何始终读取实际值，不缓存预测位置。
- 退出等确认对话框优先使用系统原生界面；macOS 用异步 NSAlert sheet，Windows 原生消息框在专用线程等待，通过带请求标识的事件回传结果，不在绘制或主事件循环借用期间同步嵌套模态循环。默认取消，过期结果不得影响新请求；等待 Windows 消息框时保留其所有者窗口。无通用原生接口的平台、演示和自动测试保留轻量应用内回退，不引入第二套 GUI 运行时。
- 开机自启默认关闭，只在用户明确勾选后写入当前用户的系统配置；关闭时仅删除本应用入口。启动和安装不得自动启用自启，测试只操作临时目录，不修改真实登录项或注册表。自启后台启动，不弹主页、不抢焦点、不截图；源码运行的 macOS 应用须先安装后才能启用。
- 截图模式只突出当前截图目标：悬停显示识别窗口的蓝框，未识别到窗口或尚无指针位置时默认选中当前整屏，拖动后仅显示框选边界；目标保持原亮度，其余区域（包括其他显示器）统一略微变暗。不在窗口或选区边界之外叠加屏幕蓝框，不用活动显示器状态决定整屏明暗。显示器切换仅更新输入目标和焦点，不在后台轮询鼠标。
- 截图模式悬停识别最前面的可见窗口，单击截取当前目标（窗口或整屏）、拖动仍自由框选；按下和松开须命中同一目标。窗口边界与屏幕画面在同一截图任务中收集，鼠标移动只做内存命中检测，不逐帧查询系统；识别失败仍支持整屏选择与框选。识别框与裁剪共用像素矩形和原生坐标网格，只截当前屏幕的可见画面，不补拍被遮挡内容。元数据仅保留边界和顺序，随截图会话释放，不保留或记录其他应用窗口标题。原生 Wayland 没有通用窗口枚举能力，不用 XWayland 的不完整窗口列表猜测被遮挡窗口。
- 完成或取消截图时恢复普通光标；演示和自动化测试不得修改系统光标或抢占用户焦点。
- 所有平台及演示均不支持双击图片关闭；连续点击或原生拖动后的点击不能关闭贴图。不保留框架双击关闭、原生点击次数适配或补判状态。关闭使用图内 ×、Esc、右键菜单或主页列表，并遵守锁定规则；主页双击名称编辑保留。
- 识别窗口内沿用界面框架的单击容差，轻微手抖仍按单击处理，长按后可框选极小区域；空白处直接接受任意非空框选。复用框架手势分类，不自行硬编码距离或时间阈值。
- 贴图采用系统阴影，不为旧边框样式增加透明边距或坐标补偿。复制、锁定、关闭在图片内部悬停横排显示，窄图从左起逐个隐藏完整按钮，不竖排、不放大图片；列表和右键菜单保留全部操作。复制使用双层照片图标。按钮点击不得触发窗口拖动或缩放。视觉样式可为更流畅的交互调整。
- 成功框选后自动复制图片到系统剪贴板，复制按钮可再次复制；取消、无效或过期选区不能改动剪贴板。测试只检查复制命令，不写入真实系统剪贴板。
- 初始位置与尺寸在创建贴图时确定并保持构造参数稳定，界面缩放变化不能把用户移动或缩放后的窗口重置回原位。系统实际结果作为后续绘制依据，不因不匹配而重试或补偿。初始化期间仍须响应 Esc 和关闭。原生移动和缩放事件驱动重绘与按钮布局；静止时不得轮询鼠标或重复定位。
- 图片移动遵守系统原生窗口约束，不借裁切模拟越界或与系统争夺位置。操作按钮根据图片与显示器可用区的交集布局，避开 Dock、菜单栏等系统界面；macOS 图片上边缘应能紧贴菜单栏下缘。不用透明留白实现换边。
- 位置根据图片、窗口和显示器的实际边界计算，禁止硬编码屏幕坐标。尺寸、间距、颜色、时间阈值等使用有明确用途的配置或常量，避免散落数值；数学常量、格式规范、图标路径和测试样例无需机械抽象。
- 操作按钮默认图内右上角，右侧被遮挡时向左移动，顶部被遮挡时向下移动；空间恢复后回到右上角。按钮限制在图内可见交集，不改变图片位置或尺寸，不新增常驻按钮窗口、全局鼠标监听或轮询。macOS/Windows 的右键菜单复用原生菜单后端，由系统管理定位、边缘避让、焦点和关闭；Linux 当前无通用原生菜单接口，保留独立于小图尺寸的轻量回退，不为少量菜单项引入第二套 GUI 运行时。沙箱仅模拟菜单。回退菜单仅在用户操作时创建，操作完成、Esc 或失焦后关闭，不能裁切在极小图片窗口里。极端越界不通过定位重试争夺系统窗口控制。
- macOS 开启“显示器具有单独的空间”时，按钮只使用原生窗口 `NSWindow.screen` 的当前可用区域；不能把几何上重叠的其他显示器视为实际呈现区域。窗口所属屏幕切换后随之更新，继续避开菜单栏和 Dock；不按按钮宽度、原始截图屏幕或猜测的过屏比例切换。不限制原生支持跨屏呈现的其他模式与平台，原生查询须验证无几何或焦点副作用。
- 后端不提供全局窗口位置时（例如部分 Wayland 桌面），按钮依据实际本地内容区布局，不能用初始截图位置或尺寸猜测当前窗口边界；系统负责窗口可达性。
- 普通贴图必须置于 Dock 和菜单栏之下，不得靠提高到系统界面之上的窗口层级解决移动限制。macOS 使用原生移动接口，保留窗口类型和系统监听的生命周期；不得修改窗口运行时类型或系统共享类。框选层仅在用户主动截图时覆盖系统界面。
- 原生内容区就是图片区域；按实际内容区绘制，接受系统原生尺寸与缩放惯例，不保留期望宿主与实际宿主相减的图片偏移。
- 框选预览边界与最终裁剪共用同一个像素矩形。平台窗口只能表达更粗坐标时，在框选阶段按实际像素密度统一边界；不得在松手后单独取整窗口原点、重新裁剪或补偿位置。非整数像素密度不能假装存在精确的整数网格。
- 使用预先准备的透明无边框选区层，不切换系统全屏、不隐藏主窗口或已有贴图。
- 新贴图绘制完成后才清除选区画面，避免截图结束时露出空白帧。
- 未使用的代码、注释、文档和旧实现要及时清理，不保留备用代码或过时说明。
- 已有 Git commit 历史的内容可以直接清理，不用担心误删；需要时从历史恢复。
- 代码使用 Rust，统一支持 Windows、macOS 和 Linux，平台差异集中在适配层。
- Windows 贴图从创建起关闭最大化能力，保留未锁定时的原生拖动与边缘缩放，避免让浮动图片参与普通文档窗口的自动平铺；主页与其他平台沿用原有窗口能力。此能力不随锁定或拖动切换，不能通过重建窗口、修改系统贴靠设置或反复恢复尺寸来规避系统平铺。实际验收需区分鼠标边缘贴靠、键盘平铺与手动缩放，不能仅用隐藏窗口样式测试替代。
- 桌面与网页共用 `ui::pin::show` 完成图片、焦点框、悬停按钮及命中排除的绘制，不在宿主重复编排。宿主明确提供输入原点和比例；焦点参数只表示焦点，不能据此推断宿主或坐标来源。
- `pin_zoom::PinZoom` 按贴图生命周期管理原生绑定，应用层不持有原生缩放句柄或传递缓存槽；保留各平台原子缩放及仅尺寸调整回退。单张关闭和确认退出复用 `Pin::release` 清理贴图资源，是否允许关闭仍由入口检查锁定。
- macOS 本机开发构建安装（`build.sh --install`）必须复用固定的代码签名证书，不能用每次随构建变化的 ad-hoc 签名；GitHub 分发包按明确选择的分发模式处理。
- 本机开发更新前验证新包符合已安装应用的签名要求；证书缺失时停止，不自动换签名身份。只有用户明确要求更换应用标识时，才使用显式迁移选项，仅替换签名要求中的 identifier，完整保留原证书和信任约束。
- 本地开发签名私钥保留在独立的本地钥匙串，不导出到 CI；不得修改 TCC 数据库、自动重置截屏授权，或放宽签名要求来保留权限。
- 正式发行凭据可保存在受审批保护的 GitHub `release` Environment Secrets；只允许手动、main 来源的签名任务读取。私钥、钥匙串、密码和令牌不得进入 Git、日志、缓存或 CI 产物；提交前检查暂存内容，不能只依赖 `.gitignore`。Secrets 并非不可导出的硬件密钥，授权任务可以读取明文，需审阅工作流和依赖并及时轮换泄露凭据。
- 更新 Finder／启动台图标时，更新应用自身的资源引用并刷新注册信息，不重启 Finder／Dock 或清空全局缓存。
- Linux 有 X11 显示服务时优先使用 X11/XWayland 窗口后端，保留置顶能力。
- X11 截图层使用框架提供的 Dock 窗口类型覆盖整个显示器，包括系统面板；不保留工作区空间，不使用普通或 Utility 窗口后再补偿位置。Windows/Linux 空闲时隐藏已准备的截图层，仅截图或解锁提醒期间显示，避免遮挡桌面或影响任务栏／面板层级；macOS 保留已验证的原生层级预热流程。按实际桌面与双屏合成结果验证。
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
- 原生窗口是当前几何的唯一权威；Rust 保留不可变原图、初始放置与图内按钮布局。鼠标／触屏原生拖动及边缘缩放不自行求解，不保留重复期望 Rect 或反馈补偿；显式键盘、滚轮和触摸板命令可根据实际几何计算一次调整。跨 DPI 先统一坐标单位。
- macOS 原生拖动使用所属窗口的原始鼠标按下事件，不能在延迟重绘后用 `NSApplication.currentEvent` 代替。应用内监听仅保留一次有效按下，松开或消费后释放；请求匹配原始按下点并绑定该次按下的代际和目标窗口，过期请求不能消费后来的点击。不安装全局监听，不吞掉事件，不自行求解拖动位置。
- 窗口与渲染时序按实际后端验证，包括 egui 多 pass 重绘和连续原生事件。图片绘制不能产生主窗口几何副作用，不能依赖某个可能被丢弃的 UI pass 提交 drawable 补丁。不得用巨大透明窗口、手动更新 GL context 或定位重试掩盖问题。
- 原生状态查询也要验证是否会改变窗口；macOS 贴图先在隐藏状态完成后端初始化，再显示，不能依赖隐藏窗口一定会绘制首帧。依赖优先使用官方发布包，不将第三方库源码整包复制到仓库，不修改本机依赖缓存或用构建脚本注入补丁；需要依赖修复时优先采用上游修复，不通过坐标补偿掩盖后端副作用。
- 截图线程使用有界队列，取消立即恢复 UI、丢弃晚到结果并阻止尚未开始的后续屏幕采集，不承诺强停正在执行的系统调用。像素转换在工作线程完成，截图数据与纹理上传共享不可变图片；上传前验证 GPU 纹理边长上限。多屏并行不跨线程搬运非 Send 原生句柄，Linux Portal 不并发请求。
- GUI 沙箱复用实际 Rust 视图和几何，不复制一套 HTML 产品界面；仅使用生成图片，禁止真实截图、系统剪贴板、权限、自启和全局快捷键操作。多 agent 评审使用各自独立的 headless 浏览器上下文；浏览器交互和隐藏原生几何验证都不能替代合成器视觉验收。
- 原生版和网页演示共用交互规则，不只共用绘制：快捷键录制与校验统一在 `src/ui/shortcut.rs`，截图请求复用 `CaptureGate`，选区映射复用 `geometry::pin_placement`，关闭贴图统一调用 `ui::pin::forget` 清理列表、编辑和输入状态。Esc 只关闭持有键盘焦点的贴图；菜单和快捷键录制不得透传输入到背后的贴图，文本编辑按键不得操纵贴图。网页只模拟窗口、焦点和平台能力，不另写排序、快捷键或坐标规则。网页宿主的生成数据回归纳入工作区测试，覆盖完整输入到动作的路径；确需平台差异时在适配层说明原因，不为统一系统窗口行为引入补偿。

## Structure

- `src/capture.rs`, `src/capture/worker.rs`: monitor enumeration, shared in-memory pixels and bounded cancellable capture work.
- `src/capture/windows.rs`, `src/capture/windows/`: one-shot visible-window metadata, platform stacking/visibility adapters and source-pixel region mapping.
- `src/geometry.rs`: validated pixel selection and original-position pin placement.
- `src/pin.rs`: initial proportional fitting, display coordinate conversion and adaptive visible image controls placement.
- `src/app.rs`: capture state machine, events, and window lifecycle.
- `src/ui/`: launcher, selection overlay, floating image views, shared style profiles, and copy artwork.
- `preview/`: WebAssembly demo and local sandbox using the same Rust UI and fictional documents; no native services. Public builds use a redistributable font; local system fonts stay local.
- `src/app/tests.rs`, `src/ui/pin/tests.rs`, `src/ui/launcher/tests.rs`: lifecycle and view regressions, separate from production logic.
- `src/platform.rs`: keyboard shortcut and local system-font adapters.
- `src/platform/macos.rs`: explicit application activation and floating levels, preserving backend classes and observation lifecycle.
- `src/platform/macos_drag.rs`: retain the original press and bind deferred native drag requests to it, without computing movement.
- `src/platform/pin_window.rs`: same-process native pin lookup and explicit keyboard-focus activation without geometry changes.
- `src/platform/pin_zoom.rs`: atomic native anchored zoom using observed content bounds and per-pin native handles.
- `src/ui/pin/session.rs`: continuous input ownership and fixed zoom anchors, without window geometry prediction.
- `src/ui/pin/keyboard.rs`: system-repeat-driven arrow steps with bounded stride growth, without animation or geometry prediction.
- `src/platform/macos_dock.rs`, `packaging/macos/DockMenu.xib`: native Dock menu, system Quit and Reopen routing, independent of optional menu resource loading; Xcode compiles the NIB at bundle time.
- `src/platform/quit_prompt.rs`: asynchronous native quit confirmation with an application-UI fallback.
- `src/settings.rs`: shortcut validation, theme/icon preferences, and local storage.
- `src/i18n.rs`: typed Chinese/English catalogue, deferred messages and locale matching.
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
After work is merged or superseded, promptly remove its unused local/remote
branches and worktrees after checking for uncommitted changes, active work and
retained build artifacts. Verify squash/cherry-pick replacements before deleting
old branches; preserve ongoing validation and release inputs. Keep machine-local
workspace paths out of the repository.

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
Install the `.pre-commit-config.yaml` hooks for development. Commit-time checks
run `cargo xtask lint` (Rust format, Clippy and repository metadata), official
YAML/TOML, large-file and merge-conflict hooks, and `cargo deny --locked check`.
Official whitespace and end-of-file fixers may edit files and block that commit;
review and stage their changes before retrying. Large files above 500 KiB must use
Git LFS; enforce the size check on all supplied files, with LFS files skipped by
the official hook. Other failures block the commit without rewriting files; never
skip missing Rust/cargo-deny tools once hooks run. Keep the local Rust hook as a thin
call to xtask and run the official general hooks in CI too. CI reuses the same lint entry point
through `check`; full tests/builds remain separate from the faster commit checks.
Preserve hooks managed by other tools; use their supported integration rather
than overwriting `core.hooksPath` or disabling security/LFS hooks. Update
CONTRIBUTING.md and docs/workflow.md together when changing hook setup or policy.
Keep xtask's own tests in `#[cfg(test)]` modules: Cargo builds the normal binary
for integration-test targets, which cannot replace a running `xtask.exe` on Windows.
Dependency updates go through PRs and `cargo deny check`; do not auto-merge or
globally suppress advisories/licenses to obtain a green check. Any scoped
exception needs an advisory ID, impact analysis and a dated follow-up.
The project and contributions use MIT; include LICENSE in application packages
and preserve required third-party license notices.
Generate notices with the pinned cargo-about tool over the application's normal
desktop dependencies. Preserve bundled font and installer-runtime notices too.
Keep upstream clarifications and reviewed fallbacks bound to source versions and
checksums; unknown attribution gaps block packaging. Never invent copyright
holders or present a standard-license reference as an upstream copyright notice.

CI uses pinned Action commit SHAs, least-privilege tokens, finite timeouts and
platform-specific caches. Release validation, draft creation and publication run
only xtask and use the shared setup action with `desktop: 'false'`; native builds,
package preparation and native verification retain their platform dependencies.
PRs cannot access signing credentials. Daily checks
may build unsigned debug packages and verify final unverified package layout
without GUI activation, uploading distribution files, using signing credentials,
tagging or creating Releases. Installation checks use only disposable hosted
runners or containers and temporary destinations; never install on a developer's
machine or alter an existing installation. macOS debug bundle integrity checks
may use an ad-hoc seal; never use the local development identity in CI.
For explicitly requested isolated native interaction tests, manual CI may retain
unsigned archives, their prepared installers and source manifests for one day via
`validation_artifacts`. The `validation_profile` choice defaults to debug; an
explicit manual test may select release optimization to distinguish runtime
performance from functional failures. Ordinary push/PR checks always use debug
packages. Compiler optimization is not publication; these test artifacts are not
formal release candidates. Keep retention disabled on push/PR events, without
signing, tags, Releases or release Environment access; never publish these builds.
Formal publication requires a complete set of four native targets, matching
version/source SHA, checksums, native checks for the explicitly selected
distribution mode, recorded
installation/upgrade acceptance and approval of the `release` Environment.
Read paginated CI workflow history and match the full SHA, main branch, trusted
workflow path, push/manual event and success locally. Bound the scan to its initial
record count; missing/invalid records or API errors must block publication.
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
Keep one unsigned candidate archive per native target. Schema 3 manifests list
typed artifacts: final macOS DMG, Windows portable ZIP plus per-user NSIS Setup.exe,
and Linux tar.gz plus Debian package. Require the complete exact format set and
preserve the original candidate hash for every target. Signed Windows mode signs
the application, embedded uninstaller and installer; signed macOS mode signs and
notarizes the outer DMG after the app, without later content changes. Debian
dependencies derive from ELF plus runtime-loaded libraries on Ubuntu 24.04; do
not claim untested Debian compatibility. AUR generation is a separate manual
post-publication command, verifies the public tarball, uses makepkg for .SRCINFO,
and never pushes or uploads to AUR. Daily CI may verify temporary silent Windows
installs and generated upgrade fixtures on hosted runners; no desktop activation
or real user settings.

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
and destruction using real winit windows, invisible with activation prohibited;
plain NSWindow geometry tests alone cannot cover backend observation teardown.
After raising macOS selection windows above system UI, restore `NSScreen.frame`
instead of the work area. Warm inactive layers too, and defer image painting
until the viewport reports the complete display size; never stretch a desktop
capture into a menu-bar-constrained window. Native geometry tests must keep
windows invisible and prohibit application activation. An ordered-window
regression must use small fully transparent, mouse-pass-through windows and
release them before exit; hidden-window checks alone do not cover ordering.
Use egui's theme preference to follow system appearance without polling. Theme
and icon changes must preserve unrelated settings and existing shortcut files.
Keep unsafe code denied by default. A platform API that requires unsafe may have
one narrowly scoped exception, with documented preconditions and owned memory.
macOS Dock icons must decode PNG in Rust, avoiding AppKit PNG loading and borrowed
pixel buffers. Updating an icon must not modify the signed application bundle.
At startup use eframe's explicit empty-icon sentinel to preserve the bundle icon;
omitting it selects eframe's fallback. Auto icons wait for a known system theme,
including logic-only background startup; never briefly install a light placeholder.
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
For input-to-window changes, cover the production deferred callback through
action dispatch and root logic to the final window command, including stale
root geometry, DPI/UI zoom, cross-viewport continuation and lock checks.
Distinguish compile/test coverage from interactive checks on Windows, macOS,
Linux X11, and Wayland. Document compositor-controlled pinning and shortcuts.

Do not commit private screenshots, credentials, toolchains, caches, or build output.
Explicitly requested README illustrations may use generated public sample images
from the shared UI sandbox; label their source and keep binary assets in Git LFS.
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

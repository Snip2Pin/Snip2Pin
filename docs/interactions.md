# 交互架构与验证

## 验收目标

四边和四角都能缩放，Shift 保持本次手势开始时的比例；移动、缩放不改原始像素，
复制始终使用原图。窄图保留所有方向。图标在图片外、避开系统栏；高图有侧边空间时
不应仅为上方按钮而整体下移。保持“截图 + pin”，不扩展历史、标注等功能。

## 边界与数据所有权

- `pin::Preparation` 拥有唯一原生桌面图片 Rect。`Frame` 推导宿主留白、局部绘制
  和操作按钮。拖动/缩放只保存手势锚点，不再分别保存图片位置与尺寸。
- 原生回报可以延迟或拆开到达，不能覆写期望几何。系统拖动显式吸收原生位移；
  不同 DPI 的宿主原点先转成同一原生单位。
- eframe Glow 会先绘制并交换缓冲，再派发 viewport 命令。macOS 通过窄适配器在
  绘制前一次设置 NSWindow frame，读取真实内容边界，同时更新 UI clip；保持原生
  窗口类、观察生命周期和浮动层级。异步平台按已观察宿主绘制，再由窗口事件重绘。
- 单一截图 worker 保留一个活跃任务和一个可替换待办，取消不等待系统调用；会话号
  拒绝晚到事件。取消也阻止后续尚未开始的屏幕截图。
- 原生采集、RGBA 转换在工作线程完成，`Arc<ColorImage>` 共享选择数据和纹理上传，
  裁剪后仅保存原始选区。Windows/macOS 可独立采集屏幕，Linux Portal 串行处理。
  跨线程仅传递显示器描述，不传非 Send 原生句柄。
- 平台适配规范化 xcap 坐标：macOS 使用原生点，Windows/Linux 使用物理像素；
  xcap 的 Linux 描述已经除过 Xft 缩放，需在入口还原。系统工作区通过 macOS
  visibleFrame、Windows rcWork、X11 EWMH 获取；读取失败降级为显示器边界，
  Wayland 仍受 compositor 限制，不宣称所有面板避让已验证。
- UI 在上传前检查纹理边长限制，超限显示可操作错误，避免 GPU 后端断言退出。

## 性能记录

生成两张 3840×2160 图片，3 次独立 context，计时仅覆盖 `accept_capture`。
前后使用同一本机、同一测试入口；重构后像素转换已移至计时前的 worker 准备阶段。

| UI 交接耗时 / ms | 重构前 | 重构后 |
| --- | --- | --- |
| debug | 429.27 / 425.86 / 453.45 | 0.341 / 0.051 / 0.030 |
| release | 19.598 / 10.453 / 9.551 | 0.112 / 0.001 / 0.006 |

这衡量 UI 阻塞的减少，不是端到端截图延迟；不含系统采集、像素转换或 GPU 上传。
不使用旧屏幕缓存制造“即时”效果，不承诺零延迟。本地安装改为 release 优化构建。

```sh
cargo test --locked --bin snip2pin capture_handoff_generated_4k_benchmark -- --ignored --nocapture
cargo test --release --locked --bin snip2pin capture_handoff_generated_4k_benchmark -- --ignored --nocapture
```

## 独立评审

三位 Astra 评审分别只获得产品名与用途、产品设计思想、完整实现。前两位不读源码，
各自在独立 headless 浏览器操作同一份 Rust UI 的生成图片沙箱。

| 角度 | 发现 | 处理 |
| --- | --- | --- |
| 普通用户 | 缩放入口和 Shift 不容易发现 | 图片和缩放边缘提供简短悬停提示，保持无常驻说明 |
| 普通用户 | 默认拉伸不如等比直观 | 保留用户明确选择的自由缩放 + Shift 等比；作为未来可讨论取舍 |
| 专业用户 | 接近全高图片被向下推，侧边有空位 | 初始化复用四边按钮布局，只有四边都不可达才调整图片位置；不自动缩小 |
| 专业用户 | 复制反馈离图片远，成功提示颜色像警告 | 沙箱模拟提示并非原生复制反馈；局部成功提示作为待讨论优化，不扩张工具栏 |
| 开发者 | 窄图禁用部分方向、Linux 重复 DPI、纹理超限、取消后继续采集 | 修复并补生成数据回归 |
| 开发者 | 工作区不含 Windows/Linux 系统栏 | 增加 Win32 / X11 适配，实际平台交互仍需验收 |
| 开发者 | 理想命令顺序不能证明原生无抖 | 修正回放顺序，增加绘制前适配、分批回报和隐藏原生检查 |

沙箱最初缺少 native host 信息导致拖不动，前景 Area 也拦截主页输入；已修复并由
评审复测。它们属于沙箱适配问题，不记录为原生产品缺陷。

自动化覆盖实际 UI 事件、八方向/窄图/Shift、乱序宿主回报、DPI、取消和原始复制。
macOS 隐藏 winit 窗口验证布局、后端尺寸与观察器生命周期，禁止激活和显示窗口。
网页测试无法验证原生 GL drawable、合成器瞬时闪动、系统剪贴板、托盘与隐私权限；
这些仍须由用户进行原生交互验收。没有操作用户桌面或真实截图。

## 本地网页沙箱

使用固定工具链、锁文件匹配的 wasm-bindgen-cli 和本机 CJK 字体（不提交字体）：

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.129 --locked
cargo xtask preview '/System/Library/Fonts/STHeiti Light.ttc'
python3 -m http.server 8765 --bind 127.0.0.1 --directory preview
```

Windows/Linux 将字体参数换成本机 `.ttf` / `.ttc` / `.otf` 路径。生成文件与字体均在
被忽略的 `preview/pkg/`；仅在回环地址访问，不对外发布。每位评审建立新的 headless
浏览器上下文，不连接用户的现有浏览器。沙箱的复制、自启只改变内存模拟状态。

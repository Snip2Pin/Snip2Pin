Snip2Pin — 截图并置顶 / Capture and pin

macOS: 打开 DMG，将 Snip2Pin.app 拖入「应用程序」。/ Open the DMG and drag Snip2Pin.app to Applications.
标有 -unverified 的包没有受信任的发布者签名：macOS 未经 Apple 公证，Windows 未签名。
系统可能拦截。macOS 可参考 https://support.apple.com/102445 ，通过系统设置确认打开；
组织策略可能禁止运行。macOS 更新可能需要重新授予截屏权限。
Packages marked -unverified have no trusted publisher signature: macOS builds are
not notarized and Windows builds are unsigned. The OS or device policy may block
them. See Apple's instructions above for user-approved opening. macOS updates may
require screen capture permission again.

Windows: 运行 Setup.exe 按当前用户安装，或解压 ZIP 运行 snip2pin.exe。
Run Setup.exe for a per-user installation, or extract the ZIP and run snip2pin.exe.
Linux: 安装 .deb，或在 tar.gz 解压目录运行 ./snip2pin。
Install the .deb, or run ./snip2pin from the extracted tar.gz directory.
Linux builds target Ubuntu 24.04 or compatible systems with the required desktop
libraries. Wayland behavior depends on the compositor; X11/XWayland is preferred.
中文界面需系统中文字体，例如 Noto CJK 或文泉驿微米黑。
Chinese UI needs a system CJK font, such as Noto CJK or WenQuanYi Micro Hei.

点击截图按钮或按快捷键，拖动框选，松手后置顶并复制。拖动贴图移动，拖动边缘
缩放，遵循系统惯例；点击 × 或按 Esc 关闭。悬停显示复制、锁定和关闭按钮，窄图通过右键操作。
主页缩略图可定位贴图；锁定后不能移动、缩放或关闭，有锁定贴图时退出需要确认。
Capture with the button or shortcut, drag to select, and release to pin and copy.
Drag pins to move and edges to resize following system conventions, and
click × or press Esc to close. Hover for Copy, Lock and Close; use the right-click menu
for controls hidden on narrow images. Select a thumbnail in Home to locate its
pin. Locked pins cannot move, resize or close; quitting asks for confirmation.

Default shortcut: macOS Command+Shift+2; Windows/Linux X11 Ctrl+Shift+2.
Settings: shortcut, theme, icons and opt-in login startup (off by default).

源码与完整说明 / Source and documentation:
https://github.com/Snip2Pin/Snip2Pin
License: MIT. See LICENSE and THIRD_PARTY_NOTICES.txt in this package.

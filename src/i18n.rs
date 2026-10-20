//! One typed catalogue shared by native UI, the sandbox and deferred messages.
use eframe::egui;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Language {
    #[default]
    System,
    Chinese,
    English,
}

impl Language {
    pub fn resolve(self, system: Locale) -> Locale {
        match self {
            Self::System => system,
            Self::Chinese => Locale::Chinese,
            Self::English => Locale::English,
        }
    }

    pub fn label(self, locale: Locale) -> &'static str {
        match self {
            Self::System => locale.text(Text::System),
            Self::Chinese => "简体中文",
            Self::English => "English",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Locale {
    Chinese,
    #[default]
    English,
}

impl Locale {
    pub fn from_preferences(locales: impl IntoIterator<Item = impl AsRef<str>>) -> Self {
        locales
            .into_iter()
            .find_map(|tag| {
                let tag = tag.as_ref().trim();
                match tag.split(['-', '_', '.', '@']).next().unwrap_or("") {
                    code if code.eq_ignore_ascii_case("zh") => Some(Self::Chinese),
                    code if code.eq_ignore_ascii_case("en") => Some(Self::English),
                    _ => None,
                }
            })
            .unwrap_or_default()
    }

    pub fn system() -> Self {
        Self::from_preferences(sys_locale::get_locales())
    }

    pub fn pin_count(self, count: usize) -> String {
        match self {
            Self::Chinese => format!("{count} 张贴图"),
            Self::English if count == 1 => "1 pin".into(),
            Self::English => format!("{count} pins"),
        }
    }
}

pub fn set_locale(ctx: &egui::Context, locale: Locale) {
    ctx.data_mut(|data| data.insert_temp(egui::Id::new("snip2pin-locale"), locale));
}

pub fn locale(ctx: &egui::Context) -> Locale {
    ctx.data(|data| data.get_temp(egui::Id::new("snip2pin-locale")))
        .unwrap_or_default()
}

macro_rules! catalogue {
    ($($key:ident => ($en:literal, $zh:literal)),+ $(,)?) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum Text { $($key),+ }
        impl Locale {
            pub fn text(self, key: Text) -> &'static str {
                match (self, key) {
                    $((Self::English, Text::$key) => $en,
                      (Self::Chinese, Text::$key) => $zh),+
                }
            }
        }
        #[cfg(test)]
        const ALL_TEXT: &[Text] = &[$(Text::$key),+];
    };
}

catalogue! {
    WindowTitle => ("Snip2Pin", "Snip2Pin · 截图置顶"),
    System => ("Follow system", "跟随系统"),
    Light => ("Light", "浅色"),
    Dark => ("Dark", "深色"),
    AutoIcon => ("Follow theme", "跟随主题"),
    LineIcon => ("Minimal lines", "极简线条"),
    MetalIcon => ("Dark metal", "深色金属"),
    Heading => ("Select. Copy. Pin.", "框选，复制，置顶。"),
    Subtitle => ("Auto-copy. Pin right where you selected.", "自动复制，留在框选原位。"),
    Capture => ("Capture & pin", "截图并置顶"),
    Capturing => ("Capturing · Click to cancel", "正在截图 · 点击取消"),
    CancelCapture => ("Cancel capture", "取消截图"),
    ContinueCapture => ("Allow access & continue", "授权后继续截图"),
    CaptureHelp => ("Drag to select; Esc cancels. Drag pins to move, resize at the edges. Hover to copy, lock or close.", "拖动框选，Esc 取消。拖动贴图移动，边缘缩放。悬停贴图可复制、锁定或关闭。"),
    LocalShortcut => ("In-app: {value}", "窗口内 {value}"),
    CancelEdit => ("Cancel", "取消修改"),
    ChangeShortcut => ("Change shortcut", "修改快捷键"),
    RecordShortcut => ("Press a new shortcut. Esc cancels.", "按下新的组合键，Esc 取消。"),
    PermissionTitle => ("Screen recording access needed", "需要屏幕录制权限"),
    PermissionHelp => ("Allow Snip2Pin in system settings, then return here to continue.", "打开系统设置，允许 Snip2Pin 后返回这里继续。"),
    OpenSettings => ("Open permission settings", "打开授权设置"),
    RestartHelp => ("If access is allowed but capture still fails, reopen the app.", "已允许但仍不可用时，请重新打开应用。"),
    Restart => ("Reopen app", "重新打开应用"),
    RestartWarning => ("Reopening closes all current pins.", "重新打开会关闭当前贴图。"),
    Theme => ("Theme", "主题"),
    Icon => ("Icon", "图标"),
    Language => ("Language", "语言"),
    Startup => ("Launch at login", "开机自启"),
    StartupHelp => ("Run in the menu bar after login. Off by default.", "登录后在菜单栏后台运行，默认关闭。"),
    Demo => ("Demo: no screen access.", "演示模式：不读取屏幕。"),
    Downloads => ("Releases ↗", "版本与下载 ↗"),
    DownloadsHelp => ("View GitHub Releases in your browser.", "在浏览器中查看 GitHub Releases。"),
    Home => ("Open home", "打开主页"),
    ClosePins => ("Close unlocked pins", "关闭未锁定贴图"),
    UnlockPins => ("Unlock all pins", "解锁全部贴图"),
    UnlockHint => ("Locked · {value} to unlock all", "已锁定 · {value} 解锁全部贴图"),
    ScreenHintError => ("Could not raise the unlock hint: {value}", "无法显示解锁提醒：{value}"),
    ReservedUnlockShortcut => ("This combination is reserved for unlocking all pins.", "此组合键用于解锁全部贴图，请选择其他截图快捷键。"),
    UnlockShortcutError => ("Could not register the unlock shortcut: {value}", "无法注册全局解锁快捷键：{value}"),
    PinZoomHint => ("Pin too small? Press + to enlarge.", "贴图太小？按 + 放大"),
    Quit => ("Quit Snip2Pin", "退出 Snip2Pin"),
    Copy => ("Copy", "复制"),
    RestoreSize => ("Restore original size", "恢复原始尺寸"),
    Close => ("Close", "关闭"),
    Lock => ("Lock", "锁定"),
    Unlock => ("Unlock", "解锁"),
    LocatePin => ("Show pin {value}", "定位贴图 {value}"),
    PinLabel => ("Pin {value}", "贴图 {value}"),
    QuitLockedTitle => ("Quit with locked pins?", "退出并关闭锁定贴图？"),
    QuitLockedHelp => ("Quitting closes all pins, including locked ones. Nothing is saved.", "退出会关闭全部贴图，包括锁定贴图；图片不会保存。"),
    QuitConfirm => ("Quit and close all", "退出并关闭全部"),
    QuitDialogUnavailable => ("Native confirmation is unavailable. Use the in-app confirmation.", "系统确认框暂不可用，请使用应用内确认框。"),
    QuitDialogError => ("Could not open native confirmation: {value}", "无法打开系统确认框：{value}"),
    Cancel => ("Cancel", "取消"),
    DockMenuMissing => ("Dock menu resources are missing. Reinstall the application.", "Dock 菜单资源缺失，请重新安装应用。"),
    DockMenuLoadFailed => ("Could not load the Dock menu.", "无法加载 Dock 菜单。"),
    DockQuitUnavailable => ("Could not connect the system Quit action. Locked pin quit confirmation is unavailable.", "无法连接系统退出操作，锁定贴图的退出确认不可用。"),
    SelectionHelp => ("Click to capture or drag a region · Esc cancels", "单击截图或拖动框选 · Esc 取消"),
    ShortcutSaved => ("Shortcut saved.", "快捷键已保存。"),
    ShortcutSavedLocal => ("Shortcut saved. This desktop supports in-app shortcuts only.", "快捷键已保存；当前桌面仅支持窗口内快捷键。"),
    AppearanceSaved => ("Appearance saved.", "外观设置已保存。"),
    DemoAppearance => ("Demo appearance updated for this session.", "演示外观已更新，本次有效。"),
    DemoPinned => ("Copied & pinned (simulated clipboard).", "已复制并置顶（沙箱仅模拟复制）。"),
    DemoCopied => ("Copied (simulated; system clipboard unchanged).", "已复制（沙箱仅模拟，不写系统剪贴板）。"),
    Sandbox => ("Local UI sandbox · Generated images · Simulated window gestures · No desktop or clipboard access", "本地 UI 沙箱 · 生成图片 · 拖动/自由缩放为模拟 · 不访问桌面/剪贴板"),
    GlobalShortcutError => ("Global shortcut unavailable: {value}", "全局快捷键不可用：{value}"),
    TrayError => ("Tray unavailable: {value}", "菜单栏图标不可用：{value}"),
    StartupError => ("Launch at login unavailable: {value}", "开机自启不可用：{value}"),
    RestartError => ("Could not prepare to reopen the app.", "无法准备重新打开应用。"),
    TextureLimit => ("This GPU supports captures up to {value} pixels per edge. The display exceeds this limit.", "当前显卡最多支持 {value} 像素边长的截图，屏幕分辨率超出限制。"),
    NoDisplays => ("No displays available.", "没有可用屏幕。"),
    InvalidShortcut => ("Invalid shortcut: {value}", "无效快捷键：{value}"),
    ShortcutModifiers => ("Hold Ctrl, Alt, or Command on macOS as well.", "请同时按住 Ctrl、Alt，或 macOS 的 Command 键。"),
    ShortcutError => ("Shortcut unavailable; try another combination: {value}", "快捷键不可用，请换一个组合：{value}"),
    UpdateShortcutError => ("Could not update shortcut: {value}", "更新快捷键失败：{value}"),
    ReadSettings => ("Could not read settings: {value}", "读取设置失败：{value}"),
    ReadLegacyShortcut => ("Could not read previous shortcut settings: {value}", "读取旧快捷键设置失败：{value}"),
    SaveSettings => ("Could not save settings: {value}", "保存设置失败：{value}"),
    NoCaptureAccess => ("Screen recording access is not allowed. Open permission settings in the app.", "尚未获得屏幕录制权限，请在应用中打开授权设置。"),
    DisplayDisconnected => ("Display disconnected. Capture again.", "显示器已断开，请重新截图。"),
    CaptureCanceled => ("Capture canceled.", "截图已取消。"),
    CaptureFailed => ("Capture failed. Check system capture permissions: {value}", "截图失败，请检查系统截图权限：{value}"),
    EmptyCapture => ("The system returned an empty capture.", "系统没有返回有效的截图。"),
    CaptureCrashed => ("The capture task ended unexpectedly.", "截图任务异常结束。"),
    CaptureBackendCrashed => ("The capture backend ended unexpectedly.", "截图后端异常结束。"),
    CaptureQueueError => ("Capture task state unavailable.", "截图任务状态不可用。"),
    InvalidIcon => ("Invalid icon asset. Run git lfs pull: {value}", "图标资源无效，请先运行 git lfs pull：{value}"),
    NoSettingsEntry => ("This platform has no standard screen recording settings entry.", "当前平台没有统一的屏幕录制授权设置入口。"),
    WaylandShortcut => ("On Wayland, capture from the app or configure a shortcut in desktop settings.", "Wayland 下请从应用窗口截图，或在桌面设置里配置启动快捷键。"),
    NoTrayHost => ("No StatusNotifier tray host. Home will remain available.", "桌面未提供 StatusNotifier 托盘，应用将保留主页。"),
    InvalidAppPath => ("The app path is not valid Unicode.", "应用路径不是有效 Unicode。"),
    InstallFirst => ("Install the app before enabling launch at login.", "请先安装应用，再开启开机自启。"),
    NoConfigDirectory => ("Could not find the user configuration directory.", "无法找到用户配置目录。"),
    StartupParent => ("The startup entry has no parent directory.", "自启配置没有父目录。"),
    WriteStartup => ("Could not change launch at login: {value}", "修改开机自启失败：{value}"),
    InvalidStartupPath => ("This path cannot be used for a desktop startup entry.", "此路径不能用于桌面自启入口。"),
    UiThread => ("This operation must run on the UI thread.", "此操作必须在 UI 线程执行。"),
    NativeDragError => ("Could not initialize native pin dragging.", "无法为贴图初始化原生拖动。"),
    PinNotFound => ("The native pin window was not found.", "未找到贴图的原生窗口。"),
    PinViewMissing => ("The pin window has no content view.", "贴图窗口没有内容视图。"),
    AllocateIcon => ("Could not allocate the Dock icon.", "无法分配 Dock 图标。"),
    IconStride => ("Invalid icon row length.", "无效图标行长度。"),
    IconBuffer => ("Invalid icon buffer.", "无效图标缓冲区。"),
    SettingsUrl => ("Could not create the permission settings URL.", "无法创建授权设置地址。"),
    OpenSettingsError => ("Could not open system permission settings.", "无法打开系统授权设置。"),
    AppIdentity => ("App metadata does not match the compiled identifier.", "应用元数据与编译标识不一致。"),
    AppPath => ("Could not read the application path.", "无法读取应用路径。"),
    AppUnregistered => ("The system has not registered this application identifier.", "系统未登记当前应用标识。"),
    AppRegistration => ("System registration points to another app. Installation is incomplete.", "系统登记指向了另一份应用，安装未完成。"),
    QuitSettings => ("Quit System Settings before migrating the app identifier to avoid cached permissions. Routine updates do not require this.", "请先完全退出系统设置，再迁移应用标识，避免授权页缓存旧身份。日常更新不需要此步骤。"),
    QuitApp => ("Quit Snip2Pin from the menu bar before migrating its identifier.", "请先从菜单栏退出 Snip2Pin，再迁移应用标识。"),
}

/// Keep our messages typed until presentation; backend diagnostics remain intact.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Message {
    Text(Text, Option<String>),
    Context(Text, Box<Message>),
    Diagnostic(String),
    Combined(Vec<Message>),
}

impl Text {
    pub fn with(self, detail: impl std::fmt::Display) -> Message {
        Message::Text(self, Some(detail.to_string()))
    }
    pub fn context(self, detail: Message) -> Message {
        Message::Context(self, Box::new(detail))
    }
}

impl Message {
    pub fn render(&self, locale: Locale) -> String {
        match self {
            Self::Text(key, Some(value)) => locale.text(*key).replace("{value}", value),
            Self::Text(key, None) => locale.text(*key).into(),
            Self::Context(key, detail) => {
                locale.text(*key).replace("{value}", &detail.render(locale))
            }
            Self::Diagnostic(text) => text.clone(),
            Self::Combined(messages) => messages
                .iter()
                .map(|message| message.render(locale))
                .collect::<Vec<_>>()
                .join("\n"),
        }
    }
}

impl From<Text> for Message {
    fn from(value: Text) -> Self {
        Self::Text(value, None)
    }
}
impl From<String> for Message {
    fn from(value: String) -> Self {
        Self::Diagnostic(value)
    }
}
impl From<&str> for Message {
    fn from(value: &str) -> Self {
        Self::Diagnostic(value.into())
    }
}
impl std::fmt::Display for Message {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.render(Locale::English))
    }
}
impl std::error::Error for Message {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_matching_handles_regions_encodings_and_preferred_languages() {
        for tag in ["zh", "zh-CN", "zh_Hans_CN.UTF-8", "ZH-tw", "zh-Hant-HK"] {
            assert_eq!(Locale::from_preferences([tag]), Locale::Chinese);
        }
        for tag in [
            "en",
            "en-US",
            "en_GB.UTF-8",
            "C",
            "POSIX",
            "fr-FR",
            "",
            "zhong",
        ] {
            assert_eq!(Locale::from_preferences([tag]), Locale::English);
        }
        assert_eq!(
            Locale::from_preferences(["fr-FR", "zh-CN", "en"]),
            Locale::Chinese
        );
        assert_eq!(
            Locale::from_preferences(["en-US", "zh-CN"]),
            Locale::English
        );
        assert_eq!(
            Locale::from_preferences(Vec::<String>::new()),
            Locale::English
        );
        assert_eq!(Language::default(), Language::System);
        assert_eq!(Language::English.resolve(Locale::Chinese), Locale::English);
        assert_eq!(Language::Chinese.resolve(Locale::English), Locale::Chinese);
    }

    #[test]
    fn both_catalogues_are_complete_and_messages_translate_at_presentation() {
        for key in ALL_TEXT {
            for locale in [Locale::English, Locale::Chinese] {
                assert!(!locale.text(*key).trim().is_empty(), "{key:?}");
            }
            assert_eq!(
                Locale::English.text(*key).matches("{value}").count(),
                Locale::Chinese.text(*key).matches("{value}").count()
            );
        }
        let message = Text::TextureLimit.with(128);
        assert!(message.render(Locale::Chinese).contains("128 像素"));
        assert!(message.render(Locale::English).contains("128 pixels"));
        assert_eq!(Locale::English.pin_count(1), "1 pin");
        assert_eq!(Locale::English.pin_count(2), "2 pins");
    }
}

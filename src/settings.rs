#[cfg(not(target_arch = "wasm32"))]
use std::{
    io::Write,
    path::{Path, PathBuf},
};

use crate::icon::Variant;
use eframe::egui;
use global_hotkey::hotkey::{HotKey, Modifiers};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Binding(pub HotKey);

impl Default for Binding {
    fn default() -> Self {
        Self(HotKey::new(
            Some(if cfg!(target_os = "macos") {
                Modifiers::SUPER | Modifiers::SHIFT
            } else {
                Modifiers::CONTROL | Modifiers::SHIFT
            }),
            global_hotkey::hotkey::Code::Digit2,
        ))
    }
}

impl Binding {
    pub fn parse(text: &str) -> Result<Self, String> {
        let key: HotKey = text
            .trim()
            .parse()
            .map_err(|error| format!("无效快捷键：{error}"))?;
        if !key
            .mods
            .intersects(Modifiers::CONTROL | Modifiers::ALT | Modifiers::SUPER)
        {
            return Err("请同时按住 Ctrl、Alt，或 macOS 的 Command 键。".into());
        }
        Ok(Self(key))
    }

    pub fn from_key(key: egui::Key, modifiers: egui::Modifiers) -> Result<Self, String> {
        let mut parts = Vec::new();
        if modifiers.ctrl {
            parts.push("Ctrl");
        }
        if modifiers.alt {
            parts.push("Alt");
        }
        if modifiers.shift {
            parts.push("Shift");
        }
        if modifiers.mac_cmd || (!cfg!(target_os = "macos") && modifiers.command && !modifiers.ctrl)
        {
            parts.push("Super");
        }
        parts.push(match key {
            egui::Key::Equals => "Equal",
            egui::Key::OpenBracket => "BracketLeft",
            egui::Key::CloseBracket => "BracketRight",
            egui::Key::Backtick => "Backquote",
            other => other.name(),
        });
        Self::parse(&parts.join("+"))
    }

    pub fn matches(self, event: &egui::Event) -> bool {
        matches!(event, egui::Event::Key { key, physical_key, modifiers, pressed: true, repeat: false, .. }
            if Self::from_key(physical_key.unwrap_or(*key), *modifiers).ok() == Some(self))
    }

    pub fn label(self) -> String {
        self.0
            .to_string()
            .replace(
                "super",
                if cfg!(target_os = "macos") {
                    "Command"
                } else {
                    "Windows"
                },
            )
            .replace("control", "Ctrl")
            .replace("ctrl", "Ctrl")
            .replace("shift", "Shift")
            .replace("alt", "Alt")
            .replace("Key", "")
            .replace("Digit", "")
            .replace('+', " + ")
    }
}

impl TryFrom<String> for Binding {
    type Error = String;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(&value)
    }
}

impl From<Binding> for String {
    fn from(binding: Binding) -> Self {
        binding.0.to_string()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Theme {
    #[default]
    System,
    Light,
    Dark,
}

impl Theme {
    pub fn label(self) -> &'static str {
        match self {
            Self::System => "跟随系统",
            Self::Light => "浅色",
            Self::Dark => "深色",
        }
    }

    pub fn preference(self) -> egui::ThemePreference {
        match self {
            Self::System => egui::ThemePreference::System,
            Self::Light => egui::ThemePreference::Light,
            Self::Dark => egui::ThemePreference::Dark,
        }
    }

    pub fn active(self, system: Option<egui::Theme>) -> egui::Theme {
        match self {
            Self::System => system.unwrap_or(egui::Theme::Light),
            Self::Light => egui::Theme::Light,
            Self::Dark => egui::Theme::Dark,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Icon {
    #[default]
    Auto,
    Line,
    Metal,
}

impl Icon {
    pub fn label(self) -> &'static str {
        match self {
            Self::Auto => "跟随主题",
            Self::Line => "极简线条",
            Self::Metal => "深色金属",
        }
    }

    pub fn resolve(self, theme: egui::Theme) -> Variant {
        match self {
            Self::Auto if theme == egui::Theme::Dark => Variant::Metal,
            Self::Auto | Self::Line => Variant::Line,
            Self::Metal => Variant::Metal,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Preferences {
    pub binding: Binding,
    pub theme: Theme,
    pub icon: Icon,
}

impl Preferences {
    pub fn icon_variant(self, system: Option<egui::Theme>) -> Variant {
        self.icon.resolve(self.theme.active(system))
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn load(path: &Path) -> Result<Self, String> {
        match std::fs::read_to_string(path) {
            Ok(text) => toml::from_str(&text).map_err(|error| format!("读取设置失败：{error}")),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                match std::fs::read_to_string(path.with_file_name("shortcut.txt")) {
                    Ok(text) => Ok(Self {
                        binding: Binding::parse(&text)?,
                        ..Default::default()
                    }),
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                        Ok(Self::default())
                    }
                    Err(error) => Err(format!("读取旧快捷键设置失败：{error}")),
                }
            }
            Err(error) => Err(format!("读取设置失败：{error}")),
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn save(self, path: &Path) -> Result<(), String> {
        let result = || -> Result<(), Box<dyn std::error::Error>> {
            let parent = path
                .parent()
                .filter(|parent| !parent.as_os_str().is_empty())
                .unwrap_or_else(|| Path::new("."));
            std::fs::create_dir_all(parent)?;
            let mut file = tempfile::NamedTempFile::new_in(parent)?;
            file.write_all(toml::to_string_pretty(&self)?.as_bytes())?;
            file.as_file().sync_all()?;
            file.persist(path)?;
            Ok(())
        };
        result().map_err(|error| format!("保存设置失败：{error}"))
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn path() -> Option<PathBuf> {
    directories::BaseDirs::new().map(|dirs| {
        dirs.config_dir()
            .join(crate::APP_NAME)
            .join("preferences.toml")
    })
}

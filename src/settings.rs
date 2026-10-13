use std::path::{Path, PathBuf};

use eframe::egui;
use global_hotkey::hotkey::{HotKey, Modifiers};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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

    pub fn load(path: &Path) -> Result<Self, String> {
        match std::fs::read_to_string(path) {
            Ok(text) => Self::parse(&text),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(error) => Err(format!("读取快捷键设置失败：{error}")),
        }
    }

    pub fn save(self, path: &Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| format!("保存设置失败：{error}"))?;
        }
        std::fs::write(path, self.0.to_string()).map_err(|error| format!("保存设置失败：{error}"))
    }
}

pub fn path() -> Option<PathBuf> {
    directories::ProjectDirs::from("io.github", "rensilin", "SnipSprig")
        .map(|dirs| dirs.config_dir().join("shortcut.txt"))
}

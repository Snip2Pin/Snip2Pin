#[cfg(not(target_arch = "wasm32"))]
use std::{
    io::Write,
    path::{Path, PathBuf},
};

use crate::i18n::{Language, Locale, Message, Text};
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
                Modifiers::META | Modifiers::SHIFT
            } else {
                Modifiers::CONTROL | Modifiers::SHIFT
            }),
            global_hotkey::hotkey::Code::Digit2,
        ))
    }
}

impl Binding {
    pub fn unlock_all() -> Self {
        Self(HotKey::new(
            Some(
                Modifiers::ALT
                    | Modifiers::SHIFT
                    | if cfg!(target_os = "macos") {
                        Modifiers::META
                    } else {
                        Modifiers::CONTROL
                    },
            ),
            global_hotkey::hotkey::Code::KeyL,
        ))
    }

    pub fn validate_capture(self) -> Result<(), Message> {
        if self == Self::unlock_all() {
            Err(Text::ReservedUnlockShortcut.into())
        } else {
            Ok(())
        }
    }

    pub fn parse(text: &str) -> Result<Self, Message> {
        let key: HotKey = text
            .trim()
            .parse()
            .map_err(|error| Text::InvalidShortcut.with(error))?;
        if !key
            .mods
            .intersects(Modifiers::CONTROL | Modifiers::ALT | Modifiers::META)
        {
            return Err(Text::ShortcutModifiers.into());
        }
        Ok(Self(key))
    }

    pub fn from_key(key: egui::Key, modifiers: egui::Modifiers) -> Result<Self, Message> {
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
            parts.push("Meta");
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

    /// The recovery shortcut must never start a capture, even in old settings.
    pub fn capture_requested(self, events: &[egui::Event]) -> bool {
        self.validate_capture().is_ok() && events.iter().any(|event| self.matches(event))
    }

    pub fn label(self) -> String {
        self.0
            .to_string()
            .split('+')
            .map(|token| match token {
                "meta" if cfg!(target_os = "macos") => "Command",
                "meta" => "Windows",
                "control" => "Ctrl",
                "shift" => "Shift",
                "alt" => "Alt",
                key if key.starts_with("Key") && key.len() == 4 => &key[3..],
                key => key.strip_prefix("Digit").unwrap_or(key),
            })
            .collect::<Vec<_>>()
            .join(" + ")
    }
}

impl TryFrom<String> for Binding {
    type Error = Message;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(&value)
    }
}

impl From<Binding> for String {
    fn from(binding: Binding) -> Self {
        // Keep files readable by versions whose parser predates the Meta alias.
        binding
            .0
            .to_string()
            .split('+')
            .map(|token| if token == "meta" { "super" } else { token })
            .collect::<Vec<_>>()
            .join("+")
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
    pub fn label(self, locale: Locale) -> &'static str {
        locale.text(match self {
            Self::System => Text::System,
            Self::Light => Text::Light,
            Self::Dark => Text::Dark,
        })
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
    pub fn label(self, locale: Locale) -> &'static str {
        locale.text(match self {
            Self::Auto => Text::AutoIcon,
            Self::Line => Text::LineIcon,
            Self::Metal => Text::MetalIcon,
        })
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
    pub language: Language,
}

impl Preferences {
    pub fn icon_variant(self, system: Option<egui::Theme>) -> Variant {
        self.icon.resolve(self.theme.active(system))
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn load(path: &Path) -> Result<Self, Message> {
        match std::fs::read_to_string(path) {
            Ok(text) => {
                let value: toml::Value =
                    toml::from_str(&text).map_err(|error| Text::ReadSettings.with(error))?;
                // Validate before Serde turns typed shortcut errors into diagnostic strings.
                if let Some(binding) = value.get("binding").and_then(toml::Value::as_str) {
                    Binding::parse(binding).map_err(|error| Text::ReadSettings.context(error))?;
                }
                value
                    .try_into()
                    .map_err(|error| Text::ReadSettings.with(error))
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                match std::fs::read_to_string(path.with_file_name("shortcut.txt")) {
                    Ok(text) => Ok(Self {
                        binding: Binding::parse(&text)?,
                        ..Default::default()
                    }),
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                        Ok(Self::default())
                    }
                    Err(error) => Err(Text::ReadLegacyShortcut.with(error)),
                }
            }
            Err(error) => Err(Text::ReadSettings.with(error)),
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn save(self, path: &Path) -> Result<(), Message> {
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
        result().map_err(|error| Text::SaveSettings.with(error))
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

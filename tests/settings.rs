use eframe::egui::{Event, Key, Modifiers};
use snip2pin::{
    icon::{self, Variant},
    settings::{Binding, Icon, Preferences, Theme},
};

#[test]
fn recorded_shortcut_matches_physical_keys_and_exact_modifiers() {
    let modifiers = Modifiers {
        ctrl: true,
        alt: true,
        command: !cfg!(target_os = "macos"),
        ..Default::default()
    };
    let binding = Binding::from_key(Key::K, modifiers).expect("valid binding");
    let event = Event::Key {
        key: Key::J,
        physical_key: Some(Key::K),
        pressed: true,
        repeat: false,
        modifiers,
    };
    assert!(binding.matches(&event));
    let extra = Event::Key {
        key: Key::K,
        physical_key: Some(Key::K),
        pressed: true,
        repeat: false,
        modifiers: Modifiers {
            shift: true,
            ..modifiers
        },
    };
    assert!(!binding.matches(&extra));
    assert_eq!(
        Binding::parse(&binding.0.to_string()).expect("round trip"),
        binding
    );
}

#[test]
fn typing_keys_and_malformed_shortcuts_are_rejected() {
    for input in ["K", "Shift+K", "Ctrl+K+L", "Ctrl+", ""] {
        assert!(Binding::parse(input).is_err(), "{input}");
    }
    let modifiers = if cfg!(target_os = "macos") {
        Modifiers {
            mac_cmd: true,
            command: true,
            shift: true,
            ..Default::default()
        }
    } else {
        Modifiers {
            ctrl: true,
            command: true,
            shift: true,
            ..Default::default()
        }
    };
    assert_eq!(
        Binding::from_key(Key::Num2, modifiers).expect("default keys"),
        Binding::default()
    );
}

#[test]
fn both_icon_assets_match_vectors_and_render_at_small_and_large_sizes() {
    for variant in [Variant::Line, Variant::Metal] {
        let png = icon::render(variant, 1024).expect("LFS PNG");
        let vector = icon::render_vector(variant, 1024).expect("SVG source");
        assert!(png == vector, "{variant:?} PNG differs from its SVG source");
        for size in [16, 64, 1024] {
            let image = icon::render(variant, size).expect("valid artwork");
            assert_eq!(image.dimensions(), (size, size));
            assert_eq!(image.get_pixel(0, 0).0[3], 0);
            assert!(image.pixels().any(|pixel| pixel.0[3] == 255));
        }
    }
}

#[test]
fn shortcut_preference_survives_restart_and_invalid_files_are_reported() {
    let directory = std::env::temp_dir().join(format!(
        "snip2pin-settings-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    let path = directory.join("preferences/preferences.toml");
    assert_eq!(
        Preferences::load(&path).expect("missing uses default"),
        Preferences::default()
    );
    let first = Binding::parse("Ctrl+Alt+K").expect("valid shortcut");
    let mut preferences = Preferences {
        binding: first,
        theme: Theme::Dark,
        icon: Icon::Line,
    };
    preferences.save(&path).expect("save");
    assert_eq!(
        Preferences::load(&path).expect("load after restart"),
        preferences
    );
    let second = Binding::parse("Ctrl+Shift+4").expect("replacement");
    preferences.binding = second;
    preferences.save(&path).expect("replace");
    assert_eq!(
        Preferences::load(&path).expect("load replacement"),
        preferences
    );
    std::fs::write(&path, "invalid").expect("fixture");
    assert!(Preferences::load(&path).is_err());
    std::fs::remove_dir_all(directory).expect("clean test preferences");
}

#[test]
fn automatic_icon_follows_active_theme_and_manual_icons_stay_fixed() {
    for (theme, system, expected) in [
        (Theme::System, None, Variant::Line),
        (
            Theme::System,
            Some(eframe::egui::Theme::Light),
            Variant::Line,
        ),
        (
            Theme::System,
            Some(eframe::egui::Theme::Dark),
            Variant::Metal,
        ),
        (Theme::Light, Some(eframe::egui::Theme::Dark), Variant::Line),
        (
            Theme::Dark,
            Some(eframe::egui::Theme::Light),
            Variant::Metal,
        ),
    ] {
        let mut preferences = Preferences {
            theme,
            ..Default::default()
        };
        assert_eq!(preferences.icon_variant(system), expected);
        preferences.icon = Icon::Line;
        assert_eq!(preferences.icon_variant(system), Variant::Line);
        preferences.icon = Icon::Metal;
        assert_eq!(preferences.icon_variant(system), Variant::Metal);
    }
}

#[test]
fn system_theme_changes_update_actual_egui_colors_and_automatic_icon() {
    let ctx = eframe::egui::Context::default();
    let preferences = Preferences::default();
    assert_eq!(preferences.theme, Theme::System);
    ctx.set_theme(preferences.theme.preference());
    for theme in [
        eframe::egui::Theme::Light,
        eframe::egui::Theme::Dark,
        eframe::egui::Theme::Light,
    ] {
        let mut output = ctx.run_ui(
            eframe::egui::RawInput {
                system_theme: Some(theme),
                ..Default::default()
            },
            |ui| {
                assert_eq!(ui.visuals().dark_mode, theme == eframe::egui::Theme::Dark);
                assert_eq!(
                    preferences.icon.resolve(ui.ctx().theme()),
                    if theme == eframe::egui::Theme::Dark {
                        Variant::Metal
                    } else {
                        Variant::Line
                    }
                );
            },
        );
        output.textures_delta.clear();
    }
}

#[test]
fn legacy_shortcut_migrates_without_being_reset_or_overwriting_old_file() {
    let directory = tempfile::tempdir().expect("temporary preferences");
    let legacy = directory.path().join("shortcut.txt");
    let path = directory.path().join("preferences.toml");
    std::fs::write(&legacy, "Ctrl+Alt+K").expect("old version fixture");
    let mut preferences = Preferences::load(&path).expect("read old shortcut");
    assert_eq!(
        preferences.binding,
        Binding::parse("Ctrl+Alt+K").expect("binding")
    );
    assert_eq!(preferences.theme, Theme::System);
    preferences.theme = Theme::Dark;
    preferences.icon = Icon::Metal;
    preferences.save(&path).expect("save new format");
    assert_eq!(Preferences::load(&path).expect("reload"), preferences);
    assert_eq!(
        std::fs::read_to_string(legacy).expect("preserved old file"),
        "Ctrl+Alt+K"
    );
}

#[test]
fn invalid_preferences_are_reported_and_missing_fields_use_defaults() {
    let directory = tempfile::tempdir().expect("temporary preferences");
    let path = directory.path().join("preferences.toml");
    for text in [
        "theme = 'unknown'",
        "icon = 'unknown'",
        "binding = 'K'",
        "theem = 'dark'",
        "theme = ",
    ] {
        std::fs::write(&path, text).expect("invalid fixture");
        assert!(Preferences::load(&path).is_err(), "{text}");
    }
    std::fs::write(&path, "theme = 'dark'\n").expect("partial preferences");
    let preferences = Preferences::load(&path).expect("default missing fields");
    assert_eq!(preferences.theme, Theme::Dark);
    assert_eq!(preferences.icon, Icon::Auto);
    assert_eq!(preferences.binding, Binding::default());
}

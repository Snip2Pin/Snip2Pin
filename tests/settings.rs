use eframe::egui::{Event, Key, Modifiers};
use snipsprig::{icon, settings::Binding};

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
fn vector_icon_renders_with_transparency_at_small_and_large_sizes() {
    for size in [16, 64, 1024] {
        let image = icon::render(size).expect("valid artwork");
        assert_eq!(image.dimensions(), (size, size));
        assert_eq!(image.get_pixel(0, 0).0[3], 0);
        assert!(image.pixels().any(|pixel| pixel.0[3] == 255));
    }
}

#[test]
fn shortcut_preference_survives_restart_and_invalid_files_are_reported() {
    let directory = std::env::temp_dir().join(format!(
        "snipsprig-settings-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    let path = directory.join("preferences/shortcut.txt");
    assert_eq!(
        Binding::load(&path).expect("missing uses default"),
        Binding::default()
    );
    let first = Binding::parse("Ctrl+Alt+K").expect("valid shortcut");
    first.save(&path).expect("save");
    assert_eq!(Binding::load(&path).expect("load after restart"), first);
    let second = Binding::parse("Ctrl+Shift+4").expect("replacement");
    second.save(&path).expect("replace");
    assert_eq!(Binding::load(&path).expect("load replacement"), second);
    std::fs::write(&path, "invalid").expect("fixture");
    assert!(Binding::load(&path).is_err());
    std::fs::remove_dir_all(directory).expect("clean test preferences");
}

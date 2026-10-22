//! Decode a shortcut recording once, before a host registers or saves it.
use eframe::egui::{Event, Key};

use crate::{i18n::Message, settings::Binding};

#[derive(Debug, PartialEq)]
pub enum Recording {
    Cancel,
    Binding(Binding),
    Invalid(Message),
}

pub fn record(events: &[Event]) -> Option<Recording> {
    events.iter().find_map(|event| {
        let Event::Key {
            key,
            physical_key,
            modifiers,
            pressed: true,
            repeat: false,
            ..
        } = event
        else {
            return None;
        };
        Some(if *key == Key::Escape {
            Recording::Cancel
        } else {
            match Binding::from_key(physical_key.unwrap_or(*key), *modifiers)
                .and_then(|binding| binding.validate_capture().map(|()| binding))
            {
                Ok(binding) => Recording::Binding(binding),
                Err(error) => Recording::Invalid(error),
            }
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::Modifiers;

    fn press(key: Key, physical_key: Option<Key>, modifiers: Modifiers) -> Event {
        Event::Key {
            key,
            physical_key,
            modifiers,
            pressed: true,
            repeat: false,
        }
    }

    #[test]
    fn recording_and_matching_use_the_same_physical_key() {
        let modifiers = Modifiers::CTRL | Modifiers::SHIFT;
        let event = press(Key::A, Some(Key::Q), modifiers);
        let expected = Binding::from_key(Key::Q, modifiers).expect("valid binding");
        assert_eq!(
            record(std::slice::from_ref(&event)),
            Some(Recording::Binding(expected))
        );
        assert!(expected.capture_requested(&[event]));
        assert!(!expected.capture_requested(&[press(Key::A, None, modifiers)]));
        assert_eq!(
            record(&[press(Key::Q, None, modifiers)]),
            Some(Recording::Binding(expected)),
            "backends without physical keys fall back to the logical key"
        );
    }

    #[test]
    fn recording_cancels_or_rejects_reserved_and_unmodified_keys() {
        assert_eq!(
            record(&[press(Key::Escape, None, Modifiers::CTRL)]),
            Some(Recording::Cancel)
        );
        assert_eq!(
            record(&[press(Key::A, None, Modifiers::NONE)]),
            Some(Recording::Invalid(
                crate::i18n::Text::ShortcutModifiers.into()
            ))
        );
        let modifiers = if cfg!(target_os = "macos") {
            Modifiers::MAC_CMD | Modifiers::ALT | Modifiers::SHIFT
        } else {
            Modifiers::CTRL | Modifiers::ALT | Modifiers::SHIFT
        };
        let reserved = press(Key::L, None, modifiers);
        assert_eq!(
            record(std::slice::from_ref(&reserved)),
            Some(Recording::Invalid(
                crate::i18n::Text::ReservedUnlockShortcut.into()
            ))
        );
        assert!(!Binding::unlock_all().capture_requested(&[reserved]));
    }

    #[test]
    fn recording_ignores_release_repeat_and_non_key_events() {
        let modifiers = Modifiers::CTRL;
        let mut released = press(Key::A, None, modifiers);
        if let Event::Key { pressed, .. } = &mut released {
            *pressed = false;
        }
        let mut repeated = press(Key::B, None, modifiers);
        if let Event::Key { repeat, .. } = &mut repeated {
            *repeat = true;
        }
        let ignored = [Event::Text("a".into()), released, repeated];
        assert_eq!(record(&ignored), None);
        let events = ignored
            .into_iter()
            .chain([
                press(Key::C, None, modifiers),
                press(Key::D, None, modifiers),
            ])
            .collect::<Vec<_>>();
        assert_eq!(
            record(&events),
            Some(Recording::Binding(
                Binding::from_key(Key::C, modifiers).expect("valid binding")
            ))
        );
    }
}

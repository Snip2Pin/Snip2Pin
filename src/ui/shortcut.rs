//! Decode a shortcut recording once, before a host registers or saves it.
use eframe::egui::{self, Event, Key};

use crate::{i18n::Message, settings::Binding};

#[derive(Default)]
pub struct Recorder {
    active: bool,
    feedback: Option<Message>,
    feedback_is_error: bool,
    consumed_frame: Option<u64>,
}

impl Recorder {
    pub fn active(&self) -> bool {
        self.active
    }

    pub fn start(&mut self) {
        self.active = true;
        self.feedback = None;
    }

    pub fn cancel(&mut self) {
        self.active = false;
        self.feedback = None;
    }

    /// Hiding the recording host ends modal ownership immediately; unlike an
    /// Escape key, there is no recording input frame left for pins to consume.
    pub fn dismiss(&mut self, ctx: &egui::Context) {
        self.cancel();
        self.consumed_frame = None;
        self.sync_input(ctx);
    }

    pub fn toggle(&mut self, ctx: &egui::Context) {
        if self.active {
            self.cancel();
        } else {
            self.start();
        }
        self.consumed_frame = Some(ctx.cumulative_frame_nr_for(egui::ViewportId::ROOT));
        self.sync_input(ctx);
        ctx.request_repaint_of(egui::ViewportId::ROOT);
    }

    pub fn feedback(&self) -> Option<&Message> {
        self.feedback.as_ref()
    }

    pub fn set_feedback(&mut self, message: Message) {
        self.feedback = Some(message);
        self.feedback_is_error = true;
    }

    pub fn saved(&mut self, message: Message) {
        self.feedback = Some(message);
        self.feedback_is_error = false;
    }

    pub fn feedback_is_error(&self) -> bool {
        self.feedback_is_error
    }

    /// A completed/cancelled recording still owns the input on repeated UI passes.
    pub fn blocks_input(&self, ctx: &egui::Context) -> bool {
        self.active
            || self.consumed_frame == Some(ctx.cumulative_frame_nr_for(egui::ViewportId::ROOT))
    }

    /// Deferred native callbacks must observe root UI changes immediately,
    /// including callbacks created before the user started recording.
    /// Keep ownership until the next root frame resynchronizes it: a child
    /// redraw can run after root painting without running the root logic.
    pub fn sync_input(&self, ctx: &egui::Context) {
        let blocked = self.blocks_input(ctx);
        ctx.data_mut(|data| {
            data.insert_temp(egui::Id::unique("shortcut-recording-input"), blocked)
        });
    }

    pub fn input_blocked(ctx: &egui::Context) -> bool {
        ctx.data(|data| data.get_temp::<bool>(egui::Id::unique("shortcut-recording-input")))
            .unwrap_or(false)
    }

    pub fn update(&mut self, ctx: &egui::Context) -> Option<Binding> {
        if !self.active {
            return None;
        }
        self.consumed_frame = Some(ctx.cumulative_frame_nr_for(egui::ViewportId::ROOT));
        let binding = ctx.input(|input| {
            if input.events.iter().any(|event| {
                matches!(
                    event,
                    Event::Key {
                        pressed: true,
                        repeat: false,
                        ..
                    }
                ) || matches!(event, Event::ModifiersChanged(modifiers) if !modifiers.is_none())
            }) {
                self.feedback = None;
            }
            match record(&input.events) {
                Some(Recording::Cancel) => self.cancel(),
                Some(Recording::Binding(binding)) => {
                    self.active = false;
                    return Some(binding);
                }
                Some(Recording::Invalid(error)) => self.set_feedback(error),
                None => {}
            }
            None
        });
        self.sync_input(ctx);
        if !self.active {
            ctx.request_repaint_of(egui::ViewportId::ROOT);
        }
        binding
    }

    pub fn live_label(&self, ctx: &egui::Context) -> String {
        ctx.input(|input| {
            let names = egui::ModifierNames {
                concat: " + ",
                mac_cmd: "Command",
                mac_alt: "Alt",
                ..egui::ModifierNames::NAMES
            };
            let mut modifiers = input.modifiers;
            let windows = !cfg!(target_os = "macos")
                && (modifiers.mac_cmd || (modifiers.command && !modifiers.ctrl));
            if windows {
                modifiers.command = false;
            }
            let mut parts = vec![names.format(&modifiers, cfg!(target_os = "macos"))];
            if windows {
                parts.push("Windows".into());
            }
            let mut keys = input.keys_down.iter().copied().collect::<Vec<_>>();
            keys.sort();
            parts.extend(
                keys.into_iter()
                    .filter(|key| (!modifier(*key) || *key == Key::Fn) && *key != Key::Escape)
                    .map(|key| key.name().to_owned()),
            );
            parts.retain(|part| !part.is_empty());
            parts.join(" + ")
        })
    }
}

fn modifier(key: Key) -> bool {
    matches!(
        key,
        Key::ShiftLeft
            | Key::ShiftRight
            | Key::ControlLeft
            | Key::ControlRight
            | Key::AltLeft
            | Key::AltRight
            | Key::SuperLeft
            | Key::SuperRight
            | Key::Fn
    )
}

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
        if modifier(*key) || physical_key.is_some_and(modifier) {
            return None;
        }
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
    fn modifier_presses_do_not_complete_or_reject_a_recording() {
        for key in [
            Key::ShiftLeft,
            Key::ShiftRight,
            Key::ControlLeft,
            Key::ControlRight,
            Key::AltLeft,
            Key::AltRight,
            Key::SuperLeft,
            Key::SuperRight,
            Key::Fn,
        ] {
            assert_eq!(record(&[press(key, Some(key), Modifiers::CTRL)]), None);
        }
        let modifiers = Modifiers::CTRL | Modifiers::SHIFT;
        let expected = Binding::from_key(Key::Q, modifiers).expect("valid combination");
        assert_eq!(
            record(&[
                press(Key::ControlLeft, Some(Key::ControlLeft), Modifiers::CTRL),
                press(Key::ShiftLeft, Some(Key::ShiftLeft), modifiers),
                press(Key::A, Some(Key::Q), modifiers),
            ]),
            Some(Recording::Binding(expected))
        );
    }

    #[test]
    fn live_modifiers_include_fn_and_validation_feedback_survives_idle_frames() {
        let ctx = egui::Context::default();
        let mut recorder = Recorder::default();
        recorder.start();
        let mut output = ctx.run_ui(
            egui::RawInput {
                events: vec![
                    Event::ModifiersChanged(Modifiers::CTRL),
                    press(Key::Fn, Some(Key::Fn), Modifiers::CTRL),
                ],
                ..Default::default()
            },
            |ui| {
                assert_eq!(recorder.update(ui.ctx()), None);
                assert_eq!(recorder.live_label(ui.ctx()), "Ctrl + Fn");
                assert!(recorder.feedback().is_none());
            },
        );
        output.textures_delta.clear();
        for events in [
            vec![
                Event::ModifiersChanged(Modifiers::NONE),
                press(Key::A, None, Modifiers::NONE),
            ],
            vec![],
            vec![press(Key::A, None, Modifiers::NONE)],
        ] {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    events,
                    ..Default::default()
                },
                |ui| {
                    assert_eq!(recorder.update(ui.ctx()), None);
                    assert!(recorder.active());
                    assert_eq!(
                        recorder.feedback(),
                        Some(&crate::i18n::Text::ShortcutModifiers.into())
                    );
                },
            );
            output.textures_delta.clear();
        }
        let mut output = ctx.run_ui(
            egui::RawInput {
                events: vec![Event::ModifiersChanged(Modifiers::CTRL)],
                ..Default::default()
            },
            |ui| {
                assert_eq!(recorder.update(ui.ctx()), None);
                assert!(
                    recorder.feedback().is_none(),
                    "a new attempt clears the error"
                );
            },
        );
        output.textures_delta.clear();
    }

    #[test]
    fn recording_meta_shortcuts_preserves_aliases_and_exact_modifiers() {
        let modifiers = Modifiers::MAC_CMD | Modifiers::SHIFT;
        let expected = Binding::parse("super+shift+KeyQ").expect("old saved binding");
        let event = press(Key::A, Some(Key::Q), modifiers);
        assert_eq!(
            record(std::slice::from_ref(&event)),
            Some(Recording::Binding(expected))
        );
        assert!(expected.capture_requested(&[event]));
        assert!(!expected.capture_requested(&[press(
            Key::Q,
            None,
            Modifiers::CTRL | Modifiers::SHIFT
        )]));
        assert!(!expected.capture_requested(&[press(Key::Q, None, modifiers | Modifiers::ALT)]));
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

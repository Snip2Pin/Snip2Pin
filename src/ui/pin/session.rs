//! Own one continuous input episode, without tracking window geometry.
use eframe::egui::{self, Pos2, TouchPhase};
use snip2pin::pin::ZoomAnchor;

// egui's wheel fallback also ends phase-less input after a 150 ms quiet gap.
// Native Start/End signals take precedence. No timer or idle polling is needed.
const PHASELESS_IDLE: f64 = 0.150;

#[derive(Clone, Copy)]
pub(super) struct Session {
    phased: bool,
    time: f64,
    anchor: Option<ZoomAnchor>,
}

#[derive(Clone, Copy)]
pub(super) struct Target {
    pub hovered: Option<Pos2>,
    pub bounds: egui::Rect,
    pub screen_origin: Option<Pos2>,
    pub screen_scale: f32,
}

#[derive(Clone, Copy)]
pub(super) struct Owner {
    pub pin: u64,
    pub viewport: egui::ViewportId,
    pub handled: (egui::ViewportId, f64),
}

pub(super) fn owner_key() -> egui::Id {
    egui::Id::new("pin-gesture-owner")
}

pub(super) fn key(id: u64) -> egui::Id {
    egui::Id::new(("pin-input-session", id))
}

impl Session {
    pub fn owns_target(&self) -> bool {
        self.anchor.is_some()
    }
    pub fn target(
        slot: &mut Option<Self>,
        phase: Option<TouchPhase>,
        time: f64,
        target: Target,
    ) -> Option<ZoomAnchor> {
        if phase == Some(TouchPhase::Cancel) {
            *slot = None;
            return None;
        }
        if phase == Some(TouchPhase::Start)
            || slot.is_some_and(|session| !session.phased && time - session.time > PHASELESS_IDLE)
        {
            *slot = None;
        }
        if slot.is_none() {
            if phase == Some(TouchPhase::End) {
                return None;
            }
            let anchor = target.hovered.and_then(|point| {
                let offset = point - target.bounds.min;
                let fraction = offset / target.bounds.size();
                fraction.is_finite().then_some(ZoomAnchor {
                    fraction: fraction.into(),
                    screen: target
                        .screen_origin
                        .map(|origin| ((origin + offset) * target.screen_scale).into()),
                })
            });
            *slot = Some(Self {
                phased: phase == Some(TouchPhase::Start),
                time,
                anchor,
            });
        }
        let session = slot.as_mut()?;
        session.time = time;
        let anchor = session.anchor;
        if phase == Some(TouchPhase::End) {
            *slot = None;
        }
        anchor
    }
}

//! Visual choices live here; positions are derived from bounds in each view.
use eframe::egui::{Color32, Shadow};

pub struct LauncherStyle {
    pub window: [f32; 2],
    pub margin: i8,
    pub heading: f32,
    pub section_gap: f32,
    pub paragraph_gap: f32,
    pub action_gap: f32,
    pub capture_button: [f32; 2],
    pub icon_preview: f32,
}

pub const LAUNCHER: LauncherStyle = LauncherStyle {
    window: [400.0, 380.0],
    margin: 22,
    heading: 24.0,
    section_gap: 12.0,
    paragraph_gap: 10.0,
    action_gap: 8.0,
    capture_button: [200.0, 40.0],
    icon_preview: 28.0,
};

pub struct OverlayStyle {
    pub stroke: f32,
    pub active: Color32,
    pub dim: Color32,
    pub hint_top: f32,
    pub hint_size: [f32; 2],
    pub hint_radius: u8,
    pub hint_fill: Color32,
    pub hint_font: f32,
}

pub const OVERLAY: OverlayStyle = OverlayStyle {
    stroke: 2.0,
    active: Color32::from_rgb(56, 132, 255),
    dim: Color32::from_black_alpha(28),
    hint_top: 20.0,
    hint_size: [340.0, 36.0],
    hint_radius: 8,
    hint_fill: Color32::from_black_alpha(180),
    hint_font: 14.0,
};

pub struct PinStyle {
    pub shadow: Shadow,
    pub shadow_radius: u8,
    pub border_alpha: u8,
    pub button_fill: Color32,
}

pub const PIN: PinStyle = PinStyle {
    shadow: Shadow {
        offset: [0, 2],
        blur: 12,
        spread: 1,
        color: Color32::from_black_alpha(90),
    },
    shadow_radius: 2,
    border_alpha: 190,
    button_fill: Color32::from_black_alpha(160),
};

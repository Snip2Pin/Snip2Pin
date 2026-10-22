//! Visual choices live here; positions are derived from bounds in each view.
use egui::Color32;

pub struct LauncherStyle {
    pub window: [f32; 2],
    pub margin: i8,
    pub compact_margin: i8,
    pub brand: f32,
    pub brand_icon: f32,
    pub heading: f32,
    pub section_gap: f32,
    pub action_gap: f32,
    pub item_gap: f32,
    pub capture_height: f32,
    pub capture_fill: Color32,
    pub capture_text: Color32,
    pub button_radius: u8,
    pub control_width: f32,
    pub row_height: f32,
    pub footer_margin: i8,
    pub pin_thumbnail: [f32; 2],
    pub pin_list_visible_rows: f32,
    pub pin_row_min_height: f32,
    pub pin_row_padding: [f32; 2],
    pub pin_column_gap: f32,
    pub pin_thumbnail_padding: f32,
    pub pin_metadata_gap: f32,
}

pub const LAUNCHER: LauncherStyle = LauncherStyle {
    window: [390.0, 640.0],
    margin: 24,
    compact_margin: 12,
    brand: 16.0,
    brand_icon: 24.0,
    heading: 20.0,
    section_gap: 16.0,
    action_gap: 8.0,
    item_gap: 4.0,
    capture_height: 46.0,
    capture_fill: Color32::from_rgb(36, 103, 223),
    capture_text: Color32::WHITE,
    button_radius: 8,
    control_width: 140.0,
    row_height: 40.0,
    footer_margin: 12,
    pin_thumbnail: [48.0, 40.0],
    pin_list_visible_rows: 3.5,
    pin_row_min_height: 64.0,
    pin_row_padding: [12.0, 12.0],
    pin_column_gap: 8.0,
    pin_thumbnail_padding: 4.0,
    pin_metadata_gap: 2.0,
};

pub struct OverlayStyle {
    pub stroke: f32,
    pub highlight: Color32,
    pub unselected_fill: Color32,
    pub hint_top: f32,
    pub hint_min_height: f32,
    pub hint_padding: [f32; 2],
    pub hint_radius: u8,
    pub hint_fill: Color32,
    pub hint_font: f32,
}

pub const OVERLAY: OverlayStyle = OverlayStyle {
    stroke: 2.0,
    highlight: Color32::from_rgb(56, 132, 255),
    unselected_fill: Color32::from_black_alpha(28),
    hint_top: 20.0,
    hint_min_height: 36.0,
    hint_padding: [12.0, 8.0],
    hint_radius: 8,
    hint_fill: Color32::from_black_alpha(180),
    hint_font: 14.0,
};

pub struct PinStyle {
    pub button_fill: Color32,
    pub highlight: Color32,
    pub highlight_stroke_pixels: f32,
    pub focused_resize_inset: f32,
    pub menu_width: f32,
}

pub const PIN: PinStyle = PinStyle {
    button_fill: Color32::from_black_alpha(160),
    highlight: Color32::from_rgb(56, 132, 255),
    highlight_stroke_pixels: 1.0,
    focused_resize_inset: 24.0,
    menu_width: 200.0,
};

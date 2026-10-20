//! Noninteractive screen notices, shared by capture and locked-pin recovery.
use std::time::Duration;

use super::style::OVERLAY;
use eframe::egui::{self, Color32, Pos2, Rect};

pub const UNLOCK_DURATION: Duration = Duration::from_secs(1);

pub fn unlock_text(ctx: &egui::Context) -> String {
    crate::i18n::Text::UnlockHint
        .with(crate::settings::Binding::unlock_all().label())
        .render(crate::i18n::locale(ctx))
}

pub fn show(ui: &egui::Ui, bounds: Rect, text: &str) {
    let painter = ui
        .ctx()
        .layer_painter(egui::LayerId::new(
            egui::Order::Tooltip,
            ui.id().with("screen-hint"),
        ))
        .with_clip_rect(bounds);
    let padding = egui::Vec2::from(OVERLAY.hint_padding);
    let galley = painter.layout(
        text.into(),
        egui::FontId::proportional(OVERLAY.hint_font),
        Color32::WHITE,
        (bounds.width() - padding.x * 2.0).max(1.0),
    );
    let size = egui::vec2(
        (galley.size().x + padding.x * 2.0).min(bounds.width()),
        (galley.size().y + padding.y * 2.0).max(OVERLAY.hint_min_height),
    );
    let rect = Rect::from_min_size(
        Pos2::new(
            bounds.center().x - size.x / 2.0,
            bounds.min.y + OVERLAY.hint_top,
        ),
        size,
    );
    painter.rect_filled(rect, OVERLAY.hint_radius, OVERLAY.hint_fill);
    painter.galley(rect.center() - galley.size() / 2.0, galley, Color32::WHITE);
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_long_notice_uses_available_width_before_wrapping() {
        use eframe::egui::{self, Pos2, Rect};
        let ctx = egui::Context::default();
        let bounds = Rect::from_min_size(Pos2::ZERO, egui::vec2(1440.0, 900.0));
        let text = "A screen notice should stay on one line when the available screen width can fit the complete message.";
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(bounds),
                ..Default::default()
            },
            |ui| super::show(ui, bounds, text),
        );
        output.textures_delta.clear();
        let galley = output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(text_shape) if text_shape.galley.text() == text => {
                    Some(&text_shape.galley)
                }
                _ => None,
            })
            .expect("complete notice");
        assert_eq!(galley.rows.len(), 1);
    }

    #[test]
    fn recovery_text_wraps_inside_the_screen_work_area_in_both_languages() {
        use eframe::egui::{self, Pos2, Rect};
        for locale in [crate::i18n::Locale::Chinese, crate::i18n::Locale::English] {
            for width in [180.0, 340.0, 1440.0] {
                let ctx = egui::Context::default();
                crate::i18n::set_locale(&ctx, locale);
                let bounds = Rect::from_min_size(Pos2::new(0.0, 28.0), egui::vec2(width, 700.0));
                let text = super::unlock_text(&ctx);
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(Rect::from_min_size(
                            Pos2::ZERO,
                            egui::vec2(width, 800.0),
                        )),
                        ..Default::default()
                    },
                    |ui| super::show(ui, bounds, &text),
                );
                output.textures_delta.clear();
                let shape = output.shapes.iter().find(|shape| matches!(&shape.shape, egui::Shape::Text(text_shape) if text_shape.galley.text() == text)).expect("complete recovery text");
                assert!(bounds.contains_rect(shape.shape.visual_bounding_rect()));
                assert!(
                    shape
                        .clip_rect
                        .contains_rect(shape.shape.visual_bounding_rect())
                );
                let egui::Shape::Text(text_shape) = &shape.shape else {
                    unreachable!("matched text shape");
                };
                if width == 1440.0 {
                    assert_eq!(text_shape.galley.rows.len(), 1, "wide screen: {locale:?}");
                } else if width == 180.0 {
                    assert!(
                        text_shape.galley.rows.len() > 1,
                        "narrow screen: {locale:?}"
                    );
                }
                let panel = output
                    .shapes
                    .iter()
                    .find_map(|shape| match &shape.shape {
                        egui::Shape::Rect(rect) if rect.fill == super::OVERLAY.hint_fill => {
                            Some(rect.rect)
                        }
                        _ => None,
                    })
                    .expect("shared capture-style hint panel");
                assert!((panel.center().x - bounds.center().x).abs() < 1.0);
                assert!(bounds.contains_rect(panel));
            }
        }
    }
}

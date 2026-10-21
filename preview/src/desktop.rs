//! Fictional local documents rendered with ordinary egui widgets. Captures
//! freeze the rendered canvas, including the source window's current position.
use eframe::egui::{self, Color32, Rect, TextureHandle, Vec2};
use snip2pin::geometry::PixelRect;

const BAR_HEIGHT: f32 = 34.0;

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum Kind {
    #[default]
    Data,
    Code,
    Tasks,
}

impl Kind {
    fn label(self) -> &'static str {
        match self {
            Self::Data => "Data comparison",
            Self::Code => "Code reading",
            Self::Tasks => "Tasks / chat",
        }
    }

    fn title(self) -> &'static str {
        match self {
            Self::Data => "orders.csv",
            Self::Code => "checkout.rs",
            Self::Tasks => "Project chat",
        }
    }

    fn tabs(self) -> [&'static str; 2] {
        match self {
            Self::Data => ["Previous week", "Current week"],
            Self::Code => ["Definition", "Caller"],
            Self::Tasks => ["Message", "Checklist"],
        }
    }
}

#[derive(Default)]
pub struct Scene {
    kind: Kind,
    alternate: bool,
    completed: [bool; 3],
}

fn sample_rect(bounds: Rect) -> Rect {
    let size = egui::vec2(
        (bounds.width() - 48.0).clamp(240.0, 510.0),
        (bounds.height() - 132.0)
            .clamp(180.0, 320.0)
            .min((bounds.height() - BAR_HEIGHT - 24.0).max(1.0)),
    );
    let right = bounds.right() - (bounds.width() * 0.065).clamp(24.0, 100.0);
    let top = bounds.top()
        + (bounds.height() * 0.19)
            .clamp(62.0, 174.0)
            .min((bounds.height() - size.y - 18.0).max(BAR_HEIGHT + 8.0));
    Rect::from_min_size(egui::pos2(right - size.x, top), size)
}

fn source_window(ui: &egui::Ui, bounds: Rect, scene: &mut Scene) -> Option<(egui::LayerId, Rect)> {
    let initial = sample_rect(bounds);
    egui::Window::new(scene.kind.title())
        .id(egui::Id::new("preview-source"))
        .default_pos(initial.min)
        .default_size(initial.size())
        .min_width(240.0)
        .max_size((bounds.size() - egui::vec2(48.0, 100.0)).max(Vec2::splat(1.0)))
        .constrain_to(bounds.shrink2(egui::vec2(12.0, 42.0)))
        .collapsible(false)
        .vscroll(true)
        .show(ui.ctx(), |ui| {
            ui.horizontal(|ui| {
                for (index, label) in scene.kind.tabs().into_iter().enumerate() {
                    ui.selectable_value(&mut scene.alternate, index == 1, label);
                }
            });
            ui.separator();
            ui.add_space(6.0);
            match scene.kind {
                Kind::Data => data_sample(ui, scene.alternate),
                Kind::Code => code_sample(ui, scene.alternate),
                Kind::Tasks => tasks_sample(ui, scene.alternate, &mut scene.completed),
            }
        })
        .map(|window| (window.response.layer_id, window.response.rect))
}

fn data_sample(ui: &mut egui::Ui, current: bool) {
    ui.label(if current { "Week 17" } else { "Week 16" });
    ui.add_space(8.0);
    let rows = if current {
        [
            ("North", "132", "4"),
            ("Central", "102", "4"),
            ("South", "66", "2"),
            ("Total", "300", "10"),
        ]
    } else {
        [
            ("North", "120", "6"),
            ("Central", "90", "3"),
            ("South", "60", "3"),
            ("Total", "270", "12"),
        ]
    };
    egui::Grid::new("orders-grid")
        .num_columns(3)
        .striped(true)
        .min_col_width(80.0)
        .spacing(egui::vec2(16.0, 10.0))
        .show(ui, |ui| {
            for heading in ["Region", "Orders", "Returns"] {
                ui.strong(heading);
            }
            ui.end_row();
            for (region, orders, returns) in rows {
                for value in [region, orders, returns] {
                    if region == "Total" {
                        ui.strong(value);
                    } else {
                        ui.label(value);
                    }
                }
                ui.end_row();
            }
        });
}

fn code_sample(ui: &mut egui::Ui, caller: bool) {
    let mut code = if caller {
        "let cart = [Item { cents: 1_250, qty: 4 }];\nlet subtotal = total_cents(&cart);\nlet delivery = if subtotal >= 5_000 {\n    0\n} else { 500 };\nlet payable = subtotal + delivery;\n\nassert_eq!(payable, 5_000);"
    } else {
        "struct Item { cents: u64, qty: u64 }\n\nfn total_cents(items: &[Item]) -> u64 {\n    items.iter()\n        .map(|i| i.cents * i.qty)\n        .sum()\n}"
    };
    ui.add(
        egui::TextEdit::multiline(&mut code)
            .code_editor()
            .desired_width(ui.available_width())
            .desired_rows(10),
    );
}

fn tasks_sample(ui: &mut egui::Ui, checklist: bool, completed: &mut [bool; 3]) {
    ui.strong("Export update");
    ui.weak("Due Friday");
    ui.add_space(10.0);
    let requirements = [
        "Keep the CSV column order unchanged.",
        "Match the file and preview timezone.",
        "Add one example to the release notes.",
    ];
    if checklist {
        for (done, requirement) in completed.iter_mut().zip(requirements) {
            ui.checkbox(done, requirement);
            ui.add_space(8.0);
        }
    } else {
        egui::Frame::group(ui.style())
            .inner_margin(10.0)
            .show(ui, |ui| {
                ui.label("Please ship the small export update by Friday.");
                ui.add_space(8.0);
                for requirement in requirements {
                    ui.label(requirement);
                }
            });
    }
}

/// Preview controls and documents have no platform hooks or saved preferences.
pub fn show(
    ui: &mut egui::Ui,
    wallpaper: &TextureHandle,
    scene: &mut Scene,
) -> Vec<(egui::LayerId, Rect)> {
    let bounds = ui.max_rect();
    let image = wallpaper.size_vec2();
    let scale = (bounds.width() / image.x).max(bounds.height() / image.y);
    let uv = Rect::from_center_size(egui::pos2(0.5, 0.5), bounds.size() / (image * scale));
    ui.painter()
        .image(wallpaper.id(), bounds, uv, Color32::WHITE);

    let bar = Rect::from_min_size(bounds.min, egui::vec2(bounds.width(), BAR_HEIGHT));
    ui.painter().rect_filled(bar, 0, ui.visuals().panel_fill);
    ui.scope_builder(
        egui::UiBuilder::new().max_rect(bar.shrink2(egui::vec2(12.0, 4.0))),
        |ui| {
            ui.horizontal(|ui| {
                ui.strong("Snip2Pin preview");
                egui::ComboBox::from_id_salt("demo-scene")
                    .selected_text(scene.kind.label())
                    .width(160.0)
                    .show_ui(ui, |ui| {
                        for kind in [Kind::Data, Kind::Code, Kind::Tasks] {
                            if ui
                                .selectable_value(&mut scene.kind, kind, kind.label())
                                .changed()
                            {
                                scene.alternate = false;
                            }
                        }
                    });
            });
        },
    );
    source_window(ui, bounds, scene).into_iter().collect()
}

/// Convert visible window bounds to the exact frozen screenshot pixel grid.
pub fn pixel_regions(bounds: Rect, pixels: [usize; 2], windows: &[Rect]) -> Vec<PixelRect> {
    let scale = egui::vec2(pixels[0] as f32, pixels[1] as f32) / bounds.size();
    windows
        .iter()
        .rev()
        .filter_map(|window| {
            let rect = window.intersect(bounds);
            if !rect.is_positive() {
                return None;
            }
            let min = ((rect.min - bounds.min) * scale).round();
            let max = ((rect.max - bounds.min) * scale).round();
            (max.x > min.x && max.y > min.y).then_some(PixelRect {
                x: min.x as u32,
                y: min.y as u32,
                width: (max.x - min.x) as u32,
                height: (max.y - min.y) as u32,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop_scene_has_no_platform_actions_and_fits_common_viewports() {
        for size in [
            egui::vec2(1440.0, 900.0),
            egui::vec2(390.0, 844.0),
            egui::vec2(390.0, 320.0),
            egui::vec2(800.0, 480.0),
        ] {
            let ctx = egui::Context::default();
            let texture = ctx.load_texture(
                "wallpaper",
                egui::ColorImage::filled([1, 1], Color32::LIGHT_BLUE),
                Default::default(),
            );
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, size)),
                    ..Default::default()
                },
                |ui| {
                    let windows = show(ui, &texture, &mut Scene::default());
                    assert_eq!(windows.len(), 1);
                    assert!(ui.max_rect().contains_rect(windows[0].1));
                },
            );
            output.textures_delta.clear();
            assert!(
                output.viewport_output[&egui::ViewportId::ROOT]
                    .commands
                    .iter()
                    .all(|command| matches!(command, egui::ViewportCommand::SetTheme(_))),
                "unexpected desktop command: {:?}",
                output.viewport_output[&egui::ViewportId::ROOT].commands
            );
        }
    }

    #[test]
    fn capture_regions_use_screenshot_pixels_and_front_to_back_order() {
        let bounds = Rect::from_min_size(egui::pos2(10.0, 20.0), egui::vec2(800.0, 600.0));
        let windows = [
            Rect::from_min_size(egui::pos2(30.0, 50.0), egui::vec2(200.0, 150.0)),
            Rect::from_min_size(egui::pos2(710.0, 570.0), egui::vec2(300.0, 100.0)),
            Rect::from_min_size(egui::pos2(-100.0, -100.0), Vec2::splat(10.0)),
        ];
        assert_eq!(
            pixel_regions(bounds, [1600, 1200], &windows),
            vec![
                PixelRect {
                    x: 1400,
                    y: 1100,
                    width: 200,
                    height: 100
                },
                PixelRect {
                    x: 40,
                    y: 60,
                    width: 400,
                    height: 300
                },
            ]
        );
    }
}

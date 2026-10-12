use eframe::egui::{self, Color32, Stroke};
pub const BACKGROUND: Color32 = Color32::from_rgb(11, 18, 32);
pub const SIDEBAR: Color32 = Color32::from_rgb(16, 27, 45);
pub const PANEL: Color32 = Color32::from_rgb(23, 38, 61);
pub const HOVER: Color32 = Color32::from_rgb(34, 55, 85);
pub const BORDER: Color32 = Color32::from_rgb(42, 59, 85);
pub const TEXT: Color32 = Color32::from_rgb(243, 247, 253);
pub const MUTED: Color32 = Color32::from_rgb(163, 179, 201);
pub const BLUE: Color32 = Color32::from_rgb(88, 166, 255);
pub const PURPLE: Color32 = Color32::from_rgb(184, 153, 255);
// Network page accents per the redesign spec: download = blue, upload = green.
pub const DOWNLOAD_BLUE: Color32 = Color32::from_rgb(74, 158, 255);
pub const UPLOAD_GREEN: Color32 = Color32::from_rgb(50, 214, 161);
pub const CYAN: Color32 = Color32::from_rgb(67, 213, 234);
pub const WARNING: Color32 = Color32::from_rgb(245, 158, 11);
pub const GREEN: Color32 = Color32::from_rgb(34, 197, 94);
pub const GREEN_TEXT: Color32 = Color32::from_rgb(5, 46, 22);
pub const DANGER: Color32 = Color32::from_rgb(248, 113, 113);

pub fn install(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    if let Ok(bytes) = std::fs::read("C:/Windows/Fonts/segoeui.ttf") {
        fonts
            .font_data
            .insert("Segoe UI".into(), egui::FontData::from_owned(bytes).into());
        fonts
            .families
            .entry(egui::FontFamily::Proportional)
            .or_default()
            .insert(0, "Segoe UI".into());
    }
    ctx.set_fonts(fonts);
    let mut v = egui::Visuals::dark();
    v.panel_fill = BACKGROUND;
    v.window_fill = PANEL;
    v.extreme_bg_color = SIDEBAR;
    v.override_text_color = Some(TEXT);
    v.selection.bg_fill = HOVER;
    v.selection.stroke = Stroke::new(1.0, BLUE);
    v.widgets.noninteractive.bg_fill = PANEL;
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, BORDER);
    v.widgets.inactive.bg_fill = PANEL;
    v.widgets.inactive.weak_bg_fill = PANEL;
    v.widgets.inactive.bg_stroke = Stroke::new(1.0, BORDER);
    v.widgets.hovered.bg_fill = HOVER;
    v.widgets.hovered.weak_bg_fill = HOVER;
    v.widgets.active.bg_fill = HOVER;
    for w in [
        &mut v.widgets.inactive,
        &mut v.widgets.hovered,
        &mut v.widgets.active,
        &mut v.widgets.noninteractive,
    ] {
        w.corner_radius = 10.into();
    }
    ctx.set_visuals(v);
    ctx.style_mut(|s| {
        s.spacing.item_spacing = egui::vec2(16.0, 12.0);
        s.spacing.button_padding = egui::vec2(14.0, 9.0);
        s.spacing.interact_size.y = 36.0;
        s.wrap_mode = Some(egui::TextWrapMode::Wrap);
        s.text_styles
            .insert(egui::TextStyle::Body, egui::FontId::proportional(14.0));
        s.text_styles
            .insert(egui::TextStyle::Button, egui::FontId::proportional(14.0));
        s.text_styles
            .insert(egui::TextStyle::Heading, egui::FontId::proportional(26.0));
        s.text_styles
            .insert(egui::TextStyle::Small, egui::FontId::proportional(12.0));
    });
}
pub fn card() -> egui::Frame {
    egui::Frame::new()
        .fill(PANEL)
        .stroke(Stroke::new(1.0, BORDER))
        .corner_radius(12)
        .inner_margin(20)
}
pub fn apply_scale(ctx: &egui::Context, scale: f32) {
    ctx.set_zoom_factor(scale);
    ctx.send_viewport_cmd(egui::ViewportCommand::MinInnerSize(egui::vec2(
        620.0 * scale,
        400.0 * scale,
    )));
}

/// One width contract for every surface. The frame's margins are already deducted.
pub fn surface<R>(ui: &mut egui::Ui, content: impl FnOnce(&mut egui::Ui) -> R) -> R {
    card()
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            content(ui)
        })
        .inner
}
pub fn section(ui: &mut egui::Ui, title: &str) {
    ui.label(egui::RichText::new(title).size(19.0).strong());
}
pub fn toggle(
    ui: &mut egui::Ui,
    value: &mut bool,
    label: impl Into<egui::WidgetText>,
) -> egui::Response {
    ui.scope(|ui| {
        ui.visuals_mut().widgets.inactive.corner_radius = 3.into();
        ui.visuals_mut().widgets.hovered.corner_radius = 3.into();
        ui.visuals_mut().widgets.active.corner_radius = 3.into();
        ui.checkbox(value, label)
    })
    .inner
}
pub fn row(ui: &mut egui::Ui, label: &str, value: &str) {
    let width = ui.available_width();
    if width < 420.0 {
        ui.vertical(|ui| {
            ui.label(egui::RichText::new(label).color(MUTED));
            ui.add(egui::Label::new(value).wrap().selectable(true));
        });
    } else {
        let label_width = (width * 0.29).clamp(140.0, 220.0);
        ui.horizontal_top(|ui| {
            ui.allocate_ui_with_layout(
                egui::vec2(label_width, 0.0),
                egui::Layout::top_down(egui::Align::LEFT),
                |ui| {
                    ui.set_min_width(label_width);
                    ui.add(egui::Label::new(egui::RichText::new(label).color(MUTED)).wrap());
                },
            );
            ui.allocate_ui_with_layout(
                egui::vec2((width - label_width - 16.0).max(80.0), 0.0),
                egui::Layout::top_down(egui::Align::LEFT),
                |ui| {
                    ui.add(egui::Label::new(value).wrap().selectable(true));
                },
            );
        });
    }
}
pub fn columns(width: f32, count: usize) -> usize {
    if count == 4 {
        if width >= 1000.0 {
            4
        } else if width >= 480.0 {
            2
        } else {
            1
        }
    } else if width >= 580.0 {
        2
    } else {
        1
    }
}
#[allow(
    clippy::too_many_arguments,
    reason = "Explicit display fields for the shared metric card"
)]
pub fn metric(
    ui: &mut egui::Ui,
    title: &str,
    value: Option<f64>,
    unit: &str,
    description: &str,
    state: &str,
    color: Color32,
    tooltip: &str,
) {
    surface(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 8.0;
        ui.set_min_height(128.0);
        ui.horizontal(|ui| {
            icon(
                ui,
                if title.to_lowercase().contains("baterai") {
                    rigpulse_core::Page::Battery
                } else {
                    rigpulse_core::Page::Hardware
                },
                color,
            );
            ui.label(egui::RichText::new(title).strong().color(color))
                .on_hover_text(tooltip);
        });
        ui.label(
            egui::RichText::new(
                value
                    .map(|v| format!("{v:.1}{unit}"))
                    .unwrap_or_else(|| "Tidak tersedia".into()),
            )
            .size(if value.is_some() { 28.0 } else { 19.0 })
            .strong(),
        );
        if unit == "%" {
            ui.add(
                egui::ProgressBar::new(value.unwrap_or(0.0).clamp(0.0, 100.0) as f32 / 100.0)
                    .fill(color)
                    .desired_height(5.0),
            );
        }
        ui.label(egui::RichText::new(description).small().color(MUTED));
        ui.label(egui::RichText::new(state).small().color(MUTED));
    });
}

pub fn icon(ui: &mut egui::Ui, page: rigpulse_core::Page, color: Color32) {
    let (r, _) = ui.allocate_exact_size(egui::vec2(20.0, 20.0), egui::Sense::hover());
    paint_icon(ui.painter(), r, page, color);
}
pub fn paint_icon(p: &egui::Painter, r: egui::Rect, page: rigpulse_core::Page, color: Color32) {
    use rigpulse_core::Page;
    let s = Stroke::new(1.5, color);
    let c = r.center();
    match page {
        Page::Dashboard | Page::Apps => {
            for x in [0.0, 11.0] {
                for y in [0.0, 11.0] {
                    p.rect_stroke(
                        egui::Rect::from_min_size(r.min + egui::vec2(x, y), egui::vec2(7.0, 7.0)),
                        2,
                        s,
                        egui::StrokeKind::Inside,
                    );
                }
            }
        }
        Page::Hardware | Page::Drivers => {
            p.rect_stroke(r.shrink(3.0), 3, s, egui::StrokeKind::Inside);
            for d in [-4.0, 4.0] {
                p.line_segment([c + egui::vec2(d, -10.0), c + egui::vec2(d, -6.0)], s);
                p.line_segment([c + egui::vec2(d, 6.0), c + egui::vec2(d, 10.0)], s);
            }
        }
        Page::Monitoring => {
            p.add(egui::Shape::line(
                vec![
                    r.left_center(),
                    c + egui::vec2(-4.0, 0.0),
                    c + egui::vec2(-1.0, -8.0),
                    c + egui::vec2(3.0, 7.0),
                    c + egui::vec2(6.0, 0.0),
                    r.right_center(),
                ],
                s,
            ));
        }
        Page::Battery => {
            p.rect_stroke(
                r.shrink2(egui::vec2(1.0, 4.0)),
                3,
                s,
                egui::StrokeKind::Inside,
            );
            p.line_segment([c + egui::vec2(-4.0, 0.0), c + egui::vec2(4.0, 0.0)], s);
        }
        Page::Diagnostics => {
            p.circle_stroke(c, 8.0, s);
            p.line_segment([c + egui::vec2(0.0, -4.0), c + egui::vec2(0.0, 2.0)], s);
            p.circle_filled(c + egui::vec2(0.0, 5.0), 1.0, color);
        }
        Page::Reports => {
            p.rect_stroke(
                r.shrink2(egui::vec2(3.0, 0.0)),
                2,
                s,
                egui::StrokeKind::Inside,
            );
            for y in [-4.0, 0.0, 4.0] {
                p.line_segment([c + egui::vec2(-3.0, y), c + egui::vec2(3.0, y)], s);
            }
        }
        Page::Settings => {
            p.circle_stroke(c, 7.0, s);
            p.circle_stroke(c, 2.5, s);
            for d in [egui::vec2(0.0, 10.0), egui::vec2(10.0, 0.0)] {
                p.line_segment([c - d, c + d], s);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn texts(shape: &egui::Shape, out: &mut Vec<(String, egui::Rect, usize)>) {
        match shape {
            egui::Shape::Text(t) => out.push((
                t.galley.text().into(),
                t.galley.rect.translate(t.pos.to_vec2()),
                t.galley.rows.len(),
            )),
            egui::Shape::Vec(shapes) => {
                for s in shapes {
                    texts(s, out)
                }
            }
            _ => {}
        }
    }
    #[test]
    fn battery_rows_align_and_never_wrap_each_character() {
        for width in [220.0, 350.0, 620.0, 1200.0] {
            let ctx = egui::Context::default();
            install(&ctx);
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(width, 700.0),
                )),
                ..Default::default()
            };
            let output = ctx.run(input, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    row(ui, "Sumber daya", "Terhubung ke adaptor AC");
                    row(ui, "Status pengisian", "Tidak sedang mengisi daya");
                    row(
                        ui,
                        "Kartu grafis",
                        "NVIDIA GeForce RTX 4090 Laptop GPU Professional Edition",
                    );
                });
            });
            let mut rendered = Vec::new();
            for s in output.shapes {
                texts(&s.shape, &mut rendered);
            }
            let power = rendered
                .iter()
                .find(|t| t.0 == "Terhubung ke adaptor AC")
                .unwrap();
            let charging = rendered
                .iter()
                .find(|t| t.0 == "Tidak sedang mengisi daya")
                .unwrap();
            assert!(
                power.2 <= 3 && charging.2 <= 3,
                "Severe wrapping at width {width}"
            );
            if width >= 620.0 {
                assert!((power.1.left() - charging.1.left()).abs() < 1.0);
            }
            for (text, bounds, _) in rendered {
                assert!(
                    bounds.right() <= width + 1.0,
                    "Text overflows at {width}: {text}"
                );
            }
        }
    }
}

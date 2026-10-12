//! Variable-height viewport rendering inside the existing page scroll container.
//! Row geometry is cached by callers when data, wrapping width or fonts change.
use eframe::egui;
#[derive(PartialEq)]
struct LayoutKey {
    width: f32,
    pixels_per_point: f32,
    body: egui::FontId,
    spacing: egui::Vec2,
    padding: egui::Vec2,
    interact: egui::Vec2,
}
#[derive(Default)]
pub struct MeasuredRows {
    key: Option<LayoutKey>,
    rows: VirtualRows,
}
impl MeasuredRows {
    /// Measure with the same widgets on layout changes; scrolling uses cached extents.
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        id: &str,
        count: usize,
        mut draw: impl FnMut(usize, &mut egui::Ui),
    ) {
        let key = LayoutKey {
            width: ui.available_width(),
            pixels_per_point: ui.ctx().pixels_per_point(),
            body: egui::TextStyle::Body.resolve(ui.style()),
            spacing: ui.spacing().item_spacing,
            padding: ui.spacing().button_padding,
            interact: ui.spacing().interact_size,
        };
        if self.key.as_ref() != Some(&key) || self.rows.offsets.len() != count + 1 {
            self.rows = VirtualRows::from_heights((0..count).map(|index| {
                let mut child = ui.new_child(
                    egui::UiBuilder::new()
                        .id_salt((id, "measure", index))
                        .max_rect(egui::Rect::from_min_size(
                            ui.cursor().min,
                            egui::vec2(key.width, f32::INFINITY),
                        ))
                        .layout(egui::Layout::top_down(egui::Align::LEFT)),
                );
                child.set_clip_rect(egui::Rect::NOTHING);
                child.disable();
                draw(index, &mut child);
                child.min_size().y + key.spacing.y
            }));
            self.key = Some(key);
        }
        self.rows.show(ui, id, draw);
    }
}
#[derive(Default)]
pub struct VirtualRows {
    offsets: Vec<f32>,
}
impl VirtualRows {
    pub fn from_heights(heights: impl IntoIterator<Item = f32>) -> Self {
        let mut offsets = vec![0.0];
        for height in heights {
            let height = if height.is_finite() {
                height.clamp(1.0, 1_000_000.0)
            } else {
                1.0
            };
            offsets.push(offsets.last().copied().unwrap_or(0.0) + height);
        }
        Self { offsets }
    }
    pub fn show(&self, ui: &mut egui::Ui, id: &str, mut row: impl FnMut(usize, &mut egui::Ui)) {
        let total = self.offsets.last().copied().unwrap_or(0.0);
        let width = ui.available_width();
        let top = ui.cursor().top();
        let clip = ui.clip_rect();
        let (_, full) = ui.allocate_space(egui::vec2(width, total));
        if top > clip.bottom() || top + total < clip.top() {
            return;
        }
        let count = self.offsets.len().saturating_sub(1);
        let first = self
            .offsets
            .partition_point(|y| *y <= (clip.top() - top) - 0.5)
            .saturating_sub(1)
            .min(count);
        let end = self
            .offsets
            .partition_point(|y| *y < (clip.bottom() - top) + 0.5)
            .saturating_add(1)
            .min(count);
        for index in first..end {
            let rect = egui::Rect::from_min_size(
                egui::pos2(full.left(), top + self.offsets[index]),
                egui::vec2(width, self.offsets[index + 1] - self.offsets[index]),
            );
            let mut child = ui.new_child(
                egui::UiBuilder::new()
                    .id_salt((id, index))
                    .max_rect(rect)
                    .layout(egui::Layout::top_down(egui::Align::LEFT)),
            );
            child.set_clip_rect(rect.intersect(clip));
            row(index, &mut child);
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui;

    #[test]
    fn nonstructural_refresh_keeps_scroll_position_and_updated_text() {
        let ctx = egui::Context::default();
        crate::theme::install(&ctx);
        let mut rows = MeasuredRows::default();
        let mut offset = 0.0;
        let mut drawn = Vec::new();
        for pass in 0..3 {
            if pass == 2 {
                rows = MeasuredRows::default();
            }
            drawn.clear();
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(500.0, 300.0),
                    )),
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        let scroll = egui::ScrollArea::vertical().id_salt("refresh_test");
                        let scroll = if pass < 2 {
                            scroll.vertical_scroll_offset(4000.0)
                        } else {
                            scroll
                        };
                        let output = scroll.show(ui, |ui| {
                            rows.show(ui, "data", 1000, |index, ui| {
                                let text = format!("Entry {index:04} version {pass}");
                                if ui.is_rect_visible(ui.max_rect()) {
                                    drawn.push(text.clone());
                                }
                                ui.label(text);
                            })
                        });
                        offset = output.state.offset.y;
                    });
                },
            );
            if pass > 0 {
                assert!((offset - 4000.0).abs() < 1.0);
            }
        }
        assert!(!drawn.is_empty());
        assert!(drawn.iter().all(|text| text.ends_with("version 2")));
        assert!(drawn.len() < 20, "Refresh laid out the complete inventory");
    }

    #[test]
    fn native_line_fractional_wheel_and_scrollbar_drag_reach_virtual_rows() {
        let ctx = egui::Context::default();
        crate::theme::install(&ctx);
        let rows = VirtualRows::from_heights(std::iter::repeat_n(40.0, 1000));
        let mut clock = 0.0;
        let mut frame = |events: Vec<egui::Event>| {
            clock += 1.0 / 60.0;
            let mut result = (0.0, egui::Rect::NOTHING);
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(500.0, 300.0),
                    )),
                    time: Some(clock),
                    events,
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        ui.spacing_mut().scroll.floating = false;
                        let out = egui::ScrollArea::vertical()
                            .id_salt("input_test")
                            .max_height(240.0)
                            .scroll_bar_visibility(
                                egui::scroll_area::ScrollBarVisibility::AlwaysVisible,
                            )
                            .show(ui, |ui| {
                                rows.show(ui, "rows", |index, ui| {
                                    ui.label(format!("Row {index}"));
                                })
                            });
                        result = (out.state.offset.y, out.inner_rect);
                    });
                },
            );
            result
        };
        frame(vec![]);
        frame(vec![]);
        let wheel = |unit, delta| {
            vec![
                egui::Event::PointerMoved(egui::pos2(200.0, 120.0)),
                egui::Event::MouseWheel {
                    unit,
                    delta: egui::vec2(0.0, delta),
                    modifiers: Default::default(),
                },
            ]
        };
        frame(wheel(egui::MouseWheelUnit::Line, -3.0));
        for _ in 0..12 {
            frame(vec![]);
        }
        let before = frame(vec![]).0;
        assert!(before > 0.0);
        for _ in 0..16 {
            frame(wheel(egui::MouseWheelUnit::Point, -0.5));
        }
        let (after, rect) = frame(vec![]);
        assert!(
            after > before,
            "Fractional high-resolution wheel input was lost"
        );
        let start = egui::pos2(rect.right() + 6.0, rect.top() + 8.0);
        let pointer = |pos, pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        };
        frame(vec![egui::Event::PointerMoved(start), pointer(start, true)]);
        let end = start + egui::vec2(0.0, 140.0);
        frame(vec![egui::Event::PointerMoved(end)]);
        let dragged = frame(vec![pointer(end, false)]).0;
        assert!(
            dragged > after + 1000.0,
            "Scrollbar drag did not reach virtualized content: {after} -> {dragged}"
        );
    }

    #[test]
    fn measured_wrapped_rows_reflow_after_resize_without_clipping() {
        let ctx = egui::Context::default();
        crate::theme::install(&ctx);
        let mut rows = MeasuredRows::default();
        let text = "NVIDIA GeForce RTX Laptop GPU with long hardware identification ".repeat(5);
        let mut wide = 0.0;
        for width in [900.0, 260.0] {
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(width, 1200.0),
                    )),
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        rows.show(ui, "wrapped", 3, |_, ui| {
                            let top = ui.cursor().top();
                            crate::theme::row(ui, "Kartu grafis", &text);
                            assert!(ui.min_rect().bottom() - top <= ui.max_rect().height() + 0.5);
                        });
                    });
                },
            );
            let height = *rows.rows.offsets.last().unwrap();
            if width > 500.0 {
                wide = height;
            } else {
                assert!(height > wide);
            }
        }
    }

    #[test]
    fn long_list_keeps_full_extent_and_only_lays_out_rows_near_scrolled_viewport() {
        let ctx = egui::Context::default();
        let rows = VirtualRows::from_heights(std::iter::repeat_n(40.0, 1000));
        let mut drawn = Vec::new();
        let mut extent = 0.0;
        for _ in 0..2 {
            drawn.clear();
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(500.0, 200.0),
                    )),
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        egui::ScrollArea::vertical()
                            .vertical_scroll_offset(4000.0)
                            .show(ui, |ui| {
                                let top = ui.cursor().top();
                                rows.show(ui, "test_rows", |index, ui| {
                                    drawn.push(index);
                                    ui.label(format!("Row {index}"));
                                });
                                extent = ui.min_rect().bottom() - top;
                            });
                    });
                },
            );
        }
        assert!((extent - 40_000.0).abs() < 1.0);
        assert!(
            drawn.len() <= 9,
            "Offscreen rows were laid out: {}",
            drawn.len()
        );
        assert!(drawn.contains(&100));
        assert!(!drawn.contains(&0));
        assert!(!drawn.contains(&999));
    }
}

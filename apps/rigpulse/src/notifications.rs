use crate::theme;
use eframe::egui;
use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Success,
    Error,
    Warning,
    Information,
}
struct Notice {
    kind: Kind,
    text: String,
    at: Instant,
}
#[derive(Default)]
pub struct Notifications {
    queue: VecDeque<Notice>,
}
impl Notifications {
    pub fn push(&mut self, kind: Kind, text: impl Into<String>) {
        self.push_at(kind, text.into(), Instant::now());
    }
    pub fn push_at(&mut self, kind: Kind, text: String, now: Instant) {
        if self.queue.iter().any(|n| n.kind == kind && n.text == text) {
            return;
        }
        if self.queue.len() == 8 {
            self.queue.pop_front();
        }
        self.queue.push_back(Notice {
            kind,
            text,
            at: now,
        });
    }
    pub fn expire(&mut self, now: Instant) {
        self.queue
            .retain(|n| now.saturating_duration_since(n.at) < Duration::from_secs(5));
    }
    pub fn dismiss(&mut self, index: usize) {
        self.queue.remove(index);
    }
    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.queue.len()
    }
    pub fn show(&mut self, ctx: &egui::Context) {
        self.expire(Instant::now());
        if self.queue.is_empty() {
            return;
        }
        let mut dismissed = None;
        egui::Area::new(egui::Id::new("shared_notifications"))
            .order(egui::Order::Foreground)
            .anchor(egui::Align2::RIGHT_BOTTOM, [-20.0, -20.0])
            .show(ctx, |ui| {
                ui.set_max_width((ctx.screen_rect().width() - 270.0).clamp(220.0, 390.0));
                for (index, notice) in self.queue.iter().enumerate().take(2) {
                    let (symbol, color) = match notice.kind {
                        Kind::Success => ("✓", theme::GREEN_TEXT),
                        Kind::Error => ("×", theme::DANGER),
                        Kind::Warning => ("!", theme::WARNING),
                        Kind::Information => ("i", theme::BLUE),
                    };
                    theme::surface(ui, |ui| {
                        ui.horizontal_wrapped(|ui| {
                            ui.colored_label(color, symbol);
                            ui.add(egui::Label::new(&notice.text).wrap());
                            if ui.small_button("Tutup").clicked() {
                                dismissed = Some(index);
                            }
                        });
                    });
                }
            });
        if let Some(index) = dismissed {
            self.dismiss(index);
        }
        ctx.request_repaint_after(Duration::from_millis(200));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn notices_deduplicate_expire_and_allow_dismissal() {
        let mut notices = Notifications::default();
        let now = std::time::Instant::now();
        notices.push_at(Kind::Success, "App dihapus".into(), now);
        notices.push_at(Kind::Success, "App dihapus".into(), now);
        assert_eq!(notices.len(), 1);
        notices.expire(now + std::time::Duration::from_secs(6));
        assert_eq!(notices.len(), 0);
        notices.push_at(Kind::Error, "Gagal".into(), now);
        notices.dismiss(0);
        assert_eq!(notices.len(), 0);
    }
}

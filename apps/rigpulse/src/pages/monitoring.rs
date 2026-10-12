use crate::{app::RigPulseApp, theme};
use eframe::egui::{self, RichText};

#[derive(Default)]
pub(crate) struct ChartView {
    key: Option<(Option<chrono::DateTime<chrono::Utc>>, i64, bool)>,
    cpu: Vec<egui_plot::PlotPoint>,
    ram: Vec<egui_plot::PlotPoint>,
    stats: [Option<(f64, f64, f64)>; 2],
}
impl ChartView {
    fn refresh(
        &mut self,
        history: &rigpulse_monitor::buffer::RingBuffer,
        minutes: i64,
        paused: bool,
    ) {
        let key = (history.last_at(), minutes, paused);
        if self.key == Some(key) {
            return;
        }
        self.cpu = history
            .points(minutes, true)
            .into_iter()
            .map(egui_plot::PlotPoint::from)
            .collect();
        self.ram = history
            .points(minutes, false)
            .into_iter()
            .map(egui_plot::PlotPoint::from)
            .collect();
        self.stats = [&self.cpu, &self.ram].map(|points| {
            if points.is_empty() {
                return None;
            }
            Some((
                points.iter().map(|p| p.y).fold(f64::INFINITY, f64::min),
                points.iter().map(|p| p.y).sum::<f64>() / points.len() as f64,
                points.iter().map(|p| p.y).fold(f64::NEG_INFINITY, f64::max),
            ))
        });
        self.key = Some(key);
    }
    fn nearest(&self, cpu: bool, x: f64) -> Option<&egui_plot::PlotPoint> {
        let points = if cpu { &self.cpu } else { &self.ram };
        if points.is_empty() {
            return None;
        }
        let right = points.partition_point(|p| p.x < x).min(points.len() - 1);
        let left = right.saturating_sub(1);
        Some(
            if (points[left].x - x).abs() <= (points[right].x - x).abs() {
                &points[left]
            } else {
                &points[right]
            },
        )
    }
}

pub fn metric(ui: &mut egui::Ui, m: &rigpulse_core::Measurement) {
    theme::surface(ui, |ui| {
        ui.add(egui::Label::new(RichText::new(&m.label).strong()).wrap());
        ui.label(RichText::new(m.text()).size(22.0).strong());
        ui.label(
            RichText::new(if m.stale(chrono::Utc::now(), 15) {
                "Data lama"
            } else {
                m.availability.label()
            })
            .small()
            .color(theme::MUTED),
        );
        egui::CollapsingHeader::new("Detail sensor")
            .id_salt((&m.label, &m.source))
            .show(ui, |ui| {
                ui.label(&m.source);
                if let Some(reason) = &m.reason {
                    ui.label(reason);
                }
                ui.label(
                    m.at.with_timezone(&chrono::Local)
                        .format("%H:%M:%S")
                        .to_string(),
                );
            });
    });
}
pub fn time_label(seconds: f64) -> String {
    if seconds >= -0.5 {
        "Sekarang".into()
    } else if seconds > -60.0 {
        format!("{:.0} dtk lalu", -seconds)
    } else {
        format!("{:.0} mnt lalu", -seconds / 60.0)
    }
}
fn chart(
    ui: &mut egui::Ui,
    view: &ChartView,
    minutes: i64,
    cpu: bool,
    combined: bool,
    paused: bool,
) {
    let name = if combined {
        "CPU dan RAM"
    } else if cpu {
        "CPU"
    } else {
        "RAM"
    };
    theme::surface(ui, |ui| {
        theme::section(ui, &format!("Penggunaan {name}"));
        let end = view.key.and_then(|key| key.0);
        if end.is_none() {
            ui.label("Menunggu sampel pertama. Riwayat mulai terkumpul saat aplikasi dibuka.");
        }
        egui_plot::Plot::new(("usage_chart", name, minutes, paused))
            .height(190.0)
            .default_x_bounds(-(minutes * 60) as f64, 0.0)
            .default_y_bounds(0.0, 100.0)
            .include_x(-(minutes * 60) as f64)
            .include_x(0.0)
            .include_y(0.0)
            .include_y(100.0)
            .set_margin_fraction(egui::vec2(0.0, 0.0))
            .allow_drag(false)
            .allow_zoom(false)
            .allow_scroll(false)
            .allow_boxed_zoom(false)
            .show_grid([false, true])
            .show_background(false)
            .y_grid_spacer(|_| {
                [0.0, 50.0, 100.0]
                    .into_iter()
                    .map(|value| egui_plot::GridMark {
                        value,
                        step_size: 50.0,
                    })
                    .collect()
            })
            .x_grid_spacer(move |_| {
                (0..=5)
                    .map(|n| egui_plot::GridMark {
                        value: -(minutes * 60) as f64 + n as f64 * (minutes * 12) as f64,
                        step_size: (minutes * 12) as f64,
                    })
                    .collect()
            })
            .x_axis_formatter(|mark, _| time_label(mark.value))
            .y_axis_formatter(|mark, _| format!("{:.0}%", mark.value))
            .label_formatter(move |name, p| {
                if name != "CPU" && name != "RAM" {
                    return String::new();
                }
                let Some(measured) = view.nearest(name == "CPU", p.x) else {
                    return String::new();
                };
                let at = end
                    .map(|end| end + chrono::Duration::milliseconds((measured.x * 1000.0) as i64));
                format!(
                    "{}\n{}: {:.1}%",
                    at.map(|at| at
                        .with_timezone(&chrono::Local)
                        .format("%H:%M:%S")
                        .to_string())
                        .unwrap_or_default(),
                    name,
                    measured.y
                )
            })
            .show(ui, |plot| {
                if cpu || combined {
                    plot.line(
                        egui_plot::Line::new("CPU", view.cpu.as_slice())
                            .color(theme::BLUE)
                            .width(2.0),
                    );
                }
                if !cpu || combined {
                    plot.line(
                        egui_plot::Line::new("RAM", view.ram.as_slice())
                            .color(theme::PURPLE)
                            .width(2.0),
                    );
                }
            });
        for (label, is_cpu) in [("CPU", true), ("RAM", false)] {
            if !combined && is_cpu != cpu {
                continue;
            }
            if let Some((min, avg, max)) = view.stats[usize::from(!is_cpu)] {
                if combined {
                    ui.strong(label);
                }
                ui.columns(3, |cols| {
                    for (col, (title, value)) in cols.iter_mut().zip([
                        ("Minimum", min),
                        ("Rata-rata", avg),
                        ("Maksimum", max),
                    ]) {
                        col.label(RichText::new(title).small().color(theme::MUTED));
                        col.strong(format!("{value:.1}%"));
                    }
                });
            }
        }
    });
}
impl RigPulseApp {
    pub(crate) fn monitoring_page(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            ui.selectable_value(&mut self.network.tab, 0, "System");
            ui.selectable_value(&mut self.network.tab, 1, "Network");
        });
        ui.add_space(8.0);
        if self.network.tab == 1 {
            self.network_page(ui);
            return;
        }
        ui.horizontal_wrapped(|ui| {
            for minutes in [5, 15, 60] {
                ui.selectable_value(&mut self.minutes, minutes, format!("{minutes} menit"));
            }
            if ui
                .button(if self.paused {
                    "Lanjutkan grafik"
                } else {
                    "Jeda grafik"
                })
                .clicked()
            {
                self.paused = !self.paused;
                self.frozen_history = if self.paused {
                    Some(self.history.clone())
                } else {
                    None
                };
            }
            theme::toggle(ui, &mut self.combined_chart, "Tampilan gabungan");
            theme::toggle(ui, &mut self.settings.power_saver, "Hemat daya");
        });
        if self.paused {
            ui.colored_label(
                theme::WARNING,
                "Grafik dibekukan. Pembacaan langsung dan pengumpulan riwayat tetap berjalan.",
            );
        }
        if let Some(s) = &self.sample {
            let count = theme::columns(ui.available_width(), 2);
            for start in (0..2).step_by(count) {
                ui.columns(count, |cols| {
                    for (n, col) in cols.iter_mut().enumerate() {
                        let m = if start + n == 0 { &s.cpu } else { &s.memory };
                        theme::metric(
                            col,
                            &format!("Penggunaan {}", m.label),
                            m.value,
                            "%",
                            "Penggunaan saat ini",
                            if m.stale(chrono::Utc::now(), 15) {
                                "Data lama"
                            } else {
                                m.availability.label()
                            },
                            if start + n == 0 {
                                theme::BLUE
                            } else {
                                theme::PURPLE
                            },
                            &m.source,
                        );
                    }
                });
            }
        }
        let history = self.frozen_history.as_ref().unwrap_or(&self.history);
        self.chart_view.refresh(history, self.minutes, self.paused);
        if self.combined_chart {
            chart(ui, &self.chart_view, self.minutes, true, true, self.paused);
        } else {
            chart(ui, &self.chart_view, self.minutes, true, false, self.paused);
            chart(
                ui,
                &self.chart_view,
                self.minutes,
                false,
                false,
                self.paused,
            );
        }
        if let Some(s) = &self.sample {
            theme::section(ui, "Suhu perangkat");
            let count = theme::columns(ui.available_width(), 2);
            for start in (0..2).step_by(count) {
                ui.columns(count, |cols| {
                    for (n, col) in cols.iter_mut().enumerate() {
                        let gpu = start + n == 1;
                        let m = s.extra.iter().find(|m| {
                            m.unit == "°C"
                                && if gpu {
                                    m.label != "Suhu CPU" && m.value.is_some()
                                } else {
                                    m.label == "Suhu CPU"
                                }
                        });
                        theme::metric(
                            col,
                            if gpu { "Suhu GPU" } else { "Suhu CPU" },
                            m.and_then(|m| m.value),
                            " °C",
                            "Dibaca dari sensor yang didukung",
                            if m.is_some_and(|m| m.value.is_some()) {
                                "Terbaca"
                            } else {
                                "Sensor tidak tersedia"
                            },
                            if gpu { theme::CYAN } else { theme::BLUE },
                            m.map(|m| m.label.as_str())
                                .unwrap_or("Sensor tidak tersedia pada perangkat ini."),
                        );
                    }
                });
            }
            egui::CollapsingHeader::new("GPU dan sensor tambahan").show(ui,|ui|{
                ui.label("NVIDIA, AMD, dan Intel bergantung pada kemampuan driver. Memori bersama menggunakan RAM.");
                for m in &s.extra{metric(ui,m);}
            });
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cached_chart_preserves_pause_window_and_nearest_measured_sample() {
        let start = chrono::Utc::now();
        let mut live = rigpulse_monitor::buffer::RingBuffer::default();
        live.push(start, 10.0, 20.0);
        live.push(start + chrono::Duration::minutes(6), 30.0, 40.0);
        let frozen = live.clone();
        let mut view = ChartView::default();
        view.refresh(&live, 5, false);
        assert_eq!(view.stats[0], Some((30.0, 30.0, 30.0)));
        view.refresh(&frozen, 15, true);
        assert_eq!(view.cpu.len(), 2);
        assert_eq!(view.nearest(true, -355.0).unwrap().y, 10.0);
        assert_eq!(view.nearest(false, 100.0).unwrap().y, 40.0);
        live.push(start + chrono::Duration::minutes(7), 50.0, 60.0);
        view.refresh(&frozen, 15, true);
        assert_eq!(view.stats[0], Some((10.0, 20.0, 30.0)));
        view.refresh(&live, 15, false);
        assert_eq!(view.stats[0], Some((10.0, 30.0, 50.0)));
        assert_eq!(view.ram.len(), 3);
    }
    #[test]
    fn axis_never_exposes_negative_seconds() {
        assert_eq!(time_label(-300.0), "5 mnt lalu");
        assert_eq!(time_label(0.0), "Sekarang");
        assert_eq!(time_label(-30.0), "30 dtk lalu");
    }
}

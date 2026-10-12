use crate::{
    app::RigPulseApp,
    network_service::{SpeedUiStage, Tool, TrafficUnit},
    theme,
};
use eframe::egui::{self, RichText};
use rigpulse_network::tools::{gauge_angle, stable_gauge_scale};

#[derive(Default)]
pub(crate) struct NetworkView {
    key: Option<(Option<chrono::DateTime<chrono::Utc>>, usize, i64, u8)>,
    down: Vec<egui_plot::PlotPoint>,
    up: Vec<egui_plot::PlotPoint>,
    totals: (u64, u64, f64),
    ymax: f64,
}
impl NetworkView {
    fn unit_id(unit: TrafficUnit) -> u8 {
        match unit {
            TrafficUnit::Mbps => 0,
            TrafficUnit::MBps => 1,
            TrafficUnit::KBps => 2,
            TrafficUnit::Bps => 3,
        }
    }
    fn refresh(
        &mut self,
        history: &std::collections::VecDeque<(
            chrono::DateTime<chrono::Utc>,
            rigpulse_network::Rate,
        )>,
        minutes: i64,
        unit: TrafficUnit,
    ) {
        let key = (
            history.back().map(|(at, _)| *at),
            history.len(),
            minutes,
            Self::unit_id(unit),
        );
        if self.key == Some(key) {
            return;
        }
        self.key = Some(key);
        self.down.clear();
        self.up.clear();
        self.totals = (0, 0, 0.0);
        self.ymax = 1.0;
        if let Some((latest, _)) = history.back() {
            for (at, r) in history
                .iter()
                .filter(|(at, _)| latest.signed_duration_since(*at).num_seconds() <= minutes * 60)
            {
                let x = at.signed_duration_since(*latest).num_milliseconds() as f64 / 1000.0;
                let d = unit.plot_value(r.down);
                let u = unit.plot_value(r.up);
                self.down.push(egui_plot::PlotPoint::new(x, d));
                self.up.push(egui_plot::PlotPoint::new(x, u));
                self.totals.0 += r.delta_down;
                self.totals.1 += r.delta_up;
                self.totals.2 += r.seconds;
                self.ymax = self.ymax.max(d).max(u);
            }
            // Stable dynamic scale: 20% headroom snapped to a nice step.
            let target = (self.ymax * 1.2).max(1.0);
            let mag = 10f64.powf(target.log10().floor());
            let norm = target / mag;
            let nice = if norm <= 1.0 {
                1.0
            } else if norm <= 2.0 {
                2.0
            } else if norm <= 5.0 {
                5.0
            } else {
                10.0
            };
            self.ymax = nice * mag;
        }
    }
}

fn optional(v: Option<f64>, unit: &str) -> String {
    v.map(|v| format!("{v:.2} {unit}"))
        .unwrap_or_else(|| "Tidak tersedia".into())
}

fn axis_time(seconds: f64, minutes: i64) -> String {
    if seconds >= -0.5 {
        "Sekarang".into()
    } else if minutes <= 5 {
        format!("{:.0} mnt lalu", -seconds / 60.0)
    } else {
        format!("{:.0} mnt", -seconds / 60.0)
    }
}

fn friendly_name(name: &str) -> String {
    let short = name.split(" - ").next().unwrap_or(name);
    if short.len() > 48 {
        format!("{}…", &short[..47])
    } else {
        short.to_owned()
    }
}

/// Vector semicircular speedometer. Needle angle is derived only from the
/// provided (genuine) measurement; no telemetry is synthesized here.
fn speedometer(ui: &mut egui::Ui, display_mbps: f64, max: f64, is_download: bool, waiting: bool) {
    let accent = if is_download {
        theme::DOWNLOAD_BLUE
    } else {
        theme::UPLOAD_GREEN
    };
    let size = egui::vec2(ui.available_width().clamp(280.0, 640.0), 230.0);
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    let painter = ui.painter_at(rect);
    let center = egui::pos2(rect.center().x, rect.bottom() - 34.0);
    let radius = (rect.width() / 2.0 - 28.0).clamp(90.0, 220.0);
    let arc_points = |from: f64, to: f64, steps: usize| -> Vec<egui::Pos2> {
        (0..=steps)
            .map(|i| {
                let a = from + (to - from) * i as f64 / steps as f64;
                center + egui::vec2(a.cos() as f32, a.sin() as f32) * radius
            })
            .collect()
    };
    // Background arc.
    painter.add(egui::Shape::line(
        arc_points(std::f64::consts::PI, 2.0 * std::f64::consts::PI, 64),
        egui::Stroke::new(14.0, egui::Color32::from_rgb(28, 45, 70)),
    ));
    // Active arc up to the genuine measurement.
    let frac = if waiting {
        0.0
    } else {
        (display_mbps / max).clamp(0.0, 1.0)
    };
    if frac > 0.001 {
        painter.add(egui::Shape::line(
            arc_points(
                std::f64::consts::PI,
                std::f64::consts::PI + frac * std::f64::consts::PI,
                (64.0 * frac).ceil() as usize,
            ),
            egui::Stroke::new(14.0, accent),
        ));
    }
    // Ticks + scale labels.
    for i in 0..=4 {
        let f = i as f64 / 4.0;
        let a = std::f64::consts::PI + f * std::f64::consts::PI;
        let dir = egui::vec2(a.cos() as f32, a.sin() as f32);
        painter.line_segment(
            [
                center + dir * (radius - 16.0),
                center + dir * (radius - 4.0),
            ],
            egui::Stroke::new(2.0, theme::MUTED),
        );
        let value = max * f;
        let label = format!("{value:.0}");
        painter.text(
            center + dir * (radius + 14.0),
            egui::Align2::CENTER_CENTER,
            label,
            egui::FontId::proportional(11.0),
            theme::MUTED,
        );
    }
    // Needle.
    let angle = gauge_angle(if waiting { 0.0 } else { display_mbps }, max);
    let dir = egui::vec2(angle.cos() as f32, angle.sin() as f32);
    painter.line_segment(
        [center, center + dir * (radius - 22.0)],
        egui::Stroke::new(3.0, egui::Color32::WHITE),
    );
    painter.circle_filled(center, 9.0, egui::Color32::WHITE);
    painter.circle_filled(center, 5.0, accent);
}

fn status_badge(ui: &mut egui::Ui, connected: bool, label: &str) {
    let (bg, fg) = if connected {
        (
            egui::Color32::from_rgb(20, 60, 40),
            egui::Color32::from_rgb(74, 222, 128),
        )
    } else {
        (
            egui::Color32::from_rgb(70, 30, 30),
            egui::Color32::from_rgb(248, 113, 113),
        )
    };
    egui::Frame::new()
        .fill(bg)
        .corner_radius(8)
        .inner_margin(egui::Margin::symmetric(10, 5))
        .show(ui, |ui| {
            ui.label(RichText::new(label).color(fg).strong().size(12.0));
        });
}

fn metric_card(ui: &mut egui::Ui, title: &str, value: &str, subtitle: &str, color: egui::Color32) {
    theme::surface(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 6.0;
        ui.label(RichText::new(title).strong().color(color).size(14.0));
        ui.label(RichText::new(value).size(30.0).strong());
        ui.label(RichText::new(subtitle).small().color(theme::MUTED));
    });
}

impl RigPulseApp {
    pub(crate) fn network_page(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        if !self.settings.network_enabled {
            theme::surface(ui, |ui| {
                ui.label("Pemantauan jaringan dinonaktifkan di Settings.");
            });
            return;
        }
        // Animate gauge with frame time; request repaints while a test runs.
        if matches!(
            self.network.speed_stage,
            SpeedUiStage::TestingDownload | SpeedUiStage::TestingUpload
        ) {
            let dt = ui.ctx().input(|i| i.stable_dt).min(0.1);
            if self.settings.reduced_animations {
                if let Some(t) = self.network.speed_live {
                    self.network.speed_display = t;
                }
            } else {
                self.network.animate_gauge(dt);
            }
            ctx.request_repaint();
        }
        let latest = self.network.monitor.latest.clone();
        let connected = latest
            .as_ref()
            .and_then(|s| s.adapters.iter().find(|a| Some(a.id) == s.selected))
            .map(|a| a.connected)
            .unwrap_or(false);

        // ---------- HEADER ----------
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(RichText::new("Network Monitoring").size(26.0).strong());
                ui.label(
                    RichText::new("Pantau aktivitas jaringan dan periksa kualitas internet.")
                        .color(theme::MUTED)
                        .size(13.0),
                );
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Segarkan").clicked() {
                    self.network
                        .monitor
                        .command(rigpulse_network::Command::Refresh);
                }
                status_badge(
                    ui,
                    connected,
                    if connected { "Terhubung" } else { "Terputus" },
                );
            });
        });
        ui.add_space(8.0);

        // ---------- ROW 1: CONNECTION OVERVIEW ----------
        theme::surface(ui, |ui| {
            theme::section(ui, "Ringkasan Koneksi");
            if let Some(s) = &latest {
                let selected = s.adapters.iter().find(|a| Some(a.id) == s.selected);
                let label = self
                    .settings
                    .preferred_adapter
                    .and_then(|id| s.adapters.iter().find(|a| a.id == id))
                    .map(|a| friendly_name(&a.name))
                    .unwrap_or_else(|| "Otomatis — Rute Windows".into());
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new("Adaptor:").color(theme::MUTED));
                    egui::ComboBox::from_id_salt("network_adapter")
                        .selected_text(label)
                        .width(260.0)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut self.settings.preferred_adapter,
                                None,
                                "Otomatis — Rute Windows",
                            );
                            for a in &s.adapters {
                                ui.selectable_value(
                                    &mut self.settings.preferred_adapter,
                                    Some(a.id),
                                    format!(
                                        "{} · {}",
                                        friendly_name(&a.name),
                                        if a.connected { "Terhubung" } else { "Terputus" }
                                    ),
                                );
                            }
                        });
                    if ui.button("Segarkan adaptor").clicked() {
                        self.network
                            .monitor
                            .command(rigpulse_network::Command::Refresh);
                    }
                });
                if let Some(a) = selected {
                    let local_v4 = a
                        .addresses
                        .iter()
                        .find(|ip| ip.contains('.') && !ip.starts_with("127."))
                        .cloned()
                        .unwrap_or_else(|| "Tidak tersedia".into());
                    let link = if a.link_down > 0 || a.link_up > 0 {
                        format!(
                            "{:.0} Mbps masuk · {:.0} Mbps keluar",
                            a.link_down as f64 / 1_000_000.0,
                            a.link_up as f64 / 1_000_000.0
                        )
                    } else {
                        "Tidak tersedia".into()
                    };
                    let local_display = if self.settings.include_sensitive {
                        local_v4
                    } else {
                        "Disembunyikan privasi".into()
                    };
                    ui.columns(4, |cols| {
                        let items = [
                            ("Status", s.connectivity.clone()),
                            ("Adaptor", friendly_name(&a.name)),
                            ("Jenis / IPv4", format!("{} · {}", a.kind, local_display)),
                            ("Link", link),
                        ];
                        for (col, (k, v)) in cols.iter_mut().zip(items) {
                            col.label(RichText::new(k).small().color(theme::MUTED));
                            col.label(RichText::new(v).strong().size(13.0));
                        }
                    });
                    ui.label(
                        RichText::new(
                            "Terhubung ke jaringan belum berarti internet terkonfirmasi. Kecepatan link adalah ke router, bukan jaminan internet.",
                        )
                        .small()
                        .color(theme::MUTED),
                    );
                } else {
                    ui.label("Menunggu sampel adaptor…");
                }
                if let Some(e) = &s.error {
                    ui.colored_label(theme::WARNING, e);
                }
            } else {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label("Membaca adaptor Windows…");
                });
            }
        });

        // ---------- ROW 2: LIVE METRICS ----------
        {
            let (down, up) = latest
                .as_ref()
                .and_then(|s| s.rate)
                .map(|r| (r.down, r.up))
                .unwrap_or((f64::NAN, f64::NAN));
            let unit = self.network.traffic_unit;
            let fmt = |v: f64| {
                if v.is_nan() {
                    "Menunggu…".into()
                } else {
                    unit.format(v)
                }
            };
            let width = ui.available_width();
            if width >= 580.0 {
                ui.columns(2, |cols| {
                    metric_card(
                        &mut cols[0],
                        "Download",
                        &fmt(down),
                        "Kecepatan penerimaan data saat ini",
                        theme::DOWNLOAD_BLUE,
                    );
                    metric_card(
                        &mut cols[1],
                        "Upload",
                        &fmt(up),
                        "Kecepatan pengiriman data saat ini",
                        theme::UPLOAD_GREEN,
                    );
                });
            } else {
                metric_card(
                    ui,
                    "Download",
                    &fmt(down),
                    "Kecepatan penerimaan data saat ini",
                    theme::DOWNLOAD_BLUE,
                );
                metric_card(
                    ui,
                    "Upload",
                    &fmt(up),
                    "Kecepatan pengiriman data saat ini",
                    theme::UPLOAD_GREEN,
                );
            }
            ui.label(
                RichText::new(
                    "Kecepatan ini dari penghitung adaptor lokal (semua aplikasi + jaringan lokal), bukan hasil Speedtest internet.",
                )
                .small()
                .color(theme::MUTED),
            );
        }

        // ---------- ROW 3: TRAFFIC CHART ----------
        theme::surface(ui, |ui| {
            theme::section(ui, "Grafik Aktivitas Jaringan");
            ui.horizontal_wrapped(|ui| {
                for n in [5, 15, 60] {
                    ui.selectable_value(
                        &mut self.settings.network_history_minutes,
                        n,
                        format!("{n} menit"),
                    );
                }
                for u in TrafficUnit::ALL {
                    ui.selectable_value(&mut self.network.traffic_unit, u, u.label());
                }
                if ui
                    .button(if self.network.paused {
                        "Lanjutkan"
                    } else {
                        "Jeda"
                    })
                    .clicked()
                {
                    self.network.paused = !self.network.paused;
                    self.network.frozen = self
                        .network
                        .paused
                        .then(|| self.network.monitor.history.clone());
                }
            });
            let history = self
                .network
                .frozen
                .as_ref()
                .unwrap_or(&self.network.monitor.history);
            if history.is_empty() {
                ui.label("Mengumpulkan sampel… grafik muncul setelah beberapa detik.");
            } else {
                let minutes = self.settings.network_history_minutes;
                let unit = self.network.traffic_unit;
                self.network_view.refresh(history, minutes, unit);
                let (down_b, up_b, seconds) = self.network_view.totals;
                let ymax = self.network_view.ymax.max(0.001);
                let down_pts = self.network_view.down.clone();
                let up_pts = self.network_view.up.clone();
                egui_plot::Plot::new("network_history")
                    .height(200.0)
                    .allow_scroll(false)
                    .allow_drag(false)
                    .allow_zoom(false)
                    .include_y(0.0)
                    .include_y(ymax)
                    .show_grid([false, true])
                    .show_background(false)
                    .x_axis_formatter(move |mark, _| axis_time(mark.value, minutes))
                    .y_axis_formatter(move |mark, _| unit.axis_label(mark.value))
                    .label_formatter(move |name, p| format!("{name}\n{:.2} {}", p.y, unit.label()))
                    .legend(egui_plot::Legend::default().position(egui_plot::Corner::LeftTop))
                    .show(ui, |plot| {
                        plot.line(
                            egui_plot::Line::new("Download", down_pts.as_slice())
                                .color(theme::BLUE)
                                .width(2.0),
                        );
                        plot.line(
                            egui_plot::Line::new("Upload", up_pts.as_slice())
                                .color(theme::UPLOAD_GREEN)
                                .width(2.0),
                        );
                    });
                ui.columns(3, |cols| {
                    let items = [
                        (
                            "Total Download",
                            format!("{:.2} MB", down_b as f64 / 1_000_000.0),
                        ),
                        (
                            "Total Upload",
                            format!("{:.2} MB", up_b as f64 / 1_000_000.0),
                        ),
                        (
                            "Rata-rata",
                            if seconds > 0.0 {
                                format!("{} masuk", unit.format(down_b as f64 / seconds))
                            } else {
                                "Tidak tersedia".into()
                            },
                        ),
                    ];
                    for (col, (k, v)) in cols.iter_mut().zip(items) {
                        col.label(RichText::new(k).small().color(theme::MUTED));
                        col.label(RichText::new(v).strong());
                    }
                });
                if let Some(s) = &latest {
                    ui.label(
                        RichText::new(format!(
                            "Sumber: Windows IP Helper · {} · interval 2 detik",
                            s.at.with_timezone(&chrono::Local).format("%H:%M:%S")
                        ))
                        .small()
                        .color(theme::MUTED),
                    );
                }
            }
            if let Some(s) = &latest
                && let Some(a) = s.adapters.iter().find(|a| Some(a.id) == s.selected)
            {
                let mut wifi_request = None;
                egui::CollapsingHeader::new("Detail adaptor jaringan").show(ui, |ui| {
                    super::row(ui, "Nama", &friendly_name(&a.name));
                    super::row(ui, "Deskripsi", &a.description);
                    super::row(ui, "Jenis", &a.kind);
                    if self.settings.include_sensitive {
                        for (name, value) in [
                            ("IP lokal", a.addresses.join(", ")),
                            ("Gateway", a.gateways.join(", ")),
                            ("DNS", a.dns.join(", ")),
                            ("MAC", a.mac.clone()),
                        ] {
                            super::row(
                                ui,
                                name,
                                if value.is_empty() {
                                    "Tidak tersedia"
                                } else {
                                    &value
                                },
                            );
                        }
                    } else {
                        ui.label("IP lokal, gateway, DNS, dan MAC disembunyikan oleh pengaturan privasi.");
                    }
                    if a.kind == "Wi-Fi" {
                        if let Some((id, ssid, signal)) = &self.network.wifi
                            && *id == a.id
                            && self.settings.include_sensitive
                        {
                            super::row(ui, "SSID", ssid);
                            super::row(ui, "Sinyal Wi-Fi", &format!("{signal}%"));
                        } else {
                            super::row(ui, "SSID / sinyal Wi-Fi", "Belum dibaca / disembunyikan");
                        }
                        if ui
                            .add_enabled(
                                self.settings.include_sensitive && !self.network.busy,
                                egui::Button::new("Baca SSID / sinyal Wi-Fi…"),
                            )
                            .clicked()
                        {
                            wifi_request = Some(a.id);
                        }
                    }
                });
                if let Some(id) = wifi_request {
                    self.network.start(Tool::Wifi(id), &self.settings, &ctx);
                }
            }
        });

        // ---------- ROW 4: SPEEDTEST + PING ----------
        {
            let narrow = ui.available_width() < 580.0;
            let mut show_pair = |ui: &mut egui::Ui| {
                // Speedtest summary
                theme::surface(ui, |ui| {
                    ui.set_min_height(210.0);
                    theme::section(ui, "Tes Kecepatan Internet");
                    ui.label(
                        RichText::new("Ukur kecepatan download, upload, dan respons koneksi.")
                            .small()
                            .color(theme::MUTED),
                    );
                    if let Some(s) = &self.network.speed {
                        let ping = s
                            .latency_ms
                            .map(|v| format!("{v:.0} ms"))
                            .unwrap_or_else(|| "—".into());
                        ui.label(
                            RichText::new(format!(
                                "Download {:.1} Mbps · Upload {:.1} Mbps · Ping {ping}",
                                s.download_mbps, s.upload_mbps
                            ))
                            .strong(),
                        );
                        ui.label(
                            RichText::new(format!(
                                "Terakhir: {}",
                                s.at.with_timezone(&chrono::Local).format("%d/%m/%Y %H:%M")
                            ))
                            .small()
                            .color(theme::MUTED),
                        );
                    } else {
                        ui.label(
                            RichText::new("Belum ada hasil. Jalankan tes untuk mengukur koneksi.")
                                .color(theme::MUTED),
                        );
                    }
                    if ui
                        .add_sized(
                            [ui.available_width(), 38.0],
                            egui::Button::new("Mulai Speedtest"),
                        )
                        .clicked()
                    {
                        self.network.speed_open = true;
                        if !self.network.ookla_agreement {
                            self.network.speed_stage = SpeedUiStage::ConsentRequired;
                        } else if !self.network.busy {
                            self.network.speed_stage = SpeedUiStage::Idle;
                        }
                    }
                });
                // Ping summary
                theme::surface(ui, |ui| {
                    ui.set_min_height(210.0);
                    theme::section(ui, "Stabilitas Koneksi");
                    ui.horizontal_wrapped(|ui| {
                        ui.label("Target");
                        ui.add(
                            egui::TextEdit::singleline(&mut self.settings.ping_target)
                                .desired_width(150.0),
                        );
                        if ui
                            .add_enabled(!self.network.busy, egui::Button::new("Mulai Ping"))
                            .clicked()
                        {
                            let target = self.settings.ping_target.clone();
                            self.network.start(Tool::Ping(target), &self.settings, &ctx);
                        }
                    });
                    if let Some(p) = &self.network.ping {
                        ui.label(
                            RichText::new(format!(
                                "Rata-rata {} · Hilang {} · Jitter {}",
                                p.average
                                    .map(|v| format!("{v:.0} ms"))
                                    .unwrap_or_else(|| "—".into()),
                                p.loss
                                    .map(|v| format!("{v:.0}%"))
                                    .unwrap_or_else(|| "—".into()),
                                p.jitter
                                    .map(|v| format!("{v:.0} ms"))
                                    .unwrap_or_else(|| "—".into()),
                            ))
                            .strong(),
                        );
                    } else {
                        ui.label(
                            RichText::new("Belum ada hasil ping (10 paket ICMP).")
                                .color(theme::MUTED),
                        );
                    }
                });
            };
            if narrow {
                show_pair(ui);
            } else {
                ui.columns(2, |cols| {
                    // Render each card in its column by duplicating minimal logic
                    // through a shared helper is complex in borrowck; inline:
                    {
                        let col = &mut cols[0];
                        theme::surface(col, |ui| {
                            ui.set_min_height(210.0);
                            theme::section(ui, "Tes Kecepatan Internet");
                    ui.label(
                        RichText::new(
                            "Ukur kecepatan download, upload, dan respons koneksi.",
                        )
                        .small()
                        .color(theme::MUTED),
                    );
                            if let Some(s) = &self.network.speed {
                                let ping = s
                                    .latency_ms
                                    .map(|v| format!("{v:.0} ms"))
                                    .unwrap_or_else(|| "—".into());
                                ui.label(
                                    RichText::new(format!(
                                        "Download {:.1} Mbps · Upload {:.1} Mbps · Ping {ping}",
                                        s.download_mbps, s.upload_mbps
                                    ))
                                    .strong(),
                                );
                                ui.label(
                                    RichText::new(format!(
                                        "Terakhir: {}",
                                        s.at.with_timezone(&chrono::Local).format("%d/%m/%Y %H:%M")
                                    ))
                                    .small()
                                    .color(theme::MUTED),
                                );
                            } else {
                                ui.label(
                                    RichText::new(
                                        "Belum ada hasil. Jalankan tes untuk mengukur koneksi.",
                                    )
                                    .color(theme::MUTED),
                                );
                            }
                            if ui
                                .add_sized(
                                    [ui.available_width(), 38.0],
                                    egui::Button::new("Mulai Speedtest"),
                                )
                                .clicked()
                            {
                                self.network.speed_open = true;
                                if !self.network.ookla_agreement {
                                    self.network.speed_stage = SpeedUiStage::ConsentRequired;
                                }
                            }
                        });
                    }
                    {
                        let col = &mut cols[1];
                        theme::surface(col, |ui| {
                            ui.set_min_height(210.0);
                            theme::section(ui, "Stabilitas Koneksi");
                            ui.horizontal_wrapped(|ui| {
                                ui.label("Target");
                                ui.add(
                                    egui::TextEdit::singleline(&mut self.settings.ping_target)
                                        .desired_width(130.0),
                                );
                                if ui
                                    .add_enabled(
                                        !self.network.busy,
                                        egui::Button::new("Mulai Ping"),
                                    )
                                    .clicked()
                                {
                                    let target = self.settings.ping_target.clone();
                                    self.network.start(Tool::Ping(target), &self.settings, &ctx);
                                }
                            });
                            if let Some(p) = &self.network.ping {
                                ui.label(
                                    RichText::new(format!(
                                        "Rata-rata {} · Hilang {} · Jitter {}",
                                        p.average
                                            .map(|v| format!("{v:.0} ms"))
                                            .unwrap_or_else(|| "—".into()),
                                        p.loss
                                            .map(|v| format!("{v:.0}%"))
                                            .unwrap_or_else(|| "—".into()),
                                        p.jitter
                                            .map(|v| format!("{v:.0} ms"))
                                            .unwrap_or_else(|| "—".into()),
                                    ))
                                    .strong(),
                                );
                            } else {
                                ui.label(
                                    RichText::new("Belum ada hasil ping (10 paket ICMP).")
                                        .color(theme::MUTED),
                                );
                            }
                        });
                    }
                });
            }
        }

        // ---------- PING DETAILS ----------
        if self.network.ping.is_some() {
            theme::surface(ui, |ui| {
                theme::section(ui, "Hasil Ping / Stabilitas");
                if let Some(p) = &self.network.ping {
                    ui.columns(3, |cols| {
                        let items = [
                            (
                                "Rata-rata",
                                p.average
                                    .map(|v| format!("{v:.1} ms"))
                                    .unwrap_or_else(|| "—".into()),
                            ),
                            (
                                "Paket Hilang",
                                p.loss
                                    .map(|v| format!("{v:.0}%"))
                                    .unwrap_or_else(|| "—".into()),
                            ),
                            (
                                "Jitter",
                                p.jitter
                                    .map(|v| format!("{v:.1} ms"))
                                    .unwrap_or_else(|| "—".into()),
                            ),
                        ];
                        for (col, (k, v)) in cols.iter_mut().zip(items) {
                            col.label(RichText::new(k).small().color(theme::MUTED));
                            col.label(RichText::new(v).size(22.0).strong());
                        }
                    });
                    ui.label(
                        RichText::new(
                            "Latensi adalah waktu respons koneksi. Paket hilang menunjukkan paket tanpa balasan. Jitter menunjukkan variasi waktu respons.",
                        )
                        .small()
                        .color(theme::MUTED),
                    );
                    // Compact latency chart; timeouts are gaps, never zero.
                    if p.samples.len() > 1 {
                        let pts: Vec<egui_plot::PlotPoint> = p
                            .samples
                            .iter()
                            .enumerate()
                            .map(|(i, s)| {
                                egui_plot::PlotPoint::new(i as f64 + 1.0, s.unwrap_or(f64::NAN))
                            })
                            .collect();
                        egui_plot::Plot::new("ping_history")
                            .height(110.0)
                            .allow_scroll(false)
                            .allow_drag(false)
                            .allow_zoom(false)
                            .include_y(0.0)
                            .show_grid([false, true])
                            .show_background(false)
                            .show(ui, |plot| {
                                plot.line(
                                    egui_plot::Line::new("Latensi (ms)", pts.as_slice())
                                        .color(theme::BLUE)
                                        .width(2.0),
                                );
                            });
                        ui.label(
                            RichText::new("Grafik: tiap titik = satu paket; jeda = timeout.")
                                .small()
                                .color(theme::MUTED),
                        );
                    }
                    egui::CollapsingHeader::new("Detail hasil").show(ui, |ui| {
                        super::row(ui, "Target", &format!("{} ({})", p.target, p.address));
                        super::row(
                            ui,
                            "Terkirim / diterima",
                            &format!("{} / {}", p.sent, p.received),
                        );
                        super::row(
                            ui,
                            "Min / rata-rata / maks",
                            &format!(
                                "{} / {} / {}",
                                optional(p.min, "ms"),
                                optional(p.average, "ms"),
                                optional(p.max, "ms")
                            ),
                        );
                        if p.cancelled {
                            ui.label("Tes dibatalkan; statistik hanya paket yang sudah dikirim.");
                        }
                        if !p.errors.is_empty() {
                            egui::CollapsingHeader::new("Timeout / status ICMP").show(ui, |ui| {
                                for e in &p.errors {
                                    ui.label(e);
                                }
                            });
                        }
                    });
                }
            });
        }

        // ---------- ROW 5: PUBLIC IP & ISP ----------
        theme::surface(ui, |ui| {
            theme::section(ui, "IP Publik & Penyedia Internet");
            ui.label(
                RichText::new("Informasi alamat IP dan penyedia jaringan internet Anda.")
                    .small()
                    .color(theme::MUTED),
            );
            if !self.settings.external_lookup {
                ui.label("Untuk menampilkan IP publik, ISP, dan ASN, RigPulse perlu menghubungi layanan informasi jaringan eksternal.");
                ui.horizontal_wrapped(|ui| {
                    if ui
                        .add(egui::Button::new("Izinkan & Periksa").fill(theme::GREEN))
                        .clicked()
                    {
                        self.settings.external_lookup = true;
                        let _ = self.settings.save(&self.dir);
                        self.network.start(Tool::Public, &self.settings, &ctx);
                    }
                    if ui.button("Batal").clicked() {}
                });
            } else if let Some(p) = &self.network.public {
                let ip_display = if self.settings.include_sensitive {
                    p.ip.clone()
                } else {
                    "Disembunyikan · lihat izin privasi".into()
                };
                ui.columns(2, |cols| {
                    {
                        let col = &mut cols[0];
                        super::row(col, "IPv4 Publik", p.ipv4.as_deref().unwrap_or("—"));
                        super::row(col, "IPv6 Publik", p.ipv6.as_deref().unwrap_or("—"));
                        super::row(col, "IP teramati", &ip_display);
                        super::row(col, "ISP", p.isp.as_deref().unwrap_or("Tidak tersedia"));
                    }
                    {
                        let col = &mut cols[1];
                        super::row(col, "ASN", p.asn.as_deref().unwrap_or("Tidak tersedia"));
                        super::row(
                            col,
                            "Organisasi AS",
                            p.asn_org.as_deref().unwrap_or("Tidak tersedia"),
                        );
                        super::row(col, "Prefix", p.prefix.as_deref().unwrap_or("—"));
                        super::row(col, "Negara", p.location.as_deref().unwrap_or("—"));
                    }
                });
                ui.label(
                    RichText::new(
                        "ISP = penyedia akses Anda; Organisasi AS = pemegang nomor AS perutean; bisa berbeda (mis. saat VPN aktif, IP teramati milik exit VPN).",
                    )
                    .small()
                    .color(theme::MUTED),
                );
                ui.label(format!(
                    "Pemeriksaan {} · cache 1 jam · Sumber: ipify / ipwho.is / RIPEstat",
                    p.at.with_timezone(&chrono::Local)
                        .format("%d/%m/%Y %H:%M:%S")
                ));
                if let Some(w) = &p.warning {
                    ui.colored_label(theme::WARNING, w);
                }
            } else {
                ui.label("Belum ada hasil. Klik Periksa untuk menghubungi penyedia.");
            }
            ui.horizontal_wrapped(|ui| {
                let busy = self.network.busy;
                if ui
                    .add_enabled(!busy, egui::Button::new("Periksa Informasi Internet"))
                    .clicked()
                {
                    self.network.start(Tool::Public, &self.settings, &ctx);
                }
                let can_whois = self.network.public.is_some() && !busy;
                if ui
                    .add_enabled(can_whois, egui::Button::new("WHOIS Details"))
                    .clicked()
                    && let Some(p) = self.network.public.clone()
                {
                    self.network.whois_query = p.ip.clone();
                    self.network.whois_open = false;
                    self.network.start(Tool::Whois(p.ip), &self.settings, &ctx);
                }
                if ui.button("Copy IP").clicked()
                    && let Some(p) = &self.network.public
                {
                    ctx.copy_text(p.ip.clone());
                }
            });
            // Advanced lookup (collapsible, validated, trusted providers only).
            egui::CollapsingHeader::new("Advanced Lookup").show(ui, |ui| {
                ui.label(
                    RichText::new(
                        "IP publik saja. Tidak menerima URL. Query ke RIPEstat / RDAP resmi.",
                    )
                    .small()
                    .color(theme::MUTED),
                );
                ui.horizontal_wrapped(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut self.network.whois_query)
                            .desired_width(220.0)
                            .hint_text("cth. 8.8.8.8"),
                    );
                    if ui
                        .add_enabled(!self.network.busy, egui::Button::new("Lookup"))
                        .clicked()
                    {
                        let q = self.network.whois_query.trim().to_owned();
                        self.network.whois_open = false;
                        self.network.start(Tool::Whois(q), &self.settings, &ctx);
                    }
                });
            });
        });

        if !self.network.status.is_empty() {
            theme::surface(ui, |ui| {
                ui.label(&self.network.status);
                if self.network.busy {
                    ui.spinner();
                    if ui.button("Batalkan pengujian").clicked() {
                        self.network.cancel();
                    }
                }
            });
        }

        self.speedtest_modal(&ctx);
        self.whois_modal(&ctx);
    }

    fn speedtest_modal(&mut self, ctx: &egui::Context) {
        if !self.network.speed_open {
            return;
        }
        let mut close = false;
        let mut start_test = false;
        let mut cancel_test = false;
        let mut copy_results = false;
        egui::Modal::new(egui::Id::new("speedtest_modal")).show(ctx, |ui| {
            ui.set_max_width(700.0);
            ui.set_min_width(560.0);
            // Header
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(RichText::new("Tes Kecepatan Internet").size(20.0).strong());
                    ui.label(
                        RichText::new("Pengujian kecepatan koneksi internet")
                            .small()
                            .color(theme::MUTED),
                    );
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Tutup").clicked() {
                        if self.network.busy {
                            self.network.cancel();
                        }
                        close = true;
                    }
                    ui.label(
                        RichText::new(self.network.speed_stage.label())
                            .small()
                            .color(theme::BLUE),
                    );
                });
            });
            ui.separator();
            egui::ScrollArea::vertical()
                .max_height(ctx.screen_rect().height() * 0.9 - 120.0)
                .show(ui, |ui| {
                    let stage = self.network.speed_stage;
                    let is_dl = self.network.speed_is_download;
                    let waiting = matches!(
                        stage,
                        SpeedUiStage::Idle
                            | SpeedUiStage::ConsentRequired
                            | SpeedUiStage::Preparing
                            | SpeedUiStage::Connecting
                            | SpeedUiStage::TestingPing
                            | SpeedUiStage::Finalizing
                    ) && self.network.speed_live.is_none();
                    // Gauge
                    let display = self.network.speed_display;
                    let gauge_max = stable_gauge_scale(self.network.gauge_max.max(10.0), display);
                    self.network.gauge_max = gauge_max;
                    speedometer(ui, display, gauge_max, is_dl, waiting);
                    ui.vertical_centered(|ui| {
                        ui.label(
                            RichText::new(if waiting {
                                "—".into()
                            } else {
                                format!("{display:.1}")
                            })
                            .size(42.0)
                            .strong(),
                        );
                        ui.label(RichText::new("Mbps").color(theme::MUTED));
                        ui.label(
                            RichText::new(if matches!(
                                stage,
                                SpeedUiStage::TestingDownload | SpeedUiStage::TestingUpload
                            ) {
                                if is_dl {
                                    "DOWNLOAD · live (sementara)"
                                } else {
                                    "UPLOAD · live (sementara)"
                                }
                            } else {
                                stage.label()
                            })
                            .strong()
                                .color(if is_dl { theme::DOWNLOAD_BLUE } else { theme::UPLOAD_GREEN }),
                        );
                        if let Some(p) = self.network.speed_progress
                            && matches!(
                                stage,
                                SpeedUiStage::TestingDownload | SpeedUiStage::TestingUpload
                            )
                        {
                            ui.add(
                                egui::ProgressBar::new(p.clamp(0.0, 1.0) as f32)
                                    .fill(if is_dl { theme::DOWNLOAD_BLUE } else { theme::UPLOAD_GREEN })
                                    .desired_width(320.0),
                            );
                        }
                    });
                    if !self.network.status.is_empty() && self.network.busy {
                        ui.label(
                            RichText::new(&self.network.status)
                                .small()
                                .color(theme::MUTED),
                        );
                    }
                    if let Some(e) = &self.network.speed_error
                        && matches!(stage, SpeedUiStage::Failed | SpeedUiStage::Cancelled)
                    {
                        ui.colored_label(theme::DANGER, e);
                    }
                    // Consent
                    if matches!(stage, SpeedUiStage::ConsentRequired | SpeedUiStage::Idle)
                        && !self.network.busy
                    {
                        ui.separator();
                        ui.label("Pengujian menggunakan koneksi internet dan dapat mengonsumsi kuota data. Data koneksi akan diproses oleh penyedia Speedtest.");
                        ui.horizontal_wrapped(|ui| {
                            ui.hyperlink_to("Lisensi", "https://www.speedtest.net/about/eula");
                            ui.hyperlink_to("Ketentuan", "https://www.speedtest.net/about/terms");
                            ui.hyperlink_to(
                                "Privasi Ookla",
                                "https://www.speedtest.net/about/privacy",
                            );
                        });
                        ui.checkbox(
                            &mut self.network.ookla_agreement,
                            "Saya menyetujui ketentuan Ookla dan pengiriman IP/data tes ke Ookla serta server tes",
                        );
                        ui.label(
                            RichText::new("Batas aman: 120 detik / 512 MiB. Tidak pernah berjalan otomatis.")
                                .small()
                                .color(theme::MUTED),
                        );
                    }
                    // Results
                    if matches!(stage, SpeedUiStage::Completed)
                        && let Some(s) = self.network.speed.clone()
                    {
                        ui.separator();
                        ui.label(RichText::new("Hasil").strong().size(16.0));
                        ui.columns(3, |cols| {
                            let items = [
                                ("Download", format!("{:.1} Mbps", s.download_mbps), theme::BLUE),
                                ("Upload", format!("{:.1} Mbps", s.upload_mbps), theme::UPLOAD_GREEN),
                                (
                                    "Ping",
                                    s.latency_ms
                                        .map(|v| format!("{v:.0} ms"))
                                        .unwrap_or_else(|| "—".into()),
                                    theme::GREEN,
                                ),
                            ];
                            for (col, (k, v, c)) in cols.iter_mut().zip(items) {
                                theme::surface(col, |ui| {
                                    ui.label(
                                        RichText::new(k).small().color(c).strong(),
                                    );
                                    ui.label(RichText::new(v).size(22.0).strong());
                                });
                            }
                        });
                        super::row(ui, "Jitter", &optional(s.jitter_ms, "ms"));
                        super::row(ui, "Paket hilang", &optional(s.packet_loss, "%"));
                        super::row(ui, "Server", &s.server);
                        super::row(
                            ui,
                            "Durasi / data",
                            &format!(
                                "{:.1} dtk · {}",
                                s.elapsed,
                                s.bytes
                                    .map(|b| format!("{:.1} MB", b as f64 / 1_000_000.0))
                                    .unwrap_or_else(|| "Tidak tersedia".into())
                            ),
                        );
                        super::row(
                            ui,
                            "Waktu",
                            &s.at
                                .with_timezone(&chrono::Local)
                                .format("%d/%m/%Y %H:%M:%S")
                                .to_string(),
                        );
                        egui::CollapsingHeader::new("Detail teknis").show(ui, |ui| {
                            ui.label("Sumber: Ookla Speedtest CLI resmi (--format=jsonl --progress=yes). Sampel gauge berasal dari record bandwidth CLI yang dilaporkan, bukan traffic adaptor lokal. Latensi Ookla berbeda metode dengan Tes Ping ICMP.");
                        });
                    }
                    // Buttons
                    ui.separator();
                    ui.horizontal_wrapped(|ui| {
                        let busy = self.network.busy;
                        if matches!(stage, SpeedUiStage::Completed | SpeedUiStage::Failed | SpeedUiStage::Cancelled)
                        {
                            if ui.button("Test Again").clicked() {
                                start_test = true;
                            }
                            if ui.button("Copy Results").clicked() {
                                copy_results = true;
                            }
                            if ui.button("Tutup").clicked() {
                                close = true;
                            }
                        } else if busy {
                            if ui.button("Batalkan Tes").clicked() {
                                cancel_test = true;
                            }
                        } else {
                            if ui
                                .add_enabled(
                                    self.network.ookla_agreement,
                                    egui::Button::new("Mulai Tes").fill(theme::GREEN),
                                )
                                .clicked()
                            {
                                start_test = true;
                            }
                            if ui.button("Tutup").clicked() {
                                close = true;
                            }
                        }
                    });
                    if self.network.busy {
                        ui.label(
                            RichText::new("Menutup popup akan membatalkan tes yang berjalan.")
                                .small()
                                .color(theme::MUTED),
                        );
                    }
                });
        });
        if cancel_test {
            self.network.cancel();
        }
        if start_test && !self.network.busy {
            let dir = self.dir.clone();
            self.network.start(Tool::Speed(dir), &self.settings, ctx);
        }
        if copy_results && let Some(s) = &self.network.speed {
            ctx.copy_text(format!(
                "RigPulse Speedtest (Ookla)\nDownload: {:.2} Mbps\nUpload: {:.2} Mbps\nPing: {}\nJitter: {}\nPacket loss: {}\nServer: {}\nWaktu: {}",
                s.download_mbps,
                s.upload_mbps,
                optional(s.latency_ms, "ms"),
                optional(s.jitter_ms, "ms"),
                optional(s.packet_loss, "%"),
                s.server,
                s.at
                    .with_timezone(&chrono::Local)
                    .format("%d/%m/%Y %H:%M:%S")
            ));
        }
        if close {
            self.network.speed_open = false;
        }
    }

    fn whois_modal(&mut self, ctx: &egui::Context) {
        if !self.network.whois_open {
            return;
        }
        let mut close = false;
        let mut copy = false;
        egui::Modal::new(egui::Id::new("whois_modal")).show(ctx, |ui| {
            ui.set_max_width(760.0);
            ui.set_min_width(560.0);
            let query = self
                .network
                .whois
                .as_ref()
                .map(|w| w.query.clone())
                .unwrap_or_default();
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(RichText::new("IP Publik & Penyedia Internet").size(20.0).strong());
                    ui.label(
                        RichText::new("Informasi jaringan berdasarkan data registrasi publik.")
                            .small()
                            .color(theme::MUTED),
                    );
                    ui.label(RichText::new(format!("IP: {query}")).strong());
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Tutup").clicked() {
                        close = true;
                    }
                    if ui.button("Copy").clicked() {
                        ctx.copy_text(query.clone());
                    }
                });
            });
            ui.separator();
            egui::ScrollArea::vertical()
                .max_height(ctx.screen_rect().height() * 0.9 - 140.0)
                .show(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        for (i, name) in ["Overview", "Routing", "WHOIS / RDAP", "Raw (Advanced)"]
                            .iter()
                            .enumerate()
                        {
                            ui.selectable_value(&mut self.network.whois_tab, i, *name);
                        }
                    });
                    ui.separator();
                    if let Some(w) = self.network.whois.clone() {
                        let na = "Tidak tersedia";
                        match self.network.whois_tab {
                            0 => {
                                let pub_info = self.network.public.clone();
                                super::row(ui, "IP Publik", w.ip.as_deref().unwrap_or(na));
                                super::row(
                                    ui,
                                    "ISP / Provider IP",
                                    pub_info
                                        .as_ref()
                                        .and_then(|p| p.isp.clone())
                                        .as_deref()
                                        .unwrap_or(na),
                                );
                                super::row(ui, "ASN", w.asn.as_deref().unwrap_or(na));
                                super::row(ui, "Organisasi AS", w.as_org.as_deref().unwrap_or(na));
                                super::row(ui, "Prefix", w.prefix.as_deref().unwrap_or(na));
                                super::row(
                                    ui,
                                    "Organisasi terdaftar",
                                    w.organization.as_deref().unwrap_or(na),
                                );
                                super::row(ui, "Negara", w.country.as_deref().unwrap_or(na));
                            }
                            1 => {
                                super::row(ui, "Origin ASN", w.asn.as_deref().unwrap_or(na));
                                super::row(
                                    ui,
                                    "Prefix diumumkan",
                                    w.prefix.as_deref().or(w.cidr.as_deref()).unwrap_or(na),
                                );
                                super::row(ui, "Pemegang AS", w.as_org.as_deref().unwrap_or(na));
                                super::row(
                                    ui,
                                    "Sumber routing",
                                    "RIPEstat network-info + as-overview",
                                );
                                super::row(
                                    ui,
                                    "Diperiksa",
                                    &w.at
                                        .with_timezone(&chrono::Local)
                                        .format("%d/%m/%Y %H:%M:%S")
                                        .to_string(),
                                );
                            }
                            2 => {
                                super::row(ui, "Rentang", w.range.as_deref().unwrap_or(na));
                                super::row(
                                    ui,
                                    "Nama jaringan",
                                    w.network_name.as_deref().unwrap_or(na),
                                );
                                super::row(ui, "Registry", w.registry.as_deref().unwrap_or(na));
                                super::row(
                                    ui,
                                    "Organisasi",
                                    w.organization.as_deref().unwrap_or(na),
                                );
                                super::row(ui, "Dibuat", w.created.as_deref().unwrap_or(na));
                                super::row(ui, "Diperbarui", w.updated.as_deref().unwrap_or(na));
                                super::row(
                                    ui,
                                    "Sumber RDAP",
                                    w.rdap_source.as_deref().unwrap_or(na),
                                );
                                super::row(
                                    ui,
                                    "Sumber WHOIS",
                                    w.whois_source.as_deref().unwrap_or(na),
                                );
                                ui.label(
                                    RichText::new("Negara registrasi bukan lokasi fisik pengguna.")
                                        .small()
                                        .color(theme::MUTED),
                                );
                            }
                            _ => {
                                ui.label(
                                    RichText::new(
                                        "Ringkasan ternormalisasi (kontak disembunyikan).",
                                    )
                                    .small()
                                    .color(theme::MUTED),
                                );
                                for (k, v) in [
                                    ("query", w.query.clone()),
                                    (
                                        "network",
                                        w.network_name.clone().unwrap_or_else(|| na.into()),
                                    ),
                                    ("org", w.organization.clone().unwrap_or_else(|| na.into())),
                                    ("range", w.range.clone().unwrap_or_else(|| na.into())),
                                    ("cidr", w.cidr.clone().unwrap_or_else(|| na.into())),
                                    ("rdap", w.rdap_source.clone().unwrap_or_else(|| na.into())),
                                    ("whois", w.whois_source.clone().unwrap_or_else(|| na.into())),
                                ] {
                                    super::row(ui, k, &v);
                                }
                            }
                        }
                        if let Some(warn) = w.warning {
                            ui.colored_label(theme::WARNING, warn);
                        }
                        if ui.button("Copy Details").clicked() {
                            copy = true;
                        }
                    } else {
                        ui.label("Tidak ada data WHOIS.");
                    }
                });
        });
        if copy && let Some(w) = &self.network.whois {
            ctx.copy_text(format!(
                "IP: {}\nNetwork: {}\nOrg: {}\nRange: {}\nCIDR: {}\nCountry: {}\nASN: {}\nAS org: {}\nPrefix: {}\nRDAP: {}\nWHOIS: {}",
                w.ip.as_deref().unwrap_or("-"),
                w.network_name.as_deref().unwrap_or("-"),
                w.organization.as_deref().unwrap_or("-"),
                w.range.as_deref().unwrap_or("-"),
                w.cidr.as_deref().unwrap_or("-"),
                w.country.as_deref().unwrap_or("-"),
                w.asn.as_deref().unwrap_or("-"),
                w.as_org.as_deref().unwrap_or("-"),
                w.prefix.as_deref().unwrap_or("-"),
                w.rdap_source.as_deref().unwrap_or("-"),
                w.whois_source.as_deref().unwrap_or("-"),
            ));
        }
        if close {
            self.network.whois_open = false;
        }
    }
}

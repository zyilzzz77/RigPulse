#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn counters_reset_and_adapter_switch_never_spike() {
        let mut sampler = Sampler::default();
        assert!(sampler.sample(1, 100, 200, 0.0).is_none());
        let rate = sampler.sample(1, 1_000_100, 500_200, 2.0).unwrap();
        assert_eq!(rate.down, 500_000.0);
        assert_eq!(rate.up, 250_000.0);
        assert_eq!(sampler.total_down, 1_000_000);
        assert!(sampler.sample(1, 2, 3, 4.0).is_none());
        assert!(sampler.sample(2, 9_000_000, 3, 6.0).is_none());
    }
}
use std::{
    collections::VecDeque,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Instant,
};
pub mod native;
pub mod ookla;
pub mod tools;

#[derive(Clone, Debug)]
pub struct Adapter {
    pub id: u64,
    pub index: u32,
    pub name: String,
    pub description: String,
    pub kind: String,
    pub connected: bool,
    pub link_down: u64,
    pub link_up: u64,
    pub addresses: Vec<String>,
    pub dns: Vec<String>,
    pub gateways: Vec<String>,
    pub mac: String,
}
#[derive(Clone, Copy, Debug)]
pub struct Rate {
    pub down: f64,
    pub up: f64,
    pub delta_down: u64,
    pub delta_up: u64,
    pub seconds: f64,
}
#[derive(Default)]
pub struct Sampler {
    previous: Option<(u64, u64, u64, f64)>,
    pub total_down: u64,
    pub total_up: u64,
}
impl Sampler {
    pub fn sample(&mut self, id: u64, down: u64, up: u64, at: f64) -> Option<Rate> {
        let prior = self.previous.replace((id, down, up, at))?;
        if prior.0 != id {
            self.total_down = 0;
            self.total_up = 0;
            return None;
        }
        let elapsed = at - prior.3;
        if elapsed <= 0.0 || !elapsed.is_finite() {
            return None;
        }
        let d = down.checked_sub(prior.1)?;
        let u = up.checked_sub(prior.2)?;
        self.total_down = self.total_down.saturating_add(d);
        self.total_up = self.total_up.saturating_add(u);
        Some(Rate {
            down: d as f64 / elapsed,
            up: u as f64 / elapsed,
            delta_down: d,
            delta_up: u,
            seconds: elapsed,
        })
    }
}
#[derive(Clone, Debug)]
pub struct Snapshot {
    pub adapters: Vec<Adapter>,
    pub selected: Option<u64>,
    pub rate: Option<Rate>,
    pub at: chrono::DateTime<chrono::Utc>,
    pub total_down: u64,
    pub total_up: u64,
    pub error: Option<String>,
    pub connectivity: String,
}
pub enum Command {
    Select(Option<u64>),
    Enabled(bool),
    Refresh,
    Stop,
}
pub struct Monitor {
    tx: std::sync::mpsc::SyncSender<Command>,
    rx: std::sync::mpsc::Receiver<Snapshot>,
    pub latest: Option<Snapshot>,
    pub history: VecDeque<(chrono::DateTime<chrono::Utc>, Rate)>,
}
impl Monitor {
    pub fn start(
        enabled: bool,
        preferred: Option<u64>,
        repaint: impl Fn() + Send + 'static,
    ) -> Self {
        let (tx, commands) = std::sync::mpsc::sync_channel(8);
        let (events, rx) = std::sync::mpsc::sync_channel(4);
        std::thread::spawn(move || {
            let start = Instant::now();
            let mut enabled = enabled;
            let mut preferred = preferred;
            let mut adapters = Vec::new();
            let mut discovered = None;
            let mut sampler = Sampler::default();
            loop {
                for cmd in commands.try_iter() {
                    match cmd {
                        Command::Stop => return,
                        Command::Select(id) => {
                            preferred = id;
                            sampler = Sampler::default();
                        }
                        Command::Enabled(e) => {
                            enabled = e;
                            sampler = Sampler::default();
                        }
                        Command::Refresh => discovered = None,
                    }
                }
                if enabled {
                    let mut error = None;
                    if discovered.is_none_or(|at: Instant| at.elapsed().as_secs() >= 30) {
                        match native::adapters() {
                            Ok(a) => adapters = a,
                            Err(e) => {
                                adapters.clear();
                                error = Some(e.to_string());
                            }
                        }
                        discovered = Some(Instant::now());
                    }
                    let selected = preferred
                        .and_then(|id| adapters.iter().find(|a| a.id == id))
                        .or_else(|| {
                            let index = native::default_index();
                            adapters
                                .iter()
                                .find(|a| Some(a.index) == index && a.connected)
                        })
                        .or_else(|| adapters.iter().find(|a| a.connected));
                    let id = selected.map(|a| a.id);
                    let rate = selected.and_then(|a| match native::counters(a.id) {
                        Ok((d, u)) => sampler.sample(a.id, d, u, start.elapsed().as_secs_f64()),
                        Err(e) => {
                            error = Some(e.to_string());
                            sampler = Sampler::default();
                            discovered = None;
                            None
                        }
                    });
                    let snapshot = Snapshot {
                        adapters: adapters.clone(),
                        selected: id,
                        rate,
                        at: chrono::Utc::now(),
                        total_down: sampler.total_down,
                        total_up: sampler.total_up,
                        error,
                        connectivity: native::connectivity(),
                    };
                    if matches!(
                        events.try_send(snapshot),
                        Err(std::sync::mpsc::TrySendError::Disconnected(_))
                    ) {
                        return;
                    }
                    repaint();
                }
                match commands.recv_timeout(std::time::Duration::from_secs(2)) {
                    Ok(Command::Stop) | Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                        return;
                    }
                    Ok(Command::Select(id)) => {
                        preferred = id;
                        sampler = Sampler::default();
                    }
                    Ok(Command::Enabled(e)) => {
                        enabled = e;
                        sampler = Sampler::default();
                    }
                    Ok(Command::Refresh) => discovered = None,
                    Err(_) => {}
                }
            }
        });
        Self {
            tx,
            rx,
            latest: None,
            history: VecDeque::with_capacity(1801),
        }
    }
    pub fn command(&self, command: Command) {
        let _ = self.tx.try_send(command);
    }
    pub fn poll(&mut self) {
        for snapshot in self.rx.try_iter() {
            if self.latest.as_ref().and_then(|s| s.selected) != snapshot.selected
                || snapshot.rate.is_none()
            {
                self.history.clear();
            }
            if let Some(rate) = snapshot.rate {
                self.history.push_back((snapshot.at, rate));
            }
            while self
                .history
                .front()
                .is_some_and(|(at, _)| snapshot.at.signed_duration_since(*at).num_seconds() > 3600)
                || self.history.len() > 1801
            {
                self.history.pop_front();
            }
            self.latest = Some(snapshot);
        }
    }
}
impl Drop for Monitor {
    fn drop(&mut self) {
        let _ = self.tx.try_send(Command::Stop);
    }
}
pub type Cancellation = Arc<AtomicBool>;
pub fn check_cancel(cancel: &Cancellation) -> anyhow::Result<()> {
    anyhow::ensure!(!cancel.load(Ordering::Relaxed), "Dibatalkan oleh pengguna");
    Ok(())
}

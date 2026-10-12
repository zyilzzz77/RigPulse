use chrono::{DateTime, Utc};
use std::collections::VecDeque;

#[derive(Clone)]
pub struct RingBuffer {
    cap: usize,
    samples: VecDeque<(DateTime<Utc>, f64, f64)>,
}
impl Default for RingBuffer {
    fn default() -> Self {
        Self::new(3600)
    }
}
impl RingBuffer {
    pub fn new(cap: usize) -> Self {
        Self {
            cap,
            samples: VecDeque::new(),
        }
    }
    pub fn push(&mut self, at: DateTime<Utc>, cpu: f64, ram: f64) {
        if self.cap == 0
            || !cpu.is_finite()
            || !ram.is_finite()
            || !(0.0..=100.0).contains(&cpu)
            || !(0.0..=100.0).contains(&ram)
            || self.samples.back().is_some_and(|s| s.0 >= at)
        {
            return;
        }
        if self.samples.len() == self.cap {
            self.samples.pop_front();
        }
        self.samples.push_back((at, cpu, ram));
    }
    pub fn points(&self, minutes: i64, cpu: bool) -> Vec<[f64; 2]> {
        let Some(last) = self.samples.back() else {
            return Vec::new();
        };
        self.samples
            .iter()
            .filter(|s| (last.0 - s.0).num_seconds() <= minutes * 60)
            .map(|s| {
                [
                    (s.0 - last.0).num_milliseconds() as f64 / 1000.0,
                    if cpu { s.1 } else { s.2 },
                ]
            })
            .collect()
    }
    pub fn stats(&self, minutes: i64, cpu: bool) -> Option<(f64, f64, f64)> {
        let p = self.points(minutes, cpu);
        if p.is_empty() {
            return None;
        }
        Some((
            p.iter().map(|p| p[1]).fold(f64::INFINITY, f64::min),
            p.iter().map(|p| p[1]).sum::<f64>() / p.len() as f64,
            p.iter().map(|p| p[1]).fold(f64::NEG_INFINITY, f64::max),
        ))
    }
    pub fn last_at(&self) -> Option<DateTime<Utc>> {
        self.samples.back().map(|s| s.0)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_and_deduplicated() {
        let now = Utc::now();
        let mut r = RingBuffer::new(2);
        for n in 0..4 {
            r.push(now + chrono::Duration::seconds(n), n as f64, 50.0);
        }
        r.push(now + chrono::Duration::seconds(3), 99.0, 99.0);
        assert_eq!(r.points(5, true).len(), 2);
        assert_eq!(r.stats(5, true), Some((2.0, 2.5, 3.0)));
    }
    #[test]
    fn rejects_invalid_samples() {
        let mut r = RingBuffer::default();
        r.push(Utc::now(), f64::NAN, 0.0);
        r.push(Utc::now(), 101.0, 20.0);
        assert!(r.stats(5, true).is_none());
    }
    #[test]
    fn window_respects_time() {
        let mut r = RingBuffer::default();
        let t = Utc::now();
        r.push(t, 10.0, 10.0);
        r.push(t + chrono::Duration::minutes(6), 20.0, 20.0);
        assert_eq!(r.points(5, true).len(), 1);
        assert_eq!(r.points(15, true).len(), 2);
    }
    #[test]
    fn frozen_view_preserves_live_history_and_selected_statistics() {
        let mut live = RingBuffer::default();
        let start = Utc::now();
        live.push(start, 10.0, 20.0);
        let frozen = live.clone();
        live.push(start + chrono::Duration::minutes(6), 60.0, 80.0);
        assert_eq!(frozen.stats(5, true), Some((10.0, 10.0, 10.0)));
        assert_eq!(live.stats(5, true), Some((60.0, 60.0, 60.0)));
        assert_eq!(live.stats(15, true), Some((10.0, 35.0, 60.0)));
        assert_ne!(frozen.last_at(), live.last_at());
    }
}

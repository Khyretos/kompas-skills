//! Live stats of the machine the server runs on. Inside a container,
//! /proc/stat, /proc/meminfo, /proc/uptime and /proc/loadavg show the host.

use std::{
    collections::{HashMap, VecDeque},
    sync::Mutex,
    time::{Duration, Instant},
};

use serde_json::{Value, json};

use crate::{AppState, events::Event};

const SAMPLES: usize = 60;
/// Never sample faster than this.
const MIN_INTERVAL: Duration = Duration::from_secs(1);

/// Samples only when asked (a GET or a live watcher), at most once a second.
#[derive(Default)]
pub struct HostStats {
    inner: Mutex<Inner>,
    /// Users with the Machines tab open on "Live", until when.
    watchers: Mutex<HashMap<String, Instant>>,
}

#[derive(Default)]
struct Inner {
    /// CPU busy share per sample, 0..1, oldest first.
    history: VecDeque<f64>,
    last: Option<(u64, u64)>,
    last_at: Option<Instant>,
    sampled_at: Option<String>,
}

fn read(path: &str) -> String {
    std::fs::read_to_string(path).unwrap_or_default()
}

/// (busy, total) jiffies from the first "cpu" line of /proc/stat.
fn parse_cpu(stat: &str) -> Option<(u64, u64)> {
    let line = stat.lines().find(|l| l.starts_with("cpu "))?;
    let f: Vec<u64> = line
        .split_whitespace()
        .skip(1)
        .take(8)
        .map(|v| v.parse().ok())
        .collect::<Option<_>>()?;
    if f.len() < 8 {
        return None;
    }
    let total: u64 = f.iter().sum();
    Some((total - f[3] - f[4], total))
}

/// (MemTotal, MemAvailable) in kB.
fn parse_meminfo(text: &str) -> (u64, u64) {
    let get = |key: &str| {
        text.lines()
            .find(|l| l.starts_with(key))
            .and_then(|l| l.split_whitespace().nth(1))
            .and_then(|v| v.parse().ok())
            .unwrap_or(0)
    };
    (get("MemTotal:"), get("MemAvailable:"))
}

fn first_number(text: &str) -> f64 {
    text.split_whitespace()
        .next()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0.0)
}

fn uptime_text(secs: f64) -> String {
    let s = secs as u64;
    let (d, h, m) = (s / 86_400, s % 86_400 / 3_600, s % 3_600 / 60);
    if d > 0 {
        format!("{d} d {h} h")
    } else {
        format!("{h} h {m} min")
    }
}

fn gb(kb: u64) -> f64 {
    (kb as f64 / 1024.0 / 1024.0 * 10.0).round() / 10.0
}

impl HostStats {
    /// Takes a CPU sample unless the last one is less than a second old.
    fn sample(&self) {
        let mut i = self.inner.lock().unwrap();
        if i.last_at.is_some_and(|t| t.elapsed() < MIN_INTERVAL) {
            return;
        }
        let now = parse_cpu(&read("/proc/stat"));
        if let (Some((b0, t0)), Some((b1, t1))) = (i.last, now)
            && t1 > t0
        {
            let share = (b1.saturating_sub(b0)) as f64 / (t1 - t0) as f64;
            i.history.push_back(share.clamp(0.0, 1.0));
            while i.history.len() > SAMPLES {
                i.history.pop_front();
            }
        }
        i.last = now;
        i.last_at = Some(Instant::now());
        i.sampled_at = Some(crate::util::now());
    }

    /// Keeps `user_id` on the live feed for the next 15 seconds.
    pub fn watch(&self, user_id: &str) {
        self.watchers
            .lock()
            .unwrap()
            .insert(user_id.to_string(), Instant::now() + Duration::from_secs(15));
    }

    /// Every second, while anyone watches live: sample and push to them.
    pub fn spawn_live(state: AppState) {
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(MIN_INTERVAL);
            loop {
                tick.tick().await;
                let users: Vec<String> = {
                    let mut w = state.host.watchers.lock().unwrap();
                    w.retain(|_, until| *until > Instant::now());
                    w.keys().cloned().collect()
                };
                if users.is_empty() {
                    continue;
                }
                let machines = vec![state.host.snapshot_now(&state)];
                for u in users {
                    state.bus.send(&u, Event::Machines { machines: machines.clone() });
                }
            }
        });
    }

    /// A fresh sample (rate-limited) as the Machines panel shows it.
    pub fn snapshot_now(&self, state: &AppState) -> Value {
        self.sample();
        let os = std::fs::read_to_string("/host/os-release")
            .ok()
            .and_then(|t| {
                t.lines()
                    .find_map(|l| l.strip_prefix("PRETTY_NAME="))
                    .map(|v| v.trim_matches('"').to_string())
            })
            .unwrap_or_else(|| "Linux".into());
        let name = state.config.machine_name.as_deref().unwrap_or("This server");
        self.snapshot(name, &os)
    }

    pub fn snapshot(&self, name: &str, os: &str) -> Value {
        let (history, sampled_at) = {
            let i = self.inner.lock().unwrap();
            (i.history.iter().copied().collect::<Vec<f64>>(), i.sampled_at.clone())
        };
        let (total, avail) = parse_meminfo(&read("/proc/meminfo"));
        let busy = format!(
            "up {}, load {:.2}",
            uptime_text(first_number(&read("/proc/uptime"))),
            first_number(&read("/proc/loadavg"))
        );
        json!({
            "id": "server",
            "name": name,
            "os": os,
            "online": true,
            "cpu": history.last().copied().unwrap_or(0.0),
            "ramUsedGb": gb(total.saturating_sub(avail)),
            "ramTotalGb": gb(total),
            "gpus": [],
            "kompanionShare": 0.0,
            "busy": busy,
            "history": history,
            "historyKind": "cpu",
            "sampledAt": sampled_at,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cpu_line() {
        let stat = "cpu  100 5 50 800 20 3 2 0 0 0\ncpu0 1 2 3 4 5 6 7 8 0 0\n";
        assert_eq!(parse_cpu(stat), Some((160, 980)));
    }

    #[test]
    fn meminfo() {
        let m = "MemTotal:       65536000 kB\nMemFree: 1 kB\nMemAvailable:   32768000 kB\n";
        assert_eq!(parse_meminfo(m), (65_536_000, 32_768_000));
        assert_eq!(gb(65_536_000), 62.5);
    }
}

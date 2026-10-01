//! Live stats of the machine the server runs on. Inside a container,
//! /proc/stat, /proc/meminfo, /proc/uptime and /proc/loadavg show the host.

use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
    time::Duration,
};

use serde_json::{Value, json};

const SAMPLES: usize = 60;

#[derive(Default)]
pub struct HostStats {
    /// CPU busy share per 5 s sample, 0..1, oldest first.
    history: Mutex<VecDeque<f64>>,
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
    pub fn spawn(self: Arc<Self>) {
        tokio::spawn(async move {
            let mut prev = parse_cpu(&read("/proc/stat"));
            loop {
                tokio::time::sleep(Duration::from_secs(5)).await;
                let now = parse_cpu(&read("/proc/stat"));
                if let (Some((b0, t0)), Some((b1, t1))) = (prev, now)
                    && t1 > t0
                {
                    let share = (b1.saturating_sub(b0)) as f64 / (t1 - t0) as f64;
                    let mut h = self.history.lock().unwrap();
                    h.push_back(share.clamp(0.0, 1.0));
                    while h.len() > SAMPLES {
                        h.pop_front();
                    }
                }
                prev = now;
            }
        });
    }

    pub fn snapshot(&self, name: &str, os: &str) -> Value {
        let history: Vec<f64> = self.history.lock().unwrap().iter().copied().collect();
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

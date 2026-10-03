//! CPU, memory, disk, uptime and load, plus the GPUs, in one snapshot.

use std::{ffi::CString, fs, path::Path, time::Instant};

use serde::{Deserialize, Serialize};

use crate::gpu::{GpuReader, GpuStats};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    /// Busy share since the previous sample, 0..1 (None on the first sample).
    pub cpu: Option<f64>,
    pub cpu_count: u32,
    pub ram_used_gb: f64,
    pub ram_total_gb: f64,
    pub disk_used_gb: f64,
    pub disk_total_gb: f64,
    pub uptime_secs: u64,
    pub load1: f64,
    pub os: String,
    #[serde(default)]
    pub gpus: Vec<GpuStats>,
}

/// Reads snapshots; keeps the previous CPU counters for the busy share.
pub struct Sampler {
    root: String,
    disk: String,
    cpu_prev: Option<(u64, u64)>,
    gpu: GpuReader,
    last: Option<Instant>,
}

impl Sampler {
    /// `root` is where /proc, /sys and os-release live ("/" normally);
    /// `disk` is the mount point whose usage is reported ("/").
    pub fn new(root: &str, disk: &str) -> Self {
        Sampler { root: root.trim_end_matches('/').to_string(), disk: disk.into(), cpu_prev: None, gpu: GpuReader::default(), last: None }
    }

    /// Seconds since the previous sample, if any.
    pub fn since_last(&self) -> Option<f64> {
        self.last.map(|t| t.elapsed().as_secs_f64())
    }

    pub fn sample(&mut self) -> Snapshot {
        let p = |rel: &str| format!("{}/{rel}", self.root);
        let stat = fs::read_to_string(p("proc/stat")).unwrap_or_default();
        let now = parse_cpu(&stat);
        let cpu = match (self.cpu_prev, now) {
            (Some((b0, t0)), Some((b1, t1))) if t1 > t0 => Some((b1.saturating_sub(b0) as f64 / (t1 - t0) as f64).clamp(0.0, 1.0)),
            _ => None,
        };
        self.cpu_prev = now;
        self.last = Some(Instant::now());
        let cpu_count = stat.lines().filter(|l| l.starts_with("cpu") && !l.starts_with("cpu ")).count() as u32;
        let (total, avail) = parse_meminfo(&fs::read_to_string(p("proc/meminfo")).unwrap_or_default());
        let (disk_used, disk_total) = disk_usage(&self.disk);
        Snapshot {
            cpu,
            cpu_count,
            ram_used_gb: gb(total.saturating_sub(avail) * 1024),
            ram_total_gb: gb(total * 1024),
            disk_used_gb: gb(disk_used),
            disk_total_gb: gb(disk_total),
            uptime_secs: first_number(&fs::read_to_string(p("proc/uptime")).unwrap_or_default()) as u64,
            load1: first_number(&fs::read_to_string(p("proc/loadavg")).unwrap_or_default()),
            // Inside a container, the host's os-release is mounted at /host/os-release.
            os: os_name(&p("host/os-release")).or_else(|| os_name(&p("etc/os-release"))).unwrap_or_else(|| "Linux".into()),
            gpus: self.gpu.read(Path::new(&p("sys"))),
        }
    }
}

/// (busy, total) jiffies from the "cpu" line of /proc/stat.
pub fn parse_cpu(stat: &str) -> Option<(u64, u64)> {
    let line = stat.lines().find(|l| l.starts_with("cpu "))?;
    let f: Vec<u64> = line.split_whitespace().skip(1).take(8).map(|v| v.parse().ok()).collect::<Option<_>>()?;
    if f.len() < 8 {
        return None;
    }
    let total: u64 = f.iter().sum();
    Some((total - f[3] - f[4], total))
}

/// (MemTotal, MemAvailable) in kB.
pub fn parse_meminfo(text: &str) -> (u64, u64) {
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
    text.split_whitespace().next().and_then(|v| v.parse().ok()).unwrap_or(0.0)
}

fn gb(bytes: u64) -> f64 {
    (bytes as f64 / 1024f64.powi(3) * 10.0).round() / 10.0
}

fn os_name(path: &str) -> Option<String> {
    fs::read_to_string(path)
        .ok()?
        .lines()
        .find_map(|l| l.strip_prefix("PRETTY_NAME="))
        .map(|v| v.trim_matches('"').to_string())
}

/// (used, total) bytes of the filesystem holding `path`.
fn disk_usage(path: &str) -> (u64, u64) {
    let Ok(c) = CString::new(path) else { return (0, 0) };
    let mut s: libc::statvfs = unsafe { std::mem::zeroed() };
    // SAFETY: valid C string and a zeroed out-struct of the right type.
    if unsafe { libc::statvfs(c.as_ptr(), &mut s) } != 0 {
        return (0, 0);
    }
    let total = s.f_blocks as u64 * s.f_frsize as u64;
    let free = s.f_bfree as u64 * s.f_frsize as u64;
    (total.saturating_sub(free), total)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_proc() {
        assert_eq!(parse_cpu("cpu  100 5 50 800 20 3 2 0 0 0\ncpu0 1 2 3 4 5 6 7 8\n"), Some((160, 980)));
        assert_eq!(parse_meminfo("MemTotal: 65536000 kB\nMemAvailable: 32768000 kB\n"), (65_536_000, 32_768_000));
    }

    #[test]
    fn samples_this_machine_and_round_trips() {
        let mut s = Sampler::new("/", "/");
        s.sample();
        std::thread::sleep(std::time::Duration::from_millis(300));
        let snap = s.sample();
        assert!(snap.cpu.is_some() && snap.ram_total_gb > 0.0 && snap.disk_total_gb > 0.0);
        let back: Snapshot = serde_json::from_str(&serde_json::to_string(&snap).unwrap()).unwrap();
        assert_eq!(back.gpus.len(), snap.gpus.len());
    }
}

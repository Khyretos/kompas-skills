//! Per-engine GPU busy share from DRM client stats in /proc/<pid>/fdinfo
//! (render/gfx, compute, video encode/decode, copy). Only processes we may
//! read are counted: for the runner, the desktop user's own apps.

use std::{collections::HashMap, fs, path::Path, time::Instant};

type Sums = HashMap<(String, String), u64>; // (pdev, engine) -> ns

#[derive(Default)]
pub struct EngineReader {
    prev: Option<(Instant, Sums)>,
}

/// One fdinfo text: (pdev, client id, [(engine, ns)]) for DRM clients.
pub fn parse(text: &str) -> Option<(String, String, Vec<(String, u64)>)> {
    let (mut pdev, mut client) = (None, None);
    let mut engines = Vec::new();
    for line in text.lines() {
        let Some((key, value)) = line.split_once(':') else { continue };
        let value = value.trim();
        match key.trim() {
            "drm-pdev" => pdev = Some(value.to_string()),
            "drm-client-id" => client = Some(value.to_string()),
            k => {
                if let Some(engine) = k.strip_prefix("drm-engine-")
                    && let Some(ns) = value.strip_suffix("ns").and_then(|n| n.trim().parse::<u64>().ok())
                {
                    engines.push((engine.to_string(), ns));
                }
            }
        }
    }
    Some((pdev?, client?, engines))
}

impl EngineReader {
    /// pci slot -> [(engine, busy 0..1)], sorted by engine. Empty on the first call.
    pub fn read(&mut self, proc_root: &Path) -> HashMap<String, Vec<(String, f64)>> {
        let mut seen = std::collections::HashSet::new();
        let mut sums: Sums = HashMap::new();
        let Ok(pids) = fs::read_dir(proc_root) else { return HashMap::new() };
        for pid in pids.flatten() {
            if !pid.file_name().to_string_lossy().chars().all(|c| c.is_ascii_digit()) {
                continue;
            }
            let Ok(fds) = fs::read_dir(pid.path().join("fdinfo")) else { continue };
            for fd in fds.flatten() {
                let Ok(text) = fs::read_to_string(fd.path()) else { continue };
                let Some((pdev, client, engines)) = parse(&text) else { continue };
                // Several fds (even in other processes) can share one client.
                if !seen.insert((pdev.clone(), client)) {
                    continue;
                }
                for (engine, ns) in engines {
                    *sums.entry((pdev.clone(), engine)).or_default() += ns;
                }
            }
        }
        let now = Instant::now();
        let mut out: HashMap<String, Vec<(String, f64)>> = HashMap::new();
        if let Some((then, prev)) = &self.prev {
            let elapsed = now.duration_since(*then).as_nanos() as f64;
            if elapsed > 0.0 {
                for ((pdev, engine), ns) in &sums {
                    // A client that exited makes a sum go down: skip that sample.
                    let Some(before) = prev.get(&(pdev.clone(), engine.clone())) else { continue };
                    if ns < before {
                        continue;
                    }
                    let share = ((ns - before) as f64 / elapsed).clamp(0.0, 1.0);
                    out.entry(pdev.clone()).or_default().push((engine.clone(), share));
                }
            }
        }
        for v in out.values_mut() {
            v.sort_by(|a, b| a.0.cmp(&b.0));
        }
        self.prev = Some((now, sums));
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "drm-driver:\tamdgpu\ndrm-pdev:\t0000:03:00.0\ndrm-client-id:\t17\ndrm-engine-gfx:\t123456789 ns\ndrm-engine-enc:\t5000 ns\ndrm-memory-vram:\t1048576 KiB\n";

    #[test]
    fn parses_pdev_with_colons_and_ns_values() {
        let (pdev, client, engines) = parse(SAMPLE).unwrap();
        assert_eq!((pdev.as_str(), client.as_str()), ("0000:03:00.0", "17"));
        assert!(engines.contains(&("gfx".to_string(), 123_456_789)));
        assert_eq!(engines.len(), 2);
        assert!(parse("pos: 0\nflags: 02\n").is_none());
    }

    #[test]
    fn counts_each_client_once_and_computes_busy_share() {
        let root = std::env::temp_dir().join(format!("kk-fdinfo-{}", std::process::id()));
        let fdinfo = root.join("1234/fdinfo");
        fs::create_dir_all(&fdinfo).unwrap();
        fs::create_dir_all(root.join("self")).unwrap(); // not numeric: ignored
        let write = |gfx: u64| {
            let t = format!("drm-pdev:\t0000:03:00.0\ndrm-client-id:\t17\ndrm-engine-gfx:\t{gfx} ns\n");
            fs::write(fdinfo.join("5"), &t).unwrap();
            fs::write(fdinfo.join("6"), &t).unwrap(); // same client again
        };
        let mut r = EngineReader::default();
        write(1_000_000_000);
        assert!(r.read(&root).is_empty());
        std::thread::sleep(std::time::Duration::from_millis(100));
        write(1_050_000_000); // +50 ms of gfx time
        let out = r.read(&root);
        let gfx = out["0000:03:00.0"][0].1;
        assert!((0.1..0.9).contains(&gfx), "{gfx}");
        let _ = fs::remove_dir_all(&root);
    }
}

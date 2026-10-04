//! Per-GPU usage from DRM client stats in /proc/<pid>/fdinfo: busy share per
//! engine (render, compute, video, video-enhance, copy) and VRAM in use. Only
//! processes we may read are counted (same user, or the root helper).
//!
//! amdgpu and i915 report engine time in ns (`drm-engine-<name>: N ns`); xe
//! reports cycles (`drm-cycles-<class>: N` with `drm-total-cycles-<class>: M`).

use std::{collections::HashMap, fs, path::Path, time::Instant};

/// One DRM client's numbers from one fdinfo file.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct ClientStats {
    pub pdev: String,
    pub client_id: String,
    pub engines_ns: Vec<(String, u64)>,
    /// (class, cycles, total cycles), xe only.
    pub cycles: Vec<(String, u64, u64)>,
    pub vram_bytes: Option<u64>,
}

/// What the reader returns per PCI slot.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct GpuUsage {
    /// (engine, busy 0..1), sorted by engine; empty on the first read.
    pub engines: Vec<(String, f64)>,
    pub vram_used_bytes: Option<u64>,
}

fn xe_class(class: &str) -> &str {
    match class {
        "rcs" => "render",
        "bcs" => "copy",
        "vcs" => "video",
        "vecs" => "video-enhance",
        "ccs" => "compute",
        other => other,
    }
}

/// "1048576 KiB" / "512 MiB" / "3 GiB" / "123 B" / "123" -> bytes.
fn bytes(value: &str) -> Option<u64> {
    let mut it = value.split_whitespace();
    let n: u64 = it.next()?.parse().ok()?;
    let mult = match it.next().unwrap_or("B") {
        "B" => 1,
        "KiB" => 1 << 10,
        "MiB" => 1 << 20,
        "GiB" => 1 << 30,
        _ => return None,
    };
    n.checked_mul(mult)
}

pub fn parse(text: &str) -> Option<ClientStats> {
    let (mut pdev, mut client) = (None, None);
    let mut c = ClientStats::default();
    let (mut cycles, mut totals): (HashMap<String, u64>, HashMap<String, u64>) = Default::default();
    let mut vram: HashMap<&str, u64> = HashMap::new();
    for line in text.lines() {
        let Some((key, value)) = line.split_once(':') else { continue };
        let (key, value) = (key.trim(), value.trim());
        match key {
            "drm-pdev" => pdev = Some(value.to_string()),
            "drm-client-id" => client = Some(value.to_string()),
            "drm-total-vram0" | "drm-resident-vram0" | "drm-total-local0" | "drm-resident-local0" | "drm-memory-vram" => {
                if let Some(b) = bytes(value) {
                    vram.insert(key, b);
                }
            }
            k => {
                if let Some(engine) = k.strip_prefix("drm-engine-")
                    && let Some(ns) = value.strip_suffix("ns").and_then(|n| n.trim().parse().ok())
                {
                    c.engines_ns.push((engine.to_string(), ns));
                } else if let Some(class) = k.strip_prefix("drm-total-cycles-")
                    && let Ok(n) = value.parse()
                {
                    totals.insert(class.to_string(), n);
                } else if let Some(class) = k.strip_prefix("drm-cycles-")
                    && let Ok(n) = value.parse()
                {
                    cycles.insert(class.to_string(), n);
                }
            }
        }
    }
    for (class, n) in cycles {
        if let Some(t) = totals.get(&class) {
            c.cycles.push((class, n, *t));
        }
    }
    c.cycles.sort();
    // Prefer "total" (allocated) over "resident"; amdgpu has only drm-memory-vram.
    c.vram_bytes = ["drm-total-vram0", "drm-total-local0", "drm-resident-vram0", "drm-resident-local0", "drm-memory-vram"]
        .iter()
        .find_map(|k| vram.get(k).copied());
    c.pdev = pdev?;
    c.client_id = client?;
    Some(c)
}

#[derive(Default)]
struct Sample {
    ns: HashMap<(String, String), u64>,            // (pdev, engine) -> ns
    cycles: HashMap<(String, String), (u64, u64)>, // (pdev, engine) -> (sum cycles, max total)
}

#[derive(Default)]
pub struct EngineReader {
    prev: Option<(Instant, Sample)>,
}

impl EngineReader {
    pub fn read(&mut self, proc_root: &Path) -> HashMap<String, GpuUsage> {
        let mut seen = std::collections::HashSet::new();
        let mut now_s = Sample::default();
        let mut out: HashMap<String, GpuUsage> = HashMap::new();
        let Ok(pids) = fs::read_dir(proc_root) else { return out };
        for pid in pids.flatten() {
            if !pid.file_name().to_string_lossy().chars().all(|c| c.is_ascii_digit()) {
                continue;
            }
            let Ok(fds) = fs::read_dir(pid.path().join("fdinfo")) else { continue };
            for fd in fds.flatten() {
                let link = pid.path().join("fd").join(fd.file_name());
                if !fs::read_link(&link).is_ok_and(|t| t.starts_with("/dev/dri")) {
                    continue;
                }
                let Ok(text) = fs::read_to_string(fd.path()) else { continue };
                let Some(c) = parse(&text) else { continue };
                // Several fds (even in other processes) can share one client.
                if !seen.insert((c.pdev.clone(), c.client_id.clone())) {
                    continue;
                }
                if let Some(b) = c.vram_bytes {
                    *out.entry(c.pdev.clone()).or_default().vram_used_bytes.get_or_insert(0) += b;
                }
                for (engine, ns) in c.engines_ns {
                    *now_s.ns.entry((c.pdev.clone(), engine)).or_default() += ns;
                }
                for (class, n, total) in c.cycles {
                    let e = now_s.cycles.entry((c.pdev.clone(), xe_class(&class).to_string())).or_default();
                    e.0 += n;
                    e.1 = e.1.max(total); // the GPU clock: the same for every client
                }
            }
        }
        let now = Instant::now();
        if let Some((then, prev)) = &self.prev {
            let elapsed = now.duration_since(*then).as_nanos() as f64;
            for ((pdev, engine), ns) in &now_s.ns {
                if let Some(before) = prev.ns.get(&(pdev.clone(), engine.clone()))
                    && ns >= before
                    && elapsed > 0.0
                {
                    let share = ((ns - before) as f64 / elapsed).clamp(0.0, 1.0);
                    out.entry(pdev.clone()).or_default().engines.push((engine.clone(), share));
                }
            }
            for ((pdev, engine), (n, total)) in &now_s.cycles {
                if let Some((n0, t0)) = prev.cycles.get(&(pdev.clone(), engine.clone()))
                    && n >= n0
                    && total > t0
                {
                    let share = ((n - n0) as f64 / (total - t0) as f64).clamp(0.0, 1.0);
                    out.entry(pdev.clone()).or_default().engines.push((engine.clone(), share));
                }
            }
        }
        for u in out.values_mut() {
            u.engines.sort_by(|a, b| a.0.cmp(&b.0));
        }
        self.prev = Some((now, now_s));
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const AMD: &str = "drm-driver:\tamdgpu\ndrm-pdev:\t0000:03:00.0\ndrm-client-id:\t17\ndrm-engine-gfx:\t123456789 ns\ndrm-engine-enc:\t5000 ns\ndrm-memory-vram:\t1048576 KiB\n";
    const XE: &str = "drm-driver:\txe\ndrm-pdev:\t0000:10:00.0\ndrm-client-id:\t4\ndrm-total-vram0:\t2048 KiB\ndrm-resident-vram0:\t1024 KiB\ndrm-cycles-rcs:\t1000\ndrm-total-cycles-rcs:\t5000\n";

    #[test]
    fn parses_amd_and_xe() {
        let a = parse(AMD).unwrap();
        assert_eq!((a.pdev.as_str(), a.client_id.as_str()), ("0000:03:00.0", "17"));
        assert!(a.engines_ns.contains(&("gfx".to_string(), 123_456_789)));
        assert_eq!(a.vram_bytes, Some(1 << 30));
        let x = parse(XE).unwrap();
        assert_eq!(x.cycles, vec![("rcs".to_string(), 1000, 5000)]);
        assert_eq!(x.vram_bytes, Some(2048 * 1024));
        assert!(parse("pos: 0\nflags: 02\n").is_none());
    }

    fn tree(name: &str) -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!("kk-fdinfo-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("1234/fd")).unwrap();
        fs::create_dir_all(root.join("1234/fdinfo")).unwrap();
        fs::create_dir_all(root.join("self")).unwrap(); // not numeric: ignored
        root
    }

    #[test]
    fn ns_engines_count_each_client_once() {
        let root = tree("ns");
        let fdinfo = root.join("1234/fdinfo");
        let write = |gfx: u64| {
            let t = format!("drm-pdev:\t0000:03:00.0\ndrm-client-id:\t17\ndrm-engine-gfx:\t{gfx} ns\n");
            fs::write(fdinfo.join("5"), &t).unwrap();
            fs::write(fdinfo.join("6"), &t).unwrap(); // same client again
        };
        std::os::unix::fs::symlink("/dev/dri/renderD128", root.join("1234/fd/5")).unwrap();
        std::os::unix::fs::symlink("/dev/dri/renderD128", root.join("1234/fd/6")).unwrap();
        let mut r = EngineReader::default();
        write(1_000_000_000);
        assert!(r.read(&root).values().all(|u| u.engines.is_empty()));
        std::thread::sleep(std::time::Duration::from_millis(100));
        write(1_050_000_000); // +50 ms of gfx time
        let gfx = r.read(&root)["0000:03:00.0"].engines[0].1;
        assert!((0.1..0.9).contains(&gfx), "{gfx}");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn xe_cycles_and_vram() {
        let root = tree("xe");
        let f = root.join("1234/fdinfo/7");
        std::os::unix::fs::symlink("/dev/dri/renderD129", root.join("1234/fd/7")).unwrap();
        let mut r = EngineReader::default();
        fs::write(&f, XE).unwrap();
        let first = r.read(&root);
        assert_eq!(first["0000:10:00.0"].vram_used_bytes, Some(2048 * 1024));
        fs::write(&f, XE.replace("cycles-rcs:\t1000", "cycles-rcs:\t3000").replace("total-cycles-rcs:\t5000", "total-cycles-rcs:\t9000")).unwrap();
        let second = r.read(&root);
        assert_eq!(second["0000:10:00.0"].engines, vec![("render".to_string(), 0.5)]);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn skips_non_drm_fds() {
        let root = tree("skip");
        fs::write(root.join("1234/fdinfo/8"), AMD).unwrap();
        std::os::unix::fs::symlink("/tmp/not-a-gpu", root.join("1234/fd/8")).unwrap();
        let mut r = EngineReader::default();
        r.read(&root);
        assert!(!r.read(&root).contains_key("0000:03:00.0"));
        let _ = fs::remove_dir_all(&root);
    }
}

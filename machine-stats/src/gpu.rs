//! GPU stats from sysfs and hwmon, the way LACT reads them, without root.
//! amdgpu exposes busy %, VRAM, power, clocks and temperature directly.
//! Intel (i915, xe) exposes clocks, temperature and an energy counter; busy
//! share comes from the idle (RC6) residency counter between two reads.

use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    time::Instant,
};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GpuStats {
    /// e.g. "Intel Arc A770"
    pub name: String,
    /// e.g. "0000:10:00.0"
    pub pci_slot: String,
    /// "i915", "xe", "amdgpu", ...
    pub driver: String,
    /// 0..1
    pub load: Option<f64>,
    pub vram_used_gb: Option<f64>,
    pub vram_total_gb: Option<f64>,
    pub watts: Option<f64>,
    pub temp_c: Option<f64>,
    pub core_mhz: Option<f64>,
    pub mem_mhz: Option<f64>,
    pub fan_rpm: Option<f64>,
    /// Highest core clock, for the clock bar.
    #[serde(default)]
    pub core_max_mhz: Option<f64>,
    /// Power limit in W, for the power bar.
    #[serde(default)]
    pub power_cap_w: Option<f64>,
    /// Busy share per engine (from DRM fdinfo), only engines we could read.
    #[serde(default)]
    pub engines: Vec<Engine>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Engine {
    pub name: String,
    pub busy: f64,
}

/// Keeps the previous counters per GPU to turn them into rates.
#[derive(Default)]
pub struct GpuReader {
    prev: HashMap<String, Prev>,
}

#[derive(Clone, Copy)]
struct Prev {
    at: Instant,
    energy_uj: Option<f64>,
    idle_ms: Option<f64>,
}

fn num(p: &Path) -> Option<f64> {
    fs::read_to_string(p).ok()?.trim().parse().ok()
}

fn text(p: &Path) -> Option<String> {
    Some(fs::read_to_string(p).ok()?.trim().to_string())
}

fn link_name(p: &Path) -> Option<String> {
    Some(fs::read_link(p).ok()?.file_name()?.to_string_lossy().into_owned())
}

/// First `hwmonN` directory of a device.
fn hwmon(dev: &Path) -> Option<PathBuf> {
    let mut dirs: Vec<PathBuf> = fs::read_dir(dev.join("hwmon")).ok()?.flatten().map(|e| e.path()).collect();
    dirs.sort();
    dirs.into_iter().next()
}

/// `<kind>N_input` whose `<kind>N_label` is one of `labels` (in that order),
/// else `<kind>1_input`.
fn labelled(hw: &Path, kind: &str, labels: &[&str]) -> Option<f64> {
    for want in labels {
        for n in 1..=8 {
            if text(&hw.join(format!("{kind}{n}_label"))).as_deref() == Some(*want) {
                return num(&hw.join(format!("{kind}{n}_input")));
            }
        }
    }
    num(&hw.join(format!("{kind}1_input")))
}

const GIB: f64 = 1024.0 * 1024.0 * 1024.0;

/// Size of the largest PCI memory region of a device, in bytes. On Intel Arc
/// cards with resizable BAR this is the VRAM size (sysfs has no VRAM total
/// for i915/xe on this kernel). Lines of `resource`: "start end flags" in hex.
fn largest_bar(dev: &Path) -> Option<u64> {
    let text = fs::read_to_string(dev.join("resource")).ok()?;
    text.lines()
        .filter_map(|l| {
            let mut it = l.split_whitespace().map(|v| u64::from_str_radix(v.trim_start_matches("0x"), 16).ok());
            let (start, end) = (it.next()??, it.next()??);
            (end > start).then(|| end - start + 1)
        })
        .max()
}

fn name(vendor: &str, device: &str) -> String {
    match (vendor, device) {
        ("0x8086", "0x56a0") => "Intel Arc A770".into(),
        ("0x8086", "0x56a1") => "Intel Arc A750".into(),
        ("0x8086", "0x56a2") => "Intel Arc A580".into(),
        ("0x1002", "0x7550") => "AMD Radeon RX 9070 XT".into(),
        _ => {
            let v = match vendor {
                "0x8086" => "Intel",
                "0x1002" => "AMD",
                "0x10de" => "NVIDIA",
                _ => "GPU",
            };
            format!("{v} {device}")
        }
    }
}

impl GpuReader {
    /// One entry per `class/drm/cardN` under `sys` (normally `/sys`).
    pub fn read(&mut self, sys: &Path) -> Vec<GpuStats> {
        let drm = sys.join("class/drm");
        let Ok(entries) = fs::read_dir(&drm) else {
            return Vec::new();
        };
        let mut cards: Vec<(String, PathBuf)> = entries
            .flatten()
            .filter_map(|e| {
                let n = e.file_name().to_string_lossy().into_owned();
                // "card1" yes, "card1-DP-1" (a connector) no.
                let digits = n.strip_prefix("card")?;
                (!digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit())).then(|| (n, e.path()))
            })
            .collect();
        cards.sort();
        cards.into_iter().filter_map(|(_, card)| self.read_card(&card)).collect()
    }

    fn read_card(&mut self, card: &Path) -> Option<GpuStats> {
        let dev = card.join("device");
        let driver = link_name(&dev.join("driver")).unwrap_or_else(|| "unknown".into());
        let pci_slot = link_name(&dev).unwrap_or_default();
        let vendor = text(&dev.join("vendor")).unwrap_or_default();
        let device = text(&dev.join("device")).unwrap_or_default();
        let hw = hwmon(&dev);
        let hwn = |file: &str| hw.as_ref().and_then(|h| num(&h.join(file)));

        let mut s = GpuStats {
            name: name(&vendor, &device),
            pci_slot,
            driver: driver.clone(),
            load: None,
            vram_used_gb: None,
            vram_total_gb: None,
            watts: None,
            temp_c: None,
            core_mhz: None,
            mem_mhz: None,
            fan_rpm: hwn("fan1_input"),
            core_max_mhz: None,
            power_cap_w: None,
            engines: Vec::new(),
        };

        let (energy_uj, idle_ms) = match driver.as_str() {
            "amdgpu" => {
                s.load = num(&dev.join("gpu_busy_percent")).map(|p| p / 100.0);
                s.vram_used_gb = num(&dev.join("mem_info_vram_used")).map(|b| b / GIB);
                s.vram_total_gb = num(&dev.join("mem_info_vram_total")).map(|b| b / GIB);
                s.watts = hwn("power1_average").or_else(|| hwn("power1_input")).map(|uw| uw / 1e6);
                s.temp_c = hwn("temp1_input").map(|m| m / 1000.0);
                s.core_mhz = hwn("freq1_input").map(|hz| hz / 1e6);
                s.mem_mhz = hwn("freq2_input").map(|hz| hz / 1e6);
                s.power_cap_w = hwn("power1_cap").map(|uw| uw / 1e6);
                // "2: 2450Mhz *" -> 2450, the last (highest) level.
                s.core_max_mhz = text(&dev.join("pp_dpm_sclk")).and_then(|t| {
                    t.lines().filter_map(|l| l.split_whitespace().nth(1)?.trim_end_matches("Mhz").parse::<f64>().ok()).reduce(f64::max)
                });
                (None, None)
            }
            "i915" => {
                s.core_mhz = num(&card.join("gt_act_freq_mhz"));
                s.vram_total_gb = largest_bar(&dev).filter(|b| *b >= 1 << 30).map(|b| (b as f64 / GIB).round());
                s.core_max_mhz = num(&card.join("gt_max_freq_mhz"));
                s.power_cap_w = hwn("power1_max").map(|uw| uw / 1e6).filter(|w| *w > 0.0);
                s.temp_c = hwn("temp1_input").map(|m| m / 1000.0);
                (hwn("energy1_input"), num(&card.join("gt/gt0/rc6_residency_ms")))
            }
            "xe" => {
                s.core_mhz = num(&dev.join("tile0/gt0/freq0/act_freq"));
                s.vram_total_gb = largest_bar(&dev).filter(|b| *b >= 1 << 30).map(|b| (b as f64 / GIB).round());
                s.core_max_mhz = num(&dev.join("tile0/gt0/freq0/max_freq"));
                s.power_cap_w = hw.as_deref().and_then(|h| {
                    (1..=4).find_map(|n| num(&h.join(format!("power{n}_max")))).map(|uw| uw / 1e6).filter(|w| *w > 0.0)
                });
                let h = hw.as_deref();
                s.temp_c = h.and_then(|h| labelled(h, "temp", &["pkg"])).map(|m| m / 1000.0);
                (
                    h.and_then(|h| labelled(h, "energy", &["card", "pkg"])),
                    num(&dev.join("tile0/gt0/gtidle/idle_residency_ms")),
                )
            }
            _ => (None, None),
        };

        // Rates need two reads; the first read leaves them empty.
        if energy_uj.is_some() || idle_ms.is_some() {
            let now = Instant::now();
            let key = format!("{}/{}", s.pci_slot, driver);
            if let Some(p) = self.prev.get(&key) {
                let secs = now.duration_since(p.at).as_secs_f64();
                if secs > 0.05 {
                    if let (Some(e1), Some(e0)) = (energy_uj, p.energy_uj)
                        && e1 >= e0
                    {
                        s.watts = Some((e1 - e0) / 1e6 / secs);
                    }
                    if let (Some(i1), Some(i0)) = (idle_ms, p.idle_ms)
                        && i1 >= i0
                    {
                        s.load = Some((1.0 - (i1 - i0) / (secs * 1000.0)).clamp(0.0, 1.0));
                    }
                }
            }
            self.prev.insert(key, Prev { at: now, energy_uj, idle_ms });
        }
        Some(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;

    fn tree(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("kk-gpu-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        root
    }

    #[test]
    fn reads_an_amd_card_and_skips_connectors() {
        let sys = tree("amd");
        let pci = sys.join("devices/pci0000:00/0000:03:00.0");
        let drv = sys.join("bus/pci/drivers/amdgpu");
        let hw = pci.join("hwmon/hwmon4");
        fs::create_dir_all(&hw).unwrap();
        fs::create_dir_all(&drv).unwrap();
        fs::create_dir_all(sys.join("class/drm/card0")).unwrap();
        fs::create_dir_all(sys.join("class/drm/card0-DP-1")).unwrap();
        symlink(&pci, sys.join("class/drm/card0/device")).unwrap();
        symlink(&drv, pci.join("driver")).unwrap();
        for (f, v) in [
            ("vendor", "0x1002"),
            ("device", "0x7550"),
            ("gpu_busy_percent", "37"),
            ("mem_info_vram_used", "2147483648"),
            ("mem_info_vram_total", "17179869184"),
        ] {
            fs::write(pci.join(f), v).unwrap();
        }
        fs::write(hw.join("power1_average"), "212000000").unwrap();
        fs::write(hw.join("temp1_input"), "61000").unwrap();
        fs::write(hw.join("freq1_input"), "2450000000").unwrap();

        let g = GpuReader::default().read(&sys);
        assert_eq!(g.len(), 1);
        let g = &g[0];
        assert_eq!(g.name, "AMD Radeon RX 9070 XT");
        assert_eq!(g.driver, "amdgpu");
        assert_eq!(g.pci_slot, "0000:03:00.0");
        assert_eq!(g.load, Some(0.37));
        assert_eq!(g.vram_total_gb, Some(16.0));
        assert_eq!(g.watts, Some(212.0));
        assert_eq!(g.temp_c, Some(61.0));
        assert_eq!(g.core_mhz, Some(2450.0));
        let _ = fs::remove_dir_all(&sys);
    }

    #[test]
    fn intel_rates_need_two_reads() {
        let sys = tree("xe");
        let pci = sys.join("devices/0000:10:00.0");
        let drv = sys.join("drivers/xe");
        let hw = pci.join("hwmon/hwmon2");
        fs::create_dir_all(pci.join("tile0/gt0/gtidle")).unwrap();
        fs::create_dir_all(&hw).unwrap();
        fs::create_dir_all(&drv).unwrap();
        fs::create_dir_all(sys.join("class/drm/card2")).unwrap();
        symlink(&pci, sys.join("class/drm/card2/device")).unwrap();
        symlink(&drv, pci.join("driver")).unwrap();
        fs::write(pci.join("vendor"), "0x8086").unwrap();
        fs::write(pci.join("device"), "0x56a0").unwrap();
        fs::write(hw.join("energy2_label"), "pkg").unwrap();
        fs::write(hw.join("energy2_input"), "1000000").unwrap();
        fs::write(pci.join("tile0/gt0/gtidle/idle_residency_ms"), "0").unwrap();

        let mut r = GpuReader::default();
        let first = r.read(&sys);
        assert_eq!(first[0].name, "Intel Arc A770");
        assert_eq!(first[0].watts, None);
        assert_eq!(first[0].load, None);
        std::thread::sleep(std::time::Duration::from_millis(200));
        fs::write(hw.join("energy2_input"), "11000000").unwrap(); // +10 J
        fs::write(pci.join("tile0/gt0/gtidle/idle_residency_ms"), "0").unwrap(); // never idle
        let second = r.read(&sys);
        assert!(second[0].watts.unwrap() > 10.0, "{:?}", second[0].watts);
        assert_eq!(second[0].load, Some(1.0));
        let _ = fs::remove_dir_all(&sys);
    }

    #[test]
    fn largest_bar_is_the_vram_window() {
        let dir = tree("bar");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("resource"),
            "0x00000000fb000000 0x00000000fbffffff 0x0000000000040200\n0x0000006000000000 0x00000063ffffffff 0x000000000014220c\n0x0000000000000000 0x0000000000000000 0x0000000000000000\n").unwrap();
        assert_eq!(largest_bar(&dir), Some(16 << 30));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn reads_this_machine_without_panicking() {
        let mut r = GpuReader::default();
        let _ = r.read(Path::new("/sys"));
        std::thread::sleep(std::time::Duration::from_millis(300));
        for g in r.read(Path::new("/sys")) {
            eprintln!("{g:?}");
        }
    }
}

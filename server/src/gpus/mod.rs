//! M6-01, the GPU ledger: per GPU, what is loaded (model or app), what it may grow to,
//! what else uses VRAM, and what is free. Every later scheduling decision reads this.
//! Probes run every 10 s; a change is sent live ("gpus"), samples are kept 24 h for the
//! trace timeline (M6-04).
pub mod jobs;
pub mod ledger;
pub mod sched;
pub mod role_policy;
pub mod role;
pub mod timeline;

use std::{collections::HashMap, sync::Mutex, time::Duration};

use axum::{Json, extract::State};
use serde_json::Value;

use crate::{AppState, config::{GpuConfig, HolderConfig}, error::ApiResult, events::Event, util};
use ledger::{GpuLedger, Holding};

const PROBE_TIMEOUT: Duration = Duration::from_secs(3);
const SAMPLE_EVERY: Duration = Duration::from_secs(10);

/// The last ledger, for GET /api/gpus between samples.
static LAST: Mutex<Option<Vec<GpuLedger>>> = Mutex::new(None);

async fn get_json(s: &AppState, url: &str) -> Option<Value> {
    let mut r = s.http.get(url).timeout(PROBE_TIMEOUT);
    if let Some(k) = std::env::var("OVMS_API_KEY").ok().filter(|k| !k.is_empty()) {
        if url.contains("ovms") {
            r = r.bearer_auth(k);
        }
    }
    let resp = r.send().await.ok()?;
    if !resp.status().is_success() {
        return None;
    }
    resp.json().await.ok()
}

fn holding(h: &HolderConfig, name: String, now_mib: u64, busy: bool) -> Holding {
    Holding { name, kind: h.kind.clone(), now_mib, peak_mib: h.peak_mb(), busy }
}

/// What one configured holder holds right now (none when it is stopped or unreachable).
async fn probe(s: &AppState, h: &HolderConfig) -> Vec<Holding> {
    let Some((kind, url)) = h.probe.split_once(':') else { return Vec::new() };
    let url = url.trim_end_matches('/');
    match kind {
        // OVMS can't say how much VRAM each model holds: a loaded model counts at its peak.
        "ovms" => {
            let Some(v) = get_json(s, &format!("{url}/v1/config")).await else { return Vec::new() };
            let want = h.model.as_deref().unwrap_or(&h.name);
            ledger::ovms_models(&v)
                .into_iter()
                .filter(|(m, loaded)| *loaded && m == want)
                .map(|_| holding(h, h.name.clone(), h.peak_mb(), false))
                .collect()
        }
        "ollama" => {
            let Some(v) = get_json(s, &format!("{url}/api/ps")).await else { return Vec::new() };
            let want = h.model.as_deref().unwrap_or("*");
            ledger::ollama_models(&v)
                .into_iter()
                .filter(|(m, _)| want == "*" || m == want)
                .map(|(m, mib)| holding(h, if want == "*" { format!("{} ({m})", h.name) } else { h.name.clone() }, mib, false))
                .collect()
        }
        "comfyui" => match get_json(s, &format!("{url}/system_stats")).await.and_then(|v| ledger::comfy_vram(&v)) {
            Some(mib) => vec![holding(h, h.name.clone(), mib, false)],
            None => Vec::new(),
        },
        "studio" => match get_json(s, &format!("{url}/health")).await.map(|v| ledger::studio_health(&v)) {
            Some((true, busy)) => vec![holding(h, h.name.clone(), h.peak_mb(), busy)],
            _ => Vec::new(),
        },
        _ => Vec::new(),
    }
}

/// A measured GPU: machine name, PCI slot (lower case), used and total MiB, watts.
type Measured = (String, String, Option<u64>, Option<u64>, Option<f64>);

/// Measured VRAM of every GPU Kompanion sees (this server and paired computers).
async fn measured(s: &AppState) -> Vec<Measured> {
    let names: HashMap<String, String> = sqlx::query_as::<_, (String, String)>("SELECT id, name FROM machines")
        .fetch_all(&s.db)
        .await
        .unwrap_or_default()
        .into_iter()
        .collect();
    let local = s.config.machine_name.clone().unwrap_or_default();
    s.host
        .gpu_vram(s)
        .into_iter()
        .filter_map(|(remote, pci, used, total, watts)| {
            let machine = match remote {
                None => local.clone(),
                Some(id) => names.get(&id)?.clone(),
            };
            let mib = |gb: f64| (gb * 1024.0).round() as u64;
            Some((machine, pci.to_lowercase(), used.map(mib), total.map(mib), watts))
        })
        .collect()
}

/// The configured GPU among the measured ones: by PCI slot, or with pci = "*" the
/// machine's GPU with the most VRAM (a paired PC whose slot nobody wrote down).
fn find<'a>(g: &GpuConfig, all: &'a [Measured]) -> Option<&'a Measured> {
    let on_machine = all.iter().filter(|m| m.0 == g.machine);
    if g.pci == "*" {
        on_machine.max_by_key(|m| m.3.unwrap_or(0))
    } else {
        on_machine.into_iter().find(|m| m.1 == g.pci.to_lowercase())
    }
}

async fn gpu_ledger(s: &AppState, g: &GpuConfig, all: &[Measured]) -> GpuLedger {
    let mut holdings = Vec::new();
    for h in &g.holders {
        holdings.extend(probe(s, h).await);
    }
    let total = (g.vram_gb * 1024.0).round() as u64;
    let used_mib = find(g, all).and_then(|m| m.2);
    let mut l = ledger::ledger(&g.id, &g.machine, total, used_mib, g.schedulable, holdings);
    l.watts = find(g, all).and_then(|m| m.4);
    l
}

/// The ledger of every configured GPU, probed now.
pub async fn build(s: &AppState) -> Vec<GpuLedger> {
    let all = measured(s).await;
    let mut out = Vec::new();
    for g in &s.config.gpus {
        out.push(gpu_ledger(s, g, &all).await);
    }
    out
}

/// The last ledger (at most 10 s old), or a fresh one before the first sample.
pub async fn current(s: &AppState) -> Vec<GpuLedger> {
    if let Some(l) = LAST.lock().unwrap().clone() {
        return l;
    }
    let l = build(s).await;
    *LAST.lock().unwrap() = Some(l.clone());
    l
}

/// GET /api/gpus.
pub async fn list(State(s): State<AppState>) -> ApiResult<Json<Vec<GpuLedger>>> {
    Ok(Json(current(&s).await))
}

async fn store(s: &AppState, ledgers: &[GpuLedger]) {
    let at = util::now();
    for g in ledgers {
        let _ = sqlx::query(
            "INSERT INTO gpu_sample (gpu_id, at, used_mib, reserved_mib, other_mib, holdings, watts) VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&g.id)
        .bind(&at)
        .bind(g.used_mib.map(|u| u as i64))
        .bind(g.reserved_mib as i64)
        .bind(g.other_mib as i64)
        .bind(serde_json::to_string(&g.holdings).unwrap_or_default())
        .bind(g.watts)
        .execute(&s.db)
        .await;
    }
    let _ = sqlx::query("DELETE FROM gpu_sample WHERE at < ?").bind(util::minutes_ago(24 * 60)).execute(&s.db).await;
    let _ = sqlx::query("DELETE FROM gpu_event WHERE at < ?").bind(util::minutes_ago(24 * 60)).execute(&s.db).await;
}

/// Every 10 s (and at once when a GPU job is queued or ends): probe, keep a sample, run a
/// scheduling round, and tell open apps when something changed.
pub fn spawn(s: AppState) {
    if s.config.gpus.is_empty() {
        return;
    }
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(SAMPLE_EVERY);
        loop {
            let sampled = tokio::select! {
                _ = tick.tick() => true,
                _ = jobs::KICK.notified() => false,
            };
            let now = build(&s).await;
            if sampled {
                store(&s, &now).await;
            }
            jobs::round(&s, &now).await;
            // M6-03: coder/artist switch of the A770 (off unless GPU_ROLE_GPU is set).
            role::step(&s, &now).await;
            let changed = {
                let mut last = LAST.lock().unwrap();
                // Small VRAM wobbles (under 64 MiB) are not news.
                let differs = last.as_ref().is_none_or(|old| !same(old, &now));
                *last = Some(now);
                differs
            };
            if changed {
                s.bus.send_all(Event::Changed { what: "gpus", machine_id: None });
            }
        }
    });
}

fn same(a: &[GpuLedger], b: &[GpuLedger]) -> bool {
    let near = |x: u64, y: u64| x.abs_diff(y) < 64;
    a.len() == b.len()
        && a.iter().zip(b).all(|(x, y)| {
            x.id == y.id
                && near(x.free_mib, y.free_mib)
                && x.holdings.len() == y.holdings.len()
                && x.holdings.iter().zip(&y.holdings).all(|(p, q)| p.name == q.name && p.busy == q.busy && near(p.now_mib, q.now_mib))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn l(free: u64, holder_now: u64) -> GpuLedger {
        ledger::ledger("a770", "kireserver", 16384, None, true, vec![Holding {
            name: "Coder".into(), kind: "model".into(), now_mib: holder_now, peak_mib: 16384 - free, busy: false,
        }])
    }

    #[test]
    fn a_gpu_is_found_by_slot_or_as_the_biggest() {
        let all: Vec<Measured> = vec![
            ("kireserver".into(), "0000:10:00.0".into(), Some(12000), Some(16384), None),
            ("soucouyant".into(), "0000:0e:00.0".into(), Some(200), Some(512), None),
            ("soucouyant".into(), "0000:03:00.0".into(), Some(8000), Some(16304), None),
        ];
        let g = |machine: &str, pci: &str| GpuConfig {
            id: "x".into(), machine: machine.into(), pci: pci.into(), vram_gb: 16.0, schedulable: true, holders: vec![],
        };
        assert_eq!(find(&g("kireserver", "0000:10:00.0"), &all).unwrap().2, Some(12000));
        assert_eq!(find(&g("soucouyant", "*"), &all).unwrap().1, "0000:03:00.0");
        assert!(find(&g("kireserver", "0000:0c:00.0"), &all).is_none());
    }

    #[test]
    fn small_wobbles_are_not_news() {
        assert!(same(&[l(2000, 100)], &[l(2030, 120)]));
        assert!(!same(&[l(2000, 100)], &[l(1000, 100)]));
        assert!(!same(&[l(2000, 100)], &[]));
    }

    #[test]
    fn holder_peak_counts_kv_cache_per_sequence() {
        let h: HolderConfig = toml::from_str(
            "name = \"Coder\"\nprobe = \"ovms:http://ovms:8000\"\nweights_mb = 9300\nkv_mb_per_seq = 1200\nmax_seqs = 2",
        )
        .unwrap();
        assert_eq!(h.peak_mb(), 11700);
        assert_eq!(h.kind, "model");
    }

    #[test]
    fn gpus_parse_from_the_config() {
        let c: crate::config::Config = toml::from_str(
            "[[gpu]]\nid = \"a580\"\nmachine = \"kireserver\"\npci = \"0000:0c:00.0\"\nvram_gb = 8\nschedulable = false\n\
             [[gpu]]\nid = \"a770\"\nmachine = \"kireserver\"\npci = \"0000:10:00.0\"\nvram_gb = 16\n\
             [[gpu.holder]]\nname = \"ComfyUI\"\nkind = \"app\"\nprobe = \"comfyui:http://comfyui:8188\"\n",
        )
        .unwrap();
        assert_eq!(c.gpus.len(), 2);
        assert!(!c.gpus[0].schedulable && c.gpus[1].schedulable);
        assert_eq!(c.gpus[1].holders[0].kind, "app");
    }
}

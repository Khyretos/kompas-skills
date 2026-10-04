//! M6-02, the scheduler's pure decision core: given each GPU's free VRAM (from the ledger),
//! the host RAM, the running jobs and the queue, which queued jobs start now and where.
//! Chat beats code, code beats asset batches; smaller jobs may backfill; "tonight" jobs
//! only take an idle GPU by day; the A580 (unschedulable) and busy PCs are never used.
//! (Types drafted by Qwen3.5 9B; decide() and the tests rewritten in review.)
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Chat,
    Code,
    Asset,
} // priority order: Chat first, then Code, then Asset

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Job {
    pub id: String,
    pub kind: Kind,
    /// GPU ids it may run on, in preference order; empty = none.
    pub gpus: Vec<String>,
    pub vram_mib: u64,
    /// Host RAM it needs on that GPU's machine.
    pub ram_mib: u64,
    /// Wait for the night window unless a GPU is idle.
    pub tonight: bool,
    /// RFC 3339; earlier first within the same kind.
    pub created: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Gpu {
    pub id: String,
    pub machine: String,
    pub schedulable: bool,
    /// From the ledger: total - reserved (loaded models at peak) - other.
    pub free_mib: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Running {
    pub job_id: String,
    pub gpu: String,
    pub vram_mib: u64,
    pub ram_mib: u64,
}

pub struct Inputs<'a> {
    pub gpus: &'a [Gpu],
    /// Per machine; a machine missing here has 0.
    pub ram_free_mib: &'a HashMap<String, u64>,
    /// A game is running there: start nothing on it.
    pub busy_machines: &'a HashSet<String>,
    pub running: &'a [Running],
    pub queue: &'a [Job],
    pub night: bool,
}

/// Host RAM kept free on every machine (2026-10-04: running out of host RAM froze kireserver).
pub const RAM_SPARE_MIB: u64 = 5 * 1024;

/// (job id, gpu id) to start now, in start order.
pub fn decide(i: &Inputs) -> Vec<(String, String)> {
    let machine_of: HashMap<&str, &str> = i.gpus.iter().map(|g| (g.id.as_str(), g.machine.as_str())).collect();
    let mut vram: HashMap<&str, u64> = i.gpus.iter().map(|g| (g.id.as_str(), g.free_mib)).collect();
    let mut ram: HashMap<&str, u64> = i.ram_free_mib.iter().map(|(m, v)| (m.as_str(), *v)).collect();
    let mut busy_gpu: HashSet<&str> = HashSet::new();
    for r in i.running {
        if let Some(v) = vram.get_mut(r.gpu.as_str()) {
            *v = v.saturating_sub(r.vram_mib);
        }
        if let Some(m) = machine_of.get(r.gpu.as_str())
            && let Some(v) = ram.get_mut(m)
        {
            *v = v.saturating_sub(r.ram_mib);
        }
        busy_gpu.insert(r.gpu.as_str());
    }
    for v in ram.values_mut() {
        *v = v.saturating_sub(RAM_SPARE_MIB);
    }
    let running: HashSet<&str> = i.running.iter().map(|r| r.job_id.as_str()).collect();

    let mut queue: Vec<&Job> = i.queue.iter().collect();
    queue.sort_by(|a, b| a.kind.cmp(&b.kind).then_with(|| a.created.cmp(&b.created)).then_with(|| a.id.cmp(&b.id)));

    let mut out = Vec::new();
    let mut started: HashSet<&str> = HashSet::new();
    for job in queue {
        if running.contains(job.id.as_str()) || !started.insert(job.id.as_str()) {
            continue;
        }
        // The job's own GPUs, in its order (backfill: a job that fits nowhere doesn't block the rest).
        let pick = job.gpus.iter().find_map(|id| {
            let g = i.gpus.iter().find(|g| &g.id == id)?;
            let fits = g.schedulable
                && !i.busy_machines.contains(&g.machine)
                && vram.get(g.id.as_str()).copied().unwrap_or(0) >= job.vram_mib
                && ram.get(g.machine.as_str()).copied().unwrap_or(0) >= job.ram_mib
                && (!job.tonight || i.night || !busy_gpu.contains(g.id.as_str()));
            fits.then_some(g)
        });
        if let Some(g) = pick {
            if let Some(v) = vram.get_mut(g.id.as_str()) {
                *v -= job.vram_mib;
            }
            if let Some(v) = ram.get_mut(g.machine.as_str()) {
                *v -= job.ram_mib;
            }
            busy_gpu.insert(g.id.as_str());
            out.push((job.id.clone(), g.id.clone()));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gpu(id: &str, machine: &str, free: u64) -> Gpu {
        Gpu { id: id.into(), machine: machine.into(), schedulable: true, free_mib: free }
    }

    fn job(id: &str, kind: Kind, gpus: &[&str], vram: u64, ram: u64, created: &str) -> Job {
        Job { id: id.into(), kind, gpus: gpus.iter().map(|g| g.to_string()).collect(), vram_mib: vram, ram_mib: ram, tonight: false, created: created.into() }
    }

    fn run(gpus: &[Gpu], ram: &[(&str, u64)], busy: &[&str], running: &[Running], queue: &[Job], night: bool) -> Vec<(String, String)> {
        let ram: HashMap<String, u64> = ram.iter().map(|(m, v)| (m.to_string(), *v)).collect();
        let busy: HashSet<String> = busy.iter().map(|m| m.to_string()).collect();
        decide(&Inputs { gpus, ram_free_mib: &ram, busy_machines: &busy, running, queue, night })
    }

    const RAM: &[(&str, u64)] = &[("m1", 64 * 1024), ("m2", 64 * 1024)];

    #[test]
    fn chat_beats_an_earlier_asset_batch() {
        let q = [job("asset", Kind::Asset, &["a770"], 8000, 100, "2026-10-03T00:00:00Z"), job("chat", Kind::Chat, &["a770"], 8000, 100, "2026-10-03T01:00:00Z")];
        assert_eq!(run(&[gpu("a770", "m1", 10000)], RAM, &[], &[], &q, false), [("chat".into(), "a770".into())]);
    }

    #[test]
    fn running_jobs_hold_their_vram_and_never_start_twice() {
        let r = [Running { job_id: "old".into(), gpu: "a770".into(), vram_mib: 6000, ram_mib: 100 }];
        let q = [job("new", Kind::Chat, &["a770"], 5000, 100, "t"), job("old", Kind::Chat, &["a770"], 1, 1, "t")];
        assert!(run(&[gpu("a770", "m1", 10000)], RAM, &[], &r, &q, false).is_empty());
    }

    #[test]
    fn the_a580_and_a_busy_pc_are_never_used_and_only_the_jobs_own_gpus() {
        let mut a580 = gpu("a580", "m1", 10000);
        a580.schedulable = false;
        let gpus = [a580, gpu("a770", "m1", 10000), gpu("rx9070", "m2", 10000)];
        let q = [job("j", Kind::Chat, &["a580", "rx9070"], 1000, 100, "t")];
        assert!(run(&gpus, RAM, &["m2"], &[], &q, false).is_empty());
        assert_eq!(run(&gpus, RAM, &[], &[], &q, false), [("j".into(), "rx9070".into())]);
    }

    #[test]
    fn five_gb_of_host_ram_stay_free() {
        let q = [job("j", Kind::Chat, &["a770"], 1000, 4000, "t")];
        assert!(run(&[gpu("a770", "m1", 10000)], &[("m1", 8000)], &[], &[], &q, false).is_empty());
        assert_eq!(run(&[gpu("a770", "m1", 10000)], &[("m1", 10000)], &[], &[], &q, false).len(), 1);
    }

    #[test]
    fn tonight_jobs_take_only_an_idle_gpu_by_day() {
        let mut t = job("t", Kind::Asset, &["a770"], 1000, 100, "t");
        t.tonight = true;
        let r = [Running { job_id: "r".into(), gpu: "a770".into(), vram_mib: 5000, ram_mib: 100 }];
        let g = [gpu("a770", "m1", 10000)];
        assert!(run(&g, RAM, &[], &r, std::slice::from_ref(&t), false).is_empty());
        assert_eq!(run(&g, RAM, &[], &[], std::slice::from_ref(&t), false).len(), 1);
        assert_eq!(run(&g, RAM, &[], &r, std::slice::from_ref(&t), true).len(), 1);
        // A job started earlier in the same round makes the GPU busy too.
        let q = [job("first", Kind::Code, &["a770"], 1000, 100, "t"), t];
        assert_eq!(run(&g, RAM, &[], &[], &q, false), [("first".into(), "a770".into())]);
    }

    #[test]
    fn a_job_that_fits_nowhere_does_not_block_smaller_ones() {
        let q = [job("big", Kind::Chat, &["a770"], 12000, 100, "1"), job("small", Kind::Chat, &["a770"], 2000, 100, "2")];
        assert_eq!(run(&[gpu("a770", "m1", 10000)], RAM, &[], &[], &q, false), [("small".into(), "a770".into())]);
    }

    /// 1,000 random rounds: no GPU or machine is ever overcommitted.
    #[test]
    fn never_overcommits() {
        let mut x: u64 = 0x9E37_79B9_7F4A_7C15;
        let mut rnd = |n: u64| {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            if n == 0 { 0 } else { x % n }
        };
        for _ in 0..1000 {
            let n = 2 + rnd(3) as usize;
            let gpus: Vec<Gpu> = (0..n)
                .map(|k| Gpu { id: format!("g{k}"), machine: format!("m{}", k % 2), schedulable: k != 0, free_mib: rnd(16385) })
                .collect();
            let ram: HashMap<String, u64> = (0..2).map(|m| (format!("m{m}"), rnd(32 * 1024))).collect();
            let busy: HashSet<String> = if rnd(4) == 0 { HashSet::from(["m1".to_string()]) } else { HashSet::new() };
            let running: Vec<Running> = (0..rnd(4))
                .map(|k| {
                    let g = &gpus[rnd(n as u64) as usize];
                    Running { job_id: format!("r{k}"), gpu: g.id.clone(), vram_mib: rnd(g.free_mib / 2 + 1), ram_mib: rnd(2048) }
                })
                .collect();
            let queue: Vec<Job> = (0..rnd(11))
                .map(|k| Job {
                    id: format!("j{k}"),
                    kind: [Kind::Chat, Kind::Code, Kind::Asset][rnd(3) as usize],
                    gpus: (0..1 + rnd(2)).map(|_| format!("g{}", rnd(n as u64))).collect(),
                    vram_mib: rnd(12000),
                    ram_mib: rnd(8192),
                    tonight: rnd(2) == 0,
                    created: format!("{:02}", rnd(60)),
                })
                .collect();
            let night = rnd(2) == 0;
            let started = decide(&Inputs { gpus: &gpus, ram_free_mib: &ram, busy_machines: &busy, running: &running, queue: &queue, night });

            let job = |id: &str| queue.iter().find(|j| j.id == id).unwrap();
            let mut seen = HashSet::new();
            for (jid, gid) in &started {
                assert!(seen.insert(jid.clone()), "{jid} started twice");
                let g = gpus.iter().find(|g| &g.id == gid).unwrap();
                assert!(g.schedulable && !busy.contains(&g.machine), "{gid} must not be used");
                assert!(job(jid).gpus.contains(gid), "{jid} on a GPU it didn't ask for");
            }
            for g in &gpus {
                let r: u64 = running.iter().filter(|r| r.gpu == g.id).map(|r| r.vram_mib).sum();
                let s: u64 = started.iter().filter(|(_, gid)| *gid == g.id).map(|(j, _)| job(j).vram_mib).sum();
                assert!(s == 0 || r + s <= g.free_mib, "{} overcommitted: {r} + {s} > {}", g.id, g.free_mib);
            }
            for m in ["m0", "m1"] {
                let on = |gid: &str| gpus.iter().any(|g| g.id == gid && g.machine == m);
                let r: u64 = running.iter().filter(|r| on(&r.gpu)).map(|r| r.ram_mib).sum();
                let s: u64 = started.iter().filter(|(_, gid)| on(gid)).map(|(j, _)| job(j).ram_mib).sum();
                let allowed = ram[m].saturating_sub(r).saturating_sub(RAM_SPARE_MIB);
                assert!(s <= allowed, "{m} RAM overcommitted: {s} > {allowed}");
            }
        }
    }
}

//! M6-02: the GPU job queue. Every GPU job asks first: `acquire()` queues it and returns
//! a `Lease` once the scheduler gives it a GPU; dropping the lease ends the job. Owners beat
//! every 30 s; a job whose owner stopped beating (a crash, a restart) is dropped after 2
//! minutes, so nothing holds a GPU forever. The scheduler runs every 10 s and at once when
//! a job is queued or ends.
use std::{collections::{HashMap, HashSet}, sync::LazyLock, time::Duration};

use anyhow::{Result, bail};
use axum::{Json, extract::State};
use serde_json::{Value, json};
use tokio::sync::Notify;

use super::sched::{self, Kind};
use crate::{AppState, error::ApiResult, events::Event, util};

const BEAT: Duration = Duration::from_secs(30);
const LOST_AFTER_MIN: i64 = 2;

/// Wakes the scheduler: a job was queued or ended.
pub static KICK: LazyLock<Notify> = LazyLock::new(Notify::new);

pub struct Spec {
    pub kind: Kind,
    pub what: String,
    pub gpus: Vec<String>,
    pub vram_mib: u64,
    pub ram_mib: u64,
    pub tonight: bool,
}

/// A GPU reservation; dropping it ends the job and frees the GPU for the next one.
pub struct Lease {
    pub job_id: String,
    pub gpu: String,
    s: AppState,
    beat: tokio::task::JoinHandle<()>,
    failed: Option<String>,
}

impl Lease {
    /// End the job as failed (the error shows in the trace).
    pub fn fail(mut self, error: impl Into<String>) {
        self.failed = Some(error.into());
    }
}

impl Drop for Lease {
    fn drop(&mut self) {
        self.beat.abort();
        let (s, id, err) = (self.s.clone(), self.job_id.clone(), self.failed.take());
        tokio::spawn(async move {
            let _ = sqlx::query("UPDATE gpu_job SET state = ?, ended_at = ?, error = ? WHERE id = ? AND state = 'running'")
                .bind(if err.is_some() { "failed" } else { "done" })
                .bind(util::now())
                .bind(err)
                .bind(&id)
                .execute(&s.db)
                .await;
            KICK.notify_one();
            s.bus.send_all(Event::Changed { what: "gpus", machine_id: None });
        });
    }
}

fn kind_str(k: Kind) -> &'static str {
    match k {
        Kind::Chat => "chat",
        Kind::Code => "code",
        Kind::Asset => "asset",
    }
}

fn kind_of(s: &str) -> Kind {
    match s {
        "chat" => Kind::Chat,
        "code" => Kind::Code,
        _ => Kind::Asset,
    }
}

async fn beat(s: &AppState, id: &str) {
    let _ = sqlx::query("UPDATE gpu_job SET beat_at = ? WHERE id = ?").bind(util::now()).bind(id).execute(&s.db).await;
}

/// Queue a job and wait until it may run (or `wait` runs out: then it is dropped).
pub async fn acquire(s: &AppState, spec: Spec, wait: Duration) -> Result<Lease> {
    let id = util::new_id();
    let now = util::now();
    sqlx::query(
        "INSERT INTO gpu_job (id, kind, what, gpus, vram_mib, ram_mib, tonight, state, created_at, beat_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, 'queued', ?, ?)",
    )
    .bind(&id)
    .bind(kind_str(spec.kind))
    .bind(&spec.what)
    .bind(serde_json::to_string(&spec.gpus)?)
    .bind(spec.vram_mib as i64)
    .bind(spec.ram_mib as i64)
    .bind(spec.tonight)
    .bind(&now)
    .bind(&now)
    .execute(&s.db)
    .await?;
    KICK.notify_one();
    s.bus.send_all(Event::Changed { what: "gpus", machine_id: None });
    let deadline = tokio::time::Instant::now() + wait;
    let mut last_beat = tokio::time::Instant::now();
    loop {
        let row: Option<(String, Option<String>)> =
            sqlx::query_as("SELECT state, gpu FROM gpu_job WHERE id = ?").bind(&id).fetch_optional(&s.db).await?;
        match row {
            Some((state, Some(gpu))) if state == "running" => {
                let (s2, id2) = (s.clone(), id.clone());
                let beat_task = tokio::spawn(async move {
                    loop {
                        tokio::time::sleep(BEAT).await;
                        beat(&s2, &id2).await;
                    }
                });
                return Ok(Lease { job_id: id, gpu, s: s.clone(), beat: beat_task, failed: None });
            }
            Some((state, _)) if state != "queued" => bail!("the GPU job was {state}"),
            None => bail!("the GPU job is gone"),
            _ => {}
        }
        if tokio::time::Instant::now() >= deadline {
            let _ = sqlx::query("UPDATE gpu_job SET state = 'dropped', ended_at = ?, error = 'waited too long' WHERE id = ? AND state = 'queued'")
                .bind(util::now())
                .bind(&id)
                .execute(&s.db)
                .await;
            bail!("no GPU became free in time");
        }
        if last_beat.elapsed() >= BEAT {
            beat(s, &id).await;
            last_beat = tokio::time::Instant::now();
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}

type JobRow = (String, String, String, i64, i64, bool, String, Option<String>, String);

/// One scheduling round: drop lost jobs, then start what fits (pure core in sched.rs).
pub async fn round(s: &AppState, ledgers: &[super::ledger::GpuLedger]) {
    let lost = util::minutes_ago(LOST_AFTER_MIN);
    let dropped = sqlx::query(
        "UPDATE gpu_job SET state = 'dropped', ended_at = ?, error = 'its owner stopped (crash or restart)'
         WHERE state IN ('queued', 'running') AND beat_at < ?",
    )
    .bind(util::now())
    .bind(&lost)
    .execute(&s.db)
    .await
    .map(|r| r.rows_affected())
    .unwrap_or(0);
    let rows: Vec<JobRow> = sqlx::query_as(
        "SELECT id, kind, gpus, vram_mib, ram_mib, tonight, state, gpu, created_at FROM gpu_job WHERE state IN ('queued', 'running')",
    )
    .fetch_all(&s.db)
    .await
    .unwrap_or_default();
    let (mut running, mut queue) = (Vec::new(), Vec::new());
    for (id, kind, gpus, vram, ram, tonight, state, gpu, created) in rows {
        if state == "running" {
            running.push(sched::Running { job_id: id, gpu: gpu.unwrap_or_default(), vram_mib: vram as u64, ram_mib: ram as u64 });
        } else {
            let gpus = serde_json::from_str(&gpus).unwrap_or_default();
            queue.push(sched::Job { id, kind: kind_of(&kind), gpus, vram_mib: vram as u64, ram_mib: ram as u64, tonight, created });
        }
    }
    if queue.is_empty() {
        if dropped > 0 {
            s.bus.send_all(Event::Changed { what: "gpus", machine_id: None });
        }
        return;
    }
    let gpus: Vec<sched::Gpu> = ledgers
        .iter()
        .map(|l| sched::Gpu { id: l.id.clone(), machine: l.machine.clone(), schedulable: l.schedulable, free_mib: l.free_mib })
        .collect();
    let ram_free = ram_free_mib(s).await;
    // A game running on a PC (M6-03 adds the signal): nothing starts there.
    let busy: HashSet<String> = HashSet::new();
    let hour = time::OffsetDateTime::now_utc().hour();
    let start = sched::decide(&sched::Inputs {
        gpus: &gpus,
        ram_free_mib: &ram_free,
        busy_machines: &busy,
        running: &running,
        queue: &queue,
        night: crate::assets::ai::night(hour),
    });
    for (job, gpu) in &start {
        let _ = sqlx::query("UPDATE gpu_job SET state = 'running', gpu = ?, started_at = ? WHERE id = ? AND state = 'queued'")
            .bind(gpu)
            .bind(util::now())
            .bind(job)
            .execute(&s.db)
            .await;
    }
    if dropped > 0 || !start.is_empty() {
        s.bus.send_all(Event::Changed { what: "gpus", machine_id: None });
    }
}

/// Free host RAM per machine name, in MiB.
async fn ram_free_mib(s: &AppState) -> HashMap<String, u64> {
    let names: HashMap<String, String> = sqlx::query_as::<_, (String, String)>("SELECT id, name FROM machines")
        .fetch_all(&s.db)
        .await
        .unwrap_or_default()
        .into_iter()
        .collect();
    let local = s.config.machine_name.clone().unwrap_or_default();
    s.host
        .ram_free(s)
        .into_iter()
        .filter_map(|(remote, gb)| {
            let name = match remote {
                None => local.clone(),
                Some(id) => names.get(&id)?.clone(),
            };
            Some((name, (gb.max(0.0) * 1024.0) as u64))
        })
        .collect()
}

/// GET /api/gpus/jobs: queued and running jobs, and the last 50 that ended.
pub async fn list(State(s): State<AppState>) -> ApiResult<Json<Vec<Value>>> {
    type Row = (String, String, String, i64, String, Option<String>, String, Option<String>, Option<String>, Option<String>);
    let rows: Vec<Row> = sqlx::query_as(
        "SELECT id, kind, what, vram_mib, state, gpu, created_at, started_at, ended_at, error FROM gpu_job
         WHERE state IN ('queued', 'running') OR id IN (SELECT id FROM gpu_job WHERE ended_at IS NOT NULL ORDER BY ended_at DESC LIMIT 50)
         ORDER BY created_at DESC",
    )
    .fetch_all(&s.db)
    .await?;
    Ok(Json(
        rows.into_iter()
            .map(|(id, kind, what, vram, state, gpu, created, started, ended, error)| {
                json!({ "id": id, "kind": kind, "what": what, "vramMib": vram, "state": state, "gpu": gpu,
                        "createdAt": created, "startedAt": started, "endedAt": ended, "error": error })
            })
            .collect(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::ledger::ledger;

    async fn state() -> AppState {
        let db = sqlx::sqlite::SqlitePoolOptions::new().max_connections(1).connect("sqlite::memory:").await.unwrap();
        sqlx::migrate!().run(&db).await.unwrap();
        AppState::for_tests(toml::from_str("").unwrap(), db)
    }

    fn spec(vram: u64) -> Spec {
        Spec { kind: Kind::Asset, what: "test".into(), gpus: vec!["a770".into()], vram_mib: vram, ram_mib: 0, tonight: false }
    }

    async fn states(s: &AppState) -> Vec<String> {
        sqlx::query_as::<_, (String,)>("SELECT state FROM gpu_job ORDER BY created_at, rowid").fetch_all(&s.db).await.unwrap().into_iter().map(|r| r.0).collect()
    }

    #[tokio::test]
    async fn a_lease_waits_for_room_and_frees_it_when_dropped() {
        let s = state().await;
        // No RAM is known in tests: the scheduler needs ram_mib 0 and RAM_SPARE subtracts to 0.
        let gpu = vec![ledger("a770", "kireserver", 16384, None, true, vec![])];
        let s1 = s.clone();
        let first = tokio::spawn(async move { acquire(&s1, spec(10000), Duration::from_secs(5)).await });
        tokio::time::sleep(Duration::from_millis(100)).await;
        round(&s, &gpu).await;
        let lease = first.await.unwrap().unwrap();
        assert_eq!(lease.gpu, "a770");
        // A second big job must wait while the first holds 10 GB.
        let s2 = s.clone();
        let second = tokio::spawn(async move { acquire(&s2, spec(10000), Duration::from_secs(5)).await });
        tokio::time::sleep(Duration::from_millis(100)).await;
        round(&s, &gpu).await;
        assert_eq!(states(&s).await, ["running", "queued"]);
        drop(lease);
        tokio::time::sleep(Duration::from_millis(100)).await;
        round(&s, &gpu).await;
        let lease2 = second.await.unwrap().unwrap();
        assert_eq!(states(&s).await, ["done", "running"]);
        lease2.fail("out of memory");
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert_eq!(states(&s).await, ["done", "failed"]);
    }

    #[tokio::test]
    async fn jobs_whose_owner_stopped_beating_are_dropped() {
        let s = state().await;
        sqlx::query(
            "INSERT INTO gpu_job (id, kind, what, gpus, vram_mib, ram_mib, state, gpu, created_at, beat_at)
             VALUES ('old', 'asset', 'x', '[\"a770\"]', 8000, 0, 'running', 'a770', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        )
        .execute(&s.db)
        .await
        .unwrap();
        round(&s, &[ledger("a770", "kireserver", 16384, None, true, vec![])]).await;
        assert_eq!(states(&s).await, ["dropped"]);
    }
}

//! The Machines panel: this server's own stats (sampled on demand, never more
//! than once a second) and the stats that paired runners push for their PCs.
//!
//! Runners authenticate with a one-time token of which only the SHA-256 is
//! stored, push over HTTPS (no inbound ports on the PC), and are told how often
//! to report: every second while someone watches "Live", the owner's slider
//! setting while the tab is open, and once a minute otherwise.

use std::{
    collections::{HashMap, VecDeque},
    sync::Mutex,
    time::{Duration, Instant},
};

use axum::{
    Extension, Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode, header},
};
use machine_stats::{Sampler, Snapshot};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{
    AppState,
    auth::User,
    error::{ApiError, ApiResult},
    events::Event,
    util,
};

const SAMPLES: usize = 60;
/// Never sample (or accept runner reports) faster than this.
const MIN_INTERVAL: Duration = Duration::from_secs(1);
/// How often a runner reports when nobody has the Machines tab open.
const IDLE_INTERVAL: u32 = 60;
const SERVER_ID: &str = "server";

#[derive(Default)]
struct History {
    cpu: VecDeque<f64>,
    watts: VecDeque<f64>,
}

impl History {
    fn push(&mut self, s: &Snapshot) {
        if let Some(c) = s.cpu {
            self.cpu.push_back(c);
        }
        let w: f64 = s.gpus.iter().filter_map(|g| g.watts).sum();
        if s.gpus.iter().any(|g| g.watts.is_some()) {
            self.watts.push_back((w * 10.0).round() / 10.0);
        }
        for q in [&mut self.cpu, &mut self.watts] {
            while q.len() > SAMPLES {
                q.pop_front();
            }
        }
    }
}

struct Local {
    sampler: Sampler,
    snap: Option<Snapshot>,
    at: Option<Instant>,
    at_iso: Option<String>,
    history: History,
}

struct Remote {
    #[allow(dead_code)] // kept for per-user cleanup
    user_id: String,
    snap: Snapshot,
    at: Instant,
    at_iso: String,
    /// The interval the runner was last told to use.
    interval: u32,
    history: History,
}

pub struct HostStats {
    local: Mutex<Local>,
    remote: Mutex<HashMap<String, Remote>>,
    /// Users with the Machines tab open on "Live", until when.
    watchers: Mutex<HashMap<String, Instant>>,
    /// Users with the Machines tab open on a slower setting: (until, seconds).
    viewers: Mutex<HashMap<String, (Instant, u32)>>,
}

impl Default for HostStats {
    fn default() -> Self {
        // In the container, /host/os-release names the host's OS; /proc and
        // /sys already show the host.
        HostStats {
            local: Mutex::new(Local {
                sampler: Sampler::new("/", "/"),
                snap: None,
                at: None,
                at_iso: None,
                history: History::default(),
            }),
            remote: Mutex::default(),
            watchers: Mutex::default(),
            viewers: Mutex::default(),
        }
    }
}

/// Adds per-engine busy shares and VRAM in use from kompanion-gpu-helper (a
/// tiny root service on the host, see gpu-helper/), when its socket is
/// mounted. Without it, the container only sees its own processes.
fn merge_helper(snap: &mut Snapshot) {
    use std::io::Read;
    let Ok(mut s) = std::os::unix::net::UnixStream::connect("/host-gpu/stats.sock") else { return };
    let _ = s.set_read_timeout(Some(Duration::from_millis(500)));
    let mut text = String::new();
    if s.take(256 * 1024).read_to_string(&mut text).is_err() {
        return;
    }
    let Ok(list) = serde_json::from_str::<Vec<Value>>(&text) else { return };
    for g in &mut snap.gpus {
        let Some(u) = list.iter().find(|u| u["pciSlot"].as_str() == Some(g.pci_slot.as_str())) else { continue };
        if let Some(engines) = u["engines"].as_array() {
            g.engines = engines
                .iter()
                .filter_map(|e| {
                    Some(machine_stats::gpu::Engine { name: e["name"].as_str()?.chars().take(40).collect(), busy: e["busy"].as_f64()?.clamp(0.0, 1.0) })
                })
                .take(16)
                .collect();
        }
        if let Some(b) = u["vramUsedBytes"].as_u64() {
            g.vram_used_gb = Some((b as f64 / 1024f64.powi(3) * 10.0).round() / 10.0);
        }
    }
}

fn uptime_text(secs: u64) -> String {
    let (d, h, m) = (secs / 86_400, secs % 86_400 / 3_600, secs % 3_600 / 60);
    if d > 0 { format!("{d} d {h} h") } else { format!("{h} h {m} min") }
}

/// A snapshot in the shape the web app's Machines panel shows.
fn view(id: &str, name: &str, s: &Snapshot, h: &History, online: bool, sampled_at: &str, labels: &HashMap<String, String>) -> Value {
    let os = if s.os == "Linux" || s.os.is_empty() { "Linux".to_string() } else { s.os.clone() };
    json!({
        "id": id, "name": name, "os": os, "online": online,
        "cpu": s.cpu.or(h.cpu.back().copied()).unwrap_or(0.0),
        "cpuCount": s.cpu_count,
        "ramUsedGb": s.ram_used_gb, "ramTotalGb": s.ram_total_gb,
        "diskUsedGb": s.disk_used_gb, "diskTotalGb": s.disk_total_gb,
        "gpus": s.gpus.iter().map(|g| {
            let mut v = serde_json::to_value(g).unwrap_or(Value::Null);
            if let Some(o) = v.as_object_mut() {
                o.insert("use".into(), json!(labels.get(&g.pci_slot).cloned().unwrap_or_default()));
            }
            v
        }).collect::<Vec<_>>(),
        "kompanionShare": 0.0,
        "busy": format!("up {}, load {:.2}", uptime_text(s.uptime_secs), s.load1),
        "history": h.cpu, "historyKind": "cpu", "powerHistory": h.watts,
        "sampledAt": sampled_at,
    })
}

impl HostStats {
    /// This server, sampled now unless the last sample is under a second old.
    fn local_view(&self, state: &AppState) -> Value {
        let mut l = self.local.lock().unwrap();
        if l.at.is_none_or(|t| t.elapsed() >= MIN_INTERVAL) {
            let mut snap = l.sampler.sample();
            merge_helper(&mut snap);
            l.history.push(&snap);
            l.snap = Some(snap);
            l.at = Some(Instant::now());
            l.at_iso = Some(util::now());
        }
        let name = state.config.machine_name.as_deref().unwrap_or("This server");
        match &l.snap {
            Some(s) => view(SERVER_ID, name, s, &l.history, true, l.at_iso.as_deref().unwrap_or(""), &state.config.gpu_labels),
            None => json!({ "id": SERVER_ID, "name": name, "online": true }),
        }
    }

    /// Everything `user_id` may see: this server plus their own paired PCs.
    async fn views(&self, state: &AppState, user_id: &str) -> ApiResult<Vec<Value>> {
        let mut out = vec![self.local_view(state)];
        let rows: Vec<(String, String)> = sqlx::query_as("SELECT id, name FROM machines WHERE user_id = ? ORDER BY created_at")
            .bind(user_id)
            .fetch_all(&state.db)
            .await?;
        let remote = self.remote.lock().unwrap();
        for (id, name) in rows {
            match remote.get(&id) {
                Some(r) => {
                    let online = r.at.elapsed() < Duration::from_secs(3 * r.interval.max(1) as u64 + 5);
                    let labels = HashMap::new();
                    out.push(view(&id, &name, &r.snap, &r.history, online, &r.at_iso, &labels));
                }
                None => out.push(json!({ "id": id, "name": name, "os": "", "online": false, "cpu": 0.0,
                    "ramUsedGb": 0.0, "ramTotalGb": 0.0, "gpus": [], "kompanionShare": 0.0, "history": [] })),
            }
        }
        Ok(out)
    }

    /// Keeps `user_id` on the live feed for the next 15 seconds.
    pub fn watch(&self, user_id: &str) {
        self.watchers.lock().unwrap().insert(user_id.into(), Instant::now() + Duration::from_secs(15));
    }

    /// How often `user_id`'s runners should report right now.
    fn interval_for(&self, user_id: &str) -> u32 {
        if self.watchers.lock().unwrap().get(user_id).is_some_and(|t| *t > Instant::now()) {
            return 1;
        }
        match self.viewers.lock().unwrap().get(user_id) {
            Some((until, secs)) if *until > Instant::now() => (*secs).clamp(1, 300),
            _ => IDLE_INTERVAL,
        }
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
                for u in users {
                    if let Ok(machines) = state.host.views(&state, &u).await {
                        state.bus.send(&u, Event::Machines { machines });
                    }
                }
            }
        });
    }
}

// ---- Web app endpoints ----

pub async fn list(State(s): State<AppState>, Extension(u): Extension<User>) -> ApiResult<Json<Vec<Value>>> {
    let refresh: Option<(i64,)> = sqlx::query_as("SELECT machines_refresh FROM users WHERE id = ?")
        .bind(&u.id)
        .fetch_optional(&s.db)
        .await?;
    let secs = refresh.map_or(5, |r| r.0.clamp(1, 300) as u32);
    s.host
        .viewers
        .lock()
        .unwrap()
        .insert(u.id.clone(), (Instant::now() + Duration::from_secs(2 * secs as u64 + 10), secs));
    Ok(Json(s.host.views(&s, &u.id).await?))
}

/// Heartbeat from a Machines tab set to "Live".
pub async fn live(State(s): State<AppState>, Extension(u): Extension<User>) -> StatusCode {
    s.host.watch(&u.id);
    StatusCode::NO_CONTENT
}

#[derive(Deserialize)]
pub struct NewMachine {
    name: String,
}

/// Pairs a new PC: returns its id and a token that is shown only this once.
pub async fn create(
    State(s): State<AppState>,
    Extension(u): Extension<User>,
    Json(b): Json<NewMachine>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let name: String = b.name.trim().chars().take(60).collect();
    if name.is_empty() {
        return Err(ApiError::BadRequest("Give the computer a name.".into()));
    }
    let id = util::new_id();
    let token = format!("kkr_{}", util::random_token());
    sqlx::query("INSERT INTO machines (id, user_id, name, token_hash, created_at) VALUES (?, ?, ?, ?, ?)")
        .bind(&id)
        .bind(&u.id)
        .bind(&name)
        .bind(util::sha256_hex(&token))
        .bind(util::now())
        .execute(&s.db)
        .await?;
    Ok((StatusCode::CREATED, Json(json!({ "id": id, "name": name, "token": token }))))
}

pub async fn delete(
    State(s): State<AppState>,
    Extension(u): Extension<User>,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    let done = sqlx::query("DELETE FROM machines WHERE id = ? AND user_id = ?")
        .bind(&id)
        .bind(&u.id)
        .execute(&s.db)
        .await?;
    if done.rows_affected() == 0 {
        return Err(ApiError::NotFound);
    }
    s.host.remote.lock().unwrap().remove(&id);
    Ok(StatusCode::NO_CONTENT)
}

// ---- Runner endpoint (bearer token, no cookie) ----

fn finite(v: Option<f64>) -> bool {
    v.is_none_or(|x| x.is_finite() && (0.0..1e7).contains(&x))
}

fn valid(s: &Snapshot) -> bool {
    s.gpus.len() <= 16
        && s.os.len() <= 120
        && finite(s.cpu)
        && s.cpu.is_none_or(|c| c <= 1.0)
        && [s.ram_used_gb, s.ram_total_gb, s.disk_used_gb, s.disk_total_gb, s.load1].iter().all(|x| finite(Some(*x)))
        && s.gpus.iter().all(|g| {
            g.name.len() <= 120
                && g.pci_slot.len() <= 40
                && g.driver.len() <= 40
                && [g.load, g.vram_used_gb, g.vram_total_gb, g.watts, g.temp_c, g.core_mhz, g.mem_mhz, g.fan_rpm,
                    g.core_max_mhz, g.power_cap_w]
                    .into_iter()
                    .all(finite)
                && g.engines.len() <= 16
                && g.engines.iter().all(|e| e.name.len() <= 40 && (0.0..=1.0).contains(&e.busy))
        })
}

/// A runner reports its PC's stats; the answer tells it when to report next.
pub async fn report(
    State(s): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(snap): Json<Snapshot>,
) -> ApiResult<Json<Value>> {
    let token = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or(ApiError::Unauthorized)?;
    let row: Option<(String,)> = sqlx::query_as("SELECT user_id FROM machines WHERE id = ? AND token_hash = ?")
        .bind(&id)
        .bind(util::sha256_hex(token.trim()))
        .fetch_optional(&s.db)
        .await?;
    s.throttle.check(&format!("runner:{id}"))?;
    let Some((user_id,)) = row else {
        s.throttle.fail(&format!("runner:{id}"));
        return Err(ApiError::Unauthorized);
    };
    if !valid(&snap) {
        return Err(ApiError::BadRequest("Those stats don't look right.".into()));
    }
    let interval = s.host.interval_for(&user_id);
    {
        let mut remote = s.host.remote.lock().unwrap();
        if let Some(r) = remote.get(&id)
            && r.at.elapsed() < MIN_INTERVAL - Duration::from_millis(100)
        {
            return Err(ApiError::TooMany);
        }
        if !remote.contains_key(&id) {
            // First report since start: say which GPU fields arrive (no values).
            for g in &snap.gpus {
                tracing::info!(machine = %id, gpu = %g.name, driver = %g.driver,
                    load = g.load.is_some(), vram = g.vram_total_gb.is_some(), watts = g.watts.is_some(),
                    temp = g.temp_c.is_some(), "runner GPU fields");
            }
        }
        let mut history = remote.remove(&id).map(|r| r.history).unwrap_or_default();
        history.push(&snap);
        remote.insert(id.clone(), Remote { user_id, snap, at: Instant::now(), at_iso: util::now(), interval, history });
    }
    sqlx::query("UPDATE machines SET last_seen = ? WHERE id = ?")
        .bind(util::now())
        .bind(&id)
        .execute(&s.db)
        .await?;
    let jobs = crate::access::take_jobs(&s.db, &id).await.unwrap_or_default();
    Ok(Json(json!({ "interval": interval, "jobs": jobs })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_nonsense() {
        let mut s = Sampler::new("/", "/").sample();
        assert!(valid(&s));
        s.cpu = Some(f64::NAN);
        assert!(!valid(&s));
        s.cpu = Some(0.5);
        s.os = "x".repeat(500);
        assert!(!valid(&s));
    }
}

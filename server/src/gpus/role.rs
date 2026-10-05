//! M6-03: switches kireserver's A770 between coder and artist (Kees approved automatic
//! switching, 2026-10-05). Decisions come from role_policy; the host helper runs them.
//! Off unless GPU_ROLE_GPU names the GPU (for example "a770").

use std::{sync::{LazyLock, Mutex}, time::{Duration, Instant}};
use anyhow::{Context, Result};
use axum::{Extension, Json, extract::State, http::StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use super::{ledger::GpuLedger, role_policy::{self, Mode, Target, View}};
use crate::{AppState, auth::User, error::{ApiError, ApiResult}, events::Event, util};

pub const SOCKET: &str = "/host-gpu-role/role.sock";

#[derive(Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Switch { 
    pub from: String, 
    pub to: String, 
    pub by: String, 
    pub started_at: String, 
    pub seconds: f64, 
    pub ok: bool, 
    pub coder_answer_s: Option<f64>, 
    pub error: Option<String> 
}

#[derive(Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RoleState { 
    pub gpu: Option<String>, 
    pub mode: String, 
    pub app: Option<String>, 
    pub switching: Option<String>, 
    pub last: Option<Switch>, 
    #[serde(skip)] 
    pub last_at: Option<Instant> 
}

static STATE: LazyLock<Mutex<RoleState>> = LazyLock::new(|| Mutex::new(RoleState { mode: "unknown".into(), ..Default::default() }));

fn gpu_id() -> Option<String> { std::env::var("GPU_ROLE_GPU").ok().filter(|g| !g.is_empty()) }

async fn helper(cmd: &str, timeout: Duration) -> Result<(bool, String)> {
    // GPU_ROLE_SOCKET: the host path, for `kompanion-server gpu-role` outside the container.
    let path = std::env::var("GPU_ROLE_SOCKET").unwrap_or_else(|_| SOCKET.to_string());
    let mut stream = tokio::net::UnixStream::connect(&path)
        .await
        .context("kompanion-gpu-role is not running")?;
    
    let cmd_bytes = format!("{cmd}\n").into_bytes();
    stream.write_all(&cmd_bytes).await?;
    
    let mut text = String::new();
    tokio::time::timeout(timeout, stream.read_to_string(&mut text))
        .await
        .map_err(|_| anyhow::anyhow!("no answer from kompanion-gpu-role in {} s", timeout.as_secs()))??;
    
    let v: Value = serde_json::from_str(&text)
        .context("invalid JSON from kompanion-gpu-role")?;
    
    let ok = v["ok"].as_bool().unwrap_or(false);
    let output = v["output"].as_str().unwrap_or("").to_string();
    
    Ok((ok, output))
}

pub fn mode_of(l: &GpuLedger) -> (Mode, Option<String>) {
    let coder = std::env::var("GPU_ROLE_CODER").ok().filter(|c| !c.is_empty()).unwrap_or_else(|| "Coder".to_string());
    if l.holdings.iter().any(|h| h.name == coder && h.kind == "model" && h.now_mib > 0) {
        (Mode::Coder, None)
    } else {
        let app = l.holdings.iter()
            .find(|h| h.kind == "app" && h.now_mib > 0)
            .and_then(|h| match h.name.to_lowercase().as_str() {
                "comfyui" => Some("comfyui".to_string()),
                "heartmula" => Some("heartmula".to_string()),
                n if n.starts_with("moss") => Some("moss-sfx".to_string()),
                _ => None,
            });
        
        (Mode::Artist, app)
    }
}

async fn coder_answers(http: &reqwest::Client, within: Duration) -> Option<f64> {
    let ai = crate::assets::ai::Ai::from_env(http.clone());
    let start = Instant::now();
    
    loop {
        if let Ok(answer) = ai.chat(json!("Reply with the single word OK."), 5).await
            && !answer.trim().is_empty()
        {
            return Some(start.elapsed().as_secs_f64());
        }
        
        if start.elapsed() >= within {
            return None;
        }
        
        tokio::time::sleep(Duration::from_secs(3)).await;
    }
}

/// Runs one switch through the helper and checks Coder afterwards; no shared state.
/// Used by the scheduler (through `switch`) and by `kompanion-server gpu-role <target>`.
pub async fn perform(http: &reqwest::Client, target: &Target, from: String, by: String) -> Switch {
    let to = match target {
        Target::Coder => "coder".to_string(),
        Target::Studio(app) => format!("artist ({app})"),
    };
    let cmd = match target {
        Target::Coder => "coder".to_string(),
        Target::Studio(app) => format!("studio {app}"),
    };
    let started_at = util::now();
    let t0 = Instant::now();
    let (ok, output) = helper(&cmd, Duration::from_secs(3900)).await.unwrap_or_else(|e| (false, e.to_string()));
    let mut coder_answer_s = None;
    let mut error = None;
    if !ok {
        let n = output.chars().count();
        error = Some(output.chars().skip(n.saturating_sub(500)).collect());
    } else if matches!(target, Target::Coder) {
        coder_answer_s = coder_answers(http, Duration::from_secs(60)).await;
        if coder_answer_s.is_none() {
            // Coder is loaded but silent: a broken GPU context (CL_INVALID_EVENT) needs an
            // OVMS restart; restart in any case, then give it two more minutes.
            let broken = helper("ovms-log", Duration::from_secs(70)).await
                .is_ok_and(|(_, log)| log.contains("CL_INVALID_EVENT"));
            tracing::warn!(broken_context = broken, "Coder silent after switching back; restarting OVMS");
            let _ = helper("restart-ovms", Duration::from_secs(330)).await;
            coder_answer_s = coder_answers(http, Duration::from_secs(120)).await;
            if coder_answer_s.is_none() {
                error = Some("Coder did not answer within 60 s after switching back, nor after an OVMS restart".to_string());
            }
        }
    }
    let sw = Switch {
        from,
        to,
        by,
        started_at,
        seconds: (t0.elapsed().as_secs_f64() * 10.0).round() / 10.0,
        ok: ok && error.is_none(),
        coder_answer_s: coder_answer_s.map(|s| (s * 10.0).round() / 10.0),
        error,
    };
    tracing::info!(from = %sw.from, to = %sw.to, seconds = sw.seconds, ok = sw.ok, coder_answer_s = ?sw.coder_answer_s, "gpu role switch");
    sw
}

/// A switch from the scheduler or an admin: marks it in the shared state, tells open apps.
pub async fn switch(s: AppState, target: Target, by: String) -> Switch {
    // Locks stay in blocks: a std MutexGuard must not live across an await.
    let from = {
        let mut state = STATE.lock().unwrap();
        state.switching = Some(match &target {
            Target::Coder => "coder".to_string(),
            Target::Studio(app) => format!("artist ({app})"),
        });
        match (&state.mode[..], &state.app) {
            ("artist", Some(app)) => format!("artist ({app})"),
            (mode, _) => mode.to_string(),
        }
    };
    s.bus.send_all(Event::Changed { what: "gpus", machine_id: None });
    let sw = perform(&s.http, &target, from, by).await;
    {
        let mut state = STATE.lock().unwrap();
        state.last = Some(sw.clone());
        state.last_at = Some(Instant::now());
        state.switching = None;
    }
    // M6-04: the switch is on the GPU timeline, so the gap in Coder's VRAM is explained.
    if let Some(gpu) = gpu_id() {
        let detail = format!("{} → {}, {:.1} s{}{}", sw.from, sw.to, sw.seconds,
            sw.coder_answer_s.map(|a| format!(", Coder answered in {a:.1} s")).unwrap_or_default(),
            if sw.ok { String::new() } else { format!(" (failed: {})", sw.error.clone().unwrap_or_default()) });
        super::timeline::event(&s, &gpu, "role", &detail).await;
    }
    s.bus.send_all(Event::Changed { what: "gpus", machine_id: None });
    sw
}

/// The target named by `coder`, `comfyui`, `heartmula` or `moss-sfx`.
pub fn target_of(name: &str) -> Option<Target> {
    match name {
        "coder" => Some(Target::Coder),
        t if role_policy::STUDIO_APPS.contains(&t) => Some(Target::Studio(t.to_string())),
        _ => None,
    }
}

pub async fn step(s: &AppState, ledgers: &[GpuLedger]) {
    let Some(gpu) = gpu_id() else { return };
    
    let Some(l) = ledgers.iter().find(|l| l.id == gpu) else { return };

    let (mode, app) = mode_of(l);
    
    {
        let mut state = STATE.lock().unwrap();
        state.gpu = Some(gpu.clone());
        state.mode = match mode {
            Mode::Coder => "coder".to_string(),
            Mode::Artist => "artist".to_string(),
        };
        state.app = app;
    }
    
    let queued = sqlx::query_as::<_, (String, String, String)>(
        "SELECT kind, what, state FROM gpu_job WHERE state IN ('queued','running') AND (gpu = ?1 OR (gpu IS NULL AND gpus LIKE ?2)) ORDER BY created_at"
    )
    .bind(&gpu)
    .bind(format!("%\"{}\"%", gpu))
    .fetch_all(&s.db)
    .await
    .unwrap_or_default();
    
    let queued_code = queued.iter().filter(|r| r.0 == "chat" || r.0 == "code").count();
    let queued_asset = queued.iter().filter(|r| r.0 == "asset").map(|r| r.1.clone()).collect::<Vec<_>>();
    let running = queued.iter().filter(|r| r.2 == "running").count();
    
    let studio_busy = l.holdings.iter().any(|h| h.kind == "app" && h.busy);
    let coder_busy = false;
    
    let mut state = STATE.lock().unwrap();
    let secs_since_switch = state.last_at.map(|t| t.elapsed().as_secs() as i64);
    let switching = state.switching.is_some();
    
    let view = View {
        mode,
        switching,
        secs_since_switch,
        queued_code,
        queued_asset: &queued_asset,
        running_jobs: running,
        studio_busy,
        coder_busy,
    };
    
    if let Some(target) = role_policy::decide(&view) {
        // Marked before spawning, so the next tick does not start a second switch.
        state.switching = Some(match &target {
            Target::Coder => "coder".to_string(),
            Target::Studio(app) => format!("artist ({app})"),
        });
        drop(state);
        let s = s.clone();
        tokio::spawn(async move {
            switch(s, target, "auto".into()).await;
        });
    }
}

/// The role state for Capabilities (None while switching is off).
pub fn current() -> Option<RoleState> {
    gpu_id().map(|_| STATE.lock().unwrap().clone())
}

pub async fn get(State(_s): State<AppState>) -> Json<RoleState> {
    Json(STATE.lock().unwrap().clone())
}

#[derive(Deserialize)]
pub struct Wanted { 
    target: String 
}

pub async fn set(State(s): State<AppState>, Extension(u): Extension<User>, Json(w): Json<Wanted>) -> ApiResult<(StatusCode, Json<Value>)> {
    if !crate::admin::is_admin(&s.db, &u.id).await? {
        return Err(ApiError::Forbidden("Only admins can switch the GPU.".into()));
    }
    
    if std::env::var("GPU_ROLE_GPU").is_err() {
        return Err(ApiError::BadRequest("GPU switching is off".into()));
    }
    
    let Some(target) = target_of(&w.target) else {
        return Err(ApiError::BadRequest("Pick coder, comfyui, heartmula or moss-sfx.".into()));
    };
    
    let (running,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM gpu_job WHERE state = 'running' AND gpu = ?")
        .bind(gpu_id().unwrap_or_default())
        .fetch_one(&s.db)
        .await?;
    if running > 0 {
        return Err(ApiError::BadRequest("A job is running on the GPU; try again when it is done.".into()));
    }
    {
        let mut state = STATE.lock().unwrap();
        if state.switching.is_some() {
            return Err(ApiError::BadRequest("A switch is already running.".into()));
        }
        state.switching = Some(match &target {
            Target::Coder => "coder".to_string(),
            Target::Studio(app) => format!("artist ({app})"),
        });
    }
    tokio::spawn(async move {
        switch(s, target, u.name).await;
    });
    
    Ok((StatusCode::ACCEPTED, Json(json!({ "started": true }))))
}

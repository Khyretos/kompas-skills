//! M6-04 trace timeline: per GPU, VRAM and watts samples, the GPU jobs that ran, and events
//! (role switches, restarts) over the last 1 or 24 hours, downsampled for drawing.

use axum::{Json, extract::{Query, State}};
use serde::Deserialize;
use serde_json::{Value, json};
use crate::{AppState, error::ApiResult, util};

pub const MAX_POINTS: usize = 360;

#[derive(Deserialize)]
pub struct Range { hours: Option<u32> }

/// One stored sample: (at, used_mib, reserved_mib, watts).
/// A job row: (id, kind, what, state, started_at, ended_at, error).
type JobRow = (String, String, String, String, Option<String>, Option<String>, Option<String>);

type Sample = (String, Option<i64>, i64, Option<f64>);

pub fn downsample(samples: &[Sample], max: usize) -> Vec<Value> {
    let point = |at: &str, used: Option<i64>, reserved: i64, watts: Option<f64>| {
        json!({ "at": at, "usedMib": used, "reservedMib": reserved, "watts": watts })
    };
    if samples.len() <= max {
        return samples.iter().map(|(at, u, r, w)| point(at, *u, *r, *w)).collect();
    }
    // Equal buckets (the last may be shorter): first time, peak VRAM, average watts.
    let size = samples.len().div_ceil(max.max(1));
    samples
        .chunks(size)
        .map(|b| {
            let used = b.iter().filter_map(|s| s.1).max();
            let reserved = b.iter().map(|s| s.2).max().unwrap_or(0);
            let ws: Vec<f64> = b.iter().filter_map(|s| s.3).collect();
            let watts = (!ws.is_empty()).then(|| (ws.iter().sum::<f64>() / ws.len() as f64 * 10.0).round() / 10.0);
            point(&b[0].0, used, reserved, watts)
        })
        .collect()
}

pub async fn timeline(State(s): State<AppState>, Query(r): Query<Range>) -> ApiResult<Json<Vec<Value>>> {
    // Two ranges only: the last hour, or the last day (samples are kept 24 h).
    let hours: u32 = if r.hours.unwrap_or(1) > 1 { 24 } else { 1 };
    let since = util::minutes_ago(hours as i64 * 60);
    
    let mut out = Vec::new();
    
    for g in s.config.gpus.iter() {
        let samples: Vec<Sample> = sqlx::query_as::<_, Sample>(
            "SELECT at, used_mib, reserved_mib, watts FROM gpu_sample WHERE gpu_id = ? AND at >= ? ORDER BY at"
        )
        .bind(&g.id)
        .bind(&since)
        .fetch_all(&s.db)
        .await?;
        
        let jobs: Vec<JobRow> = sqlx::query_as::<_, JobRow>(
            "SELECT id, kind, what, state, started_at, ended_at, error FROM gpu_job WHERE gpu = ? AND started_at IS NOT NULL AND (ended_at IS NULL OR ended_at >= ?) ORDER BY started_at"
        )
        .bind(&g.id)
        .bind(&since)
        .fetch_all(&s.db)
        .await?;
        
        let events: Vec<(String, String, String)> = sqlx::query_as::<_, (String, String, String)>(
            "SELECT at, kind, detail FROM gpu_event WHERE gpu_id = ? AND at >= ? ORDER BY at"
        )
        .bind(&g.id)
        .bind(&since)
        .fetch_all(&s.db)
        .await?;
        
        let job_values: Vec<Value> = jobs.iter().map(|(id, kind, what, state, started_at, ended_at, error)| {
            json!({
                "id": id,
                "kind": kind,
                "what": what,
                "state": state,
                "startedAt": started_at,
                "endedAt": ended_at,
                "error": error
            })
        }).collect();
        
        let event_values: Vec<Value> = events.iter().map(|(at, kind, detail)| {
            json!({
                "at": at,
                "kind": kind,
                "detail": detail
            })
        }).collect();
        
        out.push(json!({
            "gpu": g.id,
            "machine": g.machine,
            "hours": hours,
            "samples": downsample(&samples, MAX_POINTS),
            "jobs": job_values,
            "events": event_values
        }));
    }
    
    Ok(Json(out))
}

/// Records something that explains the timeline (a role switch, an OVMS restart).
pub async fn event(s: &AppState, gpu: &str, kind: &str, detail: &str) {
    let _ = sqlx::query(
        "INSERT INTO gpu_event (gpu_id, at, kind, detail) VALUES (?, ?, ?, ?)"
    )
    .bind(gpu)
    .bind(util::now())
    .bind(kind)
    .bind(detail)
    .execute(&s.db)
    .await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_downsample_small() {
        let samples = vec![
            ("t1".to_string(), Some(10), 5, Some(100.0)),
            ("t2".to_string(), Some(20), 6, Some(110.0)),
            ("t3".to_string(), Some(30), 7, Some(120.0)),
        ];
        let result = downsample(&samples, 360);
        assert_eq!(result.len(), 3);
        assert_eq!(result[0].get("at").unwrap().as_str().unwrap(), "t1");
        assert_eq!(result[0].get("usedMib").unwrap().as_i64().unwrap(), 10);
        assert_eq!(result[0].get("reservedMib").unwrap().as_i64().unwrap(), 5);
        assert_eq!(result[0].get("watts").unwrap().as_f64().unwrap(), 100.0);
    }

    #[test]
    fn test_downsample_large() {
        let mut samples = Vec::new();
        for i in 0..1000 {
            let watts = if i % 2 == 0 { Some(10.0) } else { None };
            samples.push((format!("t{:04}", i), Some(i as i64), 1, watts));
        }
        let result = downsample(&samples, 100);
        assert_eq!(result.len(), 100);
        assert_eq!(result[0].get("at").unwrap().as_str().unwrap(), "t0000");
        assert_eq!(result[0].get("usedMib").unwrap().as_i64().unwrap(), 9);
        assert_eq!(result[0].get("watts").unwrap().as_f64().unwrap(), 10.0);
    }
}

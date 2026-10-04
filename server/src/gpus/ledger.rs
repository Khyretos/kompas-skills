//! The pure part of the GPU ledger (M6-01): probe parsers and the VRAM arithmetic.
//! (Drafted by Qwen3.5 9B; passed review unchanged.)
use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Holding {
    pub name: String,
    pub kind: String, // "model" | "app"
    pub now_mib: u64,
    pub peak_mib: u64,
    pub busy: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuLedger {
    pub id: String,
    pub machine: String,
    pub total_mib: u64,
    pub used_mib: Option<u64>,
    pub reserved_mib: u64,
    pub other_mib: u64,
    pub free_mib: u64,
    pub schedulable: bool,
    pub holdings: Vec<Holding>,
}

/// Parse OVMS GET /v1/config answer.
/// Returns (model_name, loaded) for every model key found.
/// `loaded` is true if ANY entry in that model's `model_version_status` array has state "AVAILABLE".
/// If the root value is not an object, returns an empty vector.
pub fn ovms_models(v: &Value) -> Vec<(String, bool)> {
    let obj = match v.as_object() {
        Some(o) => o,
        None => return vec![],
    };

    let mut result = Vec::new();

    for (name, val) in obj {
        let status_arr = match val.get("model_version_status").and_then(|x| x.as_array()) {
            Some(arr) => arr,
            None => continue,
        };

        let loaded = status_arr.iter().any(|entry| {
            match entry.get("state").and_then(|s| s.as_str()) {
                Some("AVAILABLE") => true,
                _ => false,
            }
        });

        result.push((name.clone(), loaded));
    }

    result.sort_by(|a, b| a.0.cmp(&b.0));
    result
}

/// Parse Ollama GET /api/ps answer.
/// Returns (model_name, size_in_MiB).
/// Missing `size_vram` counts as 0. If no "models" array, returns empty vec.
pub fn ollama_models(v: &Value) -> Vec<(String, u64)> {
    let models_arr = match v.get("models").and_then(|x| x.as_array()) {
        Some(arr) => arr,
        None => return vec![],
    };

    let mut result = Vec::new();

    for entry in models_arr {
        let name = match entry.get("name").and_then(|n| n.as_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };

        let bytes = match entry.get("size_vram").and_then(|x| x.as_u64()) {
            Some(b) => b,
            None => 0,
        };

        let mib = bytes / 1_048_576;
        result.push((name, mib));
    }

    result
}

/// Parse ComfyUI GET /system_stats answer.
/// Returns the first device's `torch_vram_total` in MiB.
/// If no devices or missing field, returns None.
pub fn comfy_vram(v: &Value) -> Option<u64> {
    let devices_arr = match v.get("devices").and_then(|x| x.as_array()) {
        Some(arr) => arr,
        None => return None,
    };

    if devices_arr.is_empty() {
        return None;
    }

    let first = &devices_arr[0];
    first.get("torch_vram_total").and_then(|x| x.as_u64()).map(|bytes| bytes / 1_048_576)
}

/// Parse studio app health answer.
/// Returns (loaded, busy). Missing fields default to false.
pub fn studio_health(v: &Value) -> (bool, bool) {
    let loaded = v.get("loaded").and_then(|x| x.as_bool()).unwrap_or(false);
    let busy = v.get("busy").and_then(|x| x.as_bool()).unwrap_or(false);
    (loaded, busy)
}

/// Build a GpuLedger from raw parameters and holdings.
///
/// Calculations:
/// - reserved_mib = sum over holdings of max(now_mib, peak_mib)
/// - attributed_now = sum of now_mib
/// - other_mib = used_mib.map(|u| u.saturating_sub(attributed_now)).unwrap_or(0)
/// - free_mib = total_mib.saturating_sub(reserved_mib + other_mib)
///
/// Holdings are sorted by name before insertion.
pub fn ledger(
    id: &str,
    machine: &str,
    total_mib: u64,
    used_mib: Option<u64>,
    schedulable: bool,
    holdings: Vec<Holding>,
) -> GpuLedger {
    let mut sorted_holdings = holdings;
    sorted_holdings.sort_by(|a, b| a.name.cmp(&b.name));

    let reserved_mib: u64 = sorted_holdings
        .iter()
        .map(|h| h.now_mib.max(h.peak_mib))
        .sum();

    let attributed_now: u64 = sorted_holdings.iter().map(|h| h.now_mib).sum();

    let other_mib = used_mib.map(|u| u.saturating_sub(attributed_now)).unwrap_or(0);

    let free_mib = total_mib.saturating_sub(reserved_mib + other_mib);

    GpuLedger {
        id: id.to_string(),
        machine: machine.to_string(),
        total_mib,
        used_mib,
        reserved_mib,
        other_mib,
        free_mib,
        schedulable,
        holdings: sorted_holdings,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_ovms_models() {
        let v = json!({
            "Whisper": { "model_version_status": [ { "state": "AVAILABLE" } ] },
            "Coder": { "model_version_status": [ { "state": "LOADING" } ] }
        });
        let res = ovms_models(&v);
        assert_eq!(res, vec![("Coder".to_string(), false), ("Whisper".to_string(), true)]);
    }

    #[test]
    fn test_ollama_models() {
        let v = json!({ "models": [ { "name": "gemma4:12b", "size_vram": 7984130292u64 } ] });
        let res = ollama_models(&v);
        assert_eq!(res, vec![("gemma4:12b".to_string(), 7614)]);
    }

    #[test]
    fn test_comfy_vram() {
        let v = json!({ "devices": [ { "torch_vram_total": 2097152 } ] });
        assert_eq!(comfy_vram(&v), Some(2));

        let v_empty = json!({});
        assert_eq!(comfy_vram(&v_empty), None);
    }

    #[test]
    fn test_studio_health() {
        let v = json!({ "loaded": true });
        assert_eq!(studio_health(&v), (true, false));
    }

    #[test]
    fn test_ledger_with_used() {
        let holdings = vec![
            Holding {
                name: "Coder".to_string(),
                kind: "model".to_string(),
                now_mib: 12000,
                peak_mib: 13500,
                busy: false,
            },
            Holding {
                name: "Whisper".to_string(),
                kind: "model".to_string(),
                now_mib: 500,
                peak_mib: 400,
                busy: false,
            },
        ];

        let ledger = ledger("a770", "kireserver", 16384, Some(13000), true, holdings);

        // reserved = max(12000, 13500) + max(500, 400) = 13500 + 500 = 14000
        assert_eq!(ledger.reserved_mib, 14000);
        // attributed_now = 12000 + 500 = 12500
        // other = 13000 - 12500 = 500
        assert_eq!(ledger.other_mib, 500);
        // free = 16384 - 14000 - 500 = 1884
        assert_eq!(ledger.free_mib, 1884);
    }

    #[test]
    fn test_ledger_without_used() {
        let holdings = vec![Holding {
            name: "Test".to_string(),
            kind: "model".to_string(),
            now_mib: 1000,
            peak_mib: 2000,
            busy: false,
        }];

        let ledger = ledger("dummy", "machine", 8192, None, true, holdings);

        // reserved = 2000
        assert_eq!(ledger.reserved_mib, 2000);
        // used_mib is None -> other = 0
        assert_eq!(ledger.other_mib, 0);
        // free = 8192 - 2000 - 0 = 6192
        assert_eq!(ledger.free_mib, 6192);
    }
}

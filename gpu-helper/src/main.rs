//! kompanion-gpu-helper: the one privileged piece. It runs as its own system
//! user with only CAP_SYS_PTRACE (needed to read fdinfo of processes that hold
//! capabilities, such as OVMS in a container), reads only the drm-* lines of /proc/*/fdinfo, and answers every connection on a unix
//! socket with per-GPU aggregates: engine busy shares and VRAM in use. No
//! process names, no input is read, no network.
//!
//! Usage: kompanion-gpu-helper [/run/kompanion-gpu/stats.sock]

use std::{
    io::Write,
    os::unix::{fs::PermissionsExt, net::UnixListener},
    path::Path,
    time::{Duration, Instant},
};

use machine_stats::fdinfo::EngineReader;

fn snapshot(reader: &mut EngineReader) -> String {
    let usage = reader.read(Path::new("/proc"));
    let list: Vec<serde_json::Value> = usage
        .into_iter()
        .map(|(pdev, u)| {
            serde_json::json!({
                "pciSlot": pdev,
                "engines": u.engines.iter().map(|(n, b)| serde_json::json!({ "name": n, "busy": b })).collect::<Vec<_>>(),
                "vramUsedBytes": u.vram_used_bytes,
            })
        })
        .collect();
    serde_json::Value::Array(list).to_string()
}

fn main() {
    let path = std::env::args().nth(1).unwrap_or_else(|| "/run/kompanion-gpu/stats.sock".into());
    let _ = std::fs::remove_file(&path);
    let listener = match UnixListener::bind(&path) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("kompanion-gpu-helper: can't bind {path}: {e}");
            std::process::exit(1);
        }
    };
    // Owner root, group set by the service (the readers' group), no access for others.
    let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o660));
    let mut reader = EngineReader::default();
    let mut last = snapshot(&mut reader);
    let mut at = Instant::now();
    eprintln!("kompanion-gpu-helper: serving {path}");
    for conn in listener.incoming() {
        let Ok(mut conn) = conn else { continue };
        // Sample at most once a second, however often we're asked.
        if at.elapsed() >= Duration::from_secs(1) {
            last = snapshot(&mut reader);
            at = Instant::now();
        }
        let _ = conn.set_write_timeout(Some(Duration::from_secs(2)));
        let _ = conn.write_all(last.as_bytes());
    }
}

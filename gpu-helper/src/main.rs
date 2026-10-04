//! kompanion-gpu-helper: the one privileged piece. It runs as its own system
//! user with only CAP_SYS_PTRACE (needed to read fdinfo of processes that hold
//! capabilities, such as OVMS in a container), reads only the drm-* lines of /proc/*/fdinfo, and answers every connection on a unix
//! socket with per-GPU aggregates: engine busy shares and VRAM in use. No
//! process names, no input is read, no network.
//!
//! Samples once a second in the background; every connection gets the latest sample at once.
//!
//! Usage: kompanion-gpu-helper [/run/kompanion-gpu/stats.sock]

use std::{
    io::Write,
    os::unix::{fs::PermissionsExt, net::UnixListener},
    path::Path,
    sync::{Arc, Mutex},
    time::Duration,
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

fn probe(sock: &str) -> usize {
    use std::io::Read;
    let Ok(mut s) = std::os::unix::net::UnixStream::connect(sock) else { return 0 };
    let _ = s.set_read_timeout(Some(Duration::from_secs(2)));
    let mut text = String::new();
    if s.read_to_string(&mut text).is_err() {
        return 0;
    }
    serde_json::from_str::<Vec<serde_json::Value>>(&text)
        .map(|l| l.iter().filter(|g| g["vramUsedBytes"].as_u64().is_some()).count())
        .unwrap_or(0)
}

fn main() {
    // `kompanion-gpu-helper --probe [socket]`: ask a running helper and print how many
    // GPUs report VRAM; exit 0 when at least one does (install.sh's self-test).
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("--probe") {
        let sock = args.get(2).cloned().unwrap_or_else(|| "/run/kompanion-gpu/stats.sock".into());
        let n = probe(&sock);
        println!("{n} GPU(s) with VRAM");
        std::process::exit(if n > 0 { 0 } else { 1 });
    }
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

    let last = Arc::new(Mutex::new(snapshot(&mut EngineReader::default())));

    eprintln!("kompanion-gpu-helper: serving {path}");

    let mut reader = EngineReader::default();
    let last_clone = Arc::clone(&last);

    // Spawn a thread that owns the EngineReader and samples once a second.
    std::thread::spawn(move || {
        loop {
            let s = snapshot(&mut reader);
            *last_clone.lock().unwrap() = s;
            std::thread::sleep(Duration::from_secs(1));
        }
    });

    for conn in listener.incoming() {
        let Ok(mut conn) = conn else { continue };
        let body = last.lock().unwrap().clone();
        let _ = conn.set_write_timeout(Some(Duration::from_secs(2)));
        let _ = conn.write_all(body.as_bytes());
    }
}

//! Runs a program without a shell, with a timeout and capped output. Used by
//! the PC-control tools. (Claude wrote this after two failed model drafts; the
//! tests are the model's.)

use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use crate::tools::Outcome;

use std::cell::RefCell;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

thread_local! {
    /// Set by the job thread before it runs a tool: when it turns true, the running command
    /// is killed and reported as "stopped by you".
    pub static STOP: RefCell<Option<Arc<AtomicBool>>> = const { RefCell::new(None) };
}

/// Kills the process group `pid` (the command and everything it started).
fn kill_group(pid: u32) {
    let _ = Command::new("kill").arg("-KILL").arg(format!("-{pid}")).status();
}

const CAP: u64 = 64 * 1024;

/// Runs `program args` (no shell), stdin closed, stdout+stderr drained in threads
/// (64 KiB cap each), killed after `timeout_secs`. Output is stdout then stderr,
/// cut to 64 KiB, then a line `exit: N` (-1 on timeout or kill). ok = exit 0.
pub fn run_cmd(program: &str, args: &[String], cwd: Option<&Path>, envs: &[(String, String)], timeout_secs: u64) -> Outcome {
    let mut cmd = Command::new(program);
    cmd.args(args)
        .envs(envs.iter().map(|(k, v)| (k, v)))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(d) = cwd {
        cmd.current_dir(d);
    }
    // Put the child in its own process group so we can kill the whole tree.
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        let _ = cmd.process_group(0);
    }
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => return Outcome { ok: false, output: format!("could not start {program}: {e}") },
    };
    // Drain both pipes in threads so a chatty command can't block on a full
    // pipe; anything past the cap is read and dropped.
    let drain = |pipe: Option<Box<dyn Read + Send>>| {
        thread::spawn(move || {
            let mut buf = Vec::new();
            if let Some(mut p) = pipe {
                let _ = Read::take(&mut p, CAP).read_to_end(&mut buf);
                let _ = std::io::copy(&mut p, &mut std::io::sink());
            }
            buf
        })
    };
    let out_t = drain(child.stdout.take().map(|p| Box::new(p) as Box<dyn Read + Send>));
    let err_t = drain(child.stderr.take().map(|p| Box::new(p) as Box<dyn Read + Send>));
    let start = Instant::now();
    let mut timed_out = false;
    let stop = STOP.with(|s| s.borrow().clone());
    let mut stopped = false;
    let code = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status.code().unwrap_or(-1),
            Ok(None) if stop.as_ref().is_some_and(|f| f.load(Ordering::Relaxed)) => {
                stopped = true;
                kill_group(child.id());
                let _ = child.kill();
                let _ = child.wait();
                break -1;
            }
            Ok(None) if start.elapsed() < Duration::from_secs(timeout_secs) => thread::sleep(Duration::from_millis(50)),
            Ok(None) => {
                timed_out = true;
                kill_group(child.id());
                let _ = child.kill();
                let _ = child.wait();
                break -1;
            }
            Err(_) => break -1,
        }
    };
    let mut bytes = out_t.join().unwrap_or_default();
    bytes.extend(err_t.join().unwrap_or_default());
    bytes.truncate(CAP as usize);
    let mut output = String::from_utf8_lossy(&bytes).into_owned();
    if !output.is_empty() && !output.ends_with('\n') {
        output.push('\n');
    }
    if timed_out {
        output.push_str(&format!("timed out after {timeout_secs} s\n"));
    }
    if stopped {
        output.push_str("stopped by you\n");
    }
    output.push_str(&format!("exit: {code}"));
    Outcome { ok: code == 0, output }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a(parts: &[&str]) -> Vec<String> {
        parts.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn test_echo_hi() {
        let outcome = run_cmd("echo", &a(&["hi"]), None, &[], 10);
        assert!(outcome.ok);
        assert!(outcome.output.starts_with("hi\n"));
        assert!(outcome.output.ends_with("exit: 0"));
    }

    #[test]
    fn test_exit_3() {
        let outcome = run_cmd("sh", &a(&["-c", "exit 3"]), None, &[], 10);
        assert!(!outcome.ok);
        assert!(outcome.output.ends_with("exit: 3"));
    }

    #[test]
    fn test_timeout_kills_children() {
        let start = Instant::now();
        let outcome = run_cmd("sh", &a(&["-c", "sleep 30 & sleep 30; echo never"]), None, &[], 1);
        let elapsed = start.elapsed();
        assert!(!outcome.ok);
        assert!(outcome.output.contains("timed out"));
        assert!(elapsed < Duration::from_secs(5));
    }

    #[test]
    fn test_stop_flag() {
        let f = Arc::new(AtomicBool::new(false));
        STOP.with(|s| *s.borrow_mut() = Some(f.clone()));
        let f2 = f.clone();
        let stopper = thread::spawn(move || {
            thread::sleep(Duration::from_millis(300));
            f2.store(true, Ordering::Relaxed);
        });
        let start = Instant::now();
        let outcome = run_cmd("sleep", &a(&["30"]), None, &[], 60);
        let elapsed = start.elapsed();
        assert!(!outcome.ok);
        assert!(outcome.output.contains("stopped by you"));
        assert!(elapsed < Duration::from_secs(5));
        STOP.with(|s| *s.borrow_mut() = None);
        stopper.join().ok();
    }

    #[test]
    fn test_missing_program() {
        let outcome = run_cmd("nonexistent_program_xyz", &[], None, &[], 10);
        assert!(!outcome.ok);
        assert!(outcome.output.contains("could not start"));
    }

    #[test]
    fn test_cwd() {
        let tmpdir = std::env::temp_dir().join(format!("kk-cwd-test-{}", std::process::id()));
        std::fs::create_dir_all(&tmpdir).expect("failed to create temp dir");
        let outcome = run_cmd("pwd", &[], Some(&tmpdir), &[], 10);
        assert!(outcome.ok);
        assert!(outcome.output.contains(tmpdir.to_string_lossy().as_ref()));
        std::fs::remove_dir_all(&tmpdir).expect("failed to remove temp dir");
    }
}

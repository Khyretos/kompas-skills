use rand::RngCore;
use sha2::{Digest, Sha256};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

pub fn now() -> String {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_default()
}

pub fn in_days(days: i64) -> String {
    (OffsetDateTime::now_utc() + time::Duration::days(days))
        .format(&Rfc3339)
        .unwrap_or_default()
}

pub fn in_hours(hours: i64) -> String {
    (OffsetDateTime::now_utc() + time::Duration::hours(hours))
        .format(&Rfc3339)
        .unwrap_or_default()
}

pub fn minutes_ago(minutes: i64) -> String {
    (OffsetDateTime::now_utc() - time::Duration::minutes(minutes))
        .format(&Rfc3339)
        .unwrap_or_default()
}

pub fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

/// 32 random bytes, URL-safe base64.
pub fn random_token() -> String {
    use base64::Engine;
    let mut bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

pub fn sha256_hex(s: &str) -> String {
    let digest = Sha256::digest(s.as_bytes());
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

/// Constant-time comparison for secrets.
pub fn ct_eq(a: &str, b: &str) -> bool {
    a.len() == b.len()
        && a.bytes()
            .zip(b.bytes())
            .fold(0u8, |acc, (x, y)| acc | (x ^ y))
            == 0
}

/// Runs a background job and restarts it when it panics or ends: after 5 s, then doubling
/// up to 5 min (TEN-02: a crash in one area must not leave that area dead until the next
/// restart). `make` builds a fresh run of the job each time, so clone what it needs inside.
pub fn supervise<F, Fut>(name: &'static str, make: F)
where
    F: Fn() -> Fut + Send + Sync + 'static,
    Fut: std::future::Future<Output = ()> + Send + 'static,
{
    tokio::spawn(supervise_loop(name, make, std::time::Duration::from_secs(5)));
}

async fn supervise_loop<F, Fut>(name: &'static str, make: F, first: std::time::Duration)
where
    F: Fn() -> Fut + Send + Sync + 'static,
    Fut: std::future::Future<Output = ()> + Send + 'static,
{
    let mut wait = first;
    loop {
        let started = std::time::Instant::now();
        match tokio::spawn(make()).await {
            Ok(()) => tracing::warn!(job = name, "background job ended; starting it again"),
            Err(e) if e.is_panic() => tracing::error!(job = name, "background job panicked; starting it again"),
            Err(_) => return, // cancelled: the server is shutting down
        }
        // A job that ran for a while before failing starts again with the short wait.
        if started.elapsed() > std::time::Duration::from_secs(600) {
            wait = first;
        }
        tokio::time::sleep(wait).await;
        wait = (wait * 2).min(std::time::Duration::from_secs(300));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn supervise_restarts_a_panicking_job() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;
        let runs = Arc::new(AtomicUsize::new(0));
        let r = runs.clone();
        let h = tokio::spawn(supervise_loop(
            "test",
            move || {
                let r = r.clone();
                async move {
                    let n = r.fetch_add(1, Ordering::SeqCst);
                    if n < 2 {
                        panic!("planted crash {n}");
                    }
                    std::future::pending::<()>().await;
                }
            },
            std::time::Duration::from_millis(1),
        ));
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        assert_eq!(runs.load(Ordering::SeqCst), 3, "two crashes, then the third run keeps going");
        h.abort();
    }
}

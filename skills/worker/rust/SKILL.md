# Worker: Rust (Kompanion server, runner, machine-stats)

1. (2026-10-03) SQL: use runtime queries only, `sqlx::query(...)`/`query_as(...)` with `.bind(...)`. Never the `query!` macros: they need a database at build time and the build has none. Never `format!` values into SQL.
   Example: `sqlx::query_as::<_, (String,)>("SELECT id FROM users WHERE name = ?").bind(name).fetch_optional(&db).await?`
2. (2026-10-03) Background tasks (`tokio::spawn`) must not `unwrap()` database or network results; log with `tracing::warn!` and carry on.
3. (2026-10-03) The database is SQLite (`SqlitePool`). Never import Postgres types.
4. (2026-10-03) Use the helpers that exist instead of re-reading tables: `admin::load(&db)` for settings, `util::now()` for timestamps, `auth::create_session*` for sessions.
5. (2026-10-01) Rates from counters need the previous sample: compute the delta first, then store the new sample. Overwriting first gives zero every time. Test with two reads.
6. (2026-10-01) sysfs: `class/drm/cardN` only (skip `cardN-DP-1` connectors); every file may be missing, so every read returns an `Option`.
7. (2026-10-03) Traits must be in scope for their methods, e.g. `use openidconnect::OAuth2TokenResponse as _;` for `access_token()`.

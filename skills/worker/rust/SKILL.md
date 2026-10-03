# Worker: Rust (Kompanion server, runner, machine-stats)

1. (2026-10-03) SQL: use runtime queries only, `sqlx::query(...)`/`query_as(...)` with `.bind(...)`. Never the `query!` macros: they need a database at build time and the build has none. Never `format!` values into SQL.
   Example: `sqlx::query_as::<_, (String,)>("SELECT id FROM users WHERE name = ?").bind(name).fetch_optional(&db).await?`
2. (2026-10-03) Background tasks (`tokio::spawn`) must not `unwrap()` database or network results; log with `tracing::warn!` and carry on.
3. (2026-10-03) The database is SQLite (`SqlitePool`). Never import Postgres types.
4. (2026-10-03) Use the helpers that exist instead of re-reading tables: `admin::load(&db)` for settings, `util::now()` for timestamps, `auth::create_session*` for sessions.
5. (2026-10-01) Rates from counters need the previous sample: compute the delta first, then store the new sample. Overwriting first gives zero every time. Test with two reads.
6. (2026-10-01) sysfs: `class/drm/cardN` only (skip `cardN-DP-1` connectors); every file may be missing, so every read returns an `Option`.
7. (2026-10-03) Traits must be in scope for their methods, e.g. `use openidconnect::OAuth2TokenResponse as _;` for `access_token()`.
8. (2026-10-03) Errors shown to people: never pass a raw response body through. Use `llm::readable_error` (no HTML, at most 200 characters, a plain word for known GPU failures).
9. (2026-10-03) Model calls: bound the prompt (chat history budget), retry once on a failure before anything streamed, then fall back to another provider and say so in one line.
10. (2026-10-03) Parsing "key: value" lines: use `split_once(':')`, never `split(':')` (PCI slots like 0000:03:00.0 contain colons); strip units like " ns" before parsing numbers.
11. (2026-10-03) Access checks: a special target ("system") must never act as a wildcard for file paths; check every grant and skip expired ones instead of returning on the first expired match. Shell: drain stdout/stderr in threads (a full pipe blocks the child), kill on timeout, report the exit code.

12. Tests clean up only what they created. Never `remove_dir_all(std::env::temp_dir())`:
    that deletes the whole system temp dir. Make a unique subdir
    (`temp_dir().join(format!("kk-<test>-{}", std::process::id()))`), use it, and remove only that
    subdir; or remove just the one file with `fs::remove_file`.

13. Context files are for reading signatures. Never copy them into your output: write only the
    file you were asked for, and call the other modules through `use crate::...`.
14. `?` only works in functions that return `Result` or `Option`. In a function returning
    `Outcome`, use `let Some(x) = ... else { return Outcome { .. } };` or `match`.
15. Build a command as one argv list: the program is `argv[0]`, the args are `&argv[1..]`.
    Never pass the program name as both the program and the first argument.
16. Every element of a `Vec<String>` must be a `String`. Mixing `"--needed"` (a `&str`) in does not
    compile. Use a helper: `fn argv(parts: &[&str]) -> Vec<String> { parts.iter().map(|s| s.to_string()).collect() }`.
17. An edit replaces only the matched text: `content.replacen(old, new, 1)`. Never write `new`
    as the whole file; that destroys the rest of the file.
18. Process plumbing: `Command::current_dir` takes a path, not an `Option`
    (`if let Some(d) = cwd { cmd.current_dir(d); }`). `ChildStdout` and `ChildStderr` are
    different types, so box them as `Box<dyn Read + Send>` for one drain closure.
    `std::io::Take` has no public `new`; use `Read::take(reader, n)`.

19. `Option::and_then` needs a closure that returns an `Option`. For a plain value, use `.map`
    (`fs::read_to_string(p).ok().map(|s| s.trim().to_string())`). `?` inside a closure
    only works when the closure itself returns `Option` or `Result`.
20. Tests call functions with exactly their signature (`&[String]`, so `&["htop".to_string()]`,
    not `vec![...]`), and only use APIs that exist. `Grants` has no `Default`; an empty
    `Grants` comes from `Grants::load` on a path that does not exist. An empty file is
    invalid JSON.
21. When you read a file, keep the value: `let Ok(before) = fs::read_to_string(&f) else { ... }`.
    Calling `read_to_string` only to check `is_err()` leaves the String empty.
22. To iterate an `Option` or `Result` of an iterator, use `.into_iter().flatten()`
    (`fs::read_dir(p).into_iter().flatten()`). `Option<ReadDir>` has no `.flatten()`.
    Compare `&&str` with `String` by dereferencing: `list.iter().any(|a| *a == s)`.
23. Rust runs tests in parallel. Each test that touches files needs its own folder: put the
    test name in it (`temp_dir().join(format!("kk-edit-{name}-{}", process::id()))`). When
    tests share one folder, one test's cleanup deletes another's files.

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
24. Time: keep units straight. Seconds since the epoch become days with `div_euclid(86_400)`.
    Timestamps compared as strings must use one format (RFC 3339 UTC with time of day).
    Test date code with known real dates (`civil(20_729) == (2026, 10, 3)`). A clock bug
    once made every expiring grant look expired.
25. `.bind(x)` takes ownership. When the value is used again later (in the JSON answer, or a
    second query), bind a reference: `.bind(&id)`, `.bind(&u.id)`.
26. File names with version dots (`kompanion-runner-0.3.1-x86_64-linux-musl`): never use
    `with_extension`, which cuts at the last dot. Build the name: `dir.join(format!("{name}.sha256"))`.
27. axum responses: `([(header::CONTENT_TYPE, "text/plain")], body).into_response()`, where body is a
    `String` or `Vec<u8>`. `(StatusCode::NOT_FOUND, "Not found").into_response()` for errors.
28. Use only the crates in Cargo.toml. There is no `dirs` crate in the runner: the home folder is
    `std::env::var("HOME")`. Trait methods need their trait imported: `Permissions::from_mode` needs
    `std::os::unix::fs::PermissionsExt`, `write_all` needs `std::io::Write`.
29. Know the data shape before reading it. A runner job is flat: `{"tool": "edit_file", "path": ..,
    "old": .., "new": ..}`, with no nested `args`. Read `job["path"]`, never `job["args"]["path"]`,
    and write the tests with the same flat shape.
30. The server's database pool is `s.db` (AppState has no `pool` field).
31. Migrations of a feature built next to other work get their own number range (`0100_assets.sql`):
    sqlx applies every unapplied version in order and never checks for gaps, while two branches that
    both add `0015_*.sql` break the second deploy (checksum mismatch).
32. The `regex` crate has no lookahead or lookbehind. When the regex only answers yes or no, turn
    `(?=X)` into a plain group `(X)`; when the match text is used, match more and trim it in code.
33. Broadcast to every signed-in user with `s.bus.send_all(Event::…)` (user id `"*"`); per-user
    events keep using `s.bus.send(&user_id, …)`.
34. (2026-10-04) The deploy image builds on musl (`rust:1-alpine`). A crate with C code can build
    on glibc and fail on musl: sqlite-vec 0.1.9 uses `u_int8_t`, so the deploy broke while CI was
    green. CI's server job now uses the same image; `CFLAGS` in the Dockerfile and in build.yml map
    the BSD names to `uint8_t`/`uint16_t`/`uint64_t`. Before adding a `-sys` or C crate, build the
    Docker image (or watch the server job) once.
35. (2026-10-04) Resolve a model role with `api::user_role(&s, user_id, "worker")`: the user's own
    choice, else `[roles]` from kompanion.toml. Never query `user_roles` directly; a fresh account
    has no rows there, so W2 and `kompanion-runner ask` refused to start ("set the orchestrator
    model first") although Settings showed the defaults.
36. The drafting pipeline (`tools/qwen/pipeline.py`) strips code fences from whole-file answers, so
    a file with a ``` inside a string or doc comment comes back cut to a fragment. Use
    `"mode": "patch"` (search/replace blocks) for such files.
37. (2026-10-04) `tools/qwen/pipeline.py` finds OVMS by the container's current address; a fixed IP
    broke after a reboot (every OVMS job got a 404). Never hard-code a container IP.
38. (2026-10-05) A query flag like `?download=1` doesn't deserialize into `bool` (serde wants
    "true"/"false"; axum answers 400). Use `Option<String>` and `.is_some()`.
39. Array-of-tuple headers in a response must have one value type: `[(CONTENT_TYPE, "a".to_string()),
    (CONTENT_DISPOSITION, format!(..))]`, not a `&str` next to a `String`.
40. The `time` crate parses RFC 3339 only with its "parsing" feature; `OffsetDateTime::parse` is
    missing otherwise.

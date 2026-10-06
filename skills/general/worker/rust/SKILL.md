---
name: worker/rust
description: Writing Rust for the Kompanion server, runner and helpers.
roles: [worker, reviewer]
tags: [rust, sqlx, axum, tokio]
paths: ["**/*.rs", "**/Cargo.toml"]
---
# Worker: Rust (Kompanion server, runner, machine-stats)

Topic lessons moved into cards (sql, processes-files, axum-api), loaded when a job needs them. Add new lessons to the card they belong to.

7. (2026-10-03) Traits must be in scope for their methods, e.g. `use openidconnect::OAuth2TokenResponse as _;` for `access_token()`.
10. (2026-10-03) Parsing "key: value" lines: use `split_once(':')`, never `split(':')` (PCI slots like 0000:03:00.0 contain colons); strip units like " ns" before parsing numbers.
13. Context files are for reading signatures. Never copy them into your output: write only the
    file you were asked for, and call the other modules through `use crate::...`.
14. `?` only works in functions that return `Result` or `Option`. In a function returning
    `Outcome`, use `let Some(x) = ... else { return Outcome { .. } };` or `match`.
16. Every element of a `Vec<String>` must be a `String`. Mixing `"--needed"` (a `&str`) in does not
    compile. Use a helper: `fn argv(parts: &[&str]) -> Vec<String> { parts.iter().map(|s| s.to_string()).collect() }`.
17. An edit replaces only the matched text: `content.replacen(old, new, 1)`. Never write `new`
    as the whole file; that destroys the rest of the file.
19. `Option::and_then` needs a closure that returns an `Option`. For a plain value, use `.map`
    (`fs::read_to_string(p).ok().map(|s| s.trim().to_string())`). `?` inside a closure
    only works when the closure itself returns `Option` or `Result`.
20. Tests call functions with exactly their signature (`&[String]`, so `&["htop".to_string()]`,
    not `vec![...]`), and only use APIs that exist. `Grants` has no `Default`; an empty
    `Grants` comes from `Grants::load` on a path that does not exist. An empty file is
    invalid JSON.
22. To iterate an `Option` or `Result` of an iterator, use `.into_iter().flatten()`
    (`fs::read_dir(p).into_iter().flatten()`). `Option<ReadDir>` has no `.flatten()`.
    Compare `&&str` with `String` by dereferencing: `list.iter().any(|a| *a == s)`.
24. Time: keep units straight. Seconds since the epoch become days with `div_euclid(86_400)`.
    Timestamps compared as strings must use one format (RFC 3339 UTC with time of day).
    Test date code with known real dates (`civil(20_729) == (2026, 10, 3)`). A clock bug
    once made every expiring grant look expired.
32. The `regex` crate has no lookahead or lookbehind. When the regex only answers yes or no, turn
    `(?=X)` into a plain group `(X)`; when the match text is used, match more and trim it in code.
34. (2026-10-04) The deploy image builds on musl (`rust:1-alpine`). A crate with C code can build
    on glibc and fail on musl: sqlite-vec 0.1.9 uses `u_int8_t`, so the deploy broke while CI was
    green. CI's server job now uses the same image; `CFLAGS` in the Dockerfile and in build.yml map
    the BSD names to `uint8_t`/`uint16_t`/`uint64_t`. Before adding a `-sys` or C crate, build the
    Docker image (or watch the server job) once.
40. The `time` crate parses RFC 3339 only with its "parsing" feature; `OffsetDateTime::parse` is
    missing otherwise.
46. (2026-10-05) `Instant` and `Duration` don't compare: use `start.elapsed() < limit`. Socket helpers
    write answers to the socket, not stdout, and return errors instead of panicking.
50. (2026-10-05) Use only crates listed in `Cargo.toml` (a made-up `glob` crate broke the build);
    reuse the module's own helpers. Skill cards are Markdown with a `---` header, never JSON.
    `?` works only in a function or closure that returns Option/Result; in a closure returning
    bool chain `.and_then(..).is_some_and(..)`. `trim_matches` takes one pattern: pass a closure
    (`|c: char| "()[]".contains(c)`). Cut strings with `strip_prefix`/`strip_suffix`, never at
    hand-counted indexes (`[12..]` for the 13-character "_model-notes/" left a "/"). When porting
    Python, its `'text'` strings become `"text"` in Rust: single quotes are one `char`.
51. (2026-10-06) Slicing off quotes or brackets: check the length first (`s.len() >= 2`); a token
   that is one quote character makes `&s[1..s.len() - 1]` panic.
52. (2026-10-06) Every call to an `async fn` ends in `.await`; a match arm that returns its result
   without it hands back a future, not a String.
53. (2026-10-06) A guard that should apply to some tools names them (`matches!(job["tool"].as_str(),
   Some("edit_file" | "write_file"))`), and uses the helper the prompt names, never a hand-made
   `contains`. A refusal on every tool blocked reading the files too.
55. (2026-10-06) A fix round fixes only errors in the file you edit. An error in another file is
   not yours: leave unrelated code (another function, another query's binds) unchanged.

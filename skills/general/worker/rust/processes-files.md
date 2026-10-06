---
name: worker/rust/processes-files
description: Rust that runs processes and touches files: argv, pipes, temp folders in tests, sysfs, binary formats, the runner's job shape.
roles: [worker, reviewer]
tags: [process, command, argv, pipe, file, files, fs, path, runner, sysfs, binary, wav, stats, temp]
paths: ["runner/**", "machine-stats/**", "gpu-helper/**"]
---
# Worker: Rust (Kompanion server, runner, machine-stats): processes files

5. (2026-10-01) Rates from counters need the previous sample: compute the delta first, then store the new sample. Overwriting first gives zero every time. Test with two reads.
6. (2026-10-01) sysfs: `class/drm/cardN` only (skip `cardN-DP-1` connectors); every file may be missing, so every read returns an `Option`.
12. Tests clean up only what they created. Never `remove_dir_all(std::env::temp_dir())`:
    that deletes the whole system temp dir. Make a unique subdir
    (`temp_dir().join(format!("kk-<test>-{}", std::process::id()))`), use it, and remove only that
    subdir; or remove just the one file with `fs::remove_file`.

15. Build a command as one argv list: the program is `argv[0]`, the args are `&argv[1..]`.
    Never pass the program name as both the program and the first argument.
18. Process plumbing: `Command::current_dir` takes a path, not an `Option`
    (`if let Some(d) = cwd { cmd.current_dir(d); }`). `ChildStdout` and `ChildStderr` are
    different types, so box them as `Box<dyn Read + Send>` for one drain closure.
    `std::io::Take` has no public `new`; use `Read::take(reader, n)`.

21. When you read a file, keep the value: `let Ok(before) = fs::read_to_string(&f) else { ... }`.
    Calling `read_to_string` only to check `is_err()` leaves the String empty.
23. Rust runs tests in parallel. Each test that touches files needs its own folder: put the
    test name in it (`temp_dir().join(format!("kk-edit-{name}-{}", process::id()))`). When
    tests share one folder, one test's cleanup deletes another's files.
28. Use only the crates in Cargo.toml. There is no `dirs` crate in the runner: the home folder is
    `std::env::var("HOME")`. Trait methods need their trait imported: `Permissions::from_mode` needs
    `std::os::unix::fs::PermissionsExt`, `write_all` needs `std::io::Write`.
29. Know the data shape before reading it. A runner job is flat: `{"tool": "edit_file", "path": ..,
    "old": .., "new": ..}`, with no nested `args`. Read `job["path"]`, never `job["args"]["path"]`,
    and write the tests with the same flat shape.
45. (2026-10-03) Process and pipe plumbing: never box an `Option` as `Box<dyn Read>`, keep the result
    of `take()`, keep every parameter of the signature you were given (`cwd`).
47. (2026-10-04) Binary formats (WAV and similar): take the byte offsets from the spec in the prompt
    and test against a real file, not only a fixture you wrote with the same offsets.
48. (2026-10-06) When the prompt says to build on an existing function (`cards(root)` walks the
    folders), call it; never re-implement its body. A rewritten walk dropped subfolders, and a
    SEARCH text copied from your rewrite instead of the file fails to apply.

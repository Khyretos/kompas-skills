**Goal:** Kompanion runs on anyone's server: nothing in the code assumes kireserver, OVMS's container name, `gpu-mode.sh` or Kees's NFS paths.

**Machine / role:** kireserver; server (Rust: `server/src/config.rs`, `server/src/gpus/`, `server/src/hoststats.rs`), `tools/qwen/pipeline.py`, docs.

**Depends on:** nothing.

**Steps**
1. List every setup assumption with `grep -rn` for host names, IPs, `/home/`, `ovms`, `gpu-mode.sh`, `soucouyant`, `kireserver` in `server/`, `runner/`, `gpu-role/`, `tools/`, `web/src`; write the list into the task chat before changing anything.
2. Providers: every base URL, model name and extra body comes from `kompanion.toml`; `tools/qwen/pipeline.py` reads the same file (provider `ovms`, role `worker`) instead of `docker inspect ovms`.
3. GPU tools: the GPU role switch calls a configurable helper (`[gpu.role] helper_socket = ...`, commands listed in config); without it the switch is simply off and the UI says so.
4. Machines: names, paths and folders come only from pairing and grants, never from code.
5. `server/kompanion.example.toml` documents every setting with a working default for a single PC with one local model.
6. A fresh-install test in CI: start the container with only the example config and check `/api/status`, sign-in and the demo task run.

**Done when:** the grep finds no setup facts outside config and docs; the CI fresh-install job passes; kireserver keeps working with its own `kompanion.toml`.

**How to test:** the CI job; then deploy on kireserver and run one chat and one task.

**Principles:** FOSS only; local models draft, a stronger model reviews and writes lessons; Kreative Kompas palette with bright text (7:1 headers and buttons); every action updates without a reload; English and Spanish at least; one SSO account per person; security first (no open endpoints, grants for every action on a PC).

_Kees, 2026-10-05. Drafting by the local AI (Coder on kireserver), reviewed by Claude._

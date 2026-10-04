#!/usr/bin/env python3
"""M6, the two-brain game studio: the task plan (2026-10-04, planning only).

One source for both files next to it:
  m6-tasks.md    readable plan, one section per task
  m6-tasks.json  `kompanion-server import` file (project "Kreative Kompanion")
Run: python3 docs/plans/m6_tasks.py
"""
import json, os

HERE = os.path.dirname(os.path.abspath(__file__))

# Already built; M6 builds on these and does not plan them again.
EXISTING = [
    "Project types: a game project with linked library assets (Tasks tab panel, 0017).",
    "The Assets library (owned by the asset thread): index, previews, AI tags, games, needs and picks.",
    "W2: a task runs by itself on a computer (plan with done-when, steps under grants, check, review, fix rounds).",
    "The PC agent, runner tools and grants; the Capabilities page (models, computers, tools, skills).",
    "ComfyUI on the A770 (kireserver) and on the RX 9070 XT (soucouyant), HeartMuLa music, MOSS sound effects, "
    "gpu-mode.sh, comfy-out-mover, and the gpu-studio skill (Services/ai/ai-skills/gpu-studio).",
]

PHASES = [
    ("A", "GPU scheduler", "Nothing in M6 is safe without it: two GPUs, many models, and one VRAM mistake freezes a machine."),
    ("B", "Asset workflows", "Saved ComfyUI graphs and audio jobs that the scheduler can run on either GPU."),
    ("C", "Studio view", "Describe a game, see code and asset tasks side by side, approve assets, build and test headless."),
    ("D", "The fox demo", "The end-to-end test of the whole milestone."),
]

T = []


def task(key, phase, title, machine, depends, goal, steps, done):
    T.append(dict(key=key, phase=phase, title=title, machine=machine, depends=depends, goal=goal, steps=steps, done=done))


# ---------------------------------------------------------------- Phase A
task("M6-01", "A", "GPU ledger: what is on each GPU right now",
     "kireserver server code; reads both PCs (runner on soucouyant, gpu-helper on kireserver)",
     [],
     "Kompanion knows, per GPU, its total VRAM, what is loaded (model or app, with VRAM) and what is free, live. "
     "This ledger is what every later scheduling decision reads.",
     [
         "List the GPUs: A770 (kireserver, AI), A580 (kireserver, desktop and Jellyfin: protected, never scheduled), RX 9070 XT (soucouyant). Put them in kompanion.toml under [[gpu]] with id, machine, pci slot, vram_gb and `schedulable`.",
         "Per GPU, collect what holds VRAM: OVMS models (GET /v1/config: state per model, plus the per-container VRAM the gpu-helper already sums from fdinfo), ComfyUI (its /system_stats), HeartMuLa and MOSS (their containers), Ollama on soucouyant (GET /api/ps: size_vram per model).",
         "Add a `gpu_ledger` table (gpu_id, holder, kind model|app|job, vram_mb, since, job_id NULL) rebuilt from the probes every 10 s and on every scheduler change; keep the last 24 h of samples for the trace view (M6-04).",
         "Count what a loaded model will still grow by, not only what it holds now: an LLM's KV cache grows with every parallel sequence and context length (2026-10-04: Coder with 8 parallel sequences outgrew the A770 and OVMS segfaulted twice; now max_num_seqs 2). Per model record weights + KV cache per sequence x max sequences as its reserved peak.",
         "Expose GET /api/gpus (ledger + free VRAM + which role the machine has) and an SSE event `gpus` on change.",
         "Unit tests with recorded probe answers (OVMS config, ComfyUI stats, Ollama ps, fdinfo) for each parser.",
     ],
     "GET /api/gpus shows for each schedulable GPU what is loaded and how much VRAM is free, within 10 s of a change (checked by loading and unloading Coder with gpu-mode.sh); the A580 shows as protected.",
     )

task("M6-02", "A", "Scheduler core: jobs reserve VRAM, priorities, tonight batches",
     "kireserver server code", ["M6-01"],
     "Every GPU job asks the scheduler first. A job reserves VRAM on one GPU before it starts and releases it when it ends, "
     "so two jobs never overcommit a GPU. Chat beats code, code beats asset batches; batches can wait for the night.",
     [
         "Model a job: kind (chat | code | asset), what it needs (model or app, VRAM estimate in MB from a per-workflow table, host RAM estimate), allowed GPUs, `tonight` flag, owner task.",
         "Queue per priority (chat > code > asset). A job starts only when its GPU has the VRAM free after reservations, counting every loaded model at its reserved peak (weights plus KV cache for its parallel sequences, M6-01), AND the host has enough MemAvailable (keep 5 GB spare: the 2026-10-04 freeze came from host RAM).",
         "Idle models unload: an OVMS or Ollama model nobody used for N minutes (setting, default 10) may be unloaded to make room for a waiting job, never while a request is running (OVMS: no request for 60 s, as gpu-mode.sh does).",
         "`tonight` jobs wait until the night window (23:00-06:00 local, a setting) unless the GPU is idle anyway.",
         "Never take a GPU from a running job; never schedule on the A580; never start an asset job on soucouyant while it reports busy (game running, Steam) or while Kees uses it for games (soucouyant only works for game development, not as an always-on server).",
         "Persist the queue in SQLite (core migration, next free number below 0100) so a restart resumes it.",
         "Tests: a fake ledger and fake clock covering: overcommit refused, priority order, idle unload, tonight wait, A580 never chosen, busy PC skipped, restart resume.",
     ],
     "With a fake ledger the scheduler never overcommits a GPU in 1,000 random job sequences (property test), and on the real machines two ComfyUI jobs plus a chat in parallel never produce an out-of-memory or device-lost error.",
     )

task("M6-03", "A", "Coder and artist roles: switching a PC safely",
     "kireserver (A770) and soucouyant (RX 9070 XT)", ["M6-02"],
     "Each PC is either the coder (the LLM writes and tests game code) or the artist (ComfyUI, music, sound effects). "
     "The scheduler switches roles when one side's queue is empty, without killing running work.",
     [
         "kireserver: wrap `Services/ai/gpu-share/gpu-mode.sh` (artist, studio <app>, coder, status) in a small privileged helper like kompanion-gpu-helper: its own system user, a unix socket with a fixed command list, no shell, no arguments beyond the app name. The server never gets Docker access.",
         "soucouyant: artist means Ollama has unloaded its model (`keep_alive: 0` on an empty request) and ComfyUI-ROCm has the GPU; coder means ComfyUI is idle and Ollama may load qwen3.5:9b again. Add runner tools `gpu_role` (status/artist/coder) with a grant `system: gpu`.",
         "Rules from the gpu-studio skill: one studio app at a time on kireserver; `coder` refuses while a studio app is busy; after `coder`, check that Coder answers a real chat request, not only AVAILABLE; restart OVMS if its GPU context broke (CL_INVALID_EVENT in the log).",
         "Swap policy: when the artist queue is empty and code jobs wait, switch that PC to coder (and the other way); never switch during a job; at most one switch per PC per 5 minutes.",
         "Show each PC's role and the last switch in Machines and Capabilities.",
     ],
     "With jobs queued on both sides, each PC switches role by itself when its queue empties, a switch never interrupts a running job (log shows none cancelled), and Coder answers within 60 s after every switch back.",
     )

task("M6-04", "A", "Trace timeline per machine: jobs, VRAM, watts",
     "kireserver server code + web", ["M6-01", "M6-02"],
     "One screen shows what each GPU did over time: which job ran, how much VRAM it held and how many watts it drew, so a slow or crashed run can be explained afterwards.",
     [
         "Store job start/end, GPU, peak VRAM and errors with the ledger samples (M6-01) and the power the machine stats already report.",
         "Web: a timeline per GPU (last 1 h / 24 h): job bars coloured by kind, a VRAM line, a watts line; a click on a bar opens the job and its task. Palette colours and the Tasks card style; works on a phone (scrolls horizontally).",
         "Live: new samples arrive over SSE; no refresh.",
         "Playwright test with demo data.",
     ],
     "During the fox demo (M6-15) the timeline shows both GPUs busy in parallel, every job with its VRAM peak, and no gap without an explanation.",
     )

# ---------------------------------------------------------------- Phase B
task("M6-05", "B", "Workflow registry: saved ComfyUI graphs with parameters and licences",
     "kireserver server code; graphs run on either ComfyUI", ["M6-02"],
     "Asset jobs use saved, tested ComfyUI API graphs with named parameters, a VRAM estimate and the licences of every model they load; only FOSS-licensed models can run for company assets.",
     [
         "A folder `studio/workflows/<name>/` in the repo: `graph.json` (ComfyUI API format), `workflow.toml` (title, parameters with types and defaults mapped to node inputs, outputs, vram_mb and ram_mb measured with comfy-bench.sh, models with licence).",
         "A licence list: allowed (Apache-2.0, MIT, BSD, CC0, CC-BY with attribution recorded) and refused (OpenRAIL, non-commercial, unknown). The gpu-studio skill lists today's models: Z-Image Turbo, FLUX.2 klein 4B, HeartMuLa and MOSS are Apache-2.0; SD 1.5/SDXL/Illustrious checkpoints are not OSI and need Kees's yes; Stable Audio Open, AudioLDM2, TangoFlux, MMAudio and MusicGen are non-commercial and stay out.",
         "Run a workflow: fill parameters, POST /prompt to the chosen ComfyUI, follow progress over its websocket, collect outputs to /media/Generated (soucouyant: through comfy-out-mover), record provenance (workflow, parameters, seed, models, licences) per output.",
         "Capabilities page: a Workflows section with each workflow's status (models present on both PCs, last run, average time).",
         "Tests: parameter mapping, licence refusal, a fake ComfyUI for the run loop.",
     ],
     "A workflow with a refused licence cannot be started; Z-Image Turbo's test graph runs from Kompanion on both ComfyUI instances with identical parameters and the output carries its provenance.",
     )

task("M6-06", "B", "Character sheet and sprites workflow",
     "artist PC (either GPU)", ["M6-05"],
     "From a short character description, make a consistent character sheet and game-ready 2D sprites with transparent backgrounds.",
     [
         "Character sheet graph (Z-Image Turbo, 8 steps, VAEDecodeTiled): front, side, back and three poses on one sheet, fixed seed per character, style prompt shared by the game.",
         "Sprites graph: pose frames from the sheet (image-to-image with the sheet as reference), background removed (a FOSS matting model; check its licence), trimmed and packed into a sprite sheet PNG plus a JSON atlas (frame rects, pivot).",
         "Parameters: description, style, size (32-256 px), frame list (idle, run, jump), palette hint.",
         "Measure VRAM and time on both GPUs with comfy-bench.sh; write them into workflow.toml.",
         "Lesson notes for the gpu-studio skill (what kept the character consistent, what broke).",
     ],
     "For \"a small orange fox, pixel-art style\" the workflow returns a sheet and a 64 px idle/run/jump sprite sheet with atlas whose frames show the same character, in under 2 minutes on either GPU.",
     )

task("M6-07", "B", "Animation loops and image-to-3D (.glb)",
     "artist PC (either GPU)", ["M6-05", "M6-06"],
     "Short looping animations from sprites, and a 3D model (.glb) from a character image for 3D games, both with FOSS models only.",
     [
         "Research and pick FOSS models (licence checked per model, Kees asked before anything that is not OSI): a frame interpolation model for 2D loops, an image-to-3D model that fits 16 GB VRAM.",
         "Loop graph: in-between frames for a sprite cycle, loop point checked (first and last frame match), exported as frames + atlas and a preview GIF/WebM.",
         "Image-to-3D graph: one character image to a textured .glb (scale, origin at the feet, under a triangle budget parameter); a preview render.",
         "Write both into the registry with measured VRAM and time.",
     ],
     "A run cycle loops without a visible jump, and a .glb from the fox sheet opens in kk-engine (or a glTF viewer) with its texture; both models' licences are recorded and allowed.",
     )

task("M6-08", "B", "Music and sound effects through the scheduler",
     "kireserver A770 (HeartMuLa, MOSS) or soucouyant", ["M6-02", "M6-05"],
     "Background music loops and sound effects are asset jobs like images: queued, scheduled, with provenance, normalised and ready for the game.",
     [
         "Wrap HeartMuLa (POST /music) and MOSS (POST /sfx) as workflow kinds with parameters (tags, seconds; prompt, seconds) and measured VRAM/RAM (MOSS: about 11.2 GiB VRAM and 9 GB host RAM; one studio app at a time on kireserver).",
         "Post-process with ffmpeg: loudness normalise (EBU R128), trim silence, make music loop seamlessly (crossfade the ends), export OGG for the game and WAV for editing.",
         "The scheduler starts the right studio app (`studio heartmula` / `studio moss-sfx`) through the M6-03 helper and switches back when the queue empties.",
         "Tests with fake servers; a real run of each on the A770 under the RAM watchdog.",
     ],
     "\"A short jump sound\" and \"a calm forest loop, 30 s\" come back as OGG files that loop or end cleanly, without the host dropping under 5 GB free RAM.",
     )

task("M6-09", "B", "Generated assets in the library, with provenance",
     "kireserver; coordinate with the asset thread (it owns the Assets library)", ["M6-05"],
     "Everything the studio makes appears in the Assets library like bought packs do, marked as generated, with its prompt, models and licences, and can be attached to a game project.",
     [
         "Agree with the asset thread how /media/Generated becomes a library source (a pack per game or per batch, provenance stored next to it); do not change asset tables without them.",
         "Store provenance per output (workflow, parameters, seed, models with licences, which GPU, time) in a sidecar JSON the scan reads.",
         "Generated assets show a \"generated\" chip and their provenance in the asset detail; they can be attached to a game project (existing project_asset links).",
     ],
     "A generated sprite sheet shows up in Assets within a minute of finishing, with its prompt and licences, and attaches to a game project.",
     )

# ---------------------------------------------------------------- Phase C
task("M6-10", "C", "Game project settings: repo, engine, build and headless test",
     "kireserver server + web", [],
     "A game project knows where its code lives and how to build and test it without a screen, so the studio can check its own work.",
     [
         "Extend the game project type (project panel, core migration below 0100): repo folder and computer (as programming projects have), engine (kk-engine, or other), build command, headless test command (e.g. kk-engine's headless runner with a test scene), screenshot folder.",
         "Validate commands run under the project's grants only (no new rights); show the last build and test result on the panel.",
         "Playwright test for the settings; server test for validation.",
     ],
     "A game project with kk-engine settings builds and runs its headless test from the panel on soucouyant, and the result shows in the panel live.",
     )

task("M6-11", "C", "Describe the game, get code tasks and asset tasks",
     "orchestrator (Coder on kireserver)", ["M6-10", "M6-05"],
     "Kees describes what the game needs in plain words; Kompanion splits it into code tasks and asset tasks with dependencies (code waits for the assets it uses).",
     [
         "Planner prompt with the game's settings, the Capabilities summary (models, workflows, PCs) and the project's linked assets: output JSON with code tasks (W2 format: steps with done-when) and asset tasks (workflow + parameters, or \"use library asset X\").",
         "Prefer existing library assets (asset thread's needs and picks) before generating new ones.",
         "Show the plan for approval before anything runs; Kees can edit, drop or add tasks.",
         "Tests with recorded model answers (including bad JSON, missing fields, impossible workflows).",
     ],
     "\"A small 2D platformer level with a fox character, jump sound, background music\" becomes a plan with code tasks (level, player, camera, audio hookup) and asset tasks (fox sprites, tiles, jump SFX, music loop) that Kees can approve in one click.",
     )

task("M6-12", "C", "Studio view: asset cards, approve or redo, commit to the repo",
     "web + kireserver; commits through the runner on the coder PC", ["M6-09", "M6-11"],
     "One view per game project shows its code and asset tasks side by side; generated assets can be approved, redone with a note, or rejected, and approved assets are committed into the game repo.",
     [
         "Studio view in the game project: two columns (code, assets), Tasks card style, live progress over SSE, previews (image, sprite animation, audio player, .glb preview image).",
         "Approve: copy into the repo at the engine's asset path convention, `git add` and commit with provenance in the message (runner tools under the project's grants). Redo: same workflow with a note added to the prompt and a new seed. Reject: removed from the queue, kept in the library.",
         "Optimistic UI, rollback with an error; works on a phone.",
         "Playwright tests with demo data.",
     ],
     "Approving a sprite sheet puts it in the repo with a commit naming the prompt and models, and the code task that waited for it starts by itself.",
     )

task("M6-13", "C", "Headless build and test with screenshots, review loop",
     "coder PC (W2 on soucouyant by default)", ["M6-10", "M6-12"],
     "Every code change is built and run headless; screenshots and the test result go to the reviewer, who can send it back for a fix (up to the W2 round limit).",
     [
         "Use W2's check step with the game's build and headless test commands; collect screenshots from the screenshot folder.",
         "Reviewer gets the diff, the test output and the screenshots (vision: Coder handles images); findings go back as a fix round.",
         "Show screenshots in the task's chat and the Studio view.",
     ],
     "A deliberately broken change (player falls through the floor) is caught by the headless test or the screenshot review and fixed in a later round without Kees.",
     )

task("M6-14", "C", "Both brains at once: code and assets in parallel",
     "both PCs", ["M6-03", "M6-12", "M6-13"],
     "Code tasks run on the coder PC while asset tasks run on the artist PC; when one side runs dry, its PC switches role and helps the other side.",
     [
         "Dispatcher: code tasks to the PC in coder role (W2), asset jobs to the artist PC (scheduler); dependencies from M6-11 respected.",
         "Role swap through M6-03 when a queue empties; nothing is cancelled.",
         "Studio view shows which PC works on what (link to the M6-04 timeline).",
     ],
     "With 5 code and 5 asset tasks queued, both GPUs are busy at the same time for most of the run, and each PC switches role at least once when its side is done.",
     )

# ---------------------------------------------------------------- Phase D
task("M6-15", "D", "The fox demo, end to end",
     "both PCs; Kees plays the result", ["M6-01", "M6-02", "M6-03", "M6-04", "M6-05", "M6-06", "M6-08", "M6-09", "M6-10", "M6-11", "M6-12", "M6-13", "M6-14"],
     "One description yields a playable level: code, sprites, sound and music, built and tested headless, reviewed in Kompanion, with both GPUs busy and no VRAM crash. This is M6's done-when.",
     [
         "Create a game project \"Fox demo\" (kk-engine, repo on soucouyant) with its build and headless test commands.",
         "Describe: \"A small 2D platformer level with a fox character, jump sound and background music.\" Approve the plan.",
         "Let it run; approve or redo assets in the Studio view; watch the timeline.",
         "Kees plays the level (on soucouyant, when he chooses) and gives a verdict in the chat; fixes go through the review loop.",
         "Write the lessons into skills (orchestrator, worker, gpu-studio) and a capability note for the 9B models in docs/ai-capability.",
     ],
     "The level builds and passes its headless test; it has a fox sprite, a jump sound and a looping music track, all approved in Kompanion and committed; the M6-04 timeline shows both GPUs busy at the same time; no out-of-memory, device-lost or OVMS crash during the run; Kees has played it.",
     )


def description(t):
    deps = ", ".join(t["depends"]) if t["depends"] else "none"
    steps = "\n".join(f"{i}. {s}" for i, s in enumerate(t["steps"], 1))
    return (f"**Goal:** {t['goal']}\n\n**Machine / role:** {t['machine']}\n\n**Depends on:** {deps}\n\n"
            f"**Steps**\n{steps}\n\n**Done when:** {t['done']}\n\n_M6 two-brain game studio, phase {t['phase']}. Planning only: not started._")


def main():
    md = ["# M6: the two-brain game studio, task plan", "",
          "Planned 2026-10-04 (planning only; nothing built yet). Kees describes what his game needs, Kompanion splits it into",
          "code tasks and asset tasks, and the two PCs work in parallel as coder and artist, swapping roles when one side runs dry.",
          "", "Done when (whole milestone): one description (\"a small 2D platformer level with a fox character, jump sound,",
          "background music\") yields code, sprites, sound and a music loop, built and tested headless, played and reviewed in",
          "Kompanion, with both GPUs busy and no VRAM crash.", "", "Already built, not planned again:", ""]
    md += [f"- {e}" for e in EXISTING]
    md += ["", "Import into Kompanion (the project \"Kreative Kompanion\"; tasks land after the existing ones, as queued):", "",
           "```sh", "docker exec -i kreative-kompanion sh -c 'cat > /tmp/m6.json && kompanion-server import /tmp/m6.json khyretos; rm /tmp/m6.json' \\",
           "  < ~/Docker/Personal-projects/kreative-kompanion/docs/plans/m6-tasks.json", "```", "",
           "Running it again updates these tasks instead of adding copies.", ""]
    for key, name, why in PHASES:
        md += [f"## Phase {key}: {name}", "", why, ""]
        for t in [t for t in T if t["phase"] == key]:
            md += [f"### {t['key']} {t['title']}", "", description(t), ""]
    open(os.path.join(HERE, "m6-tasks.md"), "w").write("\n".join(md))
    project = {"id": "kompanion-next-wave", "name": "Kreative Kompanion",
               "description": "Next wave W1-W4 (2026-10-03): orange accents, a working orchestrator, the capabilities page, optional voice.",
               "tasks": [{"id": f"kompanion-{t['key']}", "title": f"{t['key']} {t['title']}", "description": description(t), "state": "queued"} for t in T]}
    json.dump({"projects": [project]}, open(os.path.join(HERE, "m6-tasks.json"), "w"), indent=1, ensure_ascii=False)
    print(f"{len(T)} tasks written")


if __name__ == "__main__":
    main()

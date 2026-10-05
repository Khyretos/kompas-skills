**Goal:** Kees's skills and lessons become useful to anyone: general knowledge ships with Kompanion and can be seen and edited by every user, while everything about Kees's own setup stays private. One copy serves both Kompanion and Open WebUI.

**Machine / role:** kireserver; Forgejo repos, `tools/skills/`, server code (`server/src/capabilities.rs`), web app (Capabilities page).

**Depends on:** "Skills: smart loading within a size budget per model" (the loader merges the layers).

**Steps**
1. Create new public repo `kompas-skills` on Forgejo (mirrored to GitHub like the others) with licence CC BY-SA 4.0 containing only general knowledge: `work-habits.md`, prompting workers and fix rounds, shell, git, Python, colour themes, and `_model-notes/<family>/` for model quirks.
2. Include `kompas-skills` in the Kompanion repo as a git subtree at `skills/general/`. The Open WebUI Kompas pipe reads the same `kompas-skills` files (its `SKILLS_DIR`), so `~/Docker/Services/ai/ai-skills` stops being a second copy.
3. Keep Kompanion's own coding lessons (templates, sqlx rules, drafting pipeline) in the Kompanion repo `skills/`.
4. Create new private repo `kompanion-skills-kees` on Forgejo to host hosts, IPs, paths, NFS mounts, soucouyant's fish shell and Hyprland, OVMS and the A770, the brand palette, and Kees's house rules (FOSS only, bright text, English and Spanish first); mount it read-only into the server at `/skills-local`.
5. Configure the loader to merge general, Kompanion, and local layers where a local card with the same `name` extends or replaces the general one using front matter `overrides: <name>`.
6. Split today's files by tagging each lesson `general` or `private` (Coder drafts tags, Claude reviews), moving them to the correct locations while preserving git history in commit messages.
7. Add CI check in `kompas-skills` that fails on IP addresses, home paths (`/home/`, `~/`), host names from a deny list, and email addresses.
8. New lessons become proposals: the reviewer writes a change (a branch and a PR, or a pending card in Kompanion), says whether it is general or private, and Kees approves it. Skills go straight into every prompt, so each change must be visible and show who made it.
9. Update Capabilities page to show a Skills section listing each file with its layer (general, Kompanion, private), allowing Kees to edit files, view git commit history, and move lessons between general and private.

**Done when:** `kompas-skills` has no setup facts (the CI check passes); Kompanion and Open WebUI load the same general files; a private card overrides a general one in a test; a lesson edited on the Capabilities page lands as a commit in the right repo.

**How to test:** run the CI check against a file with a planted IP and home path (must fail); loader unit test for an override; one edit through the Capabilities page.

_Kees, 2026-10-05. Drafting by the local AI (Coder on kireserver), reviewed by Claude. FOSS only._

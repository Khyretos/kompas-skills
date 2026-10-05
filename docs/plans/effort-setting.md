**Goal:** Kees can choose how hard his models work, the way Claude has an effort setting: next to the message box, per chat, and per task.

**Machine / role:** kireserver; server (Rust: `server/src/llm.rs`, `server/src/taskrun/`, `server/src/config.rs`), web app (`web/src/views/conversation.ts`), Open WebUI pipe (`~/Docker/Services/ai/ai-skills/_pipes/kompas_orchestrator.py`).

**Depends on:** nothing; works with the skills loader once that exists (the budget follows the effort).

**Steps**
1. Define four levels, one vocabulary everywhere: Auto (default), Low, Medium, High.
   - Low: thinking off, at most 2 tool rounds, 1 review round, small skill budget.
   - Medium: today's behaviour: thinking off, 6 tool rounds, up to 3 fix rounds.
   - High: thinking on, the reviewer also self-checks, 10 tool rounds, larger skill budget.
   - Auto: the orchestrator picks a level per task from the task size and the skills' `effort` front matter, and shows what it picked.
2. In the composer, on the line under the text box, right side (across from "Enter sends, Shift+Enter adds a line."): add one compact chip "<model> · <effort> ▾" (for example "Coder · Auto"); clicking opens a menu with the four levels, each with a one-line description. Keyboard reachable, 44 px touch target on phones, Kees's bright-text rule (7:1).
3. The choice is stored per chat and sent with each message; a task started from the chat inherits it; the task detail lets you change it before Start; task cards and the run report show the effort used.
4. Settings > Roles: add a default effort per role.
5. What a level means per model lives in the provider config, for example in `kompanion.toml`: `[provider.effort.high] extra_body = { chat_template_kwargs = { enable_thinking = true } }` for OVMS (Qwen), `model = "deepseek-reasoner"` for DeepSeek at High, a thinking budget for Anthropic. Without a mapping a level only changes rounds and budgets.
6. The call log records the effort of every model call.
7. Open WebUI pipe: rename the levels fast, balanced, deep to Low, Medium, High (same meaning as now), so both apps use the same words.

**Done when:** choosing High in a chat makes the next Coder call send `enable_thinking: true` (visible in the call log) and the task card shows "High"; Auto shows the level it picked; the chip updates without a reload.

**How to test:** Playwright test in demo mode for the chip, the menu and the per-chat memory; one real chat at Low and High, compare the call log.

_Kees, 2026-10-05. Drafting by the local AI (Coder on kireserver), reviewed by Claude. FOSS only._

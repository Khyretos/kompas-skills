// Capabilities: what Kompanion can use right now.
import { html, type SafeHtml } from "../core/html";
import { renderTimeline, type TlGpu } from "./gputimeline";
import { relTime } from "../core/time";
import { icon } from "./icons";

export interface CapModel {
  id: string;
  name: string;
  local: boolean;
  status: "ok" | "down";
  error: string | null;
  models: string[];
  roles: string[];
  lastError: { text: string; at: string } | null;
}

export interface CapGrant {
  target: string;
  rights: string[];
  expires: string | null;
}

export interface CapComputer {
  id: string;
  name: string;
  online: boolean;
  lastSeen: string;
  runnerVersion?: string | null;
  grants: CapGrant[];
}

export interface CapTool {
  name: string;
  description: string;
  needs: string;
}

export interface CapIndex {
  id: string;
  name: string;
  items: number;
  of: number;
  failed: number;
  model: string;
  status: "ok" | "partly" | "empty";
}

export interface CapSkill {
  id: string;
  title: string;
  lessons: number;
  updated: string;
}

export interface CapHolding { name: string; kind: string; nowMib: number; peakMib: number; busy: boolean }
export interface CapGpu { id: string; machine: string; totalMib: number; usedMib: number | null; reservedMib: number; otherMib: number; freeMib: number; schedulable: boolean; holdings: CapHolding[] }
export interface CapSwitch { from: string; to: string; by: string; startedAt: string; seconds: number; ok: boolean; coderAnswerS: number | null; error: string | null }
export interface CapRole { gpu: string | null; mode: string; app: string | null; switching: string | null; last: CapSwitch | null }

export interface Capabilities {
  gpus?: CapGpu[];
  gpuRole?: CapRole | null;
  models: CapModel[];
  computers: CapComputer[];
  tools: CapTool[];
  mcp: { name: string; status: string }[];
  indexes: CapIndex[];
  skills: CapSkill[];
}

function modelCard(m: CapModel): SafeHtml {
  const state = m.status === "ok" ? "done" : "failed";
  const label = m.status === "ok" ? "online" : "down";
  return html`
    <li class="task cap s-${state}">
      <div class="task-main">
        <span class="task-top">
          <span class="chip state">${label}</span>
          <span class="muted small">${m.local ? "local" : "cloud"}</span>
        </span>
        <span class="task-title">${m.name}</span>
        <span class="task-step">
          ${m.status === "down" ? (m.error ?? "") : `${m.models.length} model(s): ${m.models.slice(0, 4).join(", ")}${m.models.length > 4 ? "…" : ""}`}
        </span>
        <span class="task-meta">
          ${icon("spark")} ${m.roles.length ? m.roles.join(" · ") : "no role uses it"}
          ${m.lastError ? html` · last error <span class="cap-err" title="${m.lastError.text}">${relTime(m.lastError.at)}</span>` : ""}
        </span>
      </div>
    </li>
  `;
}

function computerCard(c: CapComputer): SafeHtml {
  const state = c.online ? "done" : "queued";
  const label = c.online ? "online" : "offline";
  const grantsText = c.grants.length
    ? c.grants.map((g) => `${g.target} (${g.rights.join(", ")})`).join(" · ")
    : "No grants: every step asks first";
  return html`
    <li class="task cap s-${state}">
      <div class="task-main">
        <span class="task-top">
          <span class="chip state">${label}</span>
        </span>
        <span class="task-title">${c.name}</span>
        <span class="task-step">${grantsText}</span>
        <span class="task-meta">
          ${icon("pc")} ${c.online ? "seen " : "last seen "}
          ${c.lastSeen ? relTime(c.lastSeen) : "never"} · runner ${c.runnerVersion ?? "before 0.4.5"}
        </span>
      </div>
    </li>
  `;
}

function toolCard(t: CapTool): SafeHtml {
  return html`
    <li class="task cap s-done">
      <div class="task-main">
        <span class="task-top">
          <span class="chip state">tool</span>
        </span>
        <span class="task-title"><code>${t.name}</code></span>
        <span class="task-step">${t.description}</span>
        <span class="task-meta">needs: ${t.needs || "nothing"}</span>
      </div>
    </li>
  `;
}

function mcpCard(m: { name: string; status: string }): SafeHtml {
  return html`
    <li class="task cap s-done">
      <div class="task-main">
        <span class="task-top">
          <span class="chip state">${m.status}</span>
        </span>
        <span class="task-title">${m.name}</span>
      </div>
    </li>
  `;
}

function indexCard(i: CapIndex): SafeHtml {
  const state = i.status === "ok" ? "done" : i.status === "partly" ? "needs_input" : "queued";
  const label = i.status === "ok" ? "ready" : i.status === "partly" ? "partly" : "empty";
  const step = `${i.items.toLocaleString("en")} of ${i.of.toLocaleString("en")} items${i.failed ? ` · ${i.failed} failed` : ""}`;
  return html`
    <li class="task cap s-${state}">
      <div class="task-main">
        <span class="task-top">
          <span class="chip state">${label}</span>
        </span>
        <span class="task-title">${i.name}</span>
        <span class="task-step">${step}</span>
        <span class="task-meta">${icon("spark")} ${i.model}</span>
      </div>
    </li>
  `;
}

function skillCard(s: CapSkill): SafeHtml {
  return html`
    <li class="task cap s-done">
      <button class="task-main" data-action="open-skill" data-id="${s.id}">
        <span class="task-top">
          <span class="chip state">${s.lessons} lessons</span>
        </span>
        <span class="task-title">${s.title}</span>
        <span class="task-step"><code>${s.id}</code></span>
        <span class="task-meta">${s.updated ? `updated ${relTime(s.updated)}` : ""}</span>
      </button>
    </li>
  `;
}

const gb = (mib: number) => `${(mib / 1024).toFixed(1)} GB`;

/** M6-01: what each GPU holds (at its peak: weights plus KV cache), what else uses it, what is free. */
function gpuCard(g: CapGpu, role?: CapRole | null): SafeHtml {
  const r = role && role.gpu === g.id ? role : null;
  const roleText = r ? (r.switching ? `switching to ${r.switching}…` : r.mode === "artist" && r.app ? `artist (${r.app})` : r.mode) : "";
  const last = r?.last ? `last switch ${r.last.startedAt.slice(11, 16)}: ${r.last.from} → ${r.last.to}, ${Math.round(r.last.seconds)} s${r.last.coderAnswerS !== null ? `, Coder answered in ${Math.round(r.last.coderAnswerS)} s` : ""}${r.last.ok ? "" : ` (failed: ${r.last.error ?? "unknown"})`}` : "";
  const state = !g.schedulable ? "queued" : g.freeMib < 1024 ? "needs_input" : "done";
  const label = !g.schedulable ? "protected" : `${gb(g.freeMib)} free`;
  const pct = (mib: number) => `${Math.min(100, Math.round((mib / Math.max(1, g.totalMib)) * 100))}%`;
  return html`
    <li class="task cap s-${state}">
      <div class="task-main">
        <span class="task-top"><span class="chip state">${label}</span><span class="muted small">${g.machine}</span>${r ? html`<span class="chip role" title="Coder (OVMS) or one studio app; switches by itself (M6-03)">${roleText}</span>` : ""}</span>
        <span class="task-title">${g.id} · ${gb(g.totalMib)}</span>
        <span class="vram-bar" role="img" aria-label="${`${gb(g.reservedMib)} reserved, ${gb(g.otherMib)} other, ${gb(g.freeMib)} free`}">
          <span class="vb-res" style="width:${pct(g.reservedMib)}"></span><span class="vb-other" style="width:${pct(g.otherMib)}"></span>
        </span>
        <span class="task-step">${g.holdings.length
          ? g.holdings.map((h) => `${h.name} ${gb(Math.max(h.nowMib, h.peakMib))}${h.busy ? " (busy)" : ""}`).join(" · ")
          : "Nothing loaded"}</span>
        <span class="task-meta">${icon("spark")} reserved ${gb(g.reservedMib)} · other ${gb(g.otherMib)}${g.usedMib === null ? "" : ` · measured ${gb(g.usedMib)}`}</span>
        ${last ? html`<span class="task-meta">${last}</span>` : ""}
      </div>
    </li>`;
}

function group(key: string, title: string, items: SafeHtml[], empty: string): SafeHtml {
  return html`
    <section class="caps-group" aria-labelledby="caps-${key}">
      <h2 id="caps-${key}">${title} <span class="muted">${items.length}</span></h2>
      ${items.length ? html`<ul class="caps-cards">${items}</ul>` : html`<p class="muted small">${empty}</p>`}
    </section>
  `;
}

export function renderCapabilities(c: Capabilities | undefined, timeline?: TlGpu[], range: 1 | 24 = 1): SafeHtml {
  if (c === undefined) {
    return html`
      <div class="caps">
        <header class="caps-head">
          <div class="caps-title">
            <h1>Capabilities</h1>
            <p class="muted">What Kompanion can use right now. Updates live.</p>
          </div>
        </header>
        <p class="muted caps-loading">Checking models and computers…</p>
      </div>
    `;
  }

  const models = c.models.map((x) => modelCard(x));
  const computers = c.computers.map((x) => computerCard(x));
  const tools = c.tools.map((x) => toolCard(x));
  const mcp = c.mcp.map((x) => mcpCard(x));
  const indexes = c.indexes.map((x) => indexCard(x));
  const skills = c.skills.map((x) => skillCard(x));

  return html`
    <div class="caps">
      <header class="caps-head">
        <div class="caps-title">
          <h1>Capabilities</h1>
          <p class="muted">What Kompanion can use right now. Updates live.</p>
        </div>
      </header>
      ${group("gpus", "GPUs", (c.gpus ?? []).map((g) => gpuCard(g, c.gpuRole)), "No GPUs configured (kompanion.toml [[gpu]]).")}
      ${(c.gpus ?? []).length ? renderTimeline(timeline, range, Object.fromEntries((c.gpus ?? []).map((g) => [g.id, g.totalMib]))) : ""}
      ${group("models", "Models", models, "No model providers are configured.")}
      ${group("computers", "Computers", computers, "No computer is paired yet.")}
      ${group("tools", "Tools", tools, "No tools found.")}
      ${group("mcp", "MCP servers", mcp, "No MCP servers yet.")}
      ${group("indexes", "Knowledge indexes", indexes, "No indexes yet.")}
      ${group("skills", "Skills", skills, "No skills found.")}
    </div>
  `;
}

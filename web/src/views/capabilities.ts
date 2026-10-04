// Capabilities: what Kompanion can use right now.
import { html, type SafeHtml } from "../core/html";
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

export interface Capabilities {
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
          ${c.lastSeen ? relTime(c.lastSeen) : "never"}
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

function group(key: string, title: string, items: SafeHtml[], empty: string): SafeHtml {
  return html`
    <section class="caps-group" aria-labelledby="caps-${key}">
      <h2 id="caps-${key}">${title} <span class="muted">${items.length}</span></h2>
      ${items.length ? html`<ul class="caps-cards">${items}</ul>` : html`<p class="muted small">${empty}</p>`}
    </section>
  `;
}

export function renderCapabilities(c: Capabilities | undefined): SafeHtml {
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
      ${group("models", "Models", models, "No model providers are configured.")}
      ${group("computers", "Computers", computers, "No computer is paired yet.")}
      ${group("tools", "Tools", tools, "No tools found.")}
      ${group("mcp", "MCP servers", mcp, "No MCP servers yet.")}
      ${group("indexes", "Knowledge indexes", indexes, "No indexes yet.")}
      ${group("skills", "Skills", skills, "No skills found.")}
    </div>
  `;
}

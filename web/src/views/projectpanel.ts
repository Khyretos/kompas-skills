// Project settings at the top of the Tasks pane: the type, and per type its extra part
// (programming: repo folder and computer; game: attached library assets and a picker).
import { html, type SafeHtml } from "../core/html";
import type { Project } from "../api/types";
import { icon } from "./icons";

export type ProjectType = "chat" | "game" | "programming";

export interface ProjectAsset {
  id: number;
  name: string;
  category: string;
  pack: string;
  preview: "image" | "audio" | null;
  pv: number;
  missing: boolean;
}

export interface PickResult {
  id: number;
  name: string;
  category: string;
  pack: string;
  preview: "image" | "audio" | null;
  pv: number;
}

export interface PanelOptions {
  machines: { id: string; name: string }[];
  assets: ProjectAsset[] | undefined;
  pick: { q: string; items: PickResult[]; busy: boolean };
  thumb: (a: { id: number; pv: number }) => string;
}

const TYPES: [ProjectType, string][] = [["chat", "Chat"], ["game", "Game (attach assets)"], ["programming", "Programming (repo folder)"]];

function assetCard(a: ProjectAsset, p: Project, o: PanelOptions): SafeHtml {
  return html`<li class="task project-asset ${a.missing ? "s-failed" : "s-done"}">
    <div class="task-main">
      ${a.preview === "image"
        ? html`<img class="pa-thumb" src="${o.thumb(a)}" alt="" loading="lazy">`
        : html`<span class="pa-thumb pa-icon" aria-hidden="true">${icon(a.preview === "audio" ? "spark" : "box")}</span>`}
      <span class="task-title">${a.name}</span>
      <span class="task-meta">${a.category} · ${a.pack}${a.missing ? " · missing from the library" : ""}</span>
    </div>
    <button class="icon-btn pa-remove" data-action="detach-asset" data-project="${p.id}" data-id="${String(a.id)}" aria-label="Detach ${a.name}">${icon("close")}</button>
  </li>`;
}

function pickRow(r: PickResult, p: Project, o: PanelOptions): SafeHtml {
  const isAttached = o.assets?.some((x) => x.id === r.id);
  return html`<li>
    <span class="pick-name">${r.name}</span>
    <span class="muted small">${r.category} · ${r.pack}</span>
    ${isAttached ? html`<button class="btn small" disabled>Attached</button>` : html`<button class="btn small" data-action="attach-asset" data-project="${p.id}" data-id="${String(r.id)}">Attach</button>`}
  </li>`;
}

function repoForm(p: Project, o: PanelOptions): SafeHtml {
  if (o.machines.length === 0) return html`<p class="muted small">Pair a computer first (Machines tab).</p>`;
  return html`
    <form class="project-repo" data-id="${p.id}">
      <label>Computer <select name="machine">
        <option value="">Pick a computer</option>
        ${o.machines.map((m) => html`<option value="${m.id}" ${m.id === p.repoMachineId ? "selected" : ""}>${m.name}</option>`)}
      </select></label>
      <label>Folder <input name="folder" value="${p.repoFolder ?? ""}" placeholder="/home/you/projects/app"></label>
      <button class="btn small primary" type="submit">Save</button>
      <p class="muted small">Used as the computer and folder when you run a task of this project.</p>
    </form>`;
}

function gameAssets(p: Project, o: PanelOptions): SafeHtml {
  const list = o.assets === undefined
    ? html`<p class="muted small">Loading…</p>`
    : o.assets.length === 0
      ? html`<p class="muted small">No assets yet. Find them below and attach them.</p>`
      : html`<ul class="project-assets">${o.assets.map((a) => assetCard(a, p, o))}</ul>`;
  return html`
    <h3 class="label">Assets <span class="count">${o.assets?.length ?? 0}</span></h3>
    ${list}
    <div class="asset-picker">
      <label class="sr-only" for="asset-pick-q">Find assets to attach</label>
      <input type="search" id="asset-pick-q" data-project="${p.id}" value="${o.pick.q}" placeholder="Find assets to attach (name, pack, category)" autocomplete="off">
      ${o.pick.busy ? html`<p class="muted small" role="status">Searching…</p>` : ""}
      ${o.pick.q && !o.pick.busy && o.pick.items.length === 0 ? html`<p class="muted small" role="status">Nothing found.</p>` : ""}
      <ul class="pick-results">${o.pick.items.map((r) => pickRow(r, p, o))}</ul>
    </div>`;
}

export function renderProjectPanel(p: Project, o: PanelOptions): SafeHtml {
  const type = p.type ?? "chat";
  return html`
    <section class="project-panel" aria-label="Project settings">
      <div class="project-type">
        <label for="project-type">Project type</label>
        <select id="project-type" data-id="${p.id}">
          ${TYPES.map(([value, label]) => html`<option value="${value}" ${value === type ? "selected" : ""}>${label}</option>`)}
        </select>
      </div>
      ${type === "programming" ? repoForm(p, o) : type === "game" ? gameAssets(p, o) : ""}
    </section>`;
}

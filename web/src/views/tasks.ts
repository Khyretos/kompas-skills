// Right pane: tasks for this project (or all), grouped by what they need.
import { html, type SafeHtml } from "../core/html";
import { clock, relTime } from "../core/time";
import { activeProject, type AppState } from "../state";
import type { Task, TaskEvent, TaskState } from "../api/types";
import { icon } from "./icons";

const labels: Record<TaskState, string> = {
  queued: "Queued",
  waiting_resources: "Needs resources",
  running: "Running",
  needs_input: "Needs you",
  in_review: "In review",
  done: "Done",
  failed: "Failed",
};
export const stateLabel = (s: TaskState) => labels[s];

const groups: { title: string; states: TaskState[] }[] = [
  { title: "Waiting for you", states: ["needs_input", "waiting_resources"] },
  { title: "Running", states: ["running", "in_review"] },
  { title: "Up next", states: ["queued"] },
  { title: "Finished", states: ["done", "failed"] },
];

function question(t: Task): SafeHtml {
  if (!t.question) return html``;
  return html`
    <div class="question">
      <p>${t.question.text}</p>
      ${t.question.action ? html`
        <dl class="action">
          <div><dt>Computer</dt><dd>${t.question.action.machine}</dd></div>
          <div><dt>Folder</dt><dd><code>${t.question.action.cwd}</code></dd></div>
          ${t.question.action.command ? html`<div class="cmd"><dt>Command</dt><dd><pre>${t.question.action.command}</pre></dd></div>` : ""}
          ${t.question.action.network?.length ? html`<div><dt>Network</dt><dd>${t.question.action.network.join(", ")}</dd></div>` : ""}
        </dl>` : ""}
      <div class="options">${t.question.options.map((o) => html`
        <button class="btn ${o.recommended ? "primary" : ""}" data-action="answer" data-task="${t.id}" data-option="${o.id}">
          ${o.label}${o.detail ? html` <small>${o.detail}</small>` : ""}
        </button>`)}</div>
    </div>`;
}

function card(t: Task, showProject: string | undefined): SafeHtml {
  const pct = Math.round(t.progress * 100);
  return html`
    <li class="task s-${t.state}">
      <button class="task-main" data-action="open-task" data-id="${t.id}">
        <span class="task-top">
          <span class="chip state">${stateLabel(t.state)}</span>
          ${showProject ? html`<span class="muted small">${showProject}</span>` : ""}
        </span>
        <span class="task-title">${t.title}</span>
        ${t.state === "running" || t.state === "in_review" ? html`
          <span class="progress" role="progressbar" aria-valuenow="${pct}" aria-valuemin="0" aria-valuemax="100" aria-label="Progress">
            <span style="width:${pct}%"></span>
          </span>` : ""}
        <span class="task-step">${t.scheduledFor && t.state === "queued" ? `Starts ${relTime(t.scheduledFor)}` : t.step}</span>
        <span class="task-meta">${icon("spark")} ${t.model}${t.runner ? html` · ${icon("pc")} ${t.runner}` : ""}</span>
      </button>
      ${question(t)}
    </li>`;
}

function eventRow(e: TaskEvent): SafeHtml {
  const time = html`<time datetime="${e.at}">${clock(e.at)}</time>`;
  switch (e.kind) {
    case "step":
      return html`<li class="ev step">${time}<span>${e.text}</span></li>`;
    case "tool":
      return html`<li class="ev tool ${e.ok ? "ok" : "bad"}">${time}<span><code>${e.tool}</code> ${e.detail}
        <span class="chip ${e.ok ? "good" : "bad"}">${e.ok ? "ok" : "failed"}</span></span></li>`;
    case "diff":
      return html`<li class="ev diff">${time}<span><code>${e.file}</code>
        <span class="add">+${e.added}</span> <span class="del">−${e.removed}</span></span></li>`;
    case "review":
      return html`<li class="ev review ${e.verdict}">${time}<span><strong>${e.model} review:</strong>
        <span class="chip ${e.verdict === "pass" ? "good" : "bad"}">${e.verdict}</span> ${e.note}</span></li>`;
    case "lesson":
      return html`<li class="ev lesson">${time}<span><strong>Lesson saved</strong> to <code>${e.skill}</code>: ${e.note}</span></li>`;
    case "call": {
      const c = e.call;
      return html`<li class="ev call">${time}<details>
        <summary><strong>${c.model}</strong> <span class="muted">(${c.role})</span>
          <span class="call-why">${c.reason}</span>
          <span class="call-meta">${c.tokensIn.toLocaleString()} in · ${c.tokensOut.toLocaleString()} out · ${(c.ms / 1000).toFixed(1)} s ·
            ${c.costEur > 0 ? `€${c.costEur.toFixed(3)}` : "local"}${c.energyWh ? ` · ${c.energyWh} Wh` : ""}</span></summary>
        <div class="call-body">
          <h5 class="label">System</h5><pre>${c.request.system}</pre>
          <h5 class="label">Context it was given</h5><ul class="ctx">${c.request.context.map((x) => html`<li><code>${x}</code></li>`)}</ul>
          <h5 class="label">Request</h5><pre>${c.request.prompt}</pre>
          <h5 class="label">Response</h5><pre>${c.response}</pre>
        </div></details></li>`;
    }
    case "screenshot":
      return html`<li class="ev shot">${time}<span>
        <span class="shot-box" role="img" aria-label="${e.caption}">${icon("image")}<small>Screenshot</small></span>
        ${e.caption}</span></li>`;
  }
}

export const TEMPLATE = "**Goal:** \n\n**Steps**\n1. \n\n**Done when:** ";

/** Add or change a task: title, description (markdown), state. */
function editor(t: Task | undefined, projectId: string, s: AppState): SafeHtml {
  const project = s.projects.find((p) => p.id === projectId);
  const state = t?.state ?? "queued";
  return html`
    <div class="pane-head">
      <button class="icon-btn" data-action="cancel-task-edit" aria-label="Cancel">${icon("back")}</button>
      <h2>${t ? "Edit task" : "New task"}</h2>
    </div>
    <form class="task-editor" id="task-editor" data-id="${t?.id ?? ""}" data-project="${projectId}">
      <span class="eyebrow">${project?.name ?? ""}</span>
      <div class="field">
        <label for="task-title">Title</label>
        <input id="task-title" name="title" value="${t?.title ?? ""}" maxlength="200" required>
      </div>
      <div class="field">
        <label for="task-description">Description</label>
        <textarea id="task-description" name="description" rows="14" required
          aria-describedby="task-description-hint">${t?.description || TEMPLATE}</textarea>
        <p class="hint" id="task-description-hint">Write a goal, numbered steps and when it is done. Markdown works.</p>
      </div>
      <div class="field">
        <label for="task-state">State</label>
        <select id="task-state" name="state">${(Object.keys(labels) as TaskState[]).map((k) =>
          html`<option value="${k}" ${k === state ? "selected" : ""}>${labels[k]}</option>`)}</select>
      </div>
      <p class="error small" id="task-msg" role="alert"></p>
      <div class="row">
        <button class="btn primary" type="submit">${t ? "Save" : "Add task"}</button>
        <button class="btn" type="button" data-action="cancel-task-edit">Cancel</button>
      </div>
    </form>`;
}

function detail(t: Task, s: AppState): SafeHtml {
  const project = s.projects.find((p) => p.id === t.projectId);
  const pct = Math.round(t.progress * 100);
  const siblings = s.tasks.filter((x) => x.projectId === t.projectId);
  const i = siblings.findIndex((x) => x.id === t.id);
  return html`
    <div class="pane-head">
      <button class="icon-btn" data-action="close-task" aria-label="Back to tasks">${icon("back")}</button>
      <h2>Task</h2>
    </div>
    <div class="task-detail">
      <span class="eyebrow">${project?.name ?? ""}${t.source?.startsWith("windshift:")
        ? html` · <span class="chip" title="Changes here are written to Windshift">Windshift ${t.source.slice(10)}</span>` : ""}</span>
      <h3>${t.title}</h3>
      <div class="task-actions">
        <button class="btn small" data-action="edit-task" data-id="${t.id}">${icon("edit")} Edit</button>
        <button class="btn small" data-action="move-task" data-id="${t.id}" data-dir="-1" ${i <= 0 ? "disabled" : ""} aria-label="Move up">↑</button>
        <button class="btn small" data-action="move-task" data-id="${t.id}" data-dir="1" ${i < 0 || i >= siblings.length - 1 ? "disabled" : ""} aria-label="Move down">↓</button>
        ${t.state !== "done" ? html`<button class="btn small" data-action="close-task-done" data-id="${t.id}">Mark done</button>` : ""}
        <button class="btn small danger" data-action="delete-task" data-id="${t.id}">${icon("trash")} Delete</button>
      </div>
      ${t.description ? html`<div class="task-desc md" data-md-task="${t.id}"></div>`
        : html`<p class="warn">This task has no description yet. <button class="link" data-action="edit-task" data-id="${t.id}">Write one</button>: goal, steps, done when.</p>`}
      <div class="detail-state">
        <span class="chip state s-${t.state}">${stateLabel(t.state)}</span>
        <span class="muted">${pct}% · ${t.step}</span>
      </div>
      <dl class="facts">
        <div><dt>Model</dt><dd>${t.model} <span class="muted">(${t.role})</span></dd></div>
        ${t.runner ? html`<div><dt>Computer</dt><dd>${t.runner}</dd></div>` : ""}
        ${t.workspace ? html`<div><dt>Workspace</dt><dd>${t.workspace}</dd></div>` : ""}
      </dl>
      ${question(t)}
      <h4 class="label">Timeline</h4>
      ${t.events.length ? html`<ol class="timeline">${[...t.events].reverse().map(eventRow)}</ol>`
        : html`<p class="muted">Nothing has happened yet.</p>`}
    </div>`;
}

export function paneTabs(s: AppState): SafeHtml {
  const attention = s.tasks.filter((t) => t.state === "needs_input" || t.state === "waiting_resources").length;
  return html`
    <div class="tabs" role="tablist" aria-label="Right pane">
      <button role="tab" data-action="tab" data-tab="tasks" aria-selected="${s.rightTab === "tasks"}">Tasks
        ${attention ? html`<span class="badge attn">${attention}</span>` : ""}</button>
      <button role="tab" data-action="tab" data-tab="machines" aria-selected="${s.rightTab === "machines"}">Machines</button>
    </div>`;
}

export function renderTasks(s: AppState): SafeHtml {
  const project = activeProject(s);
  if (s.editingTaskId) {
    const t = s.tasks.find((x) => x.id === s.editingTaskId);
    const projectId = t?.projectId ?? project?.id;
    if (projectId) return editor(t, projectId, s);
  }
  const open = s.openTaskId ? s.tasks.find((t) => t.id === s.openTaskId) : undefined;
  if (open) return detail(open, s);

  const scope = project && s.taskScope === "project" ? "project" : "all";
  const list = scope === "project" ? s.tasks.filter((t) => t.projectId === project!.id) : s.tasks;
  const projectName = (t: Task) => (scope === "all" ? s.projects.find((p) => p.id === t.projectId)?.name : undefined);

  return html`
    <div class="pane-head">
      ${paneTabs(s)}
      <div class="seg" role="group" aria-label="Show tasks for">
        <button data-action="scope" data-scope="project" aria-pressed="${scope === "project"}" ${project ? "" : "disabled"}>Project</button>
        <button data-action="scope" data-scope="all" aria-pressed="${scope === "all"}">All</button>
      </div>
      <button class="icon-btn only-narrow" data-action="pane" data-pane="main" aria-label="Close tasks">${icon("close")}</button>
    </div>
    <div class="task-groups">
      ${scope === "project" ? html`
        <div class="project-bar">
          <button class="btn small" data-action="new-task">${icon("plus")} Add task</button>
          ${project!.kind === "windshift" ? html`
            <span class="chip" title="Edits here are written to Windshift and Windshift changes come back">Synced with Windshift</span>
            <button class="btn small" data-action="make-internal" data-id="${project!.id}">Stop syncing</button>` : ""}
        </div>` : ""}
      ${list.length === 0 ? html`<p class="muted pad">No tasks yet. Ask the orchestrator for something and its tasks show up here.</p>` : ""}
      ${groups.map((g) => {
        const items = list.filter((t) => g.states.includes(t.state))
          .sort((a, b) => (a.projectId === b.projectId ? (a.position ?? 0) - (b.position ?? 0) : 0));
        if (!items.length) return "";
        return html`
          <section class="group">
            <h3 class="label">${g.title} <span class="count">${items.length}</span></h3>
            <ul class="tasks">${items.map((t) => card(t, projectName(t)))}</ul>
          </section>`;
      })}
    </div>`;
}

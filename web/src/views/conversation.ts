// Middle pane: the conversation with the project's orchestrator.
import { html, type SafeHtml } from "../core/html";
import { renderMarkdown } from "../core/markdown";
import { clock } from "../core/time";
import { activeChat, activeProject, type AppState } from "../state";
import type { Message, Task } from "../api/types";
import type { PcAction } from "../api/client";
import { icon } from "./icons";
import { stateLabel } from "./tasks";
import { fromTool, renderOutput } from "../core/output";

const STEP_LABEL: Record<string, string> = {
  approved: "starting…",
  always: "starting…",
  granting: "waiting for the computer…",
  running: "running…",
  denied: "declined",
  done: "done",
  failed: "failed",
  refused: "not allowed",
};

/** Steps the user opened stay open across re-renders (main.ts keeps this in step). */
export const openSteps = new Set<string>();
/** The user's open/closed choice per step group ("N steps on …"), by message id. Without a
 *  choice a group is open only while one of its steps runs; a choice is never overridden. */
export const groupChoice = new Map<string, boolean>();

function renderStep(a: PcAction, machines: Record<string, string>): SafeHtml {
  const busy = ["approved", "always", "granting", "running"].includes(a.state);
  return html`
    <details class="step ${a.state}" data-step="${a.id}" data-state="${a.state}" ${busy || openSteps.has(a.id) ? "open" : ""}>
      <summary data-action="step-toggle" data-id="${a.id}">
        ${icon(busy ? "spark" : a.state === "done" ? "terminal" : "close")}
        <span class="step-summary">${a.summary}</span>
        <span class="chip ${a.state}">${STEP_LABEL[a.state] ?? a.state}</span>
      </summary>
      ${a.result ? renderOutput(fromTool(a.tool, a.result, machines[a.machineId])) : ""}
    </details>`;
}

const BUSY = ["approved", "always", "granting", "running"];

function renderSteps(steps: PcAction[], machines: Record<string, string>, key: string): SafeHtml {
  if (steps.length === 0) return html``;
  const list = html`${steps.map((a) => renderStep(a, machines))}`;
  if (steps.length < 3) return html`<div class="steps">${list}</div>`;
  const where = machines[steps[0].machineId] ?? "a computer";
  const running = steps.some((a) => BUSY.includes(a.state));
  const open = groupChoice.get(key) ?? running;
  return html`<details class="steps group" data-group="${key}" ${open ? "open" : ""}>
    <summary data-action="group-toggle" data-id="${key}">${steps.length} steps on ${where}${running ? html` <span class="chip running">running…</span>` : ""}</summary>${list}</details>`;
}

export function renderHeader(s: AppState): SafeHtml {
  const chat = activeChat(s);
  const project = activeProject(s);
  const orchestrator = s.roles.find((r) => r.role === "orchestrator");
  const attention = s.tasks.filter((t) => t.state === "needs_input" || t.state === "waiting_resources").length;
  return html`
    <button class="icon-btn only-phone" data-action="pane" data-pane="left" aria-label="Projects and chats">${icon("menu")}</button>
    <div class="conv-title">
      <span class="eyebrow">${project ? project.name : "Chat"}</span>
      <h1>${chat?.title ?? "New chat"}</h1>
    </div>
    <span class="chip model" title="Orchestrator model">${orchestrator?.modelId ?? ""}</span>
    <button class="icon-btn only-narrow tasks-toggle" data-action="pane" data-pane="right" aria-label="Tasks">
      ${icon("tasks")}${attention ? html`<span class="badge attn">${attention}</span>` : ""}
    </button>`;
}

/** A message plus the tasks it created; rebuilt only when one of them changes. */
export interface MessageView {
  id: string;
  m: Message;
  tasks: Task[];
  steps: PcAction[];
  machines: Record<string, string>;
}

const cache = new Map<string, MessageView>();

export function messageViews(s: AppState): MessageView[] {
  const machines = Object.fromEntries(s.machines.map((m) => [m.id, m.name]));
  const byMsg = new Map<string, PcAction[]>();

  for (const action of s.pcActions) {
    if (action.state === "pending") continue;
    // Find the last message with at <= action.createdAt
    let bestMsg: Message | undefined = undefined;
    for (const m of s.messages) {
      if (m.at <= action.createdAt) {
        bestMsg = m;
      }
    }
    if (bestMsg) {
      const existing = byMsg.get(bestMsg.id);
      if (!existing) {
        byMsg.set(bestMsg.id, []);
      }
      const current = byMsg.get(bestMsg.id)!;
      // Check if already present to avoid duplicates
      if (!current.some((a) => a.id === action.id)) {
        current.push(action);
      }
    }
  }

  return s.messages.map((m) => {
    const tasks = (m.taskIds ?? []).map((id) => s.tasks.find((t) => t.id === id)).filter((t): t is Task => !!t);
    const steps = byMsg.get(m.id) ?? [];
    const old = cache.get(m.id);
    if (old && old.m === m && old.tasks.length === tasks.length && old.tasks.every((t, i) => t === tasks[i]) && old.steps.length === steps.length && old.steps.every((s, i) => s === steps[i])) return old;
    const view = { id: m.id, m, tasks, steps, machines };
    cache.set(m.id, view);
    return view;
  });
}

export function renderMessage({ m, tasks, steps, machines }: MessageView): SafeHtml {
  return html`
    <article class="msg ${m.author}">
      <header>
        <span class="who">${m.author === "user" ? "You" : "Kompanion"}</span>
        <time datetime="${m.at}">${clock(m.at)}</time>
      </header>
      <div class="body"></div>
      ${renderSteps(steps, machines, m.id)}
      ${tasks.length ? html`
        <ul class="msg-tasks">${tasks.map((t) => html`
          <li><button class="task-ref" data-action="open-task" data-id="${t.id}">
            <span class="state-dot s-${t.state}" aria-hidden="true"></span>${t.title}
            <span class="muted">${stateLabel(t.state)}</span>
          </button></li>`).join("")}</ul>` : ""}
    </article>`;
}

/** Fills the message body with sanitised markdown (never via the template). */
export function fillMessage(el: HTMLElement, { m }: MessageView): void {
  const body = el.querySelector(".body")!;
  body.append(renderMarkdown(m.text));
  if (m.streaming) {
    const caret = document.createElement("span");
    caret.className = "caret";
    caret.setAttribute("aria-hidden", "true");
    (body.lastElementChild ?? body).append(caret);
  }
}

export function renderEmpty(s: AppState): SafeHtml {
  const project = activeProject(s);
  return html`
    <div class="empty">
      <h2>What should we work on${project ? html` in ${project.name}` : ""}?</h2>
      <p class="muted">Describe the goal. I'll plan it, split it into tasks and ask when I need you.</p>
    </div>`;
}

export function composer(): SafeHtml {
  return html`
    <form class="composer" id="composer">
      <label class="sr-only" for="prompt">Message</label>
      <textarea id="prompt" rows="1" placeholder="Ask, plan, or hand over a task…"></textarea>
      <button class="btn mic" type="button" id="voice-mic" data-action="voice-mic" aria-label="Speak" aria-pressed="false" hidden>${icon("mic")}</button>
      <button class="btn primary send" type="submit" aria-label="Send">${icon("send")}</button>
    </form>
    <p class="composer-hint">Enter sends, Shift+Enter adds a line.
      <span id="voice-status" role="status"></span>
      <button class="btn small" type="button" id="voice-stop" data-action="voice-stop" hidden>Stop reading</button></p>`;
}

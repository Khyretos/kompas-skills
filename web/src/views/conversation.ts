// Middle pane: the conversation with the project's orchestrator.
import { html, type SafeHtml } from "../core/html";
import { renderMarkdown } from "../core/markdown";
import { clock } from "../core/time";
import { activeChat, activeProject, type AppState } from "../state";
import type { Message, Task } from "../api/types";
import { icon } from "./icons";
import { stateLabel } from "./tasks";

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
}

const cache = new Map<string, MessageView>();

export function messageViews(s: AppState): MessageView[] {
  return s.messages.map((m) => {
    const tasks = (m.taskIds ?? []).map((id) => s.tasks.find((t) => t.id === id)).filter((t): t is Task => !!t);
    const old = cache.get(m.id);
    if (old && old.m === m && old.tasks.length === tasks.length && old.tasks.every((t, i) => t === tasks[i])) return old;
    const view = { id: m.id, m, tasks };
    cache.set(m.id, view);
    return view;
  });
}

export function renderMessage({ m, tasks }: MessageView): SafeHtml {
  return html`
    <article class="msg ${m.author}">
      <header>
        <span class="who">${m.author === "user" ? "You" : "Kompanion"}</span>
        <time datetime="${m.at}">${clock(m.at)}</time>
      </header>
      <div class="body"></div>
      ${tasks.length ? html`
        <ul class="msg-tasks">${tasks.map((t) => html`
          <li><button class="task-ref" data-action="open-task" data-id="${t.id}">
            <span class="state-dot s-${t.state}" aria-hidden="true"></span>${t.title}
            <span class="muted">${stateLabel(t.state)}</span>
          </button></li>`)}</ul>` : ""}
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
      <button class="btn primary send" type="submit" aria-label="Send">${icon("send")}</button>
    </form>
    <p class="composer-hint">Enter sends, Shift+Enter adds a line.</p>`;
}

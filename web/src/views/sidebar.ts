// Left pane: projects with their chats, loose chats, settings.
import { html, type SafeHtml } from "../core/html";
import { relTime } from "../core/time";
import type { AppState } from "../state";
import type { Chat } from "../api/types";
import { icon } from "./icons";

export function renderSidebar(s: AppState): SafeHtml {
  const chatLink = (c: Chat) => html`
    <li><button class="nav-item ${c.id === s.activeChatId ? "active" : ""}" data-action="open-chat" data-id="${c.id}"
      ${c.id === s.activeChatId ? html`aria-current="page"` : ""}>
      <span class="nav-title">${c.title}</span>
      <span class="nav-meta">${relTime(c.updatedAt)}</span>
    </button></li>`;

  const needsYou = (projectId: string) =>
    s.tasks.filter((t) => t.projectId === projectId && (t.state === "needs_input" || t.state === "waiting_resources")).length;
  const running = (projectId: string) =>
    s.tasks.filter((t) => t.projectId === projectId && (t.state === "running" || t.state === "in_review")).length;

  const projects = [...s.projects].sort((a, b) => b.updatedAt.localeCompare(a.updatedAt));
  const loose = s.chats.filter((c) => !c.projectId);

  return html`
    <div class="pane-head">
      <div class="server" title="${s.server?.url ?? ""}">
        <span class="dot ok" aria-hidden="true"></span>
        <span><strong>${s.server?.name ?? ""}</strong><small>Connected</small></span>
      </div>
      <button class="icon-btn only-phone" data-action="pane" data-pane="main" aria-label="Close">${icon("close")}</button>
    </div>
    <button class="btn new-chat" data-action="new-chat">${icon("plus")} New chat</button>
    <nav class="nav" aria-label="Projects and chats">
      <h2 class="label">Projects</h2>
      <ul class="projects">
        ${projects.map((p) => {
          const n = needsYou(p.id), r = running(p.id);
          return html`
          <li class="project">
            <div class="project-head">
              ${icon("folder")}<span class="project-name">${p.name}</span>
              ${n ? html`<span class="badge attn" title="${n} waiting for you">${n}</span>` : ""}
              ${r ? html`<span class="badge run" title="${r} running">${r}</span>` : ""}
            </div>
            <ul>${s.chats.filter((c) => c.projectId === p.id).map(chatLink)}</ul>
          </li>`;
        })}
      </ul>
      <h2 class="label">Chats</h2>
      <ul class="loose">${loose.map(chatLink)}</ul>
    </nav>
    <button class="nav-item settings-link" data-action="settings">${icon("gear")} Models and roles</button>`;
}

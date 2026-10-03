// Left pane: pinned chats, projects (expandable, with their chats and tasks),
// loose chats, settings. Each chat has a menu: pin, rename, archive, delete.
import { html, type SafeHtml } from "../core/html";
import { relTime } from "../core/time";
import type { AppState } from "../state";
import type { Chat, Task } from "../api/types";
import { icon } from "./icons";
import { logoUrl } from "../core/logo";

const TASKS_SHOWN = 6;

export function renderSidebar(s: AppState): SafeHtml {
  const chatRow = (c: Chat, showProject = false) => {
    const active = c.id === s.activeChatId;
    const menuOpen = s.chatMenuId === c.id;
    const main = s.renamingChatId === c.id
      ? html`<form class="rename" data-id="${c.id}">
          <input name="title" value="${c.title}" aria-label="Chat title" maxlength="120" required autofocus></form>`
      : html`<button class="nav-item" data-action="open-chat" data-id="${c.id}" ${active ? html`aria-current="page"` : ""}>
          <span class="nav-title">${c.title}</span>
          ${showProject && c.projectId ? html`<span class="chip project-badge">${s.projects.find((p) => p.id === c.projectId)?.name ?? ""}</span>` : ""}
          <span class="nav-meta">${relTime(c.updatedAt)}</span>
        </button>`;
    return html`
      <li class="chat-row ${active ? "active" : ""} ${menuOpen ? "menu-open" : ""}">
        ${main}
        <button class="icon-btn chat-more" data-action="chat-menu" data-id="${c.id}" aria-label="Options for ${c.title}"
          aria-haspopup="menu" aria-expanded="${menuOpen ? "true" : "false"}">${icon("more")}</button>
        ${menuOpen ? html`
          <div class="menu" role="menu">
            <button role="menuitem" data-action="chat-pin" data-id="${c.id}">${icon("pin")} ${c.pinned ? "Unpin" : "Pin"}</button>
            <button role="menuitem" data-action="chat-rename" data-id="${c.id}">${icon("edit")} Rename</button>
            ${s.movingChatId === c.id ? html`
              <div class="submenu" role="group" aria-label="Move to project">
                ${s.projects.map((p) => html`<button role="menuitem" data-action="chat-move" data-id="${c.id}" data-project="${p.id}"
                  ${p.id === c.projectId ? "disabled" : ""}>${icon("folder")} ${p.name}</button>`)}
                ${c.projectId ? html`<button role="menuitem" data-action="chat-move" data-id="${c.id}" data-project="">No project</button>` : ""}
              </div>` : html`<button role="menuitem" data-action="chat-move-open" data-id="${c.id}">${icon("folder")} Move to project…</button>`}
            <button role="menuitem" data-action="chat-archive" data-id="${c.id}">${icon("archive")} Archive</button>
            <button role="menuitem" class="danger" data-action="chat-delete" data-id="${c.id}">${icon("trash")} Delete</button>
          </div>` : ""}
      </li>`;
  };

  const tasksOf = (projectId: string) =>
    s.tasks.filter((t) => t.projectId === projectId).sort((a, b) => (a.position ?? 0) - (b.position ?? 0));
  const needsYou = (ts: Task[]) => ts.filter((t) => t.state === "needs_input" || t.state === "waiting_resources").length;
  const running = (ts: Task[]) => ts.filter((t) => t.state === "running" || t.state === "in_review").length;

  const pinned = s.chats.filter((c) => c.pinned);
  const projects = [...s.projects].sort((a, b) => b.updatedAt.localeCompare(a.updatedAt));
  // The Chats list shows every chat (project chats with a badge), newest first.
  const loose = s.chats.filter((c) => !c.pinned);

  return html`
    <div class="pane-head">
      <div class="server" title="${s.server?.url ?? ""}">
        <img class="server-logo" src="${logoUrl(s.logoVersion)}" alt="" width="28" height="28">
        <span><strong>${s.server?.name ?? ""}</strong><small><span class="dot ok" aria-hidden="true"></span> Connected</small></span>
      </div>
      <button class="icon-btn only-phone" data-action="pane" data-pane="main" aria-label="Close">${icon("close")}</button>
    </div>
    <button class="btn new-chat" data-action="new-chat">${icon("plus")} New chat</button>
    <nav class="nav" aria-label="Projects and chats">
      ${pinned.length ? html`
        <h2 class="label">Pinned</h2>
        <ul class="loose">${pinned.map(chatRow)}</ul>` : ""}
      <h2 class="label">Projects</h2>
      <ul class="projects">
        ${projects.map((p) => {
          const ts = tasksOf(p.id);
          const n = needsYou(ts), r = running(ts);
          const open = s.expandedProjects.has(p.id);
          // Unfinished tasks first, then the rest, keeping their order.
          const shown = [...ts.filter((t) => t.state !== "done"), ...ts.filter((t) => t.state === "done")];
          const done = ts.length - shown.filter((t) => t.state !== "done").length;
          return html`
          <li class="project ${open ? "open" : ""}">
            <button class="project-head ${s.activeProjectId === p.id ? "active" : ""}" data-action="project" data-id="${p.id}"
              aria-expanded="${open ? "true" : "false"}" title="${p.description}">
              ${icon(open ? "chevron-down" : "chevron-right")}${icon("folder")}<span class="project-name">${p.name}</span>
              ${n ? html`<span class="badge attn" title="${n} waiting for you">${n}</span>` : ""}
              ${r ? html`<span class="badge run" title="${r} running">${r}</span>` : ""}
              ${ts.length ? html`<span class="nav-meta">${done}/${ts.length}</span>` : ""}
            </button>
            ${open ? html`
              <ul>
                ${s.chats.filter((c) => c.projectId === p.id && !c.pinned).map(chatRow)}
                ${shown.slice(0, TASKS_SHOWN).map((t) => html`
                  <li class="task-line state-${t.state}" title="${t.title}">
                    <span class="task-dot" aria-hidden="true"></span><span class="nav-title">${t.title}</span>
                  </li>`)}
                ${shown.length > TASKS_SHOWN ? html`
                  <li class="more"><button class="nav-item" data-action="project-tasks" data-id="${p.id}">
                    ${shown.length - TASKS_SHOWN} more in the task list</button></li>` : ""}
                <li><button class="nav-item new-in-project" data-action="new-chat" data-project="${p.id}">${icon("plus")} New chat here</button></li>
              </ul>` : ""}
          </li>`;
        })}
      </ul>
      <h2 class="label">Chats</h2>
      <ul class="loose">${loose.map((c) => chatRow(c, true))}</ul>
    </nav>
    <div class="account">
      <button class="nav-item settings-link" data-action="settings">${icon("gear")} Settings</button>
      <div class="user-row">
        <span class="avatar" aria-hidden="true">${(s.userName ?? "?").slice(0, 1).toUpperCase()}</span>
        <span class="nav-title">${s.userName ?? ""}</span>
        <button class="btn small" data-action="logout">${icon("logout")} Sign out</button>
      </div>
    </div>`;
}

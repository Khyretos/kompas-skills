import { showSignIn } from "./views/signin";
import { HttpApi } from "./api/http";
import { $, html, mount, onAction } from "./core/html";
import { MockApi } from "./api/mock";
import type { KompanionApi, ServerEvent } from "./api/client";
import type { Role, Server } from "./api/types";
import { store, type AppState } from "./state";
import { showConnect } from "./views/connect";
import { renderSidebar } from "./views/sidebar";
import { composer, fillMessage, messageViews, renderEmpty, renderHeader, renderMessage, type MessageView } from "./views/conversation";
import { KeyedList } from "./core/keyed";
import { paneTabs, renderTasks } from "./views/tasks";
import { renderMachines } from "./views/machines";
import { html as h } from "./core/html";
import { renderSettings } from "./views/settings";


let api: KompanionApi;
let root = $("#app");

/** Served by a Kompanion server: use it. Opened as a file, or with ?demo: demo mode. */
async function boot(): Promise<void> {
  const demo = !location.protocol.startsWith("http") || new URLSearchParams(location.search).has("demo");
  if (!demo) {
    // Never fall back to example data on a real server: retry, then say so.
    const http = new HttpApi();
    for (let attempt = 1; ; attempt++) {
      try {
        const status = await http.status();
        api = http;
        const server = { url: location.origin, name: location.hostname, version: status.version };
        if (status.user) return start(server);
        return showSignIn(root, api, status, () => fresh().then(() => start(server)));
      } catch {
        if (attempt >= 3) return showUnreachable(root, () => void boot());
        await new Promise((r) => setTimeout(r, 1500));
      }
    }
  }
  document.body.classList.add("demo");
  api = new MockApi();
  showConnect(root, api, (server) => start(server));
}

function showUnreachable(el: HTMLElement, retry: () => void): void {
  mount(el, html`
    <main class="connect">
      <div class="connect-card">
        <h1>Kreative Kompanion</h1>
        <p class="error" role="alert">Can't reach the Kompanion server right now. It may be restarting.</p>
        <button class="btn primary" type="button" id="retry">Try again</button>
      </div>
    </main>`);
  $("#retry", el).addEventListener("click", retry, { once: true });
}

/** Replace #app with a clean element (drops the previous screen's listeners). */
async function fresh(): Promise<HTMLElement> {
  const el = root.cloneNode(false) as HTMLElement;
  root.replaceWith(el);
  root = el;
  return el;
}

boot();

async function start(server: Server): Promise<void> {
  const shellRoot = await fresh();

  mount(shellRoot, html`
    <div class="shell" data-pane="main">
      <aside class="pane left" id="left" aria-label="Projects and chats"></aside>
      <main class="pane center">
        <header class="conv-head" id="conv-head"></header>
        <div class="messages" id="messages">
          <div class="empty-slot" id="empty-slot"></div>
          <div class="msg-list" id="msg-list" role="log" aria-live="polite"></div>
        </div>
        <div class="composer-wrap">${composer()}</div>
      </main>
      <aside class="pane right" id="right" aria-label="Tasks"></aside>
      <div class="scrim" data-action="pane" data-pane="main"></div>
      <div id="settings" hidden></div>
    </div>`);

  const [projects, chats, tasks, providers, roles, machines, today, status] = await Promise.all([
    api.listProjects(), api.listChats(), api.listTasks(), api.listProviders(), api.listRoles(),
    api.listMachines(), api.today(), api.status(),
  ]);
  store.set({ server, projects, chats, tasks, providers, roles, machines, today, userName: status.user ?? undefined });
  wire(shellRoot);
  await openChat(chats[0]?.id);
}

async function openChat(chatId?: string): Promise<void> {
  const messages = chatId ? await api.listMessages(chatId) : [];
  store.set({ activeChatId: chatId, messages, openTaskId: undefined, pane: "main" });
  const prompt = document.getElementById("prompt") as HTMLTextAreaElement | null;
  if (prompt && window.matchMedia("(pointer: fine)").matches) prompt.focus();
}

let messageList: KeyedList<MessageView> | undefined;

function render(s: AppState, prev: AppState): void {
  const shell = $(".shell");
  shell.dataset.pane = s.pane;

  mount($("#left"), renderSidebar(s));
  if (s.renamingChatId && s.renamingChatId !== prev.renamingChatId) {
    const input = document.querySelector<HTMLInputElement>("form.rename input");
    input?.focus();
    input?.select();
  }
  mount($("#conv-head"), renderHeader(s));
  mount($("#right"), s.rightTab === "tasks" ? renderTasks(s) : h`
    <div class="pane-head">${paneTabs(s)}
      <button class="icon-btn only-narrow" data-action="pane" data-pane="main" aria-label="Close">✕</button></div>
    ${renderMachines(s.machines, s.today)}`);

  const box = $("#messages");
  const nearBottom = box.scrollHeight - box.scrollTop - box.clientHeight < 80;
  if (s.activeChatId !== prev.activeChatId) messageList?.clear();
  messageList ??= new KeyedList<MessageView>($("#msg-list"), renderMessage, fillMessage);
  messageList.update(messageViews(s));
  const empty = $("#empty-slot");
  if (s.messages.length === 0) mount(empty, renderEmpty(s));
  else empty.replaceChildren();
  if (nearBottom || s.activeChatId !== prev.activeChatId) box.scrollTop = box.scrollHeight;

  const settings = $("#settings");
  settings.hidden = !s.settingsOpen;
  if (s.settingsOpen) mount(settings, renderSettings(s));
}

function applyEvent(ev: ServerEvent): void {
  const s = store.get();
  if (ev.type === "task") {
    const exists = s.tasks.some((t) => t.id === ev.task.id);
    store.set({ tasks: exists ? s.tasks.map((t) => (t.id === ev.task.id ? ev.task : t)) : [ev.task, ...s.tasks] });
    // Attach new tasks to the message that created them.
    if (!exists) {
      const last = [...s.messages].reverse().find((m) => m.author === "orchestrator");
      if (last) store.set({ messages: store.get().messages.map((m) => (m.id === last.id ? { ...m, taskIds: [ev.task.id] } : m)) });
    }
  } else if (ev.type === "resync") {
    reload();
  } else if (ev.type === "machines") {
    store.set({ machines: ev.machines });
  } else if (ev.type === "message" && ev.message.chatId === s.activeChatId) {
    store.set({ messages: [...s.messages, ev.message] });
  } else if (ev.type === "message-delta" && ev.chatId === s.activeChatId) {
    store.set({
      messages: s.messages.map((m) =>
        m.id === ev.messageId ? { ...m, text: m.text + ev.text, streaming: !ev.done } : m),
    });
  }
}

async function reload(): Promise<void> {
  const s = store.get();
  const [chats, tasks, messages] = await Promise.all([
    api.listChats(), api.listTasks(), s.activeChatId ? api.listMessages(s.activeChatId) : Promise.resolve([]),
  ]);
  store.set({ chats, tasks, messages });
}

function wire(shell: HTMLElement): void {
  store.subscribe(render);
  store.flush();
  api.onEvent(applyEvent);

  onAction(shell, {
    "open-chat": (el) => openChat(el.dataset.id),
    "new-chat": (el) => store.set({
      activeChatId: undefined, messages: [], pane: "main", chatMenuId: undefined,
      activeProjectId: el.dataset.project ?? store.get().activeProjectId,
    }),
    project: (el) => {
      const id = el.dataset.id ?? "";
      const s = store.get();
      const expanded = new Set(s.expandedProjects);
      // First click selects and opens; clicking the selected project closes it.
      if (s.activeProjectId === id && expanded.has(id)) expanded.delete(id);
      else expanded.add(id);
      store.set({ expandedProjects: expanded, activeProjectId: id, taskScope: "project" });
    },
    "project-tasks": (el) => store.set({ activeProjectId: el.dataset.id, taskScope: "project", rightTab: "tasks", pane: "right" }),
    "chat-menu": (el) => store.set({ chatMenuId: store.get().chatMenuId === el.dataset.id ? undefined : el.dataset.id }),
    "chat-pin": (el) => {
      const c = store.get().chats.find((x) => x.id === el.dataset.id);
      if (c) changeChat(c.id, { pinned: !c.pinned });
    },
    "chat-rename": (el) => store.set({ renamingChatId: el.dataset.id, chatMenuId: undefined }),
    "chat-archive": (el) => changeChat(el.dataset.id ?? "", { archived: true }),
    "chat-delete": (el) => {
      const c = store.get().chats.find((x) => x.id === el.dataset.id);
      if (c && confirm(`Delete "${c.title}" and its messages? This can't be undone.`)) removeChat(c.id);
      else store.set({ chatMenuId: undefined });
    },
    "open-task": (el) => store.set({ openTaskId: el.dataset.id, rightTab: "tasks", pane: "right" }),
    tab: (el) => store.set({ rightTab: el.dataset.tab as AppState["rightTab"], openTaskId: undefined }),
    "close-task": () => store.set({ openTaskId: undefined }),
    scope: (el) => store.set({ taskScope: el.dataset.scope as AppState["taskScope"] }),
    pane: (el) => store.set({ pane: el.dataset.pane as AppState["pane"] }),
    answer: (el) => api.answer(el.dataset.task ?? "", el.dataset.option ?? "").catch(showError),
    settings: () => store.set({ settingsOpen: true, pane: "main" }),
    // Ends this app's session. Keycloak keeps its own session, so "Sign in
    // with Kreative Kompas" afterwards may not ask for a password again.
    logout: () => api.logout().then(() => location.replace("/"), showError),
    "close-settings": () => store.set({ settingsOpen: false }),
  });

  shell.addEventListener("change", async (ev) => {
    const sel = ev.target as HTMLSelectElement;
    if (!sel.dataset.role || !sel.value) return;
    const [providerId, modelId] = sel.value.split("::");
    await api.setRole({ role: sel.dataset.role as Role, providerId, modelId });
    store.set({ roles: await api.listRoles() });
  });

  document.addEventListener("keydown", (ev) => {
    if (ev.key !== "Escape") return;
    const s = store.get();
    if (s.renamingChatId || s.chatMenuId) store.set({ renamingChatId: undefined, chatMenuId: undefined });
    else if (s.settingsOpen) store.set({ settingsOpen: false });
  });
  // A click anywhere outside an open chat menu closes it.
  document.addEventListener("click", (ev) => {
    const t = ev.target as HTMLElement;
    if (store.get().chatMenuId && !t.closest(".menu, .chat-more")) store.set({ chatMenuId: undefined });
  });
  // Rename in place: Enter saves, leaving the field saves too.
  const saveRename = (form: HTMLFormElement) => {
    const id = form.dataset.id ?? "";
    const title = (form.elements.namedItem("title") as HTMLInputElement).value.trim();
    store.set({ renamingChatId: undefined });
    const c = store.get().chats.find((x) => x.id === id);
    if (c && title && title !== c.title) changeChat(id, { title });
  };
  shell.addEventListener("submit", (ev) => {
    const form = (ev.target as HTMLElement).closest("form.rename") as HTMLFormElement | null;
    if (form) { ev.preventDefault(); saveRename(form); }
  });
  shell.addEventListener("focusout", (ev) => {
    const form = (ev.target as HTMLElement).closest("form.rename") as HTMLFormElement | null;
    if (form && store.get().renamingChatId) saveRename(form);
  });
  // Live stats of the server's machine.
  setInterval(() => {
    if (store.get().rightTab === "machines") api.listMachines().then((machines) => store.set({ machines })).catch(() => {});
  }, 5000);

  const form = $("#composer");
  const prompt = $("#prompt") as HTMLTextAreaElement;
  const submit = async () => {
    const text = prompt.value.trim();
    if (!text) return;
    prompt.value = "";
    autosize();
    let chatId = store.get().activeChatId;
    if (!chatId) {
      const title = text.length > 40 ? text.slice(0, 38) + "…" : text;
      const chat = await api.createChat(title, store.get().activeProjectId);
      store.set({ chats: [chat, ...store.get().chats], activeChatId: chat.id });
      chatId = chat.id;
    }
    await api.send(chatId, text).catch(showError);
  };
  const autosize = () => {
    prompt.style.height = "auto";
    prompt.style.height = `${Math.min(prompt.scrollHeight, 200)}px`;
  };
  form.addEventListener("submit", (ev) => { ev.preventDefault(); submit(); });
  prompt.addEventListener("input", autosize);
  prompt.addEventListener("keydown", (ev) => {
    if (ev.key === "Enter" && !ev.shiftKey && !ev.isComposing) { ev.preventDefault(); submit(); }
  });
}

async function changeChat(id: string, change: { title?: string; pinned?: boolean; archived?: boolean }): Promise<void> {
  store.set({ chatMenuId: undefined });
  try {
    await api.updateChat(id, change);
    if (change.archived && store.get().activeChatId === id) store.set({ activeChatId: undefined, messages: [] });
    store.set({ chats: await api.listChats() });
  } catch (e) { showError(e); }
}

async function removeChat(id: string): Promise<void> {
  store.set({ chatMenuId: undefined });
  try {
    await api.deleteChat(id);
    if (store.get().activeChatId === id) store.set({ activeChatId: undefined, messages: [] });
    store.set({ chats: await api.listChats() });
  } catch (e) { showError(e); }
}

function showError(e: unknown): void {
  const message = e instanceof Error ? e.message : String(e);
  const toast = document.createElement("div");
  toast.className = "toast";
  toast.setAttribute("role", "alert");
  toast.textContent = message;
  document.body.append(toast);
  setTimeout(() => toast.remove(), 6000);
}

import { showSignIn } from "./views/signin";
import { HttpApi } from "./api/http";
import { $, html, mount, onAction, restoreBusy } from "./core/html";
import { initResize } from "./core/resize";
import { MockApi } from "./api/mock";
import type { KompanionApi, ServerEvent } from "./api/client";
import type { AdminSettings, Role, Server, TaskState, ThemeChoice } from "./api/types";
import { renderMarkdown } from "./core/markdown";
import { onCodeAction } from "./core/codeblocks";
import { store, type AppState } from "./state";
import { showConnect } from "./views/connect";
import { renderSidebar } from "./views/sidebar";
import { composer, fillMessage, messageViews, renderEmpty, renderHeader, renderMessage, type MessageView } from "./views/conversation";
import { KeyedList } from "./core/keyed";
import { paneTabs, renderTasks } from "./views/tasks";
import { renderMachines, REFRESH_STEPS, setGpuView } from "./views/machines";
import { grantFromForm, renderAccess, type GrantView } from "./views/access";

/** Shows a grant change at once, marked pending until the computer confirms it. */
function markGrant(machineId: string, target: string, pending: "add" | "revoke", g?: Partial<GrantView>): void {
  const grants = { ...store.get().grants };
  const list = (grants[machineId] ?? []).filter((x) => x.target !== target || pending === "revoke");
  grants[machineId] = pending === "revoke"
    ? list.map((x) => (x.target === target ? { ...x, pending } : x))
    : [...list, { target, rights: [], grantedBy: store.get().userName ?? "you", grantedAt: new Date().toISOString(), expires: null, ...g, pending }];
  store.set({ grants });
}

let savePrefs: ReturnType<typeof setTimeout> | undefined;
import { html as h } from "./core/html";
import { renderSettings } from "./views/settings";


declare const __DEMO__: boolean;

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
  store.set({
    server: { ...server, name: status.name || server.name }, projects, chats, tasks, providers, roles, machines, today,
    userName: status.user ?? undefined, isAdmin: !!status.admin, theme: status.theme ?? "system",
    machinesRefresh: status.machinesRefresh ?? 5, gpuPins: status.gpuPins ?? [], windshift: status.windshift, windshiftWarning: status.windshiftWarning, logoVersion: status.logoVersion,
  });
  applyTheme(status.theme ?? "system");
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

let firstRender = true;

/** True when any of these state fields changed since the last render. */
const changed = (s: AppState, prev: AppState, keys: (keyof AppState)[]) => firstRender || keys.some((k) => s[k] !== prev[k]);

/** Re-mounts a pane but keeps its scroll position and the focused field. */
function remount(el: HTMLElement, content: ReturnType<typeof renderSidebar>): void {
  const scrollers = [el, ...el.querySelectorAll<HTMLElement>(".nav, .task-groups, .task-detail, .sheet")];
  const tops = scrollers.map((x) => x.scrollTop);
  const focusId = el.contains(document.activeElement) ? (document.activeElement as HTMLElement).id : "";
  mount(el, content);
  const after = [el, ...el.querySelectorAll<HTMLElement>(".nav, .task-groups, .task-detail, .sheet")];
  after.forEach((x, i) => { if (tops[i]) x.scrollTop = tops[i]; });
  if (focusId) document.getElementById(focusId)?.focus();
}

// Each pane re-renders only when the state it shows changes, so live machine
// stats (every second on "Live") never rebuild Settings, a task being edited,
// or the sidebar.
function render(s: AppState, prev: AppState): void {
  const shell = $(".shell");
  shell.dataset.pane = s.pane;

  if (changed(s, prev, ["chats", "projects", "tasks", "activeChatId", "activeProjectId", "expandedProjects",
    "chatMenuId", "movingChatId", "renamingChatId", "server", "userName", "logoVersion"])) {
    remount($("#left"), renderSidebar(s));
  }
  if (s.renamingChatId && s.renamingChatId !== prev.renamingChatId) {
    const input = document.querySelector<HTMLInputElement>("form.rename input");
    input?.focus();
    input?.select();
  }
  if (changed(s, prev, ["chats", "projects", "activeChatId", "activeProjectId", "messages", "roles", "tasks"])) {
    mount($("#conv-head"), renderHeader(s));
  }
  const rightKeys: (keyof AppState)[] = s.rightTab === "tasks"
    ? ["rightTab", "tasks", "projects", "openTaskId", "editingTaskId", "taskScope", "activeProjectId", "activeChatId", "chats"]
    : s.rightTab === "access" ? ["rightTab", "grants", "accessHistory", "machines"]
    : ["rightTab", "machines", "today", "machinesRefresh", "tasks", "pairing", "gpuOpen", "gpuPins"];
  // Never rebuild the task editor under the user's hands; only when it opens or closes.
  const editing = s.rightTab === "tasks" && s.editingTaskId && s.editingTaskId === prev.editingTaskId && !firstRender;
  if (!editing && changed(s, prev, rightKeys)) {
    remount($("#right"), s.rightTab === "tasks" ? renderTasks(s) : s.rightTab === "access" ? h`
      <div class="pane-head">${paneTabs(s)}
        <button class="icon-btn only-narrow" data-action="pane" data-pane="main" aria-label="Close">✕</button></div>
      ${renderAccess(s.machines.filter((m) => m.id !== "server").map((m) => ({ id: m.id, name: m.name })), s.grants, s.accessHistory)}` : h`
      <div class="pane-head">${paneTabs(s)}
        <button class="icon-btn only-narrow" data-action="pane" data-pane="main" aria-label="Close">✕</button></div>
      ${(setGpuView(s.gpuOpen, s.gpuPins), renderMachines(s.machines, s.today, s.machinesRefresh, s.pairing))}`);
    // Task descriptions are markdown, rendered sanitised after mounting.
    for (const el of document.querySelectorAll<HTMLElement>("[data-md-task]")) {
      const t = s.tasks.find((x) => x.id === el.dataset.mdTask);
      if (t?.description) el.replaceChildren(renderMarkdown(t.description));
    }
  }

  if (changed(s, prev, ["messages", "activeChatId", "activeProjectId", "projects", "tasks"])) {
    const box = $("#messages");
    const nearBottom = box.scrollHeight - box.scrollTop - box.clientHeight < 80;
    if (s.activeChatId !== prev.activeChatId) messageList?.clear();
    messageList ??= new KeyedList<MessageView>($("#msg-list"), renderMessage, fillMessage);
    messageList.update(messageViews(s));
    const empty = $("#empty-slot");
    if (s.messages.length === 0) mount(empty, renderEmpty(s));
    else empty.replaceChildren();
    if (nearBottom || s.activeChatId !== prev.activeChatId) box.scrollTop = box.scrollHeight;
  }

  const settings = $("#settings");
  settings.hidden = !s.settingsOpen;
  if (s.settingsOpen && changed(s, prev, ["settingsOpen", "providers", "roles", "admin", "theme", "isAdmin", "notifications", "windshift", "logoVersion"])) {
    remount(settings, renderSettings(s));
  }
  firstRender = false;
  restoreBusy(shell);
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
  } else if (ev.type === "changed") {
    refetch(ev.what);
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

let refetchTimers = new Map<string, number>();
function refetch(what: string): void {
  if (refetchTimers.has(what)) return;
  refetchTimers.set(what, window.setTimeout(async () => {
    refetchTimers.delete(what);
    const s = store.get();
    try {
      if (what === "tasks") store.set({ tasks: await api.listTasks() });
      else if (what === "projects") store.set({ projects: await api.listProjects(), tasks: await api.listTasks() });
      else if (what === "chats") store.set({ chats: await api.listChats() });
      else if (what === "machines") store.set({ machines: await api.listMachines() });
      else if (what === "access") await loadAccess();
      else if (what === "settings" && s.settingsOpen) {
        store.set({ notifications: await api.getNotifications(), roles: await api.listRoles() });
        if (s.isAdmin) store.set({ admin: await api.getAdmin() });
      }
    } catch { /* the next event or a resync tries again */ }
  }, 250));
}

async function reload(): Promise<void> {
  const s = store.get();
  const [chats, tasks, messages] = await Promise.all([
    api.listChats(), api.listTasks(), s.activeChatId ? api.listMessages(s.activeChatId) : Promise.resolve([]),
  ]);
  store.set({ chats, tasks, messages });
}

function wire(shell: HTMLElement): void {
  initResize($(".shell"));
  store.subscribe(render);
  store.flush();
  api.onEvent(applyEvent);

  onAction(shell, {
    "code-copy": (el) => onCodeAction(el),
    "code-wrap": (el) => onCodeAction(el),
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
    "gpu-toggle": (el) => {
      const open = new Set(store.get().gpuOpen);
      const k = el.dataset.gpu ?? "";
      if (open.has(k)) open.delete(k); else open.add(k);
      store.set({ gpuOpen: open });
    },
    "gpu-pin": (el) => {
      const pin = el.dataset.pin ?? "";
      const now = store.get().gpuPins;
      const gpuPins = now.includes(pin) ? now.filter((p) => p !== pin) : [...now, pin];
      store.set({ gpuPins });
      api.setGpuPins(gpuPins).catch(showError);
    },
    "pair-done": () => store.set({ pairing: undefined }),
    unpair: (el) => {
      const m = store.get().machines.find((x) => x.id === el.dataset.id);
      if (!m || !confirm(`Unpair ${m.name}? Its runner stops being accepted.`)) return;
      return api.unpairMachine(m.id).then(() => api.listMachines()).then((machines) => store.set({ machines }), showError);
    },
    "new-task": () => store.set({ editingTaskId: "new" }),
    "edit-task": (el) => store.set({ editingTaskId: el.dataset.id }),
    "cancel-task-edit": () => store.set({ editingTaskId: undefined }),
    "close-task-done": (el) => saveTask(el.dataset.id ?? "", { state: "done" }),
    "delete-task": (el) => {
      const t = store.get().tasks.find((x) => x.id === el.dataset.id);
      if (!t || !confirm(`Delete "${t.title}"?${t.source?.startsWith("windshift:") ? " It is closed in Windshift too." : ""}`)) return;
      const before = store.get().tasks;
      store.set({ tasks: before.filter((x) => x.id !== t.id), openTaskId: undefined });
      return api.deleteTask(t.id).catch((e) => { store.set({ tasks: before }); showError(e); });
    },
    "move-task": (el) => moveTask(el.dataset.id ?? "", Number(el.dataset.dir)),
    "make-internal": (el) => {
      if (!confirm("Stop syncing this project with Windshift? Everything stays here as an internal project.")) return;
      const id = el.dataset.id ?? "";
      return api.makeProjectInternal(id).then(() => store.set({
        projects: store.get().projects.map((p) => (p.id === id ? { ...p, kind: "internal" } : p)),
      }), showError);
    },
    "chat-move-open": (el) => store.set({ movingChatId: el.dataset.id }),
    "chat-move": (el) => {
      store.set({ movingChatId: undefined });
      changeChat(el.dataset.id ?? "", { projectId: el.dataset.project ?? "" });
    },
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
    tab: (el) => {
      store.set({ rightTab: el.dataset.tab as AppState["rightTab"], openTaskId: undefined });
      if (el.dataset.tab === "access") void loadAccess();
    },
    "grant-revoke": (el) => {
      const target = el.dataset.target ?? "";
      const machineId = el.dataset.machine ?? "";
      if (!confirm(`Revoke access to ${target}? The computer applies it at its next report.`)) return;
      markGrant(machineId, target, "revoke");
      return api.revokeGrant(machineId, target).catch((e) => { showError(e); void loadAccess(); });
    },
    "close-task": () => store.set({ openTaskId: undefined }),
    scope: (el) => store.set({ taskScope: el.dataset.scope as AppState["taskScope"] }),
    pane: (el) => store.set({ pane: el.dataset.pane as AppState["pane"] }),
    answer: (el) => api.answer(el.dataset.task ?? "", el.dataset.option ?? "").catch(showError),
    settings: () => {
      store.set({ settingsOpen: true, pane: "main" });
      api.getNotifications().then((notifications) => store.set({ notifications }), showError);
      if (store.get().isAdmin) api.getAdmin().then((admin) => store.set({ admin }), showError);
    },
    "remove-logo": () => {
      if (!confirm("Go back to the built-in logo?")) return;
      return api.removeLogo().then(() => store.set({ logoVersion: null }), showError);
    },
    "test-mail": () => {
      const to = (document.getElementById("test-to") as HTMLInputElement | null)?.value.trim() ?? "";
      if (!to) return adminMessage("Fill in an address to send the test to.", true);
      adminMessage("Sending…");
      api.testMail(to).then(() => adminMessage(`Test mail sent to ${to}.`), (e) => adminMessage(String(e.message ?? e), true));
    },
    // Ends this app's session, and the Keycloak session too after single sign-on.
    logout: () => api.logout().then((sso) => location.replace(sso ?? "/"), showError),
    "close-settings": () => store.set({ settingsOpen: false }),
  });

  shell.addEventListener("change", async (ev) => {
    const radio = ev.target as HTMLInputElement;
    if (radio.id === "logo-file" && radio.files?.[0]) {
      const file = radio.files[0];
      adminMessage("Uploading…");
      api.uploadLogo(file).then(() => api.status()).then((st) => {
        store.set({ logoVersion: st.logoVersion });
        adminMessage("Logo saved.");
      }, (e) => adminMessage(e instanceof Error ? e.message : String(e), true));
      return;
    }
    if (radio.name === "theme") {
      const theme = radio.value as ThemeChoice;
      applyTheme(theme);
      store.set({ theme });
      api.setTheme(theme).catch(showError);
      return;
    }
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
    if (store.get().chatMenuId && !t.closest(".menu, .chat-more")) store.set({ chatMenuId: undefined, movingChatId: undefined });
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
    const grantForm = (ev.target as HTMLElement).closest("form.grant-add") as HTMLFormElement | null;
    if (grantForm) {
      ev.preventDefault();
      const g = grantFromForm(grantForm);
      const machineId = grantForm.dataset.machine ?? "";
      markGrant(machineId, g.target, "add", { rights: g.rights });
      grantForm.reset();
      api.addGrant(machineId, g.target, g.rights, g.expiresHours).catch((e) => { showError(e); void loadAccess(); });
      return;
    }
    const pairForm = (ev.target as HTMLElement).closest("#pair-form") as HTMLFormElement | null;
    if (pairForm) {
      ev.preventDefault();
      const name = String(new FormData(pairForm).get("name") ?? "").trim();
      api.pairMachine(name).then(async (pairing) => store.set({ pairing, machines: await api.listMachines() }), showError);
      return;
    }
    const taskForm = (ev.target as HTMLElement).closest("#task-editor") as HTMLFormElement | null;
    if (taskForm) { ev.preventDefault(); submitTask(taskForm); return; }
    const nf = (ev.target as HTMLElement).closest("#notify-form") as HTMLFormElement | null;
    if (nf) {
      ev.preventDefault();
      const f = new FormData(nf);
      const p = { email: String(f.get("email") ?? "").trim(), onNeedsInput: f.has("onNeedsInput"), onFailed: f.has("onFailed"),
        onDone: f.has("onDone"), dailySummary: f.has("dailySummary") };
      const msg = nf.querySelector("#notify-msg");
      api.setNotifications(p).then(() => { if (msg) msg.textContent = "Saved."; },
        (e) => { if (msg) msg.textContent = e instanceof Error ? e.message : String(e); });
      return;
    }
    const admin = (ev.target as HTMLElement).closest("#admin-form") as HTMLFormElement | null;
    if (admin) { ev.preventDefault(); saveAdmin(admin); return; }
    const form = (ev.target as HTMLElement).closest("form.rename") as HTMLFormElement | null;
    if (form) { ev.preventDefault(); saveRename(form); }
  });
  shell.addEventListener("focusout", (ev) => {
    const form = (ev.target as HTMLElement).closest("form.rename") as HTMLFormElement | null;
    if (form && store.get().renamingChatId) saveRename(form);
  });
  // Drag a task card onto another to put it there (project view only).
  let dragId = "";
  shell.addEventListener("dragstart", (ev) => {
    const li = (ev.target as HTMLElement).closest<HTMLElement>("[data-drag-id]");
    if (!li) return;
    dragId = li.dataset.dragId ?? "";
    ev.dataTransfer?.setData("text/plain", dragId);
    li.classList.add("dragging");
  });
  shell.addEventListener("dragover", (ev) => {
    if (dragId && (ev.target as HTMLElement).closest("[data-drag-id]")) ev.preventDefault();
  });
  shell.addEventListener("dragend", () => {
    dragId = "";
    shell.querySelectorAll(".dragging").forEach((x) => x.classList.remove("dragging"));
  });
  shell.addEventListener("drop", (ev) => {
    const target = (ev.target as HTMLElement).closest<HTMLElement>("[data-drag-id]")?.dataset.dragId;
    if (!dragId || !target || target === dragId) return;
    ev.preventDefault();
    const s = store.get();
    const moved = s.tasks.find((t) => t.id === dragId);
    if (!moved) return;
    const ids = s.tasks.filter((t) => t.projectId === moved.projectId)
      .sort((a, b) => (a.position ?? 0) - (b.position ?? 0)).map((t) => t.id).filter((id) => id !== dragId);
    ids.splice(ids.indexOf(target), 0, dragId);
    api.reorderTasks(moved.projectId, ids).then(() => {
      const pos = new Map(ids.map((x, k) => [x, k]));
      store.set({ tasks: store.get().tasks.map((t) => (pos.has(t.id) ? { ...t, position: pos.get(t.id) } : t)) });
    }, showError);
  });

  // Machines tab: poll at the chosen rate, or on "Live" let the server push
  // over the event stream (renewing the 15 s watch). Nothing while hidden.
  let lastPoll = 0, lastWatch = 0;
  setInterval(() => {
    const s = store.get();
    if (s.rightTab !== "machines" || document.hidden) return;
    const now = Date.now();
    if (s.machinesRefresh === 1) {
      if (now - lastWatch > 10_000) { lastWatch = now; api.watchMachines().catch(() => {}); }
    } else if (now - lastPoll >= s.machinesRefresh * 1000) {
      lastPoll = now;
      api.listMachines().then((machines) => store.set({ machines })).catch(() => {});
    }
  }, 1000);
  shell.addEventListener("input", (ev) => {
    const el = ev.target as HTMLInputElement;
    if (el.id !== "machines-refresh") return;
    const seconds = REFRESH_STEPS[Number(el.value)] ?? 5;
    lastPoll = 0; lastWatch = 0; // apply at once
    store.set({ machinesRefresh: seconds });
    clearTimeout(savePrefs);
    savePrefs = setTimeout(() => api.setMachinesRefresh(seconds).catch(showError), 400);
  });

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

/** "system" follows the device; otherwise force light or dark. */
function applyTheme(theme: ThemeChoice): void {
  if (theme === "system") delete document.documentElement.dataset.theme;
  else document.documentElement.dataset.theme = theme;
}

function adminMessage(text: string, error = false): void {
  const el = document.getElementById("admin-msg");
  if (!el) return;
  el.textContent = text;
  el.classList.toggle("error", error);
}

async function saveAdmin(form: HTMLFormElement): Promise<void> {
  const f = new FormData(form);
  const v = (k: string) => String(f.get(k) ?? "").trim();
  const settings: AdminSettings = {
    appName: v("appName"), smtpHost: v("smtpHost"), smtpPort: Number(v("smtpPort")) || 587,
    smtpTls: v("smtpTls") as AdminSettings["smtpTls"], smtpUser: v("smtpUser"), smtpFrom: v("smtpFrom"), smtpReplyTo: v("smtpReplyTo"),
    colorBrand: v("colorBrand"), colorLinkDark: v("colorLinkDark"), colorLinkLight: v("colorLinkLight"), colorAccent: v("colorAccent"),
  };
  adminMessage("Saving…");
  try {
    const saved = await api.saveAdmin(settings);
    const s = store.get();
    store.set({ admin: { settings: saved, smtpPasswordSet: s.admin?.smtpPasswordSet ?? false } });
    if (s.server) store.set({ server: { ...s.server, name: saved.appName } });
    // Reload the colours from the server.
    const link = document.getElementById("theme-css") as HTMLLinkElement | null;
    if (link) link.href = `api/theme.css?v=${Date.now()}`;
    requestAnimationFrame(() => adminMessage("Saved."));
  } catch (e) {
    adminMessage(e instanceof Error ? e.message : String(e), true);
  }
}

async function saveTask(id: string, change: { title?: string; description?: string; state?: TaskState }): Promise<void> {
  try {
    const t = await api.updateTask(id, change);
    store.set({ tasks: store.get().tasks.map((x) => (x.id === id ? { ...x, ...t } : x)), editingTaskId: undefined, openTaskId: id });
  } catch (e) { showError(e); }
}

async function submitTask(form: HTMLFormElement): Promise<void> {
  const f = new FormData(form);
  const title = String(f.get("title") ?? "").trim();
  const description = String(f.get("description") ?? "").trim();
  const state = String(f.get("state") ?? "queued") as TaskState;
  const msg = form.querySelector("#task-msg");
  try {
    if (form.dataset.id) {
      const t = await api.updateTask(form.dataset.id, { title, description, state });
      store.set({ tasks: store.get().tasks.map((x) => (x.id === t.id ? { ...x, ...t } : x)), editingTaskId: undefined, openTaskId: t.id });
    } else {
      const t = await api.createTask({ projectId: form.dataset.project ?? "", title, description, state });
      store.set({ tasks: [...store.get().tasks, t], editingTaskId: undefined, openTaskId: t.id });
    }
  } catch (e) {
    if (msg) msg.textContent = e instanceof Error ? e.message : String(e);
  }
}

async function moveTask(id: string, dir: number): Promise<void> {
  const s = store.get();
  const t = s.tasks.find((x) => x.id === id);
  if (!t) return;
  const ids = s.tasks.filter((x) => x.projectId === t.projectId)
    .sort((a, b) => (a.position ?? 0) - (b.position ?? 0)).map((x) => x.id);
  const i = ids.indexOf(id), j = i + dir;
  if (i < 0 || j < 0 || j >= ids.length) return;
  [ids[i], ids[j]] = [ids[j], ids[i]];
  try {
    await api.reorderTasks(t.projectId, ids);
    const pos = new Map(ids.map((x, k) => [x, k]));
    store.set({ tasks: store.get().tasks.map((x) => (pos.has(x.id) ? { ...x, position: pos.get(x.id) } : x)) });
  } catch (e) { showError(e); }
}

/** Grants per paired machine and the access history. */
async function loadAccess(): Promise<void> {
  try {
    const machines = store.get().machines.filter((m) => m.id !== "server");
    const lists = await Promise.all(machines.map((m) => api.listGrants(m.id)));
    const grants = Object.fromEntries(machines.map((m, i) => [m.id, lists[i]]));
    store.set({ grants, accessHistory: await api.accessHistory() });
  } catch (e) { showError(e); }
}

async function changeChat(id: string, change: { title?: string; pinned?: boolean; archived?: boolean; projectId?: string }): Promise<void> {
  const before = store.get().chats;
  store.set({
    chatMenuId: undefined,
    chats: before.flatMap((c) => (c.id !== id ? [c] : change.archived ? [] : [{ ...c, ...change }])),
  });
  if (change.archived && store.get().activeChatId === id) store.set({ activeChatId: undefined, messages: [] });
  try {
    await api.updateChat(id, change);
  } catch (e) {
    store.set({ chats: before });
    showError(e);
  }
}

async function removeChat(id: string): Promise<void> {
  const before = store.get().chats;
  store.set({ chatMenuId: undefined, chats: before.filter((c) => c.id !== id) });
  if (store.get().activeChatId === id) store.set({ activeChatId: undefined, messages: [] });
  try {
    await api.deleteChat(id);
  } catch (e) {
    store.set({ chats: before });
    showError(e);
  }
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

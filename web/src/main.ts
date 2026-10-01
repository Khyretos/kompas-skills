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

const api: KompanionApi = new MockApi();
const root = $("#app");

showConnect(root, api, (server) => start(server));

async function start(server: Server): Promise<void> {
  const fresh = root.cloneNode(false) as HTMLElement; // drop the connect screen's listeners
  root.replaceWith(fresh);

  mount(fresh, html`
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

  const [projects, chats, tasks, providers, roles, machines, today] = await Promise.all([
    api.listProjects(), api.listChats(), api.listTasks(), api.listProviders(), api.listRoles(),
    api.listMachines(), api.today(),
  ]);
  store.set({ server, projects, chats, tasks, providers, roles, machines, today });
  wire(fresh);
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

function wire(shell: HTMLElement): void {
  store.subscribe(render);
  store.flush();
  api.onEvent(applyEvent);

  onAction(shell, {
    "open-chat": (el) => openChat(el.dataset.id),
    "new-chat": () => store.set({ activeChatId: undefined, messages: [], pane: "main" }),
    "open-task": (el) => store.set({ openTaskId: el.dataset.id, rightTab: "tasks", pane: "right" }),
    tab: (el) => store.set({ rightTab: el.dataset.tab as AppState["rightTab"], openTaskId: undefined }),
    "close-task": () => store.set({ openTaskId: undefined }),
    scope: (el) => store.set({ taskScope: el.dataset.scope as AppState["taskScope"] }),
    pane: (el) => store.set({ pane: el.dataset.pane as AppState["pane"] }),
    answer: (el) => api.answer(el.dataset.task ?? "", el.dataset.option ?? ""),
    settings: () => store.set({ settingsOpen: true, pane: "main" }),
    "close-settings": () => store.set({ settingsOpen: false }),
  });

  shell.addEventListener("change", async (ev) => {
    const sel = ev.target as HTMLSelectElement;
    if (!sel.dataset.role) return;
    const [providerId, modelId] = sel.value.split("::");
    await api.setRole({ role: sel.dataset.role as Role, providerId, modelId });
    store.set({ roles: await api.listRoles() });
  });

  document.addEventListener("keydown", (ev) => {
    if (ev.key === "Escape" && store.get().settingsOpen) store.set({ settingsOpen: false });
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
      const chat = await api.createChat(title);
      store.set({ chats: [chat, ...store.get().chats], activeChatId: chat.id });
      chatId = chat.id;
    }
    await api.send(chatId, text);
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

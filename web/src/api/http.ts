// The real client: talks to a Kompanion server over HTTPS (same origin) with
// a session cookie. Live updates arrive as server-sent events.
import type { KompanionApi, ServerEvent } from "./client";
import type {
  AdminSettings, NotificationPrefs, ThemeChoice, TaskState, Chat, DaySummary, MachineStats, Message, ModelProvider, Project, RoleAssignment, SearchResult, Server, ServerStatus, Task,
} from "./types";

export class ApiError extends Error {
  constructor(message: string, readonly status: number) {
    super(message);
  }
}

export class HttpApi implements KompanionApi {
  private source?: EventSource;
  private listeners = new Set<(ev: ServerEvent) => void>();

  constructor(private base = "") {}

  private async request<T>(method: string, path: string, body?: unknown): Promise<T> {
    const raw = body instanceof Blob;
    const res = await fetch(`${this.base}/api${path}`, {
      method,
      credentials: "same-origin",
      headers: {
        ...(body === undefined ? {} : { "Content-Type": raw ? (body as Blob).type || "application/octet-stream" : "application/json" }),
        // Required by the server for anything that changes state (CSRF guard).
        "X-Kompanion": "1",
      },
      body: body === undefined ? undefined : raw ? (body as Blob) : JSON.stringify(body),
    });
    if (!res.ok) {
      let message = `The server answered ${res.status}.`;
      try {
        message = (await res.json()).error ?? message;
      } catch { /* not JSON */ }
      throw new ApiError(message, res.status);
    }
    if (res.status === 204 || res.status === 202) return undefined as T;
    return res.json() as Promise<T>;
  }

  async discover(): Promise<Server[]> {
    return []; // Local-network discovery needs the native app.
  }

  async connect(url: string): Promise<Server> {
    const s = await this.status();
    return { url, name: new URL(url).hostname, version: s.version };
  }

  status() { return this.request<ServerStatus>("GET", "/status"); }
  setup(code: string, name: string, password: string) {
    return this.request<void>("POST", "/setup", { code, name, password });
  }
  login(name: string, password: string) { return this.request<void>("POST", "/login", { name, password }); }
  async logout() { return (await this.request<{ redirect?: string | null }>("POST", "/logout")).redirect ?? null; }
  getNotifications() { return this.request<NotificationPrefs>("GET", "/me/notifications"); }
  setNotifications(p: NotificationPrefs) { return this.request<void>("PUT", "/me/notifications", p); }
  setGpuPins(pins: string[]) { return this.request<void>("PUT", "/me/prefs", { gpuPins: pins }); }
  setCardStyle(style: import("../core/cardtypes").CardStyle) { return this.request<void>("PUT", "/me/prefs", { cardStyle: style }); }
  setMachinesRefresh(seconds: number) { return this.request<void>("PUT", "/me/prefs", { machinesRefresh: seconds }); }
  pairMachine(name: string) { return this.request<{ id: string; name: string; token: string }>("POST", "/machines", { name }); }
  listGrants(machineId: string) { return this.request<import("../views/access").GrantView[]>("GET", `/machines/${encodeURIComponent(machineId)}/grants`); }
  addGrant(machineId: string, target: string, rights: string[], expiresHours?: number) {
    return this.request<void>("POST", `/machines/${encodeURIComponent(machineId)}/grants`, { target, rights, expires_hours: expiresHours ?? null });
  }
  revokeGrant(machineId: string, target: string) {
    return this.request<void>("POST", `/machines/${encodeURIComponent(machineId)}/grants/revoke`, { target });
  }
  accessHistory() { return this.request<import("../views/access").AccessEvent[]>("GET", "/access"); }
  pairCode(name: string) { return this.request<{ code: string; expiresAt: string }>("POST", "/machines/pair-code", { name }); }
  unpairMachine(id: string) { return this.request<void>("DELETE", `/machines/${encodeURIComponent(id)}`); }
  watchMachines() { return this.request<void>("POST", "/machines/live"); }
  setTheme(theme: ThemeChoice) { return this.request<void>("PUT", "/me/theme", { theme }); }
  getAdmin() { return this.request<{ settings: AdminSettings; smtpPasswordSet: boolean }>("GET", "/admin/settings"); }
  saveAdmin(settings: AdminSettings) { return this.request<AdminSettings>("PUT", "/admin/settings", settings); }
  uploadLogo(file: File) { return this.request<void>("PUT", "/admin/logo", file); }
  removeLogo() { return this.request<void>("DELETE", "/admin/logo"); }
  testMail(to: string) { return this.request<void>("POST", "/admin/test-mail", { to }); }

  listProjects() { return this.request<Project[]>("GET", "/projects"); }
  setProjectSettings(projectId: string, change: { type?: Project["type"]; repoFolder?: string; repoMachineId?: string }) {
    return this.request<void>("PATCH", `/projects/${encodeURIComponent(projectId)}/settings`, change);
  }
  projectAssets(projectId: string) {
    return this.request<import("../views/projectpanel").ProjectAsset[]>("GET", `/projects/${encodeURIComponent(projectId)}/assets`);
  }
  attachAsset(projectId: string, assetId: number) {
    return this.request<void>("POST", `/projects/${encodeURIComponent(projectId)}/assets`, { assetId });
  }
  detachAsset(projectId: string, assetId: number) {
    return this.request<void>("DELETE", `/projects/${encodeURIComponent(projectId)}/assets/${assetId}`);
  }
  listChats() { return this.request<Chat[]>("GET", "/chats"); }
  async search(q: string) { return (await this.request<{ results: SearchResult[] }>("GET", `/search?q=${encodeURIComponent(q)}`)).results; }
  listMessages(chatId: string) { return this.request<Message[]>("GET", `/chats/${encodeURIComponent(chatId)}/messages`); }
  listTasks() { return this.request<Task[]>("GET", "/tasks"); }
  listProviders() { return this.request<ModelProvider[]>("GET", "/providers"); }
  listRoles() { return this.request<RoleAssignment[]>("GET", "/roles"); }
  setRole(a: RoleAssignment) { return this.request<void>("PUT", "/roles", a); }
  listMachines() { return this.request<MachineStats[]>("GET", "/machines"); }
  async today(): Promise<DaySummary> {
    return { tasks: 0, localTokens: 0, cloudTokens: 0, cloudCostEur: 0, energyKwh: 0 };
  }

  createChat(title: string, projectId?: string) { return this.request<Chat>("POST", "/chats", { title, projectId }); }
  openThread(projectId: string) { return this.request<{ chatId: string }>("POST", `/projects/${encodeURIComponent(projectId)}/thread`).then((r) => r.chatId); }
  updateChat(chatId: string, change: { title?: string; pinned?: boolean; archived?: boolean; projectId?: string }) {
    return this.request<void>("PATCH", `/chats/${encodeURIComponent(chatId)}`, change);
  }
  createTask(t: { projectId: string; title: string; description: string; state?: TaskState }) {
    return this.request<Task>("POST", "/tasks", t);
  }
  updateTask(id: string, change: { title?: string; description?: string; state?: TaskState }) {
    return this.request<Task>("PATCH", `/tasks/${encodeURIComponent(id)}`, change);
  }
  deleteTask(id: string) { return this.request<void>("DELETE", `/tasks/${encodeURIComponent(id)}`); }
  reorderTasks(projectId: string, ids: string[]) { return this.request<void>("PUT", "/tasks/order", { projectId, ids }); }
  makeProjectInternal(projectId: string) {
    return this.request<void>("PATCH", `/projects/${encodeURIComponent(projectId)}`, { kind: "internal" });
  }
  deleteChat(chatId: string) { return this.request<void>("DELETE", `/chats/${encodeURIComponent(chatId)}`); }
  send(chatId: string, text: string, machineId?: string) {
    return this.request<void>("POST", `/chats/${encodeURIComponent(chatId)}/messages`, { text, machine_id: machineId ?? null });
  }
  listActions(chatId: string) { return this.request<import("./client").PcAction[]>("GET", `/chats/${encodeURIComponent(chatId)}/actions`); }
  stopAction(id: string) { return this.request<void>("POST", `/actions/${encodeURIComponent(id)}/stop`); }
  taskRuns(taskId: string) { return this.request<{ id: string; startedAt: string; endedAt: string | null; status: string; step: string | null }[]>("GET", `/tasks/${encodeURIComponent(taskId)}/runs`); }
  taskCosts(taskId: string) { return this.request<import("./types").CostLine | null>("GET", `/tasks/${encodeURIComponent(taskId)}/costs`); }
  weeklyCosts() { return this.request<import("./types").WeeklyCosts>("GET", "/costs/weekly"); }
  stopTask(id: string) { return this.request<void>("POST", `/tasks/${encodeURIComponent(id)}/stop`); }
  checkFolder(machineId: string, path: string) {
    return this.request<{ state: "ok" | "nogrant" | "missing" | "notfolder" | "noanswer"; path: string; folders?: string[]; files?: number; message?: string }>("POST", `/machines/${encodeURIComponent(machineId)}/folder`, { path });
  }
  startTask(id: string, machineId: string, folder: string, check: string) {
    return this.request<void>("POST", `/tasks/${encodeURIComponent(id)}/start`, { machine_id: machineId, folder, check });
  }
  listActivity() { return this.request<import("../views/activity").ActivityItem[]>("GET", "/activity"); }
  getCapabilities() { return this.request<import("../views/capabilities").Capabilities>("GET", "/capabilities"); }
  gpuTimeline(hours: 1 | 24) { return this.request<import("../views/gputimeline").TlGpu[]>("GET", `/gpus/timeline?hours=${hours}`); }
  voiceInfo() { return this.request<{ enabled: boolean; voices: { id: string; label: string }[] }>("GET", "/voice"); }
  async transcribe(audio: Blob, lang: string) {
    return (await this.request<{ text: string }>("POST", `/voice/transcribe${lang ? `?lang=${lang}` : ""}`, audio)).text;
  }
  async speak(text: string, voice: string) {
    const res = await fetch(`${this.base}/api/voice/speak`, {
      method: "POST",
      credentials: "same-origin",
      headers: { "Content-Type": "application/json", "X-Kompanion": "1" },
      body: JSON.stringify({ text, voice }),
    });
    if (!res.ok) {
      let message = `The server answered ${res.status}.`;
      try { message = (await res.json()).error ?? message; } catch { /* not JSON */ }
      throw new ApiError(message, res.status);
    }
    return res.blob();
  }
  getSkill(id: string) { return this.request<{ id: string; text: string }>("GET", `/capabilities/skill?id=${encodeURIComponent(id)}`); }
  decideAction(id: string, decision: "approve" | "always" | "deny") {
    return this.request<void>("POST", `/actions/${encodeURIComponent(id)}/decide`, { decision });
  }
  async answer(): Promise<void> {
    throw new ApiError("Tasks arrive in a later milestone.", 501);
  }

  onEvent(listener: (ev: ServerEvent) => void) {
    this.listeners.add(listener);
    if (!this.source) {
      // EventSource reconnects by itself; after a reconnect we may have missed
      // events, so ask the app to reload its lists.
      this.source = new EventSource(`${this.base}/api/events`, { withCredentials: true });
      let opened = false;
      this.source.onopen = () => {
        if (opened) this.emit({ type: "resync" });
        opened = true;
      };
      this.source.onmessage = (e) => {
        try {
          this.emit(JSON.parse(e.data) as ServerEvent);
        } catch { /* ignore malformed */ }
      };
      this.source.addEventListener("resync", () => this.emit({ type: "resync" }));
    }
    return () => this.listeners.delete(listener);
  }

  private emit(ev: ServerEvent) {
    this.listeners.forEach((l) => l(ev));
  }
}

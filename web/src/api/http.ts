// The real client: talks to a Kompanion server over HTTPS (same origin) with
// a session cookie. Live updates arrive as server-sent events.
import type { KompanionApi, ServerEvent } from "./client";
import type {
  AdminSettings, ThemeChoice, TaskState, Chat, DaySummary, MachineStats, Message, ModelProvider, Project, RoleAssignment, Server, ServerStatus, Task,
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
    const res = await fetch(`${this.base}/api${path}`, {
      method,
      credentials: "same-origin",
      headers: {
        ...(body === undefined ? {} : { "Content-Type": "application/json" }),
        // Required by the server for anything that changes state (CSRF guard).
        "X-Kompanion": "1",
      },
      body: body === undefined ? undefined : JSON.stringify(body),
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
  logout() { return this.request<void>("POST", "/logout"); }
  setMachinesRefresh(seconds: number) { return this.request<void>("PUT", "/me/prefs", { machinesRefresh: seconds }); }
  pairMachine(name: string) { return this.request<{ id: string; name: string; token: string }>("POST", "/machines", { name }); }
  unpairMachine(id: string) { return this.request<void>("DELETE", `/machines/${encodeURIComponent(id)}`); }
  watchMachines() { return this.request<void>("POST", "/machines/live"); }
  setTheme(theme: ThemeChoice) { return this.request<void>("PUT", "/me/theme", { theme }); }
  getAdmin() { return this.request<{ settings: AdminSettings; smtpPasswordSet: boolean }>("GET", "/admin/settings"); }
  saveAdmin(settings: AdminSettings) { return this.request<AdminSettings>("PUT", "/admin/settings", settings); }
  testMail(to: string) { return this.request<void>("POST", "/admin/test-mail", { to }); }

  listProjects() { return this.request<Project[]>("GET", "/projects"); }
  listChats() { return this.request<Chat[]>("GET", "/chats"); }
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
  updateChat(chatId: string, change: { title?: string; pinned?: boolean; archived?: boolean }) {
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
  send(chatId: string, text: string) {
    return this.request<void>("POST", `/chats/${encodeURIComponent(chatId)}/messages`, { text });
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

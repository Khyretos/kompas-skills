// The app talks to a Kompanion server only through this interface.
// `MockApi` implements it for the draft; `HttpApi` (next milestone) will call
// the real server over HTTPS + server-sent events.
import type { NotificationPrefs, TaskState, AdminSettings, ThemeChoice, DaySummary, MachineStats, ServerStatus, Chat, Message, Project, RoleAssignment, ModelProvider, Server, Task } from "./types";

/** A step Kompanion wants to run on a paired computer (F6), waiting for approval. */
export interface PcAction {
  id: string;
  machineId: string;
  summary: string;
  state: "pending" | "approved" | "always" | "granting" | "running" | "denied" | "done" | "failed" | "refused";
  /** The grant the step needs, e.g. "packages + root (asks for the password on the PC)". */
  needs?: string | null;
  /** The runner tool call: `{ "tool": "shell", "cwd": .., "command": .. }` and so on. */
  tool?: Record<string, unknown>;
  result: string | null;
  createdAt: string;
}

export interface KompanionApi {
  discover(): Promise<Server[]>;
  connect(url: string): Promise<Server>;
  status(): Promise<ServerStatus>;
  setup(code: string, name: string, password: string): Promise<void>;
  login(name: string, password: string): Promise<void>;
  /** Signs out; returns the provider's sign-out page when single sign-on was used. */
  logout(): Promise<string | null>;
  setTheme(theme: ThemeChoice): Promise<void>;
  setMachinesRefresh(seconds: number): Promise<void>;
  setGpuPins(pins: string[]): Promise<void>;
  getNotifications(): Promise<NotificationPrefs>;
  setNotifications(p: NotificationPrefs): Promise<void>;
  /** Keeps the live machine feed on for about 15 s. */
  watchMachines(): Promise<void>;
  /** Pairs a PC; the token is returned only this once. */
  pairMachine(name: string): Promise<{ id: string; name: string; token: string }>;
  /** A one-time code for the one-line runner install (15 minutes). */
  pairCode(name: string): Promise<{ code: string; expiresAt: string }>;
  unpairMachine(id: string): Promise<void>;
  listGrants(machineId: string): Promise<import("../views/access").GrantView[]>;
  addGrant(machineId: string, target: string, rights: string[], expiresHours?: number): Promise<void>;
  revokeGrant(machineId: string, target: string): Promise<void>;
  accessHistory(): Promise<import("../views/access").AccessEvent[]>;
  getAdmin(): Promise<{ settings: AdminSettings; smtpPasswordSet: boolean }>;
  saveAdmin(settings: AdminSettings): Promise<AdminSettings>;
  testMail(to: string): Promise<void>;
  uploadLogo(file: File): Promise<void>;
  removeLogo(): Promise<void>;

  listProjects(): Promise<Project[]>;
  listChats(): Promise<Chat[]>;
  listMessages(chatId: string): Promise<Message[]>;
  listTasks(projectId?: string): Promise<Task[]>;
  listProviders(): Promise<ModelProvider[]>;
  listMachines(): Promise<MachineStats[]>;
  today(): Promise<DaySummary>;
  listRoles(projectId?: string): Promise<RoleAssignment[]>;
  setRole(assignment: RoleAssignment, projectId?: string): Promise<void>;

  createChat(title: string, projectId?: string): Promise<Chat>;
  updateChat(chatId: string, change: { title?: string; pinned?: boolean; archived?: boolean; projectId?: string }): Promise<void>;
  deleteChat(chatId: string): Promise<void>;
  createTask(t: { projectId: string; title: string; description: string; state?: TaskState }): Promise<Task>;
  updateTask(id: string, change: { title?: string; description?: string; state?: TaskState }): Promise<Task>;
  deleteTask(id: string): Promise<void>;
  reorderTasks(projectId: string, ids: string[]): Promise<void>;
  makeProjectInternal(projectId: string): Promise<void>;
  /** Sends a message; the reply streams back through `onEvent`. */
  /** With `machineId`, the answer may use that computer's tools (each step needs approval). */
  send(chatId: string, text: string, machineId?: string): Promise<void>;
  listActions(chatId: string): Promise<PcAction[]>;
  /** W2: run a task by itself on a computer, in a folder, checked by a command. */
  startTask(id: string, machineId: string, folder: string, check: string): Promise<void>;
  listActivity(): Promise<import("../views/activity").ActivityItem[]>;
  decideAction(id: string, decision: "approve" | "always" | "deny"): Promise<void>;
  answer(taskId: string, optionId: string): Promise<void>;

  /** Live updates: streamed tokens, task progress, new messages. A "resync"
   * event means some updates were missed and lists should be reloaded. */
  onEvent(listener: (ev: ServerEvent) => void): () => void;
}

export type ServerEvent =
  | { type: "message"; message: Message }
  | { type: "message-delta"; messageId: string; chatId: string; text: string; done: boolean }
  | { type: "task"; task: Task }
  | { type: "machines"; machines: MachineStats[] }
  | { type: "changed"; what: "tasks" | "projects" | "chats" | "machines" | "access" | "settings" | "actions"; machineId?: string }
  | { type: "assets"; scan?: unknown; previews?: unknown } // Assets section news (api/assets.ts)
  | { type: "resync" };

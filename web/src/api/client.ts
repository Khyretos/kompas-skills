// The app talks to a Kompanion server only through this interface.
// `MockApi` implements it for the draft; `HttpApi` (next milestone) will call
// the real server over HTTPS + server-sent events.
import type { AdminSettings, ThemeChoice, DaySummary, MachineStats, ServerStatus, Chat, Message, Project, RoleAssignment, ModelProvider, Server, Task } from "./types";

export interface KompanionApi {
  discover(): Promise<Server[]>;
  connect(url: string): Promise<Server>;
  status(): Promise<ServerStatus>;
  setup(code: string, name: string, password: string): Promise<void>;
  login(name: string, password: string): Promise<void>;
  logout(): Promise<void>;
  setTheme(theme: ThemeChoice): Promise<void>;
  setMachinesRefresh(seconds: number): Promise<void>;
  /** Keeps the live machine feed on for about 15 s. */
  watchMachines(): Promise<void>;
  getAdmin(): Promise<{ settings: AdminSettings; smtpPasswordSet: boolean }>;
  saveAdmin(settings: AdminSettings): Promise<AdminSettings>;
  testMail(to: string): Promise<void>;

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
  updateChat(chatId: string, change: { title?: string; pinned?: boolean; archived?: boolean }): Promise<void>;
  deleteChat(chatId: string): Promise<void>;
  /** Sends a message; the reply streams back through `onEvent`. */
  send(chatId: string, text: string): Promise<void>;
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
  | { type: "resync" };

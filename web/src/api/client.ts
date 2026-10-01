// The app talks to a Kompanion server only through this interface.
// `MockApi` implements it for the draft; `HttpApi` (next milestone) will call
// the real server over HTTPS + server-sent events.
import type { DaySummary, MachineStats, Chat, Message, Project, RoleAssignment, ModelProvider, Server, Task } from "./types";

export interface KompanionApi {
  discover(): Promise<Server[]>;
  connect(url: string): Promise<Server>;

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
  /** Sends a message; the reply streams back through `onEvent`. */
  send(chatId: string, text: string): Promise<void>;
  answer(taskId: string, optionId: string): Promise<void>;

  /** Live updates: streamed tokens, task progress, new messages. */
  onEvent(listener: (ev: ServerEvent) => void): () => void;
}

export type ServerEvent =
  | { type: "message"; message: Message }
  | { type: "message-delta"; messageId: string; chatId: string; text: string; done: boolean }
  | { type: "task"; task: Task }
  | { type: "machines"; machines: MachineStats[] };

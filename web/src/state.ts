import { Store } from "./core/store";
import type { DaySummary, MachineStats, Chat, Message, ModelProvider, Project, RoleAssignment, Server, Task } from "./api/types";

export interface AppState {
  server?: Server;
  projects: Project[];
  chats: Chat[];
  activeChatId?: string;
  messages: Message[]; // messages of the active chat
  tasks: Task[];
  providers: ModelProvider[];
  roles: RoleAssignment[];
  machines: MachineStats[];
  today?: DaySummary;
  rightTab: "tasks" | "machines";
  openTaskId?: string; // task shown in detail
  taskScope: "project" | "all";
  settingsOpen: boolean;
  pane: "main" | "left" | "right"; // which pane is visible on a phone
}

export const store = new Store<AppState>({
  projects: [],
  chats: [],
  messages: [],
  tasks: [],
  providers: [],
  roles: [],
  machines: [],
  rightTab: "tasks",
  taskScope: "project",
  settingsOpen: false,
  pane: "main",
});

export function activeChat(s: AppState): Chat | undefined {
  return s.chats.find((c) => c.id === s.activeChatId);
}

export function activeProject(s: AppState): Project | undefined {
  const chat = activeChat(s);
  return chat?.projectId ? s.projects.find((p) => p.id === chat.projectId) : undefined;
}

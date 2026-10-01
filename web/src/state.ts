import { Store } from "./core/store";
import type { AdminSettings, ThemeChoice, DaySummary, MachineStats, Chat, Message, ModelProvider, Project, RoleAssignment, Server, Task } from "./api/types";

export interface AppState {
  server?: Server;
  userName?: string; // signed-in user
  isAdmin: boolean;
  theme: ThemeChoice;
  machinesRefresh: number; // seconds between Machines updates; 1 = live
  admin?: { settings: AdminSettings; smtpPasswordSet: boolean }; // loaded when an admin opens settings
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
  expandedProjects: Set<string>; // projects open in the sidebar
  activeProjectId?: string; // project picked in the sidebar (tasks pane, new chats)
  chatMenuId?: string; // chat whose options menu is open
  renamingChatId?: string; // chat being renamed in place
  editingTaskId?: string; // task open in the editor ("new" for a new one)
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
  expandedProjects: new Set(),
  isAdmin: false,
  theme: "system",
  machinesRefresh: 5,
});

export function activeChat(s: AppState): Chat | undefined {
  return s.chats.find((c) => c.id === s.activeChatId);
}

export function activeProject(s: AppState): Project | undefined {
  const chat = activeChat(s);
  const id = chat?.projectId ?? s.activeProjectId;
  return id ? s.projects.find((p) => p.id === id) : undefined;
}

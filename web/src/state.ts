import { Store } from "./core/store";
import type { NotificationPrefs, AdminSettings, ThemeChoice, DaySummary, MachineStats, Chat, Message, ModelProvider, Project, RoleAssignment, Server, Task } from "./api/types";

export interface AppState {
  server?: Server;
  userName?: string; // signed-in user
  isAdmin: boolean;
  grants: Record<string, import("./views/access").GrantView[]>; // per paired machine
  accessHistory: import("./views/access").AccessEvent[];
  logoVersion?: string | null;
  notifications?: NotificationPrefs; // loaded when Settings opens
  windshiftWarning?: string | null;
  windshift?: string; // "connected" | "not configured" (set in the server's compose file only)
  theme: ThemeChoice;
  machinesRefresh: number; // seconds between Machines updates; 1 = live
  gpuOpen: Set<string>; // GPU panels expanded (this session)
  gpuPins: string[]; // pinned GPU bars, saved per user
  pcMachineId?: string; // the computer picked in the composer: answers may use its tools
  pcActions: import("./api/client").PcAction[]; // approval cards of the open chat
  pairing?: { code: string; expiresAt: string; name: string }; // the one-line install command, until done
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
  rightTab: "tasks" | "machines" | "access" | "activity";
  activity: import("./views/activity").ActivityItem[];
  activityFilter: import("./views/activity").ActivityFilter;
  openTaskId?: string; // task shown in detail
  taskScope: "project" | "all";
  settingsOpen: boolean;
  pane: "main" | "left" | "right"; // which pane is visible on a phone
  section: "chat" | "assets" | "capabilities"; // what the middle of the screen shows
  capabilities?: import("./views/capabilities").Capabilities; // loaded when the Capabilities section opens
  expandedProjects: Set<string>; // projects open in the sidebar
  allTasksShown: Set<string>; // projects whose sidebar task list shows every task ("N more" pressed)
  projectAssets: Record<string, import("./views/projectpanel").ProjectAsset[]>; // game projects, loaded when shown
  assetPick: { project: string; q: string; items: import("./views/projectpanel").PickResult[]; busy: boolean }; // the picker
  activeProjectId?: string; // project picked in the sidebar (tasks pane, new chats)
  chatMenuId?: string; // chat whose options menu is open
  movingChatId?: string; // chat whose "Move to project" list is open
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
  section: "chat",
  expandedProjects: new Set(),
  allTasksShown: new Set(),
  projectAssets: {},
  assetPick: { project: "", q: "", items: [], busy: false },
  isAdmin: false,
  theme: "system",
  machinesRefresh: 5,
  grants: {},
  pcActions: [],
  activity: [],
  activityFilter: {},
  accessHistory: [],
  gpuOpen: new Set(),
  gpuPins: [],
});

export function activeChat(s: AppState): Chat | undefined {
  return s.chats.find((c) => c.id === s.activeChatId);
}

export function activeProject(s: AppState): Project | undefined {
  const chat = activeChat(s);
  const id = chat?.projectId ?? s.activeProjectId;
  return id ? s.projects.find((p) => p.id === id) : undefined;
}

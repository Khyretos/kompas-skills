// Shapes shared with the Kompanion server. Later these are generated from the
// Rust `protocol` crate so both sides always agree.

export type Role = "orchestrator" | "worker" | "reviewer";

export interface ModelProvider {
  id: string;
  name: string; // "OVMS on kireserver"
  kind: "openai-compatible" | "anthropic";
  baseUrl: string;
  local: boolean;
  models: ModelInfo[];
  error?: string; // set when the server couldn't reach it
}

export interface ModelInfo {
  id: string; // "qwen3.5-9b"
  // Measured by the setup check; undefined until tested.
  contextTokens?: number;
  tokensPerSecond?: number;
  toolCalls?: boolean;
  jsonSchema?: boolean;
}

export interface RoleAssignment {
  role: Role;
  providerId: string;
  modelId: string;
}

export interface Project {
  id: string;
  name: string;
  description: string;
  updatedAt: string; // ISO
  kind?: "internal" | "windshift";
  /** chat (default), game (library assets attached) or programming (a repo folder on a computer). */
  type?: "chat" | "game" | "programming";
  repoFolder?: string | null;
  repoMachineId?: string | null;
}

export interface Chat {
  id: string;
  title: string;
  projectId?: string; // undefined = loose chat
  updatedAt: string;
  pinned?: boolean;
}

export interface Message {
  id: string;
  chatId: string;
  author: "user" | "orchestrator";
  text: string;
  at: string;
  streaming?: boolean;
  taskIds?: string[]; // tasks this message created
}

export type TaskState =
  | "queued"
  | "waiting_resources"
  | "running"
  | "needs_input"
  | "in_review"
  | "done"
  | "failed";

/** A hit of the global search (Ctrl+K). The snippet marks the matched words with \u0002 … \u0003. */
export interface SearchResult {
  kind: "task" | "chat" | "message" | "project" | "setting";
  id: string;
  parent: string | null; // task: its project; message: its chat
  title: string;
  snippet: string;
}

export interface Task {
  id: string;
  projectId: string;
  title: string;
  state: TaskState;
  progress: number; // 0..1
  step: string; // what it is doing right now
  role: Role;
  model: string; // display name of the model doing it
  runner?: string; // "soucouyant (Linux)"
  workspace?: string; // "container: rust-1.83, 4 cores, 8 GB"
  scheduledFor?: string;
  description?: string; // markdown: goal, steps, done when
  position?: number;
  source?: string | null; // "windshift:SRV-12" for synced tasks
  events: TaskEvent[];
  question?: TaskQuestion;
}

export type TaskEvent =
  | { kind: "step"; at: string; text: string }
  | { kind: "tool"; at: string; tool: string; detail: string; ok: boolean }
  | { kind: "diff"; at: string; file: string; added: number; removed: number }
  | { kind: "review"; at: string; model: string; verdict: "pass" | "fail"; note: string }
  | { kind: "lesson"; at: string; skill: string; note: string }
  | { kind: "screenshot"; at: string; caption: string }
  | { kind: "call"; at: string; call: ModelCall };

/** One request to a model, stored in full so you can see exactly what it was asked. */
export interface ModelCall {
  id: string;
  role: Role;
  model: string;
  provider: string;
  reason: string; // why the orchestrator made this call
  request: { system: string; context: string[]; prompt: string };
  response: string;
  tokensIn: number;
  tokensOut: number;
  ms: number;
  costEur: number; // 0 for local models
  energyWh?: number; // local models: measured on the GPU
}

/** Live stats a runner (or the server) reports every few seconds. */
export interface MachineStats {
  id: string;
  name: string;
  os: string;
  /** The paired computer's runner version (null before 0.4.5), and the newest the server ships. */
  runnerVersion?: string | null;
  runnerLatest?: string | null;
  online: boolean;
  cpu: number; // 0..1
  ramUsedGb: number;
  ramTotalGb: number;
  gpus: GpuStats[];
  kompanionShare: number; // share of CPU used by Kompanion workspaces, 0..1
  busy?: string; // "Steam is running"
  history: number[]; // recent total power draw in W (or CPU share 0..1 when historyKind is "cpu"), oldest first
  historyKind?: "watts" | "cpu";
  sampledAt?: string; // ISO time of the newest sample
  powerHistory?: number[]; // total GPU power in W, oldest first
  diskUsedGb?: number;
  diskTotalGb?: number;
  cpuCount?: number;
}

export interface GpuStats {
  name: string;
  pciSlot?: string;
  driver?: string;
  load: number | null; // 0..1
  vramUsedGb: number | null;
  vramTotalGb: number | null;
  watts: number | null;
  tempC: number | null;
  coreMhz?: number | null;
  memMhz?: number | null;
  fanRpm?: number | null;
  coreMaxMhz?: number | null;
  powerCapW?: number | null;
  engines?: { name: string; busy: number }[];
  /** The engine and VRAM numbers are last-known values, a few seconds old. */
  stale?: boolean;
  use: string; // what it is used for, e.g. "AI: OVMS (Qwen)"
}

export interface DaySummary {
  tasks: number;
  localTokens: number;
  cloudTokens: number;
  cloudCostEur: number;
  energyKwh: number;
}

export interface TaskQuestion {
  kind: "resources" | "approval" | "choice";
  text: string;
  /** For approvals: exactly what will run, where, and what it touches. */
  action?: { machine: string; cwd: string; command?: string; diff?: string; network?: string[] };
  options: { id: string; label: string; detail?: string; recommended?: boolean }[];
}

export interface Server {
  url: string;
  name: string;
  version: string;
  discovered?: boolean; // found on the local network
  demo?: boolean; // example data, not a real server
}

export interface ServerStatus {
  name: string;
  version: string;
  setupNeeded: boolean;
  user: string | null;
  admin?: boolean;
  theme?: ThemeChoice;
  machinesRefresh?: number; // seconds; 1 = live
  gpuPins?: string[];
  cardStyle?: import("../core/cardtypes").CardStyle;
  windshift?: "connected" | "not configured";
  windshiftWarning?: string | null;
  logoVersion?: string | null; // set when an admin uploaded a logo
  /** Areas switched on in the server's [features]; absent on older servers (all on). */
  features?: Features;
  /** How people can sign in; absent on older servers (password only). */
  signIn?: { password: boolean; oidc: string | null };
}

/** The server's [features] switches (TEN-02). */
export interface Features { assets: boolean; gpus: boolean; voice: boolean; windshift: boolean }
export const ALL_FEATURES: Features = { assets: true, gpus: true, voice: true, windshift: true };

export type ThemeChoice = "system" | "light" | "dark";

export interface AdminSettings {
  appName: string;
  smtpHost: string;
  smtpPort: number;
  smtpTls: "starttls" | "tls" | "none";
  smtpUser: string;
  smtpFrom: string;
  smtpReplyTo: string;
  colorBrand: string;
  colorLinkDark: string;
  colorLinkLight: string;
  colorAccent: string;
}

export interface NotificationPrefs {
  email: string;
  onNeedsInput: boolean;
  onFailed: boolean;
  onDone: boolean;
  dailySummary: boolean;
}

/** TEN-05: one task's cost line (Coder versus Claude tokens), from tools/qwen/summary.py. */
export interface CostLine {
  coder: { jobs: number; output: number; prompt: number; gpu_seconds: number; lines: number };
  claude: { answers: number; output: number; input: number; cache_read: number; cache_write: number };
  output_share_coder: number;
}
/** TEN-05: totals of the last 7 days. */
export interface WeeklyCosts { tasks: number; coderOutput: number; claudeOutput: number; coderShare: number }

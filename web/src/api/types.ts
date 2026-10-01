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
}

export interface ModelInfo {
  id: string; // "qwen3.5-9b"
  contextTokens: number;
  tokensPerSecond?: number;
  toolCalls: boolean;
  jsonSchema: boolean;
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
}

export interface Chat {
  id: string;
  title: string;
  projectId?: string; // undefined = loose chat
  updatedAt: string;
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
  online: boolean;
  cpu: number; // 0..1
  ramUsedGb: number;
  ramTotalGb: number;
  gpus: GpuStats[];
  kompanionShare: number; // share of CPU used by Kompanion workspaces, 0..1
  busy?: string; // "Steam is running"
  history: number[]; // recent total power draw in W, oldest first
}

export interface GpuStats {
  name: string;
  load: number; // 0..1
  vramUsedGb: number;
  vramTotalGb: number;
  watts: number;
  tempC: number;
  use: string; // "Qwen3.5-9B (OVMS)"
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
}

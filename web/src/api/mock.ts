// A fake server so the UI can be built and tried before the real one exists.
// Everything here is example data.
import type { AccessEvent, GrantView } from "../views/access";
import type { KompanionApi, ServerEvent } from "./client";
import type { PcAction } from "./client";
import type { TaskState, AdminSettings, DaySummary, MachineStats, Chat, Message, ModelProvider, Project, RoleAssignment, Server, Task } from "./types";

const grants: Record<string, GrantView[]> = {
  soucouyant: [{ target: "/home/kees/projects/kompanion", rights: ["read", "write"], grantedBy: "demo", grantedAt: "2026-10-01T10:00:00Z", expires: null }],
};
const actions: PcAction[] = [];
const history: AccessEvent[] = [];

const now = Date.now();
const ago = (min: number) => new Date(now - min * 60_000).toISOString();
const later = (min: number) => new Date(now + min * 60_000).toISOString();
let seq = 100;
const id = (p: string) => `${p}${++seq}`;

const projects: Project[] = [
  { id: "p-kompanion", name: "Kreative Kompanion", description: "This app", updatedAt: ago(2) },
  { id: "p-kk", name: "kk-engine", description: "C++17 Vulkan game engine", updatedAt: ago(14) },
  { id: "p-3dco", name: "3dco-plus", description: "3D controller overlay for streamers", updatedAt: ago(60 * 26) },
  { id: "p-nohboard", name: "nohboard-qt", description: "Keyboard overlay, Qt", updatedAt: ago(60 * 24 * 4) },
];

const chats: Chat[] = [
  { id: "c-kk", title: "Debug tooling and tests", projectId: "p-kk", updatedAt: ago(14) },
  { id: "c-kk-2", title: "Renderer performance", projectId: "p-kk", updatedAt: ago(60 * 5) },
  { id: "c-kompanion", title: "First UI draft", projectId: "p-kompanion", updatedAt: ago(2) },
  { id: "c-3dco", title: "SDL3 port", projectId: "p-3dco", updatedAt: ago(60 * 26) },
  { id: "c-nohboard", title: "Wayland input capture", projectId: "p-nohboard", updatedAt: ago(60 * 24 * 4) },
  { id: "c-loose", title: "Which local model next?", updatedAt: ago(60 * 3) },
];

const adminSettings: AdminSettings = {
  appName: "Kreative Kompanion", smtpHost: "", smtpPort: 587, smtpTls: "starttls", smtpUser: "", smtpFrom: "", smtpReplyTo: "",
  colorBrand: "#5c398e", colorLinkDark: "#f3941f", colorLinkLight: "#8f4700", colorAccent: "#f3941f",
};

const messages: Message[] = [
  {
    id: "m1", chatId: "c-kk", author: "user", at: ago(20),
    text: "I keep turning the Vulkan validation layers on and off by editing code. Can you make that a setting, and finally add tests for the module loader?",
  },
  {
    id: "m2", chatId: "c-kk", author: "orchestrator", at: ago(19),
    text:
      "I split this into four tasks. Qwen does the work in a container on **soucouyant**, DeepSeek reviews each result before anything touches your repo.\n\n" +
      "- Add a `--vk-validation` flag and an `engine.toml` setting\n- Write unit tests for `ModuleLoader`\n- Update the README build steps\n- Benchmark the renderer at 1440p, to check the flag costs nothing when off\n\n" +
      "The benchmark needs the GPU, so I asked you when it can run.",
    taskIds: ["t1", "t2", "t3", "t4"],
  },
  {
    id: "m3", chatId: "c-kk", author: "user", at: ago(15),
    text: "Good. And what about the SDL3 input port in 3dco-plus, can that run too?",
  },
  {
    id: "m4", chatId: "c-kk", author: "orchestrator", at: ago(14),
    text: "That one is in the 3dco-plus project. It is waiting on one choice from you: keep the raw joystick fallback, or rely on SDL3's gamepad mapping only. The card is in the task list.",
  },
  {
    id: "m5", chatId: "c-kompanion", author: "user", at: ago(3),
    text: "Start building the first draft, starting with the UI.",
  },
  {
    id: "m6", chatId: "c-kompanion", author: "orchestrator", at: ago(2),
    text: "This is that draft. Projects and chats on the left, me in the middle, running tasks on the right. Everything you see is example data from a fake server.",
  },
  {
    id: "m7", chatId: "c-loose", author: "user", at: ago(60 * 3),
    text: "Is there a local model that fits on the A770 and beats Qwen3.5-9B at tool calls?",
  },
  {
    id: "m8", chatId: "c-loose", author: "orchestrator", at: ago(60 * 3 - 1),
    text: "I can test that for you. Each candidate runs the worker test set (42 tasks from past failures) at night, and you get a pass-rate comparison in the morning.",
  },
];

const tasks: Task[] = [
  {
    id: "t1", projectId: "p-kk", title: "Add a --vk-validation flag and setting",
    state: "running", progress: 0.62, step: "Building in the container", role: "worker",
    model: "Qwen3.5-9B", runner: "soucouyant (Linux)", workspace: "Container · 4 cores · 8 GB",
    events: [
      { kind: "step", at: ago(18), text: "Read the code map: Instance.cpp, Config.cpp, main.cpp" },
      { kind: "tool", at: ago(17), tool: "files.write", detail: "src/core/Config.cpp", ok: true },
      { kind: "diff", at: ago(17), file: "src/core/Config.cpp", added: 14, removed: 2 },
      { kind: "diff", at: ago(16), file: "src/render/Instance.cpp", added: 9, removed: 5 },
      { kind: "tool", at: ago(15), tool: "shell", detail: "cmake --build build -j4", ok: false },
      {
        kind: "call", at: ago(15), call: {
          id: "call-1", role: "worker", model: "Qwen3.5-9B", provider: "OVMS on kireserver",
          reason: "The build failed, so the worker gets the compiler error and the two files it touched.",
          request: {
            system: "You are the worker. Change only the files listed. Answer with a JSON patch.",
            context: ["skill: worker/cpp-includes", "skill: worker/cmake", "file: src/render/Instance.cpp (lines 1-60)", "build log: 12 lines"],
            prompt: "Instance.cpp:41: error: 'string_view' is not a member of 'std'. Fix the build error.",
          },
          response: '{"file":"src/render/Instance.cpp","insert_after_line":3,"text":"#include <string_view>"}',
          tokensIn: 2140, tokensOut: 38, ms: 1900, costEur: 0, energyWh: 0.09,
        },
      },
      { kind: "step", at: ago(15), text: "Fixed missing include <string_view>, rebuilding" },
    ],
  },
  {
    id: "t2", projectId: "p-kk", title: "Write unit tests for ModuleLoader",
    state: "in_review", progress: 0.9, step: "DeepSeek is reviewing", role: "reviewer",
    model: "DeepSeek", runner: "soucouyant (Linux)", workspace: "Container · 2 cores · 4 GB",
    events: [
      { kind: "diff", at: ago(12), file: "tests/ModuleLoaderTests.cpp", added: 118, removed: 0 },
      { kind: "tool", at: ago(10), tool: "shell", detail: "ctest --output-on-failure  (11/11 passed)", ok: true },
      { kind: "step", at: ago(9), text: "Cheap checks passed, sent to the reviewer" },
    ],
  },
  {
    id: "t3", projectId: "p-kk", title: "Update the README build steps",
    state: "done", progress: 1, step: "Merged into branch kompanion/vk-validation", role: "worker",
    model: "Qwen3.5-9B", runner: "soucouyant (Linux)",
    events: [
      { kind: "diff", at: ago(13), file: "README.md", added: 6, removed: 3 },
      {
        kind: "call", at: ago(12), call: {
          id: "call-2", role: "reviewer", model: "deepseek-chat", provider: "DeepSeek",
          reason: "Cheap checks passed (Markdown lint), so the README change goes to the reviewer.",
          request: {
            system: "You are the reviewer. Check the change against the repository. Reply pass or fail with one reason.",
            context: ["skill: reviewer/rubric", "diff: README.md (+6 -3)", "file: CMakeLists.txt (options only)"],
            prompt: "Review this README change.",
          },
          response: "fail: README mentions -DKK_ENABLE_VALIDATION=ON, but CMakeLists.txt defines no such option.",
          tokensIn: 3310, tokensOut: 41, ms: 2600, costEur: 0.001,
        },
      },
      { kind: "review", at: ago(12), model: "DeepSeek", verdict: "fail", note: "Invented a CMake option KK_ENABLE_VALIDATION that does not exist." },
      { kind: "lesson", at: ago(12), skill: "worker/cmake", note: "Only document options found in CMakeLists.txt; list them with cmake -LH first." },
      { kind: "diff", at: ago(11), file: "README.md", added: 5, removed: 3 },
      { kind: "review", at: ago(10), model: "DeepSeek", verdict: "pass", note: "Matches CMakeLists.txt." },
    ],
  },
  {
    id: "t4", projectId: "p-kk", title: "Benchmark the renderer at 1440p",
    state: "waiting_resources", progress: 0, step: "Needs the GPU", role: "worker",
    model: "Qwen3.5-9B", runner: "soucouyant (Linux)", workspace: "Container · 8 cores · 16 GB · GPU",
    events: [{ kind: "step", at: ago(19), text: "Steam is running, so I asked before taking the GPU" }],
    question: {
      kind: "resources",
      text: "This needs 8 cores, 16 GB and the GPU on soucouyant for about 40 minutes.",
      options: [
        { id: "now", label: "Now" },
        { id: "tonight", label: "Tonight", detail: "01:00", recommended: true },
        { id: "less", label: "With less", detail: "4 cores, no GPU" },
      ],
    },
  },
  {
    id: "t5", projectId: "p-3dco", title: "Port the input layer to SDL3",
    state: "needs_input", progress: 0.3, step: "Waiting for your choice", role: "orchestrator",
    model: "DeepSeek", runner: "soucouyant (Linux)",
    events: [
      { kind: "step", at: ago(60 * 26), text: "Mapped every SDL2 input call (37 call sites)" },
      { kind: "screenshot", at: ago(60 * 26), caption: "Overlay running on SDL3 in the headless workspace" },
    ],
    question: {
      kind: "choice",
      text: "Keep the raw joystick fallback, or use only SDL3's gamepad mapping?",
      options: [
        { id: "keep", label: "Keep fallback", detail: "Odd controllers keep working", recommended: true },
        { id: "drop", label: "SDL3 only", detail: "Less code to maintain" },
      ],
    },
  },
  {
    id: "t7", projectId: "p-nohboard", title: "Install Qt 6.8 build dependencies",
    state: "needs_input", progress: 0.1, step: "Waiting for your approval", role: "worker",
    model: "Qwen3.5-9B", runner: "soucouyant (Linux)",
    events: [{ kind: "step", at: ago(40), text: "CMake needs Qt6::WaylandClient, which is not installed" }],
    question: {
      kind: "approval",
      text: "Run this command on your own system (not in a container)?",
      action: {
        machine: "soucouyant (Linux)", cwd: "~/development/nohboard-qt",
        command: "sudo pacman -S --needed qt6-wayland qt6-tools",
        network: ["mirror.cachyos.org"],
      },
      options: [
        { id: "once", label: "Allow once", recommended: true },
        { id: "always", label: "Always allow", detail: "pacman -S qt6-*" },
        { id: "reject", label: "Deny" },
      ],
    },
  },
  {
    id: "t6", projectId: "p-kompanion", title: "Scaffold the Rust server",
    state: "queued", progress: 0, step: "Starts after the UI draft", role: "worker",
    model: "Qwen3.5-9B", runner: "kireserver", scheduledFor: later(60), events: [],
  },
];

const providers: ModelProvider[] = [
  {
    id: "ovms", name: "OVMS on kireserver", kind: "openai-compatible", baseUrl: "https://ai.example.lan/v3", local: true,
    models: [{ id: "qwen3.5-9b", contextTokens: 32768, tokensPerSecond: 38, toolCalls: true, jsonSchema: true }],
  },
  {
    id: "deepseek", name: "DeepSeek", kind: "openai-compatible", baseUrl: "https://api.deepseek.com", local: false,
    models: [{ id: "deepseek-chat", contextTokens: 131072, toolCalls: true, jsonSchema: true }],
  },
  {
    id: "anthropic", name: "Claude (Anthropic API)", kind: "anthropic", baseUrl: "https://api.anthropic.com", local: false,
    models: [{ id: "claude-sonnet-5-5", contextTokens: 200000, toolCalls: true, jsonSchema: true }],
  },
];

let roles: RoleAssignment[] = [
  { role: "orchestrator", providerId: "deepseek", modelId: "deepseek-chat" },
  { role: "worker", providerId: "ovms", modelId: "qwen3.5-9b" },
  { role: "reviewer", providerId: "deepseek", modelId: "deepseek-chat" },
];

const machines: MachineStats[] = [
  {
    id: "kireserver", name: "kireserver", os: "Ubuntu", online: true, cpu: 0.34, ramUsedGb: 41, ramTotalGb: 64,
    kompanionShare: 0.12, history: [92, 95, 140, 151, 148, 150, 97, 94, 149, 152, 150, 149],
    gpus: [{ name: "Arc A770", load: 0.71, vramUsedGb: 11.2, vramTotalGb: 16, watts: 162, tempC: 67, use: "Qwen3.5-9B (OVMS)" }],
  },
  {
    id: "soucouyant", name: "soucouyant", os: "CachyOS", online: true, cpu: 0.58, ramUsedGb: 19, ramTotalGb: 32,
    kompanionShare: 0.31, busy: "Steam is running", history: [180, 190, 260, 255, 270, 262, 258, 265, 270, 268, 266, 271],
    gpus: [{ name: "GPU", load: 0.64, vramUsedGb: 6.1, vramTotalGb: 8, watts: 141, tempC: 72, use: "Game (not Kompanion)" }],
  },
  {
    id: "macbook", name: "MacBook", os: "macOS", online: false, cpu: 0, ramUsedGb: 0, ramTotalGb: 16,
    kompanionShare: 0, history: [], gpus: [],
  },
];

const servers: Server[] = [
  { url: "https://kompanion.kireserver.lan", name: "kireserver", version: "0.1.0", discovered: true },
];

const replies = [
  "Here is how I would split that up. I will check each piece with the cheap checks first, then the reviewer.\n\n- Read the relevant files through the code map\n- Make the change in a container workspace\n- Run the build and tests\n\nI added it to the task list on the right.",
  "Got it. I made that a task for Qwen with a small, self-contained scope. If it fails review I will write the lesson into the matching skill so the next attempt starts smarter.",
];

export class MockApi implements KompanionApi {
  private listeners = new Set<(ev: ServerEvent) => void>();
  private replyIndex = 0;

  constructor() {
    setInterval(() => this.tick(), 1200);
  }

  private emit(ev: ServerEvent) {
    this.listeners.forEach((l) => l(ev));
  }

  private tick() {
    for (const m of machines) {
      if (!m.online) continue;
      const jitter = () => (Math.random() - 0.5) * 0.08;
      m.cpu = clamp(m.cpu + jitter());
      for (const g of m.gpus) {
        g.load = clamp((g.load ?? 0) + jitter());
        g.watts = Math.round(40 + g.load * 175);
      }
      m.history = [...m.history.slice(-23), Math.round(m.gpus.reduce((w, g) => w + (g.watts ?? 0), 0) + 60 * m.cpu)];
    }
    this.emit({ type: "machines", machines: structuredClone(machines) });
    for (const t of tasks) {
      if (t.state !== "running") continue;
      t.progress = Math.min(1, t.progress + 0.03);
      if (t.progress >= 1) {
        t.state = "in_review";
        t.step = "DeepSeek is reviewing";
        t.events.push({ kind: "step", at: new Date().toISOString(), text: "Cheap checks passed, sent to the reviewer" });
      } else if (t.progress > 0.8 && t.step !== "Running the tests") {
        t.step = "Running the tests";
        t.events.push({ kind: "tool", at: new Date().toISOString(), tool: "shell", detail: "ctest --output-on-failure", ok: true });
      }
      this.emit({ type: "task", task: structuredClone(t) });
    }
  }

  async discover() {
    await wait(500);
    return servers;
  }
  async connect(url: string) {
    await wait(400);
    const clean = url.trim().replace(/\/+$/, "");
    if (!/^https?:\/\/[^\s]+$/.test(clean)) throw new Error("That doesn't look like a link. It should start with https://");
    return { url: clean, name: new URL(clean).hostname, version: "0.1.0" };
  }
  async status() {
    return { name: "Kreative Kompanion (demo)", version: "0.1.0", setupNeeded: false, user: "Kees", admin: true, theme: "system" as const };
  }
  async setup() {}
  async logout() { return null; }
  async setTheme() {}
  async setMachinesRefresh() {}
  async setGpuPins() {}
  async getNotifications() { return { email: "", onNeedsInput: true, onFailed: true, onDone: false, dailySummary: false }; }
  async setNotifications() {}
  async watchMachines() {}
  async pairMachine(name: string) { return { id: "demo", name, token: "demo-token" }; }
  async pairCode() { return { code: "DEMO-C0DE", expiresAt: new Date(Date.now() + 900_000).toISOString() }; }
  async unpairMachine() {}
  async listGrants(machineId: string) { return structuredClone(grants[machineId] ?? []); }
  async addGrant(machineId: string, target: string, rights: string[], expiresHours?: number) {
    setTimeout(() => {
      const list = (grants[machineId] ??= []).filter((g) => g.target !== target);
      list.push({ target, rights, grantedBy: "demo", grantedAt: new Date().toISOString(),
        expires: expiresHours ? new Date(Date.now() + expiresHours * 3_600_000).toISOString() : null });
      grants[machineId] = list;
      history.unshift({ at: new Date().toISOString(), kind: "granted", target, detail: rights.join(", "), machine: machineId });
      this.emit({ type: "changed", what: "access", machineId });
    }, 400);
  }
  async revokeGrant(machineId: string, target: string) {
    setTimeout(() => {
      grants[machineId] = (grants[machineId] ?? []).filter((g) => g.target !== target);
      history.unshift({ at: new Date().toISOString(), kind: "revoked", target, detail: null, machine: machineId });
      this.emit({ type: "changed", what: "access", machineId });
    }, 400);
  }
  async accessHistory() { return structuredClone(history); }
  async getAdmin() { return { settings: structuredClone(adminSettings), smtpPasswordSet: false }; }
  async saveAdmin(s: AdminSettings) { Object.assign(adminSettings, s); return structuredClone(adminSettings); }
  async uploadLogo() {}
  async removeLogo() {}
  async testMail() { throw new Error("The demo can't send mail."); }
  async login() {}
  async listProjects() { return structuredClone(projects); }
  async listChats() {
    return structuredClone([...chats].sort((a, b) => Number(!!b.pinned) - Number(!!a.pinned)));
  }
  async listMessages(chatId: string) { return structuredClone(messages.filter((m) => m.chatId === chatId)); }
  async listTasks(projectId?: string) {
    return structuredClone(projectId ? tasks.filter((t) => t.projectId === projectId) : tasks);
  }
  async listProviders() { return structuredClone(providers); }
  async listMachines() { return structuredClone(machines); }
  async today(): Promise<DaySummary> {
    return { tasks: 41, localTokens: 2_140_000, cloudTokens: 182_000, cloudCostEur: 0.09, energyKwh: 1.4 };
  }
  async listRoles() { return structuredClone(roles); }
  async setRole(a: RoleAssignment) {
    roles = roles.map((r) => (r.role === a.role ? a : r));
  }

  async createChat(title: string, projectId?: string) {
    const chat: Chat = { id: id("c"), title, projectId, updatedAt: new Date().toISOString() };
    chats.unshift(chat);
    return structuredClone(chat);
  }

  async updateChat(chatId: string, change: { title?: string; pinned?: boolean; archived?: boolean; projectId?: string }) {
    const i = chats.findIndex((c) => c.id === chatId);
    if (i < 0) return;
    if (change.archived) chats.splice(i, 1);
    else Object.assign(chats[i], { title: change.title ?? chats[i].title, pinned: change.pinned ?? chats[i].pinned });
  }

  async createTask(t: { projectId: string; title: string; description: string; state?: TaskState }) {
    const task: Task = { id: id("t"), projectId: t.projectId, title: t.title, description: t.description, state: t.state ?? "queued",
      progress: 0, step: "", role: "worker", model: "", events: [] };
    tasks.unshift(task);
    return structuredClone(task);
  }
  async updateTask(taskId: string, change: { title?: string; description?: string; state?: TaskState }) {
    const t = tasks.find((x) => x.id === taskId);
    if (!t) throw new Error("No such task.");
    Object.assign(t, Object.fromEntries(Object.entries(change).filter(([, v]) => v !== undefined)));
    return structuredClone(t);
  }
  async deleteTask(taskId: string) {
    const i = tasks.findIndex((x) => x.id === taskId);
    if (i >= 0) tasks.splice(i, 1);
  }
  async reorderTasks(_projectId: string, ids: string[]) {
    ids.forEach((tid, i) => { const t = tasks.find((x) => x.id === tid); if (t) t.position = i; });
  }
  async makeProjectInternal() {}
  async deleteChat(chatId: string) {
    const i = chats.findIndex((c) => c.id === chatId);
    if (i >= 0) chats.splice(i, 1);
  }

  async getCapabilities() {
    return {
      models: [
        { id: "ovms", name: "OVMS on kireserver", local: true, status: "ok" as const, error: null, models: ["Coder", "Whisper"],
          roles: ["orchestrator: Coder", "reviewer: Coder", "worker: Coder"], lastError: null },
        { id: "ollama-soucouyant", name: "Ollama on soucouyant", local: true, status: "down" as const, error: "connection refused",
          models: [], roles: [], lastError: { text: "connection refused", at: ago(42) } },
      ],
      // Live from the demo's own machines and grants, so the page follows changes made elsewhere.
      computers: machines.filter((m) => m.id !== "kireserver").map((m) => ({
        id: m.id, name: m.name, online: m.online, lastSeen: new Date().toISOString(),
        grants: (grants[m.id] ?? []).map((g) => ({ target: g.target, rights: [...g.rights], expires: g.expires })),
      })),
      tools: [
        { name: "read_file", description: "Read a text file (up to 256 KiB).", needs: "read on the folder" },
        { name: "edit_file", description: "Replace one exact piece of a file and show the diff.", needs: "write on the folder" },
        { name: "shell", description: "Run a command in a folder.", needs: "shell in the folder" },
        { name: "capabilities", description: "What Kompanion can use right now.", needs: "nothing" },
      ],
      mcp: [],
      indexes: [{ id: "assets", name: "Asset search by meaning", items: 41_230, of: 47_012, failed: 12, model: "Embedder (ovms-cpu)", status: "partly" as const }],
      skills: [
        { id: "orchestrator", title: "Orchestrator", lessons: 9, updated: ago(30) },
        { id: "worker/rust", title: "Worker: Rust", lessons: 37, updated: ago(5) },
        { id: "worker/web", title: "Worker: web app (vanilla TypeScript)", lessons: 25, updated: ago(90) },
      ],
    };
  }
  async voiceInfo() {
    return { enabled: true, voices: [{ id: "af_heart", label: "English (US), Heart" }, { id: "ef_dora", label: "Spanish, Dora" }] };
  }
  async transcribe(audio: Blob) {
    await new Promise((r) => setTimeout(r, 300));
    return audio.size >= 0 ? "install htop" : "";
  }
  /** 0.4 s of silence per sentence, so the demo "reads" without sound. */
  async speak() {
    await new Promise((r) => setTimeout(r, 100));
    const rate = 8000, n = rate * 0.4, b = new DataView(new ArrayBuffer(44 + n * 2));
    const put = (o: number, s: string) => { for (let i = 0; i < s.length; i++) b.setUint8(o + i, s.charCodeAt(i)); };
    put(0, "RIFF"); b.setUint32(4, 36 + n * 2, true); put(8, "WAVEfmt "); b.setUint32(16, 16, true); b.setUint16(20, 1, true);
    b.setUint16(22, 1, true); b.setUint32(24, rate, true); b.setUint32(28, rate * 2, true); b.setUint16(32, 2, true);
    b.setUint16(34, 16, true); put(36, "data"); b.setUint32(40, n * 2, true);
    return new Blob([b.buffer], { type: "audio/wav" });
  }
  async getSkill(id: string) {
    return { id, text: `# ${id}\n\nLessons for this role, newest last.\n\n1. (2026-10-04) Plan steps are changes, each with a **done when**.\n2. Never \`test.skip\` inside a test.\n` };
  }

  async listActions(chatId: string) { return structuredClone(actions.filter((a) => (a as PcAction & { chatId?: string }).chatId === chatId)); }
  async listActivity() {
    const now = Date.now();
    return [
      { at: new Date(now - 120_000).toISOString(), kind: "step", state: "done", text: "Edit hyprland.lua", machineId: "soucouyant", machine: "soucouyant",
        chatId: "c1", chat: "Hyprland gaps", id: "a1", tool: { tool: "edit_file", path: "/home/kees/.config/hypr/hyprland.lua" },
        result: "@@ line 12 @@\n-gaps_out = 8\n+gaps_out = 12\n", grant: "allowed once · 10 min · removed after" },
      { at: new Date(now - 300_000).toISOString(), kind: "step", state: "refused", text: "Read /etc/shadow", machineId: "soucouyant", machine: "soucouyant",
        chatId: "c1", chat: "Hyprland gaps", id: "a2", tool: { tool: "read_file", path: "/etc/shadow" }, result: "not granted: read on /etc/shadow", grant: null },
      { at: new Date(now - 600_000).toISOString(), kind: "grant", text: "system", target: "system", detail: "always allow, 24 h", machineId: "soucouyant", machine: "soucouyant" },
    ];
  }
  async startTask() {}
  async decideAction(id: string, decision: "approve" | "always" | "deny") {
    const a = actions.find((x) => x.id === id);
    if (!a || a.state !== "pending") throw new Error("This step was already decided.");
    const step = (state: PcAction["state"], result: string | null, ms: number) =>
      setTimeout(() => { a.state = state; a.result = result; this.emit({ type: "changed", what: "actions" }); }, ms);
    if (decision === "deny") { step("denied", null, 0); return; }
    step("granting", null, 0);
    if (decision === "always") {
      (grants[a.machineId] ??= []).push({ target: "system", rights: ["packages", "root"], grantedBy: "demo",
        grantedAt: new Date().toISOString(), expires: new Date(Date.now() + 86_400_000).toISOString() });
      this.emit({ type: "changed", what: "access", machineId: a.machineId });
    }
    step("running", null, 300);
    step("done", "resolving dependencies...\ninstalling htop...\nexit: 0", 600);
  }
  async send(chatId: string, text: string, machineId?: string) {
    if (machineId) {
      const a = { id: id("a"), chatId, machineId, summary: "install htop with paru", tool: { tool: "package", manager: "paru", action: "install", names: ["htop"] }, needs: "packages + root (asks for the password on the PC)",
        state: "pending", result: null, createdAt: new Date().toISOString() } as PcAction & { chatId: string };
      actions.push(a);
      setTimeout(() => this.emit({ type: "changed", what: "actions" }), 100);
    }
    const user: Message = { id: id("m"), chatId, author: "user", text, at: new Date().toISOString() };
    messages.push(user);
    this.emit({ type: "message", message: structuredClone(user) });

    const chat = chats.find((c) => c.id === chatId);
    const reply: Message = { id: id("m"), chatId, author: "orchestrator", text: "", at: new Date().toISOString(), streaming: true };
    messages.push(reply);
    await wait(500);
    this.emit({ type: "message", message: structuredClone(reply) });

    const full = replies[this.replyIndex++ % replies.length];
    for (const word of full.split(/(?<=\s)/)) {
      await wait(35);
      reply.text += word;
      this.emit({ type: "message-delta", messageId: reply.id, chatId, text: word, done: false });
    }
    reply.streaming = false;
    this.emit({ type: "message-delta", messageId: reply.id, chatId, text: "", done: true });

    if (chat?.projectId) {
      const task: Task = {
        id: id("t"), projectId: chat.projectId, title: text.length > 60 ? text.slice(0, 57) + "…" : text,
        state: "running", progress: 0.05, step: "Reading the code map", role: "worker", model: "Qwen3.5-9B",
        runner: "soucouyant (Linux)", workspace: "Container · 4 cores · 8 GB",
        events: [{ kind: "step", at: new Date().toISOString(), text: "Planned by the orchestrator" }],
      };
      tasks.unshift(task);
      reply.taskIds = [task.id];
      this.emit({ type: "task", task: structuredClone(task) });
    }
  }

  async answer(taskId: string, optionId: string) {
    const t = tasks.find((x) => x.id === taskId);
    if (!t || !t.question) return;
    const option = t.question.options.find((o) => o.id === optionId);
    t.events.push({ kind: "step", at: new Date().toISOString(), text: `You chose: ${option?.label ?? optionId}` });
    t.question = undefined;
    if (optionId === "reject") {
      t.state = "failed";
      t.step = "You denied the command";
    } else if (optionId === "tonight") {
      t.state = "queued";
      t.step = "Scheduled for 01:00";
      t.scheduledFor = nextAt(1);
    } else {
      t.state = "running";
      t.step = "Starting";
    }
    this.emit({ type: "task", task: structuredClone(t) });
  }

  onEvent(listener: (ev: ServerEvent) => void) {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }
}

function clamp(n: number) {
  return Math.min(0.98, Math.max(0.02, n));
}

function wait(ms: number) {
  return new Promise((r) => setTimeout(r, ms));
}

function nextAt(hour: number): string {
  const d = new Date();
  d.setHours(hour, 0, 0, 0);
  if (d.getTime() < Date.now()) d.setDate(d.getDate() + 1);
  return d.toISOString();
}

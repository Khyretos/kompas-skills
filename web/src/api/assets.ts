// Assets section API: the game asset library index on the server (/api/assets...).
// `HttpAssets` talks to the server; `MockAssets` is example data for demo mode and the
// browser tests. Scan progress arrives live as "assets" server events.
import type { KompanionApi } from "./client";

export interface AssetItem {
  id: number;
  packId: number;
  pack: string;
  container: string; // the zip or Unity package it sits in, "" when loose
  path: string;
  name: string;
  ext: string;
  size: number;
  category: string;
  meta: boolean; // licence, readme, credits
  dupOf: number | null;
}

export interface AssetDetail extends AssetItem {
  packKind: string;
  mtime: number | null;
  rule: string; // why it got its category
  missingSince: string | null;
  copies: { id: number; container: string; path: string }[];
  packDocs: { id: number; path: string }[];
}

export interface ScanProgress {
  running: boolean;
  phase: "" | "walking" | "packs" | "saving" | "done" | "failed";
  done: number;
  total: number;
  current: string;
  error: string | null;
}

export interface AssetStatus {
  configured: boolean;
  mounted: boolean;
  assets: number;
  bytes: number;
  packs: number;
  scan: ScanProgress;
  lastScan: null | {
    startedAt: string; finishedAt: string; files: number; entries: number; unity: number;
    packsRead: number; errors: string[]; tookMs: number;
  };
  categories: string[];
}

export interface AssetFilter {
  q?: string;
  category?: string;
  pack?: number;
  dups?: boolean;
}

export interface PackFacet {
  id: number; name: string; kind: string; size: number; duplicateOf: number | null; error: string | null;
  files: number; bytes: number;
}

export interface AssetFacets {
  categories: { name: string; files: number; bytes: number }[];
  packs: PackFacet[];
}

export interface AssetsApi {
  status(): Promise<AssetStatus>;
  list(f: AssetFilter, offset: number, limit: number): Promise<{ total: number; offset: number; items: AssetItem[] }>;
  facets(f: AssetFilter): Promise<AssetFacets>;
  detail(id: number): Promise<AssetDetail>;
  scan(): Promise<void>;
  /** Scan progress, live; `null` after the live stream missed events (reload everything). */
  onScan(listener: (p: ScanProgress | null) => void): () => void;
}

function query(f: AssetFilter, extra: Record<string, number> = {}): string {
  const p = new URLSearchParams();
  if (f.q?.trim()) p.set("q", f.q.trim());
  if (f.category) p.set("category", f.category);
  if (f.pack !== undefined) p.set("pack", String(f.pack));
  if (f.dups) p.set("dups", "true");
  for (const [k, v] of Object.entries(extra)) p.set(k, String(v));
  const s = p.toString();
  return s ? `?${s}` : "";
}

export class HttpAssets implements AssetsApi {
  constructor(private events: KompanionApi) {}

  private async get<T>(path: string): Promise<T> {
    const res = await fetch(`/api${path}`, { credentials: "same-origin" });
    if (!res.ok) throw new Error((await res.json().catch(() => ({}))).error ?? `The server answered ${res.status}.`);
    return res.json() as Promise<T>;
  }

  status() { return this.get<AssetStatus>("/assets/status"); }
  list(f: AssetFilter, offset: number, limit: number) {
    return this.get<{ total: number; offset: number; items: AssetItem[] }>(`/assets${query(f, { offset, limit })}`);
  }
  facets(f: AssetFilter) { return this.get<AssetFacets>(`/assets/facets${query(f)}`); }
  detail(id: number) { return this.get<AssetDetail>(`/assets/${id}`); }
  async scan() {
    const res = await fetch("/api/assets/scan", { method: "POST", credentials: "same-origin", headers: { "X-Kompanion": "1" } });
    if (!res.ok) throw new Error((await res.json().catch(() => ({}))).error ?? `The server answered ${res.status}.`);
  }
  onScan(listener: (p: ScanProgress | null) => void) {
    return this.events.onEvent((ev) => {
      if (ev.type === "assets") listener(ev.scan as ScanProgress);
      else if (ev.type === "resync") listener(null);
    });
  }
}

// ---- Demo data: names only, made up in the style of the real library ----

const PACKS = [
  ["Fantasy RPG Music Pack", "music", ["Tracks/mp3/Action", "Tracks/mp3/Town", "Tracks/wav/Calm"], "mp3"],
  ["POLYGON_Nature_Source_Files_v2", "3d-model", ["Models/SM_Env_Tree", "Models/SM_Env_Rock", "Models/SM_Prop_Log"], "fbx"],
  ["POLYGON_Nature_Source_Files_v2", "texture", ["Textures/PolygonNature_Texture"], "png"],
  ["NOX Sound Essentials", "sound-effect", ["SFX/Footsteps/Grass_Step", "SFX/UI/Click", "SFX/Combat/Sword_Hit"], "wav"],
  ["NOX Sound Essentials", "voice", ["Voices_Essentials/Voice_Female/Voice_Female"], "wav"],
  ["Horror SFX", "ambience", ["Ambient/Rooms/Room_Tone", "Ambient/Wind/Wind_Loop"], "ogg"],
  ["INTERFACE_SciFi_Soldier_HUD", "sprite", ["UI/Icons/ICON_Ammo", "UI/Icons/ICON_Health"], "png"],
  ["ANIMATION_Goblin_Locomotion", "animation", ["Animations/A_Walk", "Animations/A_Run", "Animations/A_Idle"], "fbx"],
  ["fonts", "font", ["Fonts/Display"], "ttf"],
] as const;

function demoItems(): AssetItem[] {
  const items: AssetItem[] = [];
  let id = 0;
  const packIds = new Map<string, number>();
  for (const [pack, category, stems, ext] of PACKS) {
    if (!packIds.has(pack)) packIds.set(pack, packIds.size + 1);
    for (const stem of stems) {
      for (let n = 1; n <= 40; n++) {
        const path = `${stem}_${String(n).padStart(2, "0")}.${ext}`;
        items.push({
          id: ++id, packId: packIds.get(pack)!, pack, container: `${pack}.zip`, path,
          name: path.split("/").pop()!, ext, size: 2048 + ((id * 7919) % 900_000), category, meta: false, dupOf: null,
        });
      }
    }
  }
  items.push({ id: ++id, packId: 1, pack: PACKS[0][0], container: `${PACKS[0][0]}.zip`, path: "LICENSE.pdf", name: "LICENSE.pdf", ext: "pdf", size: 52_000, category: "doc", meta: true, dupOf: null });
  return items.sort((a, b) => a.pack.localeCompare(b.pack) || a.packId - b.packId || a.path.localeCompare(b.path));
}

export class MockAssets implements AssetsApi {
  private items = demoItems();
  private listeners = new Set<(p: ScanProgress | null) => void>();
  private progress: ScanProgress = { running: false, phase: "done", done: 0, total: 0, current: "", error: null };
  private finishedAt = new Date(Date.now() - 3_600_000).toISOString();

  private filtered(f: AssetFilter, skip = ""): AssetItem[] {
    const words = (f.q ?? "").toLowerCase().split(/[^\p{L}\p{N}]+/u).filter(Boolean);
    const cats = (f.category ?? "").split(",").filter(Boolean);
    return this.items.filter((a) =>
      (skip === "category" || !cats.length || cats.includes(a.category)) &&
      (cats.includes("junk") || a.category !== "junk") &&
      (f.dups || a.dupOf === null) &&
      (skip === "pack" || f.pack === undefined || a.packId === f.pack) &&
      words.every((w) => `${a.name} ${a.container}/${a.path} ${a.pack} ${a.category}`.toLowerCase().split(/[^\p{L}\p{N}]+/u).some((t) => t.startsWith(w))));
  }

  async status(): Promise<AssetStatus> {
    return {
      configured: true, mounted: true, assets: this.items.length, bytes: this.items.reduce((n, a) => n + a.size, 0),
      packs: new Set(this.items.map((a) => a.packId)).size, scan: { ...this.progress },
      lastScan: { startedAt: this.finishedAt, finishedAt: this.finishedAt, files: 12, entries: this.items.length, unity: 0, packsRead: 8, errors: [], tookMs: 2100 },
      categories: ["sound-effect", "music", "ambience", "voice", "3d-model", "animation", "texture", "material", "sprite", "vfx", "image", "shader", "font", "video", "print-model", "engine-file", "archive", "doc", "other", "junk"],
    };
  }

  async list(f: AssetFilter, offset: number, limit: number) {
    const all = this.filtered(f);
    await new Promise((r) => setTimeout(r, 30));
    return { total: all.length, offset, items: all.slice(offset, offset + limit) };
  }

  async facets(f: AssetFilter): Promise<AssetFacets> {
    const cats = new Map<string, { files: number; bytes: number }>();
    for (const a of this.filtered(f, "category")) {
      const c = cats.get(a.category) ?? { files: 0, bytes: 0 };
      cats.set(a.category, { files: c.files + 1, bytes: c.bytes + a.size });
    }
    const packs = new Map<number, PackFacet>();
    for (const a of this.filtered(f, "pack")) {
      const p = packs.get(a.packId) ?? { id: a.packId, name: a.pack, kind: "zip", size: 0, duplicateOf: null, error: null, files: 0, bytes: 0 };
      packs.set(a.packId, { ...p, files: p.files + 1, bytes: p.bytes + a.size });
    }
    return {
      categories: [...cats].map(([name, c]) => ({ name, ...c })),
      packs: [...packs.values()].sort((a, b) => a.name.localeCompare(b.name)),
    };
  }

  async detail(id: number): Promise<AssetDetail> {
    const a = this.items.find((x) => x.id === id);
    if (!a) throw new Error("Not found.");
    const docs = this.items.filter((x) => x.packId === a.packId && x.meta).map((x) => ({ id: x.id, path: x.path }));
    return { ...a, packKind: "zip", mtime: null, rule: `path word in "${a.pack}"`, missingSince: null, copies: [], packDocs: docs };
  }

  async scan() {
    if (this.progress.running) return;
    const total = 8;
    const step = (done: number) => {
      if (done === total) {
        // A new pack arrived while scanning.
        const id = Math.max(...this.items.map((x) => x.id)) + 1;
        this.items = [...this.items, { id, packId: 99, pack: "Free Ambience Loops", container: "free-ambience-loops.zip", path: "city-night-loop.wav", name: "city-night-loop.wav", ext: "wav", size: 3_400_000, category: "ambience", meta: false, dupOf: null }]
          .sort((a, b) => a.pack.localeCompare(b.pack) || a.packId - b.packId || a.path.localeCompare(b.path));
        this.finishedAt = new Date().toISOString();
      }
      this.progress = done < total
        ? { running: true, phase: "packs", done, total, current: PACKS[done % PACKS.length][0], error: null }
        : { running: false, phase: "done", done: total, total, current: "", error: null };
      this.listeners.forEach((l) => l({ ...this.progress }));
      if (done < total) setTimeout(() => step(done + 1), 120);
    };
    setTimeout(() => step(0), 50);
  }

  onScan(listener: (p: ScanProgress | null) => void) {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }
}

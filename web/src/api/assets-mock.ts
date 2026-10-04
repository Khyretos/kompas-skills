// Example data for demo mode and the browser tests (drafted by the local Coder model, reviewed).
import type { AssetItem, AssetDetail, AssetStatus, AssetFilter, PackFacet, AssetFacets, AssetsApi, ScanProgress, PreviewProgress, AssetsLive } from "./assets";

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

function previewFields(id: number, category: string): Pick<AssetItem, "preview" | "pv" | "duration" | "peaks" | "width" | "height"> {
  if (category === "texture" || category === "sprite") {
    // Every third picture gets its preview later (wantPreviews), so tests can watch one arrive.
    const later = id % 3 === 0;
    return { preview: later ? null : "image", pv: later ? 0 : 1, duration: null, peaks: null, width: category === "texture" ? 1024 : 128, height: category === "texture" ? 1024 : 128 };
  } else if (category === "music") {
    return { preview: "audio", pv: 1, duration: 90 + (id * 37) % 120, peaks: generatePeaks(id), width: null, height: null };
  } else if (category === "sound-effect" || category === "ambience" || category === "voice") {
    return { preview: "audio", pv: 1, duration: 0.4 + ((id * 13) % 35) / 10, peaks: generatePeaks(id), width: null, height: null };
  }
  return { preview: null, pv: 0, duration: null, peaks: null, width: null, height: null };
}

function demoItems(): AssetItem[] {
  const items: AssetItem[] = [];
  let id = 0;
  const packIds = new Map<string, number>();
  for (const [pack, category, stems, ext] of PACKS) {
    if (!packIds.has(pack)) packIds.set(pack, packIds.size + 1);
    for (const stem of stems) {
      for (let n = 1; n <= 40; n++) {
        const path = `${stem}_${String(n).padStart(2, "0")}.${ext}`;
        const fields = previewFields(id, category);
        items.push({
          id: ++id, packId: packIds.get(pack)!, pack, container: `${pack}.zip`, path,
          name: path.split("/").pop()!, ext, size: 2048 + ((id * 7919) % 900_000), category, meta: false, dupOf: null,
          ...fields,
        });
      }
    }
  }
  const licenseId = ++id;
  const licenseFields = previewFields(licenseId, "doc");
  items.push({ id: licenseId, packId: 1, pack: PACKS[0][0], container: `${PACKS[0][0]}.zip`, path: "LICENSE.pdf", name: "LICENSE.pdf", ext: "pdf", size: 52_000, category: "doc", meta: true, dupOf: null, ...licenseFields });
  
  // Fill in preview, pv, duration, peaks, width, height based on requirements
  for (const item of items) {
    const isTextureOrSprite = item.category === "texture" || item.category === "sprite";
    
    if (isTextureOrSprite) {
      const isLateArrival = item.id % 3 === 0;
      item.preview = isLateArrival ? null : "image";
      item.pv = isLateArrival ? 0 : 1;
      item.width = isLateArrival ? null : (item.category === "texture" ? 1024 : 128);
      item.height = isLateArrival ? null : (item.category === "texture" ? 1024 : 128);
      item.duration = null;
      item.peaks = null;
    } else {
      // Audio, Voice, Ambience, Music, etc.
      item.preview = null;
      item.pv = 0;
      item.duration = null;
      item.peaks = null;
      item.width = null;
      item.height = null;

      if (item.category === "music") {
        item.preview = "audio";
        item.pv = 1;
        item.duration = 90 + (item.id * 37) % 120;
        item.peaks = generatePeaks(item.id);
      } else if (item.category === "sound-effect" || item.category === "ambience" || item.category === "voice") {
        item.preview = "audio";
        item.pv = 1;
        item.duration = 0.4 + ((item.id * 13) % 35) / 10;
        item.peaks = generatePeaks(item.id);
      }
    }
  }

  return items.sort((a, b) => a.pack.localeCompare(b.pack) || a.packId - b.packId || a.path.localeCompare(b.path));
}

function generatePeaks(id: number): string {
  let result = "";
  for (let i = 0; i < 64; i++) {
    const val = ((id * 7 + i * 11) % 36).toString(36);
    result += val;
  }
  return result;
}

export class MockAssets implements AssetsApi {
  private items = demoItems();
  private listeners = new Set<(ev: AssetsLive) => void>();
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
    const todo = this.items.filter((a) => a.preview === null && (a.category === "texture" || a.category === "sprite")).length;
    return {
      configured: true, mounted: true, assets: this.items.length, bytes: this.items.reduce((n, a) => n + a.size, 0),
      packs: new Set(this.items.map((a) => a.packId)).size, scan: { ...this.progress },
      previews: { running: false, todo, made: 0, failed: 0 },
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
    
    const isAudio = a.preview === "audio";
    const isSprite = a.category === "sprite";
    
    return { 
      ...a, 
      packKind: "zip", 
      mtime: null, 
      rule: `path word in "${a.pack}"`, 
      missingSince: null, 
      sampleRate: isAudio ? 44100 : null,
      channels: isAudio ? 2 : null,
      hasAlpha: isSprite ? true : null,
      previewState: a.preview !== null ? "ok" : null,
      previewError: null,
      copies: [], 
      packDocs: docs 
    };
  }

  async scan() {
    if (this.progress.running) return;
    const total = 8;
    const step = (done: number) => {
      if (done === total) {
        // A new pack arrived while scanning.
        const id = Math.max(...this.items.map((x) => x.id)) + 1;
        const newItem = { id, packId: 99, pack: "Free Ambience Loops", container: "free-ambience-loops.zip", path: "city-night-loop.wav", name: "city-night-loop.wav", ext: "wav", size: 3_400_000, category: "ambience", meta: false, dupOf: null, ...previewFields(id, "ambience") };
        
        this.items = [...this.items, newItem].sort((a, b) => a.pack.localeCompare(b.pack) || a.packId - b.packId || a.path.localeCompare(b.path));
        
        this.finishedAt = new Date().toISOString();
      }
      this.progress = done < total
        ? { running: true, phase: "packs", done, total, current: PACKS[done % PACKS.length][0], error: null }
        : { running: false, phase: "done", done: total, total, current: "", error: null };
      
      this.listeners.forEach((l) => l({ scan: { ...this.progress } }));
      if (done < total) setTimeout(() => step(done + 1), 120);
    };
    setTimeout(() => step(0), 50);
  }

  onLive(listener: (ev: AssetsLive) => void): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  async wantPreviews(ids: number[]): Promise<void> {
    const want = new Set(ids);
    const isPicture = (a: AssetItem) => a.category === "texture" || a.category === "sprite";
    const changed = new Set(this.items.filter((a) => want.has(a.id) && a.preview === null && isPicture(a)).map((a) => a.id));
    if (!changed.size) return;
    await new Promise((resolve) => setTimeout(resolve, 300));
    // New objects, never in-place changes: the view holds the old ones until the event says so.
    this.items = this.items.map((a) => (changed.has(a.id) ? { ...a, preview: "image" as const, pv: 1 } : a));
    const progress: PreviewProgress = {
      running: false, todo: this.items.filter((a) => a.preview === null && isPicture(a)).length, made: changed.size, failed: 0,
    };
    this.listeners.forEach((l) => l({ previews: { ids: [...changed], progress } }));
  }

  previewUrl(a: { id: number; pv: number }, kind: "t" | "l" | "a"): string {
    if (kind === "a") return "";
    
    const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="64" height="64"><rect width="64" height="64" fill="#5c398e"/><text x="32" y="32" text-anchor="middle" dominant-baseline="central" fill="#f4eefc">${a.id}</text></svg>`;
    return `data:image/svg+xml,${encodeURIComponent(svg)}`;
  }
}

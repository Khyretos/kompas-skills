// Example data for demo mode and the browser tests (drafted by the local Coder model, reviewed).
import type { AiMode, AiProgress, AssetItem, Game, GameDetail, GameProfile, Licence, LicenceInput, Need, GamePick, PickStatus, AssetDetail, AssetStatus, AssetFilter, AssetTag, PackFacet, AssetFacets, AssetsApi, ScanProgress, PreviewProgress, AssetsLive } from "./assets";

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

function previewFields(id: number, category: string): Pick<AssetItem, "preview" | "pv" | "duration" | "peaks" | "width" | "height" | "aiCategory"> {
  // Every tenth sound effect is music to the AI: the "Check categories" list.
  return { ...pictureOrSound(id, category), aiCategory: category === "sound-effect" && id % 10 === 0 ? "music" : null };
}

function pictureOrSound(id: number, category: string): Pick<AssetItem, "preview" | "pv" | "duration" | "peaks" | "width" | "height"> {
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

const MUSIC_TAGS: AssetTag[] = [{ id: 1, kind: "mood", name: "epic", by: "ai" }, { id: 2, kind: "style", name: "orchestral", by: "ai" }];

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
  private aiMode: AiMode = "off";
  private nextTag = 100;
  private tags = new Map<number, AssetTag[]>(
    this.items.filter((a) => a.category === "music").map((a) => [a.id, MUSIC_TAGS]),
  );

  private aiProgress(): AiProgress {
    return { mode: this.aiMode, running: false, waiting: false, todo: 0, done: 0, failed: 0, lastError: null };
  }

  /** Waits a moment like a server, changes one item, and tells the page. */
  private async aiChange(id: number, change?: (a: AssetItem) => AssetItem): Promise<void> {
    await new Promise((r) => setTimeout(r, 100));
    if (change) this.items = this.items.map((a) => (a.id === id ? change(a) : a));
    const ev: AssetsLive = { ai: { ids: id ? [id] : [], progress: this.aiProgress() } };
    this.listeners.forEach((l) => l(ev));
  }

  async setAiMode(mode: AiMode) { this.aiMode = mode; await this.aiChange(0); }
  async describe(id: number) {
    if (this.aiMode === "off") throw new Error("AI tagging is off.");
    this.tags.set(id, MUSIC_TAGS);
    await this.aiChange(id);
  }
  async setCategory(id: number, category: string) { await this.aiChange(id, (a) => ({ ...a, category, aiCategory: null })); }
  async keepCategory(id: number) { await this.aiChange(id, (a) => ({ ...a, aiCategory: null })); }
  async addTag(id: number, name: string): Promise<AssetTag> {
    const tag: AssetTag = { id: this.nextTag++, kind: "custom", name: name.trim().toLowerCase(), by: "kees" };
    this.tags.set(id, [...(this.tags.get(id) ?? []), tag]);
    await this.aiChange(id);
    return tag;
  }
  async removeTag(id: number, tagId: number) {
    this.tags.set(id, (this.tags.get(id) ?? []).filter((t) => t.id !== tagId));
    await this.aiChange(id);
  }
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
      (skip === "review" || !f.review || a.aiCategory !== null) &&
      (!f.tag || (this.tags.get(a.id) ?? []).some((t) => `${t.kind}:${t.name}` === f.tag)) &&
      (f.usedBy === undefined || this.demoUses.some((u) => u.gameId === f.usedBy && u.assetId === a.id)) &&
      (f.similar === undefined || (a.id !== f.similar && a.category === this.items.find((x) => x.id === f.similar)?.category)) &&
      words.every((w) => `${a.name} ${a.container}/${a.path} ${a.pack} ${a.category}`.toLowerCase().split(/[^\p{L}\p{N}]+/u).some((t) => t.startsWith(w))));
  }

  async status(): Promise<AssetStatus> {
    const todo = this.items.filter((a) => a.preview === null && (a.category === "texture" || a.category === "sprite")).length;
    return {
      configured: true, mounted: true, assets: this.items.length, bytes: this.items.reduce((n, a) => n + a.size, 0),
      packs: new Set(this.items.map((a) => a.packId)).size, scan: { ...this.progress },
      previews: { running: false, todo, made: 0, failed: 0 },
      ai: this.aiProgress(),
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
      review: this.filtered(f, "review").filter((a) => a.aiCategory !== null).length,
      categories: [...cats].map(([name, c]) => ({ name, ...c })),
      packs: [...packs.values()].sort((a, b) => a.name.localeCompare(b.name)),
    };
  }

  async detail(id: number): Promise<AssetDetail> {
    const a = this.items.find((x) => x.id === id);
    if (!a) throw new Error("Not found.");
    const docs = this.items.filter((x) => x.packId === a.packId && x.meta).map((x) => ({ id: x.id, path: x.path }));
    
    const tags = this.tags.get(a.id) ?? [];
    const isAudio = a.preview === "audio";
    const isSprite = a.category === "sprite";
    
    return { 
      ...a, 
      licence: this.licenceOfPack(a.packId),
      usedIn: this.demoGames.flatMap((g) => {
        const scenes = this.demoUses.filter((u) => u.gameId === g.id && u.assetId === a.id).map((u) => ({ scene: u.scene, count: u.count }));
        return scenes.length ? [{ gameId: g.id, game: g.name, scenes }] : [];
      }),
      packKind: "zip", 
      mtime: null, 
      rule: `path word in "${a.pack}"`, 
      missingSince: null, 
      sampleRate: isAudio ? 44100 : null,
      channels: isAudio ? 2 : null,
      hasAlpha: isSprite ? true : null,
      previewState: a.preview !== null ? "ok" : null,
      previewError: null,
      aiState: tags.length ? "ok" : null,
      aiError: null,
      aiCaption: tags.length ? `Demo caption for ${a.name}` : null,
      aiSubject: null,
      transcript: null,
      aiModel: tags.length ? "Demo" : null,
      categoryBy: "rule",
      similar: tags.length > 0,
      tags,
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

  // ---- Games, needs and picks (milestone 4). Drafted by the local Coder in two rounds;
  // Claude fixed the start data, the in-place edits, the Pick shape and the game ids. ----

  private natureLicence: Licence = { id: 1, name: "Synty Standard EULA", commercial: true, attribution: false, url: "https://syntystore.com/pages/end-user-licence-agreement", notes: null };
  private demoLicences: Licence[] = [this.natureLicence];
  private packLicence = new Map<number, number>(
    this.items.filter((a) => a.pack === "POLYGON_Nature_Source_Files_v2").slice(0, 1).map((a) => [a.packId, 1]),
  );
  private demoGames: Omit<Game, "needs" | "candidates" | "drafting">[] = [
    { id: 1, key: "kk-engine/showcase", name: "KKE Showcase", source: "kk-engine", path: "games/showcase", about: "Walk around a small level with an animated character.", genre: "", artStyle: "", setting: "", commercial: false, profileBy: "none", missingSince: null },
    { id: 2, key: "own/1", name: "Forest Walk", source: "own", path: null, about: "", genre: "Exploration", artStyle: "low-poly", setting: "forest", commercial: true, profileBy: "kees", missingSince: null },
  ];
  private demoNeeds: { id: number; gameId: number; position: number; text: string; category: string | null; by: "ai" | "kees"; pickedAt: string | null; error: string | null }[] = [
    { id: 1, gameId: 2, position: 0, text: "footsteps on grass", category: "sound-effect", by: "kees", pickedAt: null, error: null },
    { id: 2, gameId: 2, position: 1, text: "pine trees", category: "3d-model", by: "kees", pickedAt: null, error: null },
  ];
  // "Used in": KKE Showcase's scene places six nature models (one of them twice).
  private demoUses = this.items.filter((a) => a.category === "3d-model" && a.pack.startsWith("POLYGON_Nature")).slice(0, 6)
    .map((a, i) => ({ gameId: 1, assetId: a.id, scene: "games/showcase/course_art.scene.json", count: i === 0 ? 2 : 1 }));
  private demoPicks: { needId: number; assetId: number; status: PickStatus; reason: string | null; by: "ai" | "kees"; rank: number }[] = [];
  private nextGameId = 3;
  private nextNeedId = 3;
  private nextLicenceId = 2;
  private pickingNeeds = new Set<number>();
  private draftingGames = new Set<number>();

  private async gamesChanged(game: number, error?: string): Promise<void> {
    await new Promise((r) => setTimeout(r, 80));
    this.listeners.forEach((l) => l({ games: error ? { game, error } : { game } }));
  }

  private gameOfNeed(need: number): number {
    const n = this.demoNeeds.find((x) => x.id === need);
    if (!n) throw new Error("Not found.");
    return n.gameId;
  }

  private licenceOfPack(pack: number): Licence | null {
    const id = this.packLicence.get(pack);
    const l = id === undefined ? undefined : this.demoLicences.find((x) => x.id === id);
    return l ? { ...l } : null;
  }

  async games(): Promise<Game[]> {
    return this.demoGames.map((g) => {
      const needIds = this.demoNeeds.filter((n) => n.gameId === g.id).map((n) => n.id);
      return {
        ...g, drafting: this.draftingGames.has(g.id), needs: needIds.length,
        used: new Set(this.demoUses.filter((u) => u.gameId === g.id).map((u) => u.assetId)).size,
        candidates: this.demoPicks.filter((p) => needIds.includes(p.needId) && p.status === "candidate").length,
      };
    }).sort((a, b) => (a.source === "own" ? 0 : 1) - (b.source === "own" ? 0 : 1) || a.name.localeCompare(b.name));
  }

  async game(id: number): Promise<GameDetail> {
    const game = this.demoGames.find((g) => g.id === id);
    if (!game) throw new Error("Not found.");
    const order: Record<PickStatus, number> = { candidate: 0, suggested: 1, rejected: 2 };
    const needs: Need[] = this.demoNeeds
      .filter((n) => n.gameId === id)
      .sort((a, b) => a.position - b.position)
      .map((n) => ({
        id: n.id, text: n.text, category: n.category, by: n.by, pickedAt: n.pickedAt, error: n.error,
        state: this.pickingNeeds.has(n.id) ? "picking" : null,
        picks: this.demoPicks
          .filter((p) => p.needId === n.id)
          .sort((a, b) => order[a.status] - order[b.status] || a.rank - b.rank)
          .flatMap((p): GamePick[] => {
            const item = this.items.find((i) => i.id === p.assetId);
            return item ? [{ asset: { ...item, licence: this.licenceOfPack(item.packId) }, status: p.status, reason: p.reason, by: p.by }] : [];
          }),
      }));
    const uses = this.demoUses.filter((u) => u.gameId === id);
    const scenes = {
      assets: new Set(uses.map((u) => u.assetId)).size,
      packs: new Set(uses.map((u) => this.items.find((i) => i.id === u.assetId)?.packId)).size,
      scenes: new Set(uses.map((u) => u.scene)).size,
      missing: uses.length ? [{ kind: "asset" as const, name: "SM_Prop_Lantern_01", scene: uses[0].scene }] : [],
    };
    return { ...game, needs, styleWarning: null, aiOn: this.aiMode !== "off", drafting: this.draftingGames.has(id), scenes };
  }

  async addGame(name: string): Promise<Game> {
    const n = name.trim();
    if (!n || n.length > 80) throw new Error("Give the game a name (up to 80 characters).");
    const id = this.nextGameId++;
    const g = { id, key: `own/${id}`, name: n, source: "own", path: null, about: "", genre: "", artStyle: "", setting: "", commercial: false, profileBy: "none" as const, missingSince: null };
    this.demoGames = [...this.demoGames, g];
    await this.gamesChanged(id);
    return { ...g, drafting: false, needs: 0, candidates: 0 };
  }

  async removeGame(id: number): Promise<void> {
    const game = this.demoGames.find((g) => g.id === id);
    if (!game) throw new Error("Not found.");
    if (game.source !== "own") throw new Error("This game comes from its repo; it stays while the repo has it.");
    const needIds = this.demoNeeds.filter((n) => n.gameId === id).map((n) => n.id);
    this.demoGames = this.demoGames.filter((g) => g.id !== id);
    this.demoNeeds = this.demoNeeds.filter((n) => n.gameId !== id);
    this.demoPicks = this.demoPicks.filter((p) => !needIds.includes(p.needId));
    await this.gamesChanged(id);
  }

  async setProfile(id: number, p: GameProfile): Promise<void> {
    if (!this.demoGames.some((g) => g.id === id)) throw new Error("Not found.");
    this.demoGames = this.demoGames.map((g) => g.id !== id ? g : {
      ...g, genre: p.genre.trim().slice(0, 60), artStyle: p.artStyle.trim().slice(0, 60), setting: p.setting.trim().slice(0, 60),
      commercial: p.commercial, profileBy: "kees",
    });
    this.demoNeeds = this.demoNeeds.map((n) => (n.gameId === id ? { ...n, by: "kees" } : n));
    await this.gamesChanged(id);
  }

  async draftProfile(id: number): Promise<void> {
    if (this.aiMode === "off") throw new Error("AI tagging is off, so there is no AI to draft with.");
    if (!this.demoGames.some((g) => g.id === id)) throw new Error("Not found.");
    this.draftingGames.add(id);
    void this.gamesChanged(id);
    void (async () => {
      await new Promise((r) => setTimeout(r, 300));
      this.demoGames = this.demoGames.map((g) => g.id !== id || g.profileBy === "kees" ? g : {
        ...g, genre: "Action adventure", artStyle: "stylized", setting: "small town", profileBy: "ai",
      });
      const have = this.demoNeeds.filter((n) => n.gameId === id);
      const known = new Set(have.map((n) => n.text.toLowerCase()));
      const add = ([["footsteps on stone", "sound-effect"], ["calm town music", "music"], ["UI click sound", "sound-effect"]] as const)
        .filter(([t]) => !known.has(t.toLowerCase()))
        .map(([text, category], i) => ({ id: this.nextNeedId++, gameId: id, position: have.length + i, text, category, by: "ai" as const, pickedAt: null, error: null }));
      this.demoNeeds = [...this.demoNeeds, ...add];
      this.draftingGames.delete(id);
      await this.gamesChanged(id);
    })();
  }

  private checkNeedText(text: string): string {
    const t = text.trim();
    if (!t || t.length > 120) throw new Error("Say what the game needs in up to 120 characters.");
    return t;
  }

  async addNeed(game: number, text: string, category: string | null): Promise<void> {
    const t = this.checkNeedText(text);
    if (!this.demoGames.some((g) => g.id === game)) throw new Error("Not found.");
    const position = this.demoNeeds.filter((n) => n.gameId === game).length;
    this.demoNeeds = [...this.demoNeeds, { id: this.nextNeedId++, gameId: game, position, text: t, category, by: "kees", pickedAt: null, error: null }];
    await this.gamesChanged(game);
  }

  async editNeed(id: number, text: string, category: string | null): Promise<void> {
    const t = this.checkNeedText(text);
    const game = this.gameOfNeed(id);
    this.demoNeeds = this.demoNeeds.map((n) => (n.id === id ? { ...n, text: t, category, by: "kees" } : n));
    await this.gamesChanged(game);
  }

  async removeNeed(id: number): Promise<void> {
    const game = this.gameOfNeed(id);
    this.demoNeeds = this.demoNeeds.filter((n) => n.id !== id);
    this.demoPicks = this.demoPicks.filter((p) => p.needId !== id);
    await this.gamesChanged(game);
  }

  /** Like the server: answers at once, the picks arrive live. */
  async pickNeed(id: number): Promise<void> {
    if (this.aiMode === "off") throw new Error("AI tagging is off, so there is no AI to pick with.");
    const game = this.gameOfNeed(id);
    this.pickingNeeds.add(id);
    void this.gamesChanged(game);
    void this.runPick(id, game);
  }

  private async runPick(id: number, game: number): Promise<void> {
    await new Promise((r) => setTimeout(r, 300));
    const need = this.demoNeeds.find((n) => n.id === id);
    if (!need) return;
    const gameNeeds = new Set(this.demoNeeds.filter((n) => n.gameId === game).map((n) => n.id));
    const rejected = new Set(this.demoPicks.filter((p) => gameNeeds.has(p.needId) && p.status === "rejected").map((p) => p.assetId));
    this.demoPicks = this.demoPicks.filter((p) => p.needId !== id || p.status !== "suggested");
    const taken = new Set(this.demoPicks.filter((p) => p.needId === id).map((p) => p.assetId));
    const pool = this.items.filter((i) => (need.category === null || i.category === need.category) && !i.meta && !taken.has(i.id) && !rejected.has(i.id));
    const words = need.text.toLowerCase().split(/[^\p{L}\p{N}]+/u).filter((w) => w.length >= 3);
    let picked: { item: AssetItem; reason: string }[] = [];
    for (const item of pool) {
      const word = words.find((w) => item.name.toLowerCase().includes(w));
      if (word) picked.push({ item, reason: `Matches "${word}" and suits the game.` });
      if (picked.length === 3) break;
    }
    if (!picked.length) picked = pool.slice(0, 3).map((item) => ({ item, reason: "Same category as the need." }));
    this.demoPicks = [...this.demoPicks, ...picked.map((p, rank) => ({ needId: id, assetId: p.item.id, status: "suggested" as const, reason: p.reason, by: "ai" as const, rank }))];
    this.demoNeeds = this.demoNeeds.map((n) => (n.id === id ? { ...n, pickedAt: new Date().toISOString(), error: null } : n));
    this.pickingNeeds.delete(id);
    await this.gamesChanged(game);
  }

  async pickAll(game: number): Promise<void> {
    const todo = this.demoNeeds.filter((n) => n.gameId === game
      && !this.demoPicks.some((p) => p.needId === n.id && (p.status === "suggested" || p.status === "candidate")));
    if (!todo.length) throw new Error("Every need already has picks. Use a need's own button to pick again.");
    for (const n of todo) await this.pickNeed(n.id);
  }

  async setPick(need: number, asset: number, status: "candidate" | "rejected"): Promise<void> {
    const game = this.gameOfNeed(need);
    const exists = this.demoPicks.some((p) => p.needId === need && p.assetId === asset);
    this.demoPicks = exists
      ? this.demoPicks.map((p) => (p.needId === need && p.assetId === asset ? { ...p, status, by: "kees" } : p))
      : [...this.demoPicks, { needId: need, assetId: asset, status, reason: null, by: "kees", rank: 99 }];
    await this.gamesChanged(game);
  }

  async removePick(need: number, asset: number): Promise<void> {
    const game = this.gameOfNeed(need);
    this.demoPicks = this.demoPicks.filter((p) => !(p.needId === need && p.assetId === asset));
    await this.gamesChanged(game);
  }

  async licences(): Promise<Licence[]> {
    return this.demoLicences.map((l) => ({ ...l, packs: [...this.packLicence.values()].filter((v) => v === l.id).length }));
  }

  async addLicence(l: LicenceInput): Promise<Licence> {
    const name = l.name.trim();
    if (!name || name.length > 80) throw new Error("Give the licence a name (up to 80 characters).");
    const licence: Licence = { id: this.nextLicenceId++, name, commercial: l.commercial, attribution: l.attribution, url: l.url ?? null, notes: l.notes ?? null };
    this.demoLicences = [...this.demoLicences, licence];
    return { ...licence, packs: 0 };
  }

  async linkLicence(pack: number, licence: number | null): Promise<void> {
    if (licence === null) this.packLicence.delete(pack);
    else this.packLicence.set(pack, licence);
    await this.gamesChanged(0);
  }

}

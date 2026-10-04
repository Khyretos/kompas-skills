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
  preview: "image" | "audio" | null; // a finished preview
  pv: number; // preview version: part of its URL, so a new preview gets a new URL
  duration: number | null; // seconds (audio)
  peaks: string | null; // 64 waveform levels, base-36 characters 0..z
  width: number | null; // pixels (images)
  height: number | null;
  aiCategory: string | null; // set when the AI disagrees with the category (review)
}

export interface AssetTag { id: number; kind: string; name: string; by: "ai" | "kees" }

/** A licence Kees links to packs (never guessed). */
export interface Licence { id: number; name: string; commercial: boolean; attribution: boolean; url: string | null; notes?: string | null; packs?: number }
export type LicenceInput = { name: string; commercial: boolean; attribution: boolean; url?: string; notes?: string };

export interface AssetDetail extends AssetItem {
  licence: Licence | null; // the pack's licence
  packKind: string;
  mtime: number | null;
  rule: string; // why it got its category
  missingSince: string | null;
  sampleRate: number | null;
  channels: number | null;
  hasAlpha: boolean | null;
  previewState: null | "ok" | "none" | "error"; // null: not made yet
  previewError: string | null;
  aiState: null | "ok" | "error";
  aiError: string | null;
  aiCaption: string | null;
  aiSubject: string | null;
  transcript: string | null;
  aiModel: string | null;
  categoryBy: "rule" | "kees";
  similar: boolean; // has an AI vector: "Find similar" works
  tags: AssetTag[];
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

export interface PreviewProgress {
  running: boolean;
  todo: number; // previews still to make
  made: number;
  failed: number;
}

export type AiMode = "off" | "night" | "always";

export interface AiProgress {
  mode: AiMode;
  running: boolean;
  waiting: boolean; // on "night", outside 23:00-06:00 UTC
  todo: number;
  done: number;
  failed: number;
  lastError: string | null;
}

/** A game the picks are for: from a game repo (kk-engine) or added by hand ("own"). */
export interface Game {
  id: number; key: string; name: string; source: string; path: string | null; about: string;
  genre: string; artStyle: string; setting: string;
  commercial: boolean; // will be sold: only packs whose licence allows it
  profileBy: "none" | "ai" | "kees"; // "ai": a draft to confirm
  missingSince: string | null;
  drafting: boolean;
  needs?: number; candidates?: number; // in the list only
}
export type PickStatus = "suggested" | "candidate" | "rejected";
export interface GamePick {
  asset: AssetItem & { licence: Licence | null };
  status: PickStatus;
  reason: string | null; // the AI's one-line reason
  by: "ai" | "kees";
}
export interface Need {
  id: number; text: string; category: string | null; by: "ai" | "kees";
  pickedAt: string | null; error: string | null;
  state: null | "queued" | "picking";
  picks: GamePick[];
}
export interface GameDetail extends Omit<Game, "needs" | "candidates"> {
  needs: Need[];
  styleWarning: string | null;
  aiOn: boolean;
}
export type GameProfile = Pick<Game, "genre" | "artStyle" | "setting" | "commercial">;

/** A live Assets event: scan progress, or previews that are ready. `null`: events were missed. */
export type AssetsLive = null | {
  scan?: ScanProgress;
  previews?: { ids: number[]; progress: PreviewProgress };
  ai?: { ids: number[]; progress: AiProgress };
  games?: { game: number; error?: string }; // a game's profile, needs or picks changed
};

export interface AssetStatus {
  configured: boolean;
  mounted: boolean;
  assets: number;
  bytes: number;
  packs: number;
  scan: ScanProgress;
  previews?: PreviewProgress;
  ai?: AiProgress;
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
  review?: boolean; // only assets whose category the AI disagrees with
  tag?: string; // "kind:name"
  similar?: number; // assets most like this one
  similarName?: string; // (shown in the chip only, not sent)
  meaning?: boolean; // search q by meaning instead of words
}

export interface PackFacet {
  id: number; name: string; kind: string; size: number; duplicateOf: number | null; error: string | null;
  files: number; bytes: number;
}

export interface AssetFacets {
  review?: number; // assets the AI files differently
  categories: { name: string; files: number; bytes: number }[];
  packs: PackFacet[];
}

export interface AssetsApi {
  status(): Promise<AssetStatus>;
  list(f: AssetFilter, offset: number, limit: number): Promise<{ total: number; offset: number; items: AssetItem[] }>;
  facets(f: AssetFilter): Promise<AssetFacets>;
  detail(id: number): Promise<AssetDetail>;
  scan(): Promise<void>;
  setAiMode(mode: AiMode): Promise<void>;
  /** "Describe now": this asset goes first while AI tagging is on. */
  describe(id: number): Promise<void>;
  setCategory(id: number, category: string): Promise<void>;
  keepCategory(id: number): Promise<void>;
  addTag(id: number, name: string): Promise<AssetTag>;
  removeTag(id: number, tagId: number): Promise<void>;
  games(): Promise<Game[]>;
  game(id: number): Promise<GameDetail>;
  addGame(name: string): Promise<Game>;
  removeGame(id: number): Promise<void>;
  setProfile(id: number, p: GameProfile): Promise<void>;
  /** The AI drafts the profile and needs from the game's own docs (arrives live). */
  draftProfile(id: number): Promise<void>;
  addNeed(game: number, text: string, category: string | null): Promise<void>;
  editNeed(id: number, text: string, category: string | null): Promise<void>;
  removeNeed(id: number): Promise<void>;
  /** Search, rerank and let the AI pick (arrives live). */
  pickNeed(id: number): Promise<void>;
  pickAll(game: number): Promise<void>;
  setPick(need: number, asset: number, status: "candidate" | "rejected"): Promise<void>;
  removePick(need: number, asset: number): Promise<void>;
  licences(): Promise<Licence[]>;
  addLicence(l: LicenceInput): Promise<Licence>;
  linkLicence(pack: number, licence: number | null): Promise<void>;
  /** Asks for these previews first (the cards on screen). */
  wantPreviews(ids: number[]): Promise<void>;
  /** URL of a preview: t = 256 px image, l = 1024 px image, a = audio clip. */
  previewUrl(a: { id: number; pv: number }, kind: "t" | "l" | "a"): string;
  /** Scan progress and finished previews, live. */
  onLive(listener: (ev: AssetsLive) => void): () => void;
}

function query(f: AssetFilter, extra: Record<string, number> = {}): string {
  const p = new URLSearchParams();
  if (f.q?.trim()) p.set("q", f.q.trim());
  if (f.category) p.set("category", f.category);
  if (f.pack !== undefined) p.set("pack", String(f.pack));
  if (f.dups) p.set("dups", "true");
  if (f.review) p.set("review", "true");
  if (f.tag) p.set("tag", f.tag);
  if (f.similar !== undefined) p.set("similar", String(f.similar));
  if (f.meaning && f.q?.trim()) p.set("meaning", "true");
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
  private async send<T = void>(method: string, path: string, body?: unknown): Promise<T> {
    const res = await fetch(`/api${path}`, {
      method, credentials: "same-origin",
      headers: { "X-Kompanion": "1", ...(body === undefined ? {} : { "Content-Type": "application/json" }) },
      body: body === undefined ? undefined : JSON.stringify(body),
    });
    if (!res.ok) throw new Error((await res.json().catch(() => ({}))).error ?? `The server answered ${res.status}.`);
    return (res.status === 204 || res.status === 202 ? undefined : await res.json()) as T;
  }
  scan() { return this.send("POST", "/assets/scan"); }
  setAiMode(mode: AiMode) { return this.send("PUT", "/assets/ai", { mode }); }
  describe(id: number) { return this.send("POST", `/assets/${id}/describe`); }
  setCategory(id: number, category: string) { return this.send("POST", `/assets/${id}/category`, { category }); }
  keepCategory(id: number) { return this.send("POST", `/assets/${id}/category/keep`); }
  addTag(id: number, name: string) { return this.send<AssetTag>("POST", `/assets/${id}/tags`, { name }); }
  removeTag(id: number, tagId: number) { return this.send("DELETE", `/assets/${id}/tags/${tagId}`); }
  games() { return this.get<Game[]>("/assets/games"); }
  game(id: number) { return this.get<GameDetail>(`/assets/games/${id}`); }
  addGame(name: string) { return this.send<Game>("POST", "/assets/games", { name }); }
  removeGame(id: number) { return this.send("DELETE", `/assets/games/${id}`); }
  setProfile(id: number, p: GameProfile) { return this.send("PUT", `/assets/games/${id}`, p); }
  draftProfile(id: number) { return this.send("POST", `/assets/games/${id}/draft`); }
  addNeed(game: number, text: string, category: string | null) { return this.send("POST", `/assets/games/${game}/needs`, { text, category }).then(() => undefined); }
  editNeed(id: number, text: string, category: string | null) { return this.send("PUT", `/assets/needs/${id}`, { text, category }); }
  removeNeed(id: number) { return this.send("DELETE", `/assets/needs/${id}`); }
  pickNeed(id: number) { return this.send("POST", `/assets/needs/${id}/pick`); }
  pickAll(game: number) { return this.send("POST", `/assets/games/${game}/pick`); }
  setPick(need: number, asset: number, status: "candidate" | "rejected") { return this.send("PUT", `/assets/needs/${need}/picks/${asset}`, { status }); }
  removePick(need: number, asset: number) { return this.send("DELETE", `/assets/needs/${need}/picks/${asset}`); }
  licences() { return this.get<Licence[]>("/assets/licences"); }
  addLicence(l: LicenceInput) { return this.send<Licence>("POST", "/assets/licences", l); }
  linkLicence(pack: number, licence: number | null) { return this.send("PUT", `/assets/packs/${pack}/licence`, { licence }); }
  async wantPreviews(ids: number[]) {
    await fetch("/api/assets/previews/want", {
      method: "POST", credentials: "same-origin",
      headers: { "X-Kompanion": "1", "Content-Type": "application/json" }, body: JSON.stringify({ ids }),
    });
  }
  previewUrl(a: { id: number; pv: number }, kind: "t" | "l" | "a") {
    return `/asset-preview/${a.id}-${kind}.${kind === "a" ? "webm" : "webp"}?v=${a.pv}`;
  }
  onLive(listener: (ev: AssetsLive) => void) {
    return this.events.onEvent((ev) => {
      if (ev.type === "assets") listener(ev as unknown as AssetsLive);
      else if (ev.type === "resync") listener(null);
    });
  }
}

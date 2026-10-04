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
}

export interface AssetDetail extends AssetItem {
  packKind: string;
  mtime: number | null;
  rule: string; // why it got its category
  missingSince: string | null;
  sampleRate: number | null;
  channels: number | null;
  hasAlpha: boolean | null;
  previewState: null | "ok" | "none" | "error"; // null: not made yet
  previewError: string | null;
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

/** A live Assets event: scan progress, or previews that are ready. `null`: events were missed. */
export type AssetsLive = null | { scan?: ScanProgress; previews?: { ids: number[]; progress: PreviewProgress } };

export interface AssetStatus {
  configured: boolean;
  mounted: boolean;
  assets: number;
  bytes: number;
  packs: number;
  scan: ScanProgress;
  previews?: PreviewProgress;
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

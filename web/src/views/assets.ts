// Assets section: the game asset library as a virtualised grid of cards (only the
// visible rows are in the DOM), with search, category chips, a pack filter and a
// detail panel. Scan progress and its results arrive live; nothing needs a reload.
import { html, mount, onAction, type SafeHtml } from "../core/html";
import type { AssetDetail, AssetFacets, AssetFilter, AssetItem, AssetsApi, AssetsLive, AssetStatus, PreviewProgress, ScanProgress } from "../api/assets";
import { icon } from "./icons";
import { relTime } from "../core/time";

const PAGE = 200; // items per request
const MIN_CARD = 190; // px, smallest card width
const CARD_H = 188; // px, card height (thumbnail + three text lines)
const GAP = 12; // px between cards: cards never touch
const ROW = CARD_H + GAP;
const OVERSCAN = 2; // rows above and below the visible ones

const LABEL: Record<string, string> = {
  "sound-effect": "Sound effects", music: "Music", ambience: "Ambience", voice: "Voice", "3d-model": "3D models",
  animation: "Animations", texture: "Textures", material: "Materials", sprite: "Sprites", vfx: "VFX", image: "Images",
  shader: "Shaders", font: "Fonts", video: "Video", "print-model": "3D print", "engine-file": "Engine files",
  archive: "Archives", doc: "Docs", other: "Other", junk: "Junk",
};
const ONE: Record<string, string> = {
  "sound-effect": "Sound effect", "3d-model": "3D model", animation: "Animation", texture: "Texture", sprite: "Sprite",
  image: "Image", shader: "Shader", font: "Font", "print-model": "3D print", "engine-file": "Engine file",
  archive: "Archive", doc: "Doc", material: "Material",
};
const label1 = (c: string) => ONE[c] ?? LABEL[c] ?? c;

// Category glyphs (same stroke style as views/icons.ts).
const GLYPH: Record<string, string> = {
  audio: "M9 18V6l10-2v12M9 18a2.5 2.5 0 1 1-5 0 2.5 2.5 0 0 1 5 0zM19 16a2.5 2.5 0 1 1-5 0 2.5 2.5 0 0 1 5 0z",
  wave: "M3 12h2l2-6 3 12 3-9 2 6 2-3h4",
  voice: "M12 4a3 3 0 0 0-3 3v5a3 3 0 0 0 6 0V7a3 3 0 0 0-3-3zM6 11a6 6 0 0 0 12 0M12 17v3",
  model: "M12 3l8 4.5v9L12 21l-8-4.5v-9zM4 7.5l8 4.5 8-4.5M12 12v9",
  anim: "M5 19c3-1 4-6 7-6s4 5 7 6M12 13V9M10 5a2 2 0 1 0 4 0 2 2 0 0 0-4 0",
  image: "M4 5h16v14H4zM4 16l5-5 4 4 2-2 5 5M15 9.5a1 1 0 1 0 0-.01",
  vfx: "M12 3v4M12 17v4M3 12h4M17 12h4M6 6l2.5 2.5M15.5 15.5L18 18M6 18l2.5-2.5M15.5 8.5L18 6",
  font: "M5 19l6-14h2l6 14M8 14h8",
  file: "M6 3h8l4 4v14H6zM14 3v4h4",
  code: "M9 7l-5 5 5 5M15 7l5 5-5 5",
  junk: "M4 7h16M9 7V4h6v3M6 7l1 13h10l1-13",
};
const GLYPH_OF: Record<string, string> = {
  "sound-effect": "wave", music: "audio", ambience: "wave", voice: "voice", "3d-model": "model", "print-model": "model",
  animation: "anim", texture: "image", sprite: "image", image: "image", vfx: "vfx", font: "font", shader: "code",
  "engine-file": "code", material: "image", junk: "junk",
};
function glyph(category: string): SafeHtml {
  const d = GLYPH[GLYPH_OF[category] ?? "file"];
  return html`<svg class="icon" viewBox="0 0 24 24" aria-hidden="true" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><path d="${d}"/></svg>`;
}

/** Extensions the server makes previews for (assets/preview.rs). */
const PREVIEWABLE = new Set("png jpg jpeg tga bmp gif webp tif tiff psd dds exr hdr wav ogg mp3 flac aif aiff m4a aac opus wma".split(" "));

/** 64 waveform levels (base-36 characters) as bars around the middle of a 64 x 36 box. */
export function waveform(peaks: string): SafeHtml {
  let d = "";
  for (let i = 0; i < peaks.length; i++) {
    const h = Math.max(0.5, (parseInt(peaks[i], 36) / 35) * 16);
    d += `M${i + 0.5} ${(18 - h).toFixed(1)}v${(2 * h).toFixed(1)}`;
  }
  return html`<svg class="wave" viewBox="0 0 64 36" preserveAspectRatio="none" aria-hidden="true"><path d="${d}"/></svg>`;
}

export function clockTime(seconds: number): string {
  if (seconds < 10) return `${seconds.toFixed(1)} s`; // short sound effects: "0.6 s", not "0:00"
  const s = Math.round(seconds);
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
}

const num = new Intl.NumberFormat("en");
export function bytes(n: number): string {
  const units = ["B", "KB", "MB", "GB", "TB"];
  let i = 0;
  while (n >= 1024 && i < units.length - 1) { n /= 1024; i++; }
  return i === 0 ? `${n} B` : `${n.toFixed(n < 10 ? 1 : 0)} ${units[i]}`;
}

function toast(message: string): void {
  const t = document.createElement("div");
  t.className = "toast";
  t.setAttribute("role", "alert");
  t.textContent = message;
  document.body.append(t);
  setTimeout(() => t.remove(), 6000);
}

export class AssetsView {
  private filter: AssetFilter = {};
  private total = 0;
  private pages = new Map<number, AssetItem[]>();
  private loading = new Set<number>();
  private gen = 0; // bumped when the filter changes: stale answers are dropped
  private status?: AssetStatus;
  private facets?: AssetFacets;
  private selected?: number;
  private cols = 1;
  private lastWindow = "";
  private frame = 0;
  private qTimer = 0;
  private started = false;
  private audio = new Audio();
  private playing?: number; // asset whose clip is playing
  private wanted = new Set<number>(); // previews already asked for
  private wantTimer = 0;
  private stale = new Set<number>(); // pages to reload because previews arrived
  private staleTimer = 0;

  constructor(private el: HTMLElement, private api: AssetsApi, private isAdmin: () => boolean) {}

  /** Builds the page the first time it is shown; later calls only refresh it. */
  show(): void {
    if (this.started) { this.renderGrid(true); return; }
    this.started = true;
    mount(this.el, html`
      <header class="assets-head">
        <button class="icon-btn only-phone" data-action="pane" data-pane="left" aria-label="Projects and chats">${icon("menu")}</button>
        <div class="assets-title">
          <h1>Assets</h1>
          <p class="muted" id="asset-summary" aria-live="polite"></p>
          <p class="asset-previews-left" id="asset-previews" hidden></p>
        </div>
        <div class="asset-scan" id="asset-scan"></div>
      </header>
      <div class="assets-filters">
        <input type="search" id="asset-q" placeholder="Search names, paths and packs" aria-label="Search assets" autocomplete="off">
        <select id="asset-pack" aria-label="Pack"><option value="">All packs</option></select>
        <label class="asset-check"><input type="checkbox" id="asset-dups"> Show copies</label>
      </div>
      <div class="asset-chips" id="asset-chips" role="group" aria-label="Category"></div>
      <div class="assets-body">
        <div class="asset-scroll" id="asset-scroll" tabindex="-1">
          <div class="asset-space" id="asset-space"><ul class="asset-rows" id="asset-rows" aria-label="Assets"></ul></div>
          <div class="asset-empty" id="asset-empty" hidden></div>
        </div>
        <aside class="asset-detail" id="asset-detail" aria-label="Asset details" hidden></aside>
      </div>`);

    const q = this.el.querySelector<HTMLInputElement>("#asset-q")!;
    q.addEventListener("input", () => {
      clearTimeout(this.qTimer);
      this.qTimer = window.setTimeout(() => this.setFilter({ q: q.value }), 180);
    });
    this.el.querySelector<HTMLSelectElement>("#asset-pack")!.addEventListener("change", (ev) => {
      const v = (ev.target as HTMLSelectElement).value;
      this.setFilter({ pack: v ? Number(v) : undefined });
    });
    this.el.querySelector<HTMLInputElement>("#asset-dups")!.addEventListener("change", (ev) => {
      this.setFilter({ dups: (ev.target as HTMLInputElement).checked });
    });
    const scroll = this.scroller();
    scroll.addEventListener("scroll", () => this.schedule(), { passive: true });
    new ResizeObserver(() => this.schedule()).observe(scroll);
    this.el.addEventListener("keydown", (ev) => {
      if (ev.key === "Escape" && this.selected !== undefined) this.closeDetail();
    });

    onAction(this.el, {
      "asset-cat": (b) => this.setFilter({ category: b.dataset.cat || undefined }),
      "asset-open": (b) => this.open(Number(b.dataset.id)),
      "asset-close": () => this.closeDetail(),
      "asset-pack": (b) => {
        this.closeDetail();
        const pack = Number(b.dataset.pack);
        this.el.querySelector<HTMLSelectElement>("#asset-pack")!.value = String(pack);
        this.setFilter({ pack });
      },
      "asset-clear": () => {
        this.el.querySelector<HTMLInputElement>("#asset-q")!.value = "";
        this.el.querySelector<HTMLSelectElement>("#asset-pack")!.value = "";
        this.el.querySelector<HTMLInputElement>("#asset-dups")!.checked = false;
        this.filter = {};
        this.setFilter({});
      },
      "asset-scan": () => this.scan(),
      "asset-play": (b) => this.play(Number(b.dataset.id)),
    });
    this.audio.preload = "none";
    this.audio.addEventListener("timeupdate", () => this.showPlayback());
    this.audio.addEventListener("ended", () => this.stopPlayback());
    this.audio.addEventListener("pause", () => this.showPlayback());
    this.audio.addEventListener("error", () => { if (this.playing !== undefined) { this.stopPlayback(); toast("This clip can't be played."); } });

    this.api.onLive((ev) => this.onLive(ev));
    void this.refresh(true);
  }

  private scroller(): HTMLElement { return this.el.querySelector<HTMLElement>("#asset-scroll")!; }

  private setFilter(change: Partial<AssetFilter>): void {
    this.filter = { ...this.filter, ...change };
    this.scroller().scrollTop = 0;
    void this.refresh(true);
  }

  /** Reloads status, counts and the visible cards. `reset` drops the loaded pages at once
   *  (a filter change); otherwise the old cards stay until the new ones arrive (a scan). */
  private async refresh(reset: boolean): Promise<void> {
    const gen = ++this.gen;
    if (reset) {
      this.pages.clear();
      this.loading.clear();
      this.total = 0;
    }
    try {
      const [status, facets, first] = await Promise.all([
        this.api.status(), this.api.facets(this.filter), this.api.list(this.filter, this.pageOf(this.firstVisible()) * PAGE, PAGE),
      ]);
      if (gen !== this.gen) return;
      this.status = status;
      this.facets = facets;
      this.pages.clear();
      this.loading.clear();
      this.pages.set(first.offset / PAGE, first.items);
      this.total = first.total;
      this.renderHead();
      this.renderPreviewProgress();
      this.renderFilters();
      this.renderGrid(true);
      if (this.selected !== undefined) void this.open(this.selected, false);
    } catch (e) {
      if (gen === this.gen) toast(e instanceof Error ? e.message : String(e));
    }
  }

  private firstVisible(): number {
    return Math.floor(this.scroller().scrollTop / ROW) * this.cols;
  }
  private pageOf(index: number): number { return Math.floor(index / PAGE); }

  private async loadPage(page: number): Promise<void> {
    if (this.pages.has(page) || this.loading.has(page)) return;
    this.loading.add(page);
    const gen = this.gen;
    try {
      const res = await this.api.list(this.filter, page * PAGE, PAGE);
      if (gen !== this.gen) return;
      this.pages.set(page, res.items);
      this.total = res.total;
      this.renderGrid(true);
    } catch (e) {
      if (gen === this.gen) toast(e instanceof Error ? e.message : String(e));
    } finally {
      if (gen === this.gen) this.loading.delete(page);
    }
  }

  private schedule(): void {
    if (this.frame) return;
    this.frame = requestAnimationFrame(() => { this.frame = 0; this.renderGrid(false); });
  }

  private renderHead(): void {
    const s = this.status;
    const summary = this.el.querySelector<HTMLElement>("#asset-summary")!;
    if (!s) return;
    if (!s.configured) summary.textContent = "No asset library is set up on this server (ASSET_LIBRARY).";
    else if (!s.mounted) summary.textContent = "The asset library folder is not mounted right now.";
    else {
      const when = s.lastScan ? ` · scanned ${relTime(s.lastScan.finishedAt)}` : " · not scanned yet";
      summary.textContent = `${num.format(s.assets)} files in ${num.format(s.packs)} packs · ${bytes(s.bytes)}${when}`;
    }
    this.renderScan(s.scan);
  }

  private renderScan(p: ScanProgress): void {
    const box = this.el.querySelector<HTMLElement>("#asset-scan")!;
    const phase: Record<string, string> = { walking: "Looking through folders", packs: "Listing packs", saving: "Saving the index" };
    if (p.running) {
      const pct = p.total ? Math.round((p.done / p.total) * 100) : 0;
      mount(box, html`
        <div class="scan-live" role="status">
          <span class="spinner" aria-hidden="true"></span>
          <span><strong>${phase[p.phase] ?? "Scanning"}</strong>${p.phase === "packs" ? html` ${p.done}/${p.total}` : ""}
            ${p.current ? html`<small class="muted">${p.current}</small>` : ""}</span>
          <progress max="100" value="${pct}" aria-label="Scan progress"></progress>
        </div>`);
      return;
    }
    const failed = p.phase === "failed" ? html`<span class="chip bad" role="alert">${p.error ?? "Scan failed"}</span>` : "";
    const errors = this.status?.lastScan?.errors.length
      ? html`<span class="chip warn-chip" title="${this.status.lastScan.errors.join("\n")}">${this.status.lastScan.errors.length} unreadable</span>` : "";
    mount(box, html`${failed}${errors}${this.isAdmin() && this.status?.mounted
      ? html`<button class="btn" data-action="asset-scan">${icon("spark")} Scan now</button>` : ""}`);
  }

  private onLive(ev: AssetsLive): void {
    if (!this.started) return;
    if (ev === null) { void this.refresh(false); return; } // missed events: reload
    if (ev.scan) this.onProgress(ev.scan);
    if (ev.previews) this.onPreviews(ev.previews.ids, ev.previews.progress);
  }

  /** Previews arrived: reload only the loaded pages that hold them, then redraw. */
  private onPreviews(ids: number[], progress: PreviewProgress): void {
    if (this.status) this.status = { ...this.status, previews: progress };
    this.renderPreviewProgress();
    if (!ids.length) return;
    const want = new Set(ids);
    for (const [page, items] of this.pages) if (items.some((a) => want.has(a.id))) this.stale.add(page);
    if (this.selected !== undefined && want.has(this.selected)) void this.open(this.selected, false);
    if (!this.stale.size || this.staleTimer) return;
    this.staleTimer = window.setTimeout(async () => {
      this.staleTimer = 0;
      const pages = [...this.stale];
      this.stale.clear();
      const gen = this.gen;
      try {
        const got = await Promise.all(pages.map((p) => this.api.list(this.filter, p * PAGE, PAGE)));
        if (gen !== this.gen) return;
        got.forEach((res, i) => this.pages.set(pages[i], res.items));
        this.renderGrid(true);
      } catch { /* the next event tries again */ }
    }, 700);
  }

  private renderPreviewProgress(): void {
    const p = this.status?.previews;
    const box = this.el.querySelector<HTMLElement>("#asset-previews");
    if (!box) return;
    box.hidden = !p?.running || !p.todo;
    if (!box.hidden && p) box.textContent = `Making previews · ${num.format(p.todo)} left`;
  }

  private onProgress(p: ScanProgress): void {
    const wasRunning = this.status?.scan.running;
    if (this.status) this.status = { ...this.status, scan: p };
    this.renderScan(p);
    if (wasRunning && !p.running) void this.refresh(false);
  }

  private async scan(): Promise<void> {
    if (!this.status) return;
    const before = this.status.scan;
    this.onProgress({ running: true, phase: "walking", done: 0, total: 0, current: "", error: null });
    try {
      await this.api.scan();
    } catch (e) {
      this.status = { ...this.status, scan: before };
      this.renderScan(before);
      toast(e instanceof Error ? e.message : String(e));
    }
  }

  private renderFilters(): void {
    const f = this.facets;
    if (!f) return;
    const counts = new Map(f.categories.map((c) => [c.name, c.files]));
    const all = f.categories.filter((c) => c.name !== "junk").reduce((n, c) => n + c.files, 0);
    const cats = (this.status?.categories ?? []).filter((c) => counts.get(c) || c === this.filter.category);
    mount(this.el.querySelector("#asset-chips")!, html`
      <button class="chip-btn" data-action="asset-cat" data-cat="" aria-pressed="${String(!this.filter.category)}">All <span>${num.format(all)}</span></button>
      ${cats.map((c) => html`<button class="chip-btn cat-${c}" data-action="asset-cat" data-cat="${c}" aria-pressed="${String(this.filter.category === c)}">
        ${LABEL[c] ?? c} <span>${num.format(counts.get(c) ?? 0)}</span></button>`)}`);
    const select = this.el.querySelector<HTMLSelectElement>("#asset-pack")!;
    const current = this.filter.pack;
    mount(select, html`<option value="">All packs (${num.format(f.packs.length)})</option>
      ${f.packs.map((p) => html`<option value="${p.id}" ${p.id === current ? "selected" : ""}>${p.name}${p.duplicateOf ? " (copy)" : ""} · ${num.format(p.files)}</option>`)}`);
  }

  /** Draws the visible rows. `force` redraws even when the visible window is the same. */
  private renderGrid(force: boolean): void {
    const scroll = this.scroller();
    const width = scroll.clientWidth - 2 * GAP;
    if (width <= 0) return; // hidden
    this.cols = Math.max(1, Math.floor((width + GAP) / (MIN_CARD + GAP)));
    const rows = Math.ceil(this.total / this.cols);
    const space = this.el.querySelector<HTMLElement>("#asset-space")!;
    space.style.height = `${rows * ROW + GAP}px`;
    const first = Math.max(0, Math.floor(scroll.scrollTop / ROW) - OVERSCAN);
    const last = Math.min(rows, Math.ceil((scroll.scrollTop + scroll.clientHeight) / ROW) + OVERSCAN);
    const key = `${first}:${last}:${this.cols}:${this.total}:${this.selected}`;
    if (!force && key === this.lastWindow) return;
    this.lastWindow = key;

    const empty = this.el.querySelector<HTMLElement>("#asset-empty")!;
    empty.hidden = this.total > 0 || !this.status;
    if (!empty.hidden) {
      const filtered = this.filter.q || this.filter.category || this.filter.pack !== undefined;
      mount(empty, filtered
        ? html`<p>Nothing matches these filters.</p><button class="btn" data-action="asset-clear">Clear filters</button>`
        : html`<p>The index is empty.${this.isAdmin() ? " Start a scan to fill it." : ""}</p>`);
    }

    const list = this.el.querySelector<HTMLElement>("#asset-rows")!;
    list.style.transform = `translateY(${first * ROW}px)`;
    list.style.gridTemplateColumns = `repeat(${this.cols}, minmax(0, 1fr))`;
    const cards: SafeHtml[] = [];
    const from = first * this.cols, to = Math.min(this.total, last * this.cols);
    for (let i = from; i < to; i++) {
      const page = this.pageOf(i);
      const item = this.pages.get(page)?.[i - page * PAGE];
      if (!item) {
        void this.loadPage(page);
        cards.push(html`<li><div class="asset-card skeleton" aria-hidden="true"><span class="asset-thumb"></span><span class="asset-text"></span></div></li>`);
        continue;
      }
      cards.push(this.card(item));
    }
    // Keep keyboard focus on the same card (or its play button) across redraws.
    const active = document.activeElement as HTMLElement | null;
    const focused = active?.closest<HTMLElement>(".asset-cell")?.dataset.id;
    const onPlay = active?.classList.contains("asset-play");
    mount(list, html`${cards}`);
    if (focused) list.querySelector<HTMLElement>(`.asset-cell[data-id="${focused}"] ${onPlay ? ".asset-play" : ".asset-card"}`)?.focus();
    this.showPlayback();
    this.askForPreviews(from, to);
  }

  /** The cards on screen without a preview yet: ask the server to make theirs first. */
  private askForPreviews(from: number, to: number): void {
    const ids: number[] = [];
    for (let i = from; i < to; i++) {
      const a = this.pages.get(this.pageOf(i))?.[i % PAGE];
      if (a && !a.preview && PREVIEWABLE.has(a.ext) && !this.wanted.has(a.id) && !a.container.endsWith(".unitypackage")) ids.push(a.id);
    }
    if (!ids.length) return;
    ids.forEach((id) => this.wanted.add(id));
    clearTimeout(this.wantTimer);
    this.wantTimer = window.setTimeout(() => void this.api.wantPreviews(ids).catch(() => undefined), 400);
  }

  private play(id: number): void {
    const a = this.find(id);
    if (!a || a.preview !== "audio") return;
    if (this.playing === id && !this.audio.paused) { this.audio.pause(); return; }
    if (this.playing !== id) {
      this.stopPlayback();
      this.playing = id;
      this.audio.src = this.api.previewUrl(a, "a");
    }
    this.audio.play().catch(() => { this.stopPlayback(); toast("This clip can't be played."); });
    this.showPlayback();
  }

  private stopPlayback(): void {
    this.audio.pause();
    this.playing = undefined;
    this.showPlayback();
  }

  /** Marks the playing card and its progress (no redraw: only a class and a CSS variable). */
  private showPlayback(): void {
    for (const el of this.el.querySelectorAll<HTMLElement>(".asset-cell.playing")) {
      if (Number(el.dataset.id) !== this.playing || this.audio.paused) {
        el.classList.remove("playing");
        el.querySelector(".asset-play")?.setAttribute("aria-pressed", "false");
      }
    }
    if (this.playing === undefined) return;
    const cell = this.el.querySelector<HTMLElement>(`.asset-cell[data-id="${this.playing}"]`);
    if (!cell) return;
    const on = !this.audio.paused;
    cell.classList.toggle("playing", on);
    cell.querySelector(".asset-play")?.setAttribute("aria-pressed", String(on));
    const d = this.audio.duration;
    cell.style.setProperty("--played", d > 0 ? String(this.audio.currentTime / d) : "0");
  }

  private find(id: number): AssetItem | undefined {
    for (const items of this.pages.values()) {
      const a = items.find((x) => x.id === id);
      if (a) return a;
    }
    return undefined;
  }

  private card(a: AssetItem): SafeHtml {
    const where = a.container ? `${a.container}/${a.path}` : a.path;
    const thumb = a.preview === "image"
      ? html`<img src="${this.api.previewUrl(a, "t")}" alt="" loading="lazy" decoding="async">`
      : a.preview === "audio" && a.peaks ? waveform(a.peaks) : glyph(a.category);
    const audio = a.preview === "audio";
    return html`<li class="asset-cell ${audio ? "has-audio" : ""}" data-id="${a.id}"><button class="asset-card cat-${a.category}" data-action="asset-open" data-id="${a.id}"
        aria-pressed="${String(this.selected === a.id)}" title="${where}">
      <span class="asset-thumb ${a.preview ? `is-${a.preview}` : ""}">${thumb}<span class="asset-ext">${a.ext}</span>
        ${a.duration ? html`<span class="asset-dur">${clockTime(a.duration)}</span>` : ""}</span>
      <span class="asset-text">
        <span class="asset-name">${a.name}</span>
        <span class="asset-pack">${a.pack}</span>
        <span class="asset-meta"><span>${label1(a.category)}</span><span>${bytes(a.size)}</span></span>
      </span>
    </button>${audio ? html`<button class="asset-play" data-action="asset-play" data-id="${a.id}" aria-pressed="false"
      aria-label="Play ${a.name}"><svg viewBox="0 0 24 24" aria-hidden="true"><path class="i-play" d="M8 5v14l11-7z"/><path class="i-pause" d="M7 5h4v14H7zM13 5h4v14h-4z"/></svg></button>` : ""}</li>`;
  }

  private async open(id: number, focus = true): Promise<void> {
    const opening = this.selected !== id;
    this.selected = id;
    const panel = this.el.querySelector<HTMLElement>("#asset-detail")!;
    panel.hidden = false;
    this.el.classList.add("detail-open");
    if (opening) this.renderGrid(true);
    try {
      const d = await this.api.detail(id);
      if (this.selected !== id) return;
      if (!d.preview && d.previewState === null && PREVIEWABLE.has(d.ext)) void this.api.wantPreviews([id]).catch(() => undefined);

      mount(panel, this.renderDetail(d));
      if (focus && opening) panel.querySelector<HTMLElement>("h2")?.focus();
    } catch (e) {
      toast(e instanceof Error ? e.message : String(e));
    }
  }

  private closeDetail(): void {
    const id = this.selected;
    this.selected = undefined;
    const panel = this.el.querySelector<HTMLElement>("#asset-detail")!;
    panel.hidden = true;
    this.el.classList.remove("detail-open");
    this.renderGrid(true);
    if (id !== undefined) this.el.querySelector<HTMLElement>(`.asset-card[data-id="${id}"]`)?.focus();
  }

  private detailPreview(d: AssetDetail): SafeHtml {
    if (d.preview === "image") {
      return html`<div class="asset-preview is-image"><img src="${this.api.previewUrl(d, "l")}" alt="Preview of ${d.name}"></div>`;
    }
    if (d.preview === "audio") {
      return html`<div class="asset-preview is-audio">${d.peaks ? waveform(d.peaks) : ""}
        <audio controls preload="none" src="${this.api.previewUrl(d, "a")}" aria-label="Play ${d.name}"></audio></div>`;
    }
    const why = d.previewState === "none" || d.previewState === "error"
      ? `No preview: ${d.previewError ?? "this file can't be read"}`
      : PREVIEWABLE.has(d.ext) && !d.container.endsWith(".unitypackage") ? "Making the preview…" : "No preview for this kind of file yet";
    return html`<div class="asset-preview cat-${d.category}">${glyph(d.category)}<span class="hint">${why}</span></div>`;
  }

  private renderDetail(d: AssetDetail): SafeHtml {
    const where = d.container ? `${d.container} › ${d.path}` : d.path;
    return html`
      <div class="detail-head">
        <h2 tabindex="-1">${d.name}</h2>
        <button class="icon-btn" data-action="asset-close" aria-label="Close details">${icon("close")}</button>
      </div>
      ${this.detailPreview(d)}
      ${d.missingSince ? html`<p class="warn">Gone from the library since ${relTime(d.missingSince)}.</p>` : ""}
      <dl class="asset-facts">
        <dt>Where</dt><dd><code>${where}</code></dd>
        <dt>Pack</dt><dd><button class="link" data-action="asset-pack" data-pack="${d.packId}">${d.pack}</button></dd>
        <dt>Licence</dt><dd><span class="chip ship-no">Not cleared to ship</span>
          <small class="muted">No licence linked to this pack yet.</small></dd>
        <dt>Category</dt><dd>${label1(d.category)} <small class="muted">(${d.rule})</small></dd>
        <dt>Size</dt><dd>${bytes(d.size)} <small class="muted">.${d.ext}${d.width && d.height
          ? ` · ${d.width} × ${d.height} px${d.hasAlpha ? ", transparent" : ""}` : ""}</small></dd>
        ${d.duration ? html`<dt>Length</dt><dd>${clockTime(d.duration)} <small class="muted">${[
          d.sampleRate ? `${(d.sampleRate / 1000).toFixed(1)} kHz` : "", d.channels === 1 ? "mono" : d.channels === 2 ? "stereo" : d.channels ? `${d.channels} channels` : "",
        ].filter(Boolean).join(" · ")}${d.duration > 30 ? " · the preview plays the first 30 s" : ""}</small></dd>` : ""}
      </dl>
      ${d.packDocs.length ? html`
        <h3 class="label">Licence and readme files in this pack</h3>
        <ul class="asset-docs">${d.packDocs.map((x) => html`<li><button class="link" data-action="asset-open" data-id="${x.id}">${x.path}</button></li>`)}</ul>` : ""}
      ${d.copies.length ? html`
        <h3 class="label">${d.dupOf ? "Same file elsewhere" : "Copies (hidden from the grid)"}</h3>
        <ul class="asset-docs">${d.copies.map((x) => html`<li><button class="link" data-action="asset-open" data-id="${x.id}">${x.container ? `${x.container} › ` : ""}${x.path}</button></li>`)}</ul>` : ""}`;
  }
}

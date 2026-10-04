// Games tab of the Assets section: per game a profile and a needs list, and per need the
// AI's picks (each with its reason) that Kees keeps as candidates or rejects. Everything
// arrives live over the server events ("games"); nothing needs a reload.
import { html, mount, onAction, restoreBusy, type SafeHtml } from "../core/html";
import type { AssetsApi, Game, GameDetail, GameProfile, Licence, Need, GamePick } from "../api/assets";
import { glyph, label1, toast, waveform } from "./assets";
import { relTime } from "../core/time";

/** "OK to ship" and friends, for a pick's licence in a game that is or isn't sold. */
export function shipBadge(licence: Licence | null, commercial: boolean): SafeHtml {
  if (!licence) return html`<span class="chip ship-no" title="Link a licence to this pack in the Library tab">Licence not linked</span>`;
  if (commercial && !licence.commercial) return html`<span class="chip ship-no" title="${licence.name}">Not for a sold game</span>`;
  return html`<span class="chip ship-ok" title="${licence.name}">OK to ship${licence.attribution ? " · credit" : ""}</span>`;
}

export class GamesView {
  private games: Game[] = [];
  private selected?: number;
  private detail?: GameDetail;
  private gen = 0; // stale answers are dropped
  private started = false;
  private reloadTimer = 0;

  constructor(
    private el: HTMLElement,
    private api: AssetsApi,
    private categories: () => string[],
    private openAsset: (id: number) => void,
  ) {}

  show(): void {
    if (!this.started) {
      this.started = true;
      this.wire();
    }
    void this.loadList();
  }

  private wire(): void {
    onAction(this.el, {
      "game-select": (b) => this.select(Number(b.dataset.id)),
      "game-draft": () => this.run(() => this.api.draftProfile(this.selected!)),
      "game-pick-all": () => this.run(() => this.api.pickAll(this.selected!)),
      "game-remove": () => this.removeGame(),
      "need-pick": (b) => this.pickNeed(Number(b.dataset.need)),
      "need-remove": (b) => this.removeNeed(Number(b.dataset.need)),
      "pick-keep": (b) => this.setPick(Number(b.dataset.need), Number(b.dataset.asset), "candidate"),
      "pick-reject": (b) => this.setPick(Number(b.dataset.need), Number(b.dataset.asset), "rejected"),
      "pick-undo": (b) => this.undoPick(Number(b.dataset.need), Number(b.dataset.asset)),
      "pick-open": (b) => this.openAsset(Number(b.dataset.asset)),
    });
    this.el.addEventListener("input", (ev) => {
      const form = (ev.target as HTMLElement).closest("form");
      if (form) form.dataset.dirty = "1";
    });
    this.el.addEventListener("submit", (ev) => {
      const form = ev.target as HTMLFormElement;
      ev.preventDefault();
      if (form.id === "game-add") void this.addGame(form);
      else if (form.id === "game-profile") void this.saveProfile(form);
      else if (form.id === "need-add") void this.addNeed(form);
    });
  }

  /** A live event: this game changed (or a draft failed). */
  onGames(ev: { game: number; error?: string }): void {
    if (!this.started) return;
    if (ev.error && ev.game === this.selected) toast(ev.error);
    // Coalesce bursts (a pick run sends one event per step).
    if (this.reloadTimer) return;
    this.reloadTimer = window.setTimeout(() => {
      this.reloadTimer = 0;
      void this.loadList();
    }, 120);
  }

  private async run(work: () => Promise<void>): Promise<void> {
    try {
      await work();
    } catch (e) {
      toast(e instanceof Error ? e.message : String(e));
    }
  }

  private async loadList(): Promise<void> {
    const gen = ++this.gen;
    try {
      const games = await this.api.games();
      if (gen !== this.gen) return;
      this.games = games;
      if (this.selected === undefined || !games.some((g) => g.id === this.selected)) this.selected = games[0]?.id;
      const detail = this.selected === undefined ? undefined : await this.api.game(this.selected);
      if (gen !== this.gen) return;
      this.detail = detail;
      this.render();
    } catch (e) {
      if (gen === this.gen) toast(e instanceof Error ? e.message : String(e));
    }
  }

  private async select(id: number): Promise<void> {
    if (id === this.selected) return;
    this.selected = id;
    this.detail = undefined;
    this.render();
    await this.loadList();
    this.el.querySelector<HTMLElement>(".game-main h2")?.focus();
  }

  private render(): void {
    // Keep what someone is typing across live re-renders: forms they changed keep their
    // values, and the focused field keeps the focus.
    type Field = HTMLInputElement | HTMLSelectElement;
    const fields = (f: HTMLFormElement) => [...f.elements].filter((e): e is Field => e instanceof HTMLInputElement || e instanceof HTMLSelectElement);
    const dirty = [...this.el.querySelectorAll<HTMLFormElement>("form[data-dirty]")].map((f) => ({
      id: f.id, values: fields(f).map((e) => [e.name, e instanceof HTMLInputElement && e.type === "checkbox" ? e.checked : e.value] as const),
    }));
    const active = document.activeElement;
    const focus = (active instanceof HTMLInputElement || active instanceof HTMLSelectElement) && active.form && this.el.contains(active)
      ? { form: active.form.id, name: active.name } : undefined;
    mount(this.el, html`
      <nav class="game-list" aria-label="Games">
        <ul>${this.games.map((g) => html`<li><button class="game-item" data-action="game-select" data-id="${g.id}"
          aria-current="${g.id === this.selected ? "true" : "false"}">
          <span class="game-name">${g.name}</span>
          <small class="muted">${g.source === "own" ? "added by you" : g.source}${g.missingSince ? " · gone from its repo" : ""}${g.needs ? ` · ${g.needs} needs` : ""}${g.candidates ? ` · ${g.candidates} kept` : ""}</small></button></li>`)}</ul>
        <form id="game-add" class="game-add">
          <input name="name" maxlength="80" placeholder="Add a game" aria-label="New game name" autocomplete="off">
          <button class="btn" type="submit">Add</button></form>
      </nav>
      <section class="game-main">${this.renderGame()}</section>`);
    for (const d of dirty) {
      const f = this.el.querySelector<HTMLFormElement>(`#${d.id}`);
      if (!f) continue;
      f.dataset.dirty = "1";
      for (const [name, v] of d.values) {
        const e = f.elements.namedItem(name);
        if (e instanceof HTMLInputElement && e.type === "checkbox") e.checked = v === true;
        else if (e instanceof HTMLInputElement || e instanceof HTMLSelectElement) e.value = String(v);
      }
    }
    if (focus) this.el.querySelector<HTMLElement>(`#${focus.form} [name="${focus.name}"]`)?.focus();
    restoreBusy(this.el);
  }

  private renderGame(): SafeHtml {
    const g = this.detail;
    if (!this.games.length) return html`<p class="muted game-empty">No games yet. Add one, or mount a game repo (GAME_REPOS).</p>`;
    if (!g) return html`<p class="muted game-empty">Loading…</p>`;
    const cats = this.categories().filter((c) => c !== "junk");
    const kept = g.needs.reduce((n, x) => n + x.picks.filter((p) => p.status === "candidate").length, 0);
    return html`
      <header class="game-head">
        <h2 tabindex="-1">${g.name}</h2>
        <span class="chip">${g.source === "own" ? "Added by you" : g.source}</span>
        ${g.source === "own" || g.missingSince ? html`<button class="btn subtle" data-action="game-remove">Remove</button>` : ""}
      </header>
      ${g.about ? html`<p class="muted">${g.about}</p>` : ""}
      ${g.profileBy === "ai" ? html`<p class="game-note">The AI drafted this profile and the needs marked AI. Check them and save to confirm.</p>` : ""}
      <form id="game-profile" class="game-profile">
        <label>Genre <input name="genre" maxlength="60" value="${g.genre}" placeholder="Exploration"></label>
        <label>Art style <input name="artStyle" maxlength="60" value="${g.artStyle}" placeholder="Low-poly"></label>
        <label>Setting <input name="setting" maxlength="60" value="${g.setting}" placeholder="Forest"></label>
        <label class="asset-check"><input type="checkbox" name="commercial" ${g.commercial ? "checked" : ""}> Will be sold
          <small class="muted">(only packs whose licence allows it)</small></label>
        <div class="row">
          <button class="btn primary" type="submit">${g.profileBy === "kees" ? "Save" : "Save and confirm"}</button>
          ${g.aiOn ? html`<button class="btn" type="button" data-action="game-draft" ${g.drafting ? "disabled" : ""}>
            ${g.drafting ? "Drafting…" : "Draft from the game's docs"}</button>` : html`<small class="muted">Turn AI tagging on for drafts and picks.</small>`}
        </div>
      </form>
      ${g.styleWarning ? html`<p class="game-warn" role="status">${g.styleWarning}</p>` : ""}
      <div class="needs-head">
        <h3>Needs <small class="muted">${g.needs.length} · ${kept} kept</small></h3>
        ${g.aiOn && g.needs.length ? html`<button class="btn" data-action="game-pick-all">Find assets for new needs</button>` : ""}
      </div>
      <ol class="needs">${g.needs.map((n) => this.renderNeed(n, g))}</ol>
      <form id="need-add" class="need-add">
        <input name="text" maxlength="120" placeholder="What does the game need? e.g. footsteps on grass" aria-label="New need" autocomplete="off">
        <select name="category" aria-label="Category of the need"><option value="">Any category</option>
          ${cats.map((c) => html`<option value="${c}">${label1(c)}</option>`)}</select>
        <button class="btn" type="submit">Add</button></form>`;
  }

  private renderNeed(n: Need, g: GameDetail): SafeHtml {
    const state = n.state === "queued" ? "Waiting…" : n.state === "picking" ? "Searching and picking…" : "";
    const shown = n.picks.filter((p) => p.status !== "rejected");
    const rejected = n.picks.length - shown.length;
    return html`<li class="need" data-need="${n.id}">
      <div class="need-line">
        <span class="need-text">${n.text}</span>
        ${n.category ? html`<span class="chip">${label1(n.category)}</span>` : ""}
        ${n.by === "ai" ? html`<span class="chip ai-chip" title="Drafted by the AI">AI</span>` : ""}
        <span class="need-state muted" aria-live="polite">${state}</span>
        <span class="need-actions">
          ${g.aiOn ? html`<button class="btn small" data-action="need-pick" data-need="${n.id}" ${n.state ? "disabled" : ""}>${n.pickedAt ? "Pick again" : "Find assets"}</button>` : ""}
          <button class="icon-btn tag-x" data-action="need-remove" data-need="${n.id}" aria-label="Remove the need ${n.text}">×</button>
        </span>
      </div>
      ${n.error ? html`<p class="warn">${n.error}</p>` : ""}
      ${n.pickedAt && !shown.length && !n.state ? html`<p class="muted">Nothing in the library fits. Try other words or another category.</p>` : ""}
      ${shown.length ? html`<ul class="picks">${shown.map((p) => this.renderPick(n, p, g.commercial))}</ul>` : ""}
      ${rejected ? html`<p class="muted small">${rejected} rejected (never suggested again)</p>` : ""}
      ${n.pickedAt ? html`<p class="muted small">Picked ${relTime(n.pickedAt)}</p>` : ""}
    </li>`;
  }

  private renderPick(n: Need, p: GamePick, commercial: boolean): SafeHtml {
    const a = p.asset;
    const thumb = a.preview === "image"
      ? html`<img src="${this.api.previewUrl(a, "t")}" alt="" loading="lazy">`
      : a.peaks ? html`<span class="pick-wave">${waveform(a.peaks)}</span>` : html`<span class="pick-glyph">${glyph(a.category)}</span>`;
    return html`<li class="pick ${p.status}">
      <button class="pick-thumb" data-action="pick-open" data-asset="${a.id}" aria-label="Open ${a.name} in the library">${thumb}</button>
      <div class="pick-body">
        <div class="pick-name"><strong>${a.name}</strong> <small class="muted">${a.pack}</small></div>
        ${p.reason ? html`<p class="pick-reason">${p.reason}</p>` : ""}
        <div class="row">
          ${shipBadge(a.licence, commercial)}
          ${p.status === "candidate"
            ? html`<span class="chip kept">Kept</span><button class="btn small subtle" data-action="pick-undo" data-need="${n.id}" data-asset="${a.id}">Undo</button>`
            : html`<button class="btn small" data-action="pick-keep" data-need="${n.id}" data-asset="${a.id}">Keep</button>
              <button class="btn small subtle" data-action="pick-reject" data-need="${n.id}" data-asset="${a.id}">Reject</button>`}
        </div>
      </div></li>`;
  }

  /** Changes one pick here first (optimistic), then on the server; undone on failure. */
  private patch(need: number, asset: number, change: (p: GamePick) => GamePick | undefined): GameDetail | undefined {
    const before = this.detail;
    if (!before) return undefined;
    this.detail = {
      ...before,
      needs: before.needs.map((n) => n.id !== need ? n : {
        ...n, picks: n.picks.flatMap((p) => { if (p.asset.id !== asset) return [p]; const q = change(p); return q ? [q] : []; }),
      }),
    };
    this.render();
    return before;
  }

  private async setPick(need: number, asset: number, status: "candidate" | "rejected"): Promise<void> {
    const before = this.patch(need, asset, (p) => ({ ...p, status, by: "kees" }));
    try {
      await this.api.setPick(need, asset, status);
    } catch (e) {
      if (before) { this.detail = before; this.render(); }
      toast(e instanceof Error ? e.message : String(e));
    }
  }

  /** "Undo" on a kept pick: back to a suggestion if the AI made it, else gone. */
  private async undoPick(need: number, asset: number): Promise<void> {
    const before = this.patch(need, asset, () => undefined);
    try {
      await this.api.removePick(need, asset);
    } catch (e) {
      if (before) { this.detail = before; this.render(); }
      toast(e instanceof Error ? e.message : String(e));
    }
  }

  private async pickNeed(need: number): Promise<void> {
    if (this.detail) {
      this.detail = { ...this.detail, needs: this.detail.needs.map((n) => (n.id === need ? { ...n, state: "queued", error: null } : n)) };
      this.render();
    }
    await this.run(() => this.api.pickNeed(need));
  }

  private async removeNeed(need: number): Promise<void> {
    const before = this.detail;
    if (before) { this.detail = { ...before, needs: before.needs.filter((n) => n.id !== need) }; this.render(); }
    try {
      await this.api.removeNeed(need);
    } catch (e) {
      if (before) { this.detail = before; this.render(); }
      toast(e instanceof Error ? e.message : String(e));
    }
  }

  private async addNeed(form: HTMLFormElement): Promise<void> {
    if (this.selected === undefined) return;
    const text = (form.elements.namedItem("text") as HTMLInputElement).value.trim();
    const category = (form.elements.namedItem("category") as HTMLSelectElement).value || null;
    if (!text) return;
    const before = this.detail;
    if (before) {
      const temp: Need = { id: -Date.now(), text, category, by: "kees", pickedAt: null, error: null, state: null, picks: [] };
      this.detail = { ...before, needs: [...before.needs, temp] };
    }
    form.reset();
    delete form.dataset.dirty;
    this.render();
    try {
      await this.api.addNeed(this.selected, text, category);
    } catch (e) {
      if (before) { this.detail = before; this.render(); }
      toast(e instanceof Error ? e.message : String(e));
    }
  }

  private async saveProfile(form: HTMLFormElement): Promise<void> {
    if (this.selected === undefined) return;
    const val = (k: string) => (form.elements.namedItem(k) as HTMLInputElement).value;
    const p: GameProfile = { genre: val("genre"), artStyle: val("artStyle"), setting: val("setting"), commercial: (form.elements.namedItem("commercial") as HTMLInputElement).checked };
    delete form.dataset.dirty;
    const before = this.detail;
    if (before) { this.detail = { ...before, ...p, profileBy: "kees", needs: before.needs.map((n) => ({ ...n, by: "kees" })) }; this.render(); }
    try {
      await this.api.setProfile(this.selected, p);
    } catch (e) {
      if (before) { this.detail = before; this.render(); }
      toast(e instanceof Error ? e.message : String(e));
    }
  }

  private async addGame(form: HTMLFormElement): Promise<void> {
    const input = form.elements.namedItem("name") as HTMLInputElement;
    const name = input.value.trim();
    if (!name) return;
    input.value = "";
    delete form.dataset.dirty;
    try {
      const g = await this.api.addGame(name);
      this.selected = g.id;
      await this.loadList();
    } catch (e) {
      input.value = name;
      toast(e instanceof Error ? e.message : String(e));
    }
  }

  private async removeGame(): Promise<void> {
    const g = this.detail;
    if (!g || !confirm(`Remove ${g.name} with its needs and picks?`)) return;
    try {
      await this.api.removeGame(g.id);
      this.selected = undefined;
      await this.loadList();
    } catch (e) {
      toast(e instanceof Error ? e.message : String(e));
    }
  }
}

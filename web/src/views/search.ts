import { modal } from "../core/modal";
import { fuzzyMatch } from "../core/fuzzy";
import type { SearchResult } from "../api/types";

export interface SearchOptions {
  search(q: string): Promise<SearchResult[]>;
  settings: string[];
  go(r: SearchResult): void;
}

/** The global search palette (Ctrl+K, or the Search field in the header): settings match
 *  here, everything else on the server. Hits are marked with <mark>; text is never HTML. */
export function openSearch(o: SearchOptions, opener?: HTMLElement | null): void {
  const backdrop = document.querySelector(".search-backdrop");
  if (backdrop) {
    backdrop.querySelector<HTMLInputElement>("#search-input")?.focus();
    return;
  }

  const container = document.createElement("div");
  container.className = "search-backdrop";
  container.innerHTML = `
    <div class="search-box" role="dialog" aria-modal="true" aria-label="Search">
      <input id="search-input" type="search" placeholder="Search tasks, chats, projects, settings…" autocomplete="off" role="combobox" aria-expanded="false" aria-controls="search-results" aria-autocomplete="list">
      <ul id="search-results" role="listbox"></ul>
      <p class="search-hint muted small">↑ ↓ to move · Enter to open · Esc to close</p>
    </div>
  `;
  document.body.appendChild(container);

  const m = modal(container, () => container.remove());
  const input = container.querySelector("#search-input") as HTMLInputElement;
  const list = container.querySelector("#search-results") as HTMLUListElement;
  let seq = 0;
  let activeIndex = -1;

  let shown: SearchResult[] = [];

  const query = async (): Promise<void> => {
    const q = input.value.trim();
    if (!q) {
      seq++;
      list.replaceChildren();
      shown = [];
      input.setAttribute("aria-expanded", "false");
      return;
    }
    input.setAttribute("aria-expanded", "true");
    const now = ++seq;
    const hits = o.settings.filter((s) => fuzzyMatch(q, s)).slice(0, 3).map((name) => ({ kind: "setting" as const, id: name, parent: null, title: name, snippet: "" }));
    try {
      const results = await o.search(q);
      if (now !== seq) return;
      list.replaceChildren();
      const all = [...hits, ...results];
      shown = all;
      if (shown.length === 0) {
        const empty = document.createElement("li");
        empty.className = "search-empty";
        empty.textContent = `Nothing found for "${q}"`;
        list.appendChild(empty);
        return;
      }
      shown.forEach((r, i) => {
        const li = document.createElement("li");
        li.className = "search-item";
        li.id = `search-opt-${i}`;
        li.setAttribute("role", "option");
        li.setAttribute("data-index", String(i));
        const kind = document.createElement("span");
        kind.className = "search-kind";
        kind.textContent = r.kind === "setting" ? "Setting" : r.kind.charAt(0).toUpperCase() + r.kind.slice(1);
        const title = document.createElement("span");
        title.className = "search-title";
        title.textContent = r.title;
        const snippetSpan = document.createElement("span");
        snippetSpan.className = "search-snippet";
        if (r.snippet) {
          const parts = r.snippet.split("\u0002");
          const first = document.createTextNode(parts[0]);
          snippetSpan.appendChild(first);
          for (let j = 1; j < parts.length; j++) {
            const chunk = parts[j].split("\u0003");
            const mark = document.createElement("mark");
            mark.textContent = chunk[0];
            const rest = document.createTextNode(chunk[1] || "");
            snippetSpan.appendChild(mark);
            snippetSpan.appendChild(rest);
          }
        }
        li.appendChild(kind);
        li.appendChild(title);
        li.appendChild(snippetSpan);
        list.appendChild(li);
      });
      activeIndex = 0;
      updateActive();
    } catch {
      if (now !== seq) return;
      shown = [];
      list.replaceChildren();
      const empty = document.createElement("li");
      empty.className = "search-empty";
      empty.textContent = "Search failed. Try again.";
      list.appendChild(empty);
    }
  };

  const debounce = (fn: () => void, ms: number): (() => void) => {
    let t: ReturnType<typeof setTimeout>;
    return () => { clearTimeout(t); t = setTimeout(fn, ms); };
  };
  const debouncedQuery = debounce(query, 60);
  input.addEventListener("input", debouncedQuery);

  const updateActive = (): void => {
    const items = Array.from(list.querySelectorAll("li.search-item"));
    if (items.length === 0) return;
    items.forEach((item) => item.setAttribute("aria-selected", "false"));
    const active = items[activeIndex < shown.length ? activeIndex : 0];
    if (active) {
      active.setAttribute("aria-selected", "true");
      input.setAttribute("aria-activedescendant", active.id);
      active.scrollIntoView({ block: "nearest" });
    }
  };

  input.addEventListener("keydown", (e) => {
    if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation(); // the app's own Escape would close Settings under the palette too
      m.requestClose();
      return;
    }
    if (e.key === "ArrowDown") {
      e.preventDefault();
      if (shown.length === 0) return;
      activeIndex = (activeIndex + 1) % shown.length;
      updateActive();
      return;
    }
    if (e.key === "ArrowUp") {
      e.preventDefault();
      if (shown.length === 0) return;
      activeIndex = activeIndex > 0 ? activeIndex - 1 : shown.length - 1;
      updateActive();
      return;
    }
    if (e.key === "Enter") {
      e.preventDefault();
      if (shown.length === 0) return;
      const idx = activeIndex < 0 ? 0 : activeIndex;
      const item = shown[idx];
      m.requestClose();
      o.go(item);
    }
  });

  list.addEventListener("pointermove", (e) => {
    const target = (e.target as HTMLElement).closest<HTMLElement>("li.search-item");
    if (target) {
      const index = Number(target.dataset.index);
      activeIndex = index;
      updateActive();
    }
  });

  list.addEventListener("click", (e) => {
    const target = (e.target as HTMLElement).closest<HTMLElement>("li.search-item");
    if (target) {
      const index = Number(target.dataset.index);
      if (index >= 0 && index < shown.length) {
        m.requestClose();
        o.go(shown[index]);
      }
    }
  });

  m.open(opener);
  input.focus();
}

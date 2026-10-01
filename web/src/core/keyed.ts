// Keyed list update, the vanilla version of React/Vue keys: one DOM node per
// item id, re-rendered only when the item object changed, moved only when the
// order changed.
import { fragment, type SafeHtml } from "./html";

export class KeyedList<T extends { id: string }> {
  private nodes = new Map<string, { item: T; el: HTMLElement }>();

  constructor(
    private container: HTMLElement,
    private render: (item: T) => SafeHtml,
    private after?: (el: HTMLElement, item: T) => void,
  ) {}

  update(items: T[]): void {
    const seen = new Set<string>();
    let cursor: ChildNode | null = this.container.firstChild;
    for (const item of items) {
      seen.add(item.id);
      let entry = this.nodes.get(item.id);
      if (!entry || entry.item !== item) {
        const el = fragment(this.render(item));
        this.after?.(el, item);
        if (entry) {
          if (entry.el === cursor) cursor = el;
          entry.el.replaceWith(el);
        }
        entry = { item, el };
        this.nodes.set(item.id, entry);
      }
      if (entry.el !== cursor) this.container.insertBefore(entry.el, cursor);
      cursor = entry.el.nextSibling;
    }
    for (const [id, entry] of this.nodes) {
      if (!seen.has(id)) {
        entry.el.remove();
        this.nodes.delete(id);
      }
    }
  }

  clear(): void {
    this.nodes.clear();
    this.container.replaceChildren();
  }
}

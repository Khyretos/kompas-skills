// Tiny HTML templating. Every interpolated value is escaped unless it is
// already SafeHtml (the result of another html`` call). Untrusted rich text
// (model markdown) never goes through here; it uses core/markdown.ts.

export class SafeHtml {
  constructor(readonly value: string) {}
  toString(): string {
    return this.value;
  }
}

type Value = SafeHtml | string | number | boolean | null | undefined | Value[];

export function esc(text: string): string {
  return text
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;");
}

function render(value: Value): string {
  if (value === null || value === undefined || value === false) return "";
  if (Array.isArray(value)) return value.map(render).join("");
  if (value instanceof SafeHtml) return value.value;
  return esc(String(value));
}

export function html(strings: TemplateStringsArray, ...values: Value[]): SafeHtml {
  let out = strings[0];
  values.forEach((v, i) => {
    out += render(v) + strings[i + 1];
  });
  return new SafeHtml(out);
}

// Trusted Types: with the CSP `require-trusted-types-for 'script'`, the
// browser refuses raw strings in innerHTML. Our templates are escaped by
// construction, so this policy is the one place that vouches for them.
type TTPolicy = { createHTML(s: string): unknown };
const tt = (globalThis as { trustedTypes?: { createPolicy(n: string, r: { createHTML(s: string): string }): TTPolicy } }).trustedTypes;
const policy: TTPolicy | undefined = tt?.createPolicy("app", { createHTML: (s) => s });

/** Replace an element's content with a template. */
export function mount(el: Element, content: SafeHtml): void {
  (el as { innerHTML: unknown }).innerHTML = policy ? policy.createHTML(content.value) : content.value;
}

/** Build a detached element from a template with one root element. */
export function fragment(content: SafeHtml): HTMLElement {
  const t = document.createElement("template");
  mount(t, content);
  return t.content.firstElementChild as HTMLElement;
}

export function $(selector: string, root: ParentNode = document): HTMLElement {
  const el = root.querySelector<HTMLElement>(selector);
  if (!el) throw new Error(`Missing element: ${selector}`);
  return el;
}

/**
 * Event delegation: one listener on `root` handles clicks on any element with
 * data-action="name", now or added later by a re-render.
 */
export function onAction(
  root: HTMLElement,
  handlers: Record<string, (el: HTMLElement, ev: Event) => void>,
): void {
  root.addEventListener("click", (ev) => {
    const el = (ev.target as HTMLElement).closest<HTMLElement>("[data-action]");
    if (!el || !root.contains(el)) return;
    const handler = handlers[el.dataset.action ?? ""];
    if (handler) {
      ev.preventDefault();
      handler(el, ev);
    }
  });
}

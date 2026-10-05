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

// Actions still running, by a key built from the element's data attributes, so
// the busy state survives a re-render that replaces the element.
const busy = new Set<string>();
const busyKey = (el: HTMLElement) => JSON.stringify(Object.entries(el.dataset).sort());

function markBusy(el: HTMLElement, on: boolean): void {
  if (on) el.setAttribute("aria-busy", "true");
  else el.removeAttribute("aria-busy");
  if (el instanceof HTMLButtonElement) el.disabled = on;
}

/**
 * Event delegation: one listener on `root` handles clicks on any element with
 * data-action="name", now or added later by a re-render.
 */
export function onAction(
  root: HTMLElement,
  handlers: Record<string, (el: HTMLElement, ev: Event) => void | Promise<unknown>>,
): void {
  const run = (el: HTMLElement, ev: Event): void => {
    if (!root.contains(el) || el.getAttribute("aria-busy") === "true") return;
    const handler = handlers[el.dataset.action ?? ""];
    if (!handler) return;
    ev.preventDefault();
    const result = handler(el, ev);
    if (!(result instanceof Promise)) return;
    const key = busyKey(el);
    busy.add(key);
    markBusy(el, true);
    result.catch(() => undefined).finally(() => {
      busy.delete(key);
      for (const x of root.querySelectorAll<HTMLElement>("[aria-busy]")) if (busyKey(x) === key) markBusy(x, false);
    });
  };
  // A live re-render (a step landing, a timer) can replace the element between press and
  // release; the browser then sends no click. Run the action on release when the element
  // under the pointer is the re-rendered twin of the one pressed (same action, id and data-id).
  const twin = (el: HTMLElement) => `${el.dataset.action}|${el.id}|${el.dataset.id ?? ""}`;
  let pressed: { el: HTMLElement; key: string } | null = null;
  root.addEventListener("pointerdown", (ev) => {
    const el = (ev.target as HTMLElement).closest<HTMLElement>("[data-action]");
    pressed = el && ev.button === 0 ? { el, key: twin(el) } : null;
  });
  root.addEventListener("pointerup", (ev) => {
    const p = pressed;
    pressed = null;
    if (!p || p.el.isConnected) return; // still there: the browser sends the click itself
    const el = (ev.target as HTMLElement).closest<HTMLElement>("[data-action]");
    if (el && twin(el) === p.key) run(el, ev);
  });
  root.addEventListener("click", (ev) => {
    const el = (ev.target as HTMLElement).closest<HTMLElement>("[data-action]");
    if (el) run(el, ev);
  });
}

/** Call after re-rendering: marks the new elements of actions that are still running. */
export function restoreBusy(root: HTMLElement): void {
  if (busy.size === 0) return;
  for (const el of root.querySelectorAll<HTMLElement>("[data-action]")) if (busy.has(busyKey(el))) markBusy(el, true);
}

/** While `work` runs, the form's buttons are disabled and its submit button shows
 *  the busy spinner (worker/web lesson 11). Returns `work`. */
export function busyWhile<T>(form: HTMLFormElement, work: Promise<T>): Promise<T> {
  const buttons = [...form.querySelectorAll<HTMLButtonElement>("button")];
  for (const b of buttons) {
    b.disabled = true;
    if (b.type === "submit") b.setAttribute("aria-busy", "true");
  }
  const done = () => {
    for (const b of buttons) {
      b.disabled = false;
      b.removeAttribute("aria-busy");
    }
  };
  work.then(done, done);
  return work;
}

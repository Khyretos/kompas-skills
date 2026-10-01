// A single observable store. Updates are immutable (new objects for changed
// items), so views can skip anything whose object identity didn't change.
// Listeners run at most once per animation frame, however many updates
// arrive (streamed tokens, task ticks): the batching React and OpenCode do.

type Listener<T> = (state: T, prev: T) => void;

export class Store<T extends object> {
  private listeners = new Set<Listener<T>>();
  private flushed: T;
  private pending = false;

  constructor(private state: T) {
    this.flushed = state;
  }

  get(): T {
    return this.state;
  }

  set(patch: Partial<T> | ((s: T) => Partial<T>)): void {
    const next = typeof patch === "function" ? patch(this.state) : patch;
    this.state = { ...this.state, ...next };
    if (this.pending) return;
    this.pending = true;
    const schedule = typeof requestAnimationFrame === "function" ? requestAnimationFrame : (f: () => void) => setTimeout(f, 16);
    schedule(() => this.flush());
  }

  /** Run listeners now (also used for the first render). */
  flush(): void {
    this.pending = false;
    const prev = this.flushed;
    this.flushed = this.state;
    this.listeners.forEach((l) => l(this.state, prev));
  }

  subscribe(listener: Listener<T>): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }
}

export function initResize(shell: HTMLElement): void {
  type Side = "left" | "right";
  const MIN = 260;

  const storageKey = "kk.layout";
  let layout: { left?: number; right?: number; leftCollapsed?: boolean; rightCollapsed?: boolean } = {};
  let savedLayout: typeof layout | null = null;

  function load(): void {
    try {
      const raw = localStorage.getItem(storageKey);
      if (!raw) return;
      savedLayout = JSON.parse(raw);
    } catch {
      savedLayout = null;
    }
  }

  function save(layout: typeof layout): void {
    try {
      localStorage.setItem(storageKey, JSON.stringify(layout));
    } catch {
      // ignore
    }
  }

  const panel = (side: Side): HTMLElement | null => shell.querySelector<HTMLElement>(side === "left" ? "#left" : "#right");
  const width = (side: Side): number => (shell.hasAttribute(`data-${side}-collapsed`) ? 0 : Math.round(panel(side)?.getBoundingClientRect().width ?? 0));

  function clamp(side: Side, px: number): number {
    const other: Side = side === "left" ? "right" : "left";
    const max = Math.min(Math.round(window.innerWidth * 0.45), window.innerWidth - width(other) - 420);
    return Math.round(Math.max(MIN, Math.min(max, px)));
  }

  function setWidth(side: Side, px: number): void {
    const w = clamp(side, px);
    const prop = `--${side}-size`;
    shell.style.removeProperty(prop);
    shell.style.setProperty(prop, `${w}px`);
    layout[side] = w;
    save(layout);
    updateAriaValues(side);
  }

  function setCollapsed(side: Side, on: boolean): void {
    const attr = `data-${side}-collapsed`;
    const prop = `--${side}-size`;
    if (on) {
      shell.setAttribute(attr, "true");
      shell.style.setProperty(prop, "0px");
    } else {
      shell.removeAttribute(attr);
      shell.style.removeProperty(prop);
    }
    layout[`${side}Collapsed`] = on;
    save(layout);
    updateCollapseBtn(side);
  }

  function makeHandle(side: Side): HTMLDivElement {
    const el = document.createElement("div");
    el.className = "resize-handle";
    el.dataset.side = side;
    el.setAttribute("role", "separator");
    el.setAttribute("aria-orientation", "vertical");
    el.setAttribute("tabindex", "0");
    el.setAttribute("aria-label", side === "left" ? "Resize the projects panel" : "Resize the tasks panel");
    return el;
  }

  function makeCollapse(side: Side): HTMLButtonElement {
    const el = document.createElement("button");
    el.type = "button";
    el.className = "collapse-btn";
    el.dataset.side = side;
    el.setAttribute("aria-expanded", "true");
    const textOpen = side === "left" ? "‹" : "›";
    const textClosed = side === "left" ? "›" : "‹";
    const labelHide = side === "left" ? "Hide the projects panel" : "Hide the tasks panel";
    const labelShow = side === "left" ? "Show the projects panel" : "Show the tasks panel";
    el.textContent = textOpen;
    el.setAttribute("aria-label", labelHide);
    return el;
  }

  function updateAriaValues(side: Side): void {
    const handle = shell.querySelector<HTMLDivElement>(`.resize-handle[data-side='${side}']`);
    if (!handle) return;
    const rect = handle.getBoundingClientRect();
    const min = MIN;
    const max = Math.round(window.innerWidth * 0.45);
    const now = side === "left"
      ? Math.round(rect.left)
      : Math.round(shell.offsetWidth - rect.right);
    handle.setAttribute("aria-valuemin", String(min));
    handle.setAttribute("aria-valuemax", String(max));
    handle.setAttribute("aria-valuenow", String(now));
  }

  function updateCollapseBtn(side: Side): void {
    const btn = shell.querySelector<HTMLButtonElement>(`.collapse-btn[data-side='${side}']`);
    if (!btn) return;
    const isCollapsed = shell.hasAttribute(`data-${side}-collapsed`);
    const textOpen = side === "left" ? "‹" : "›";
    const textClosed = side === "left" ? "›" : "‹";
    const labelHide = side === "left" ? "Hide the projects panel" : "Hide the tasks panel";
    const labelShow = side === "left" ? "Show the projects panel" : "Show the tasks panel";
    btn.textContent = isCollapsed ? textClosed : textOpen;
    btn.setAttribute("aria-expanded", String(!isCollapsed));
    btn.setAttribute("aria-label", isCollapsed ? labelShow : labelHide);
  }

  function applySavedLayout(): void {
    if (!savedLayout) return;
    if (typeof savedLayout.left === "number") setWidth("left", savedLayout.left);
    else delete layout.left;
    if (typeof savedLayout.right === "number") setWidth("right", savedLayout.right);
    else delete layout.right;
    if (typeof savedLayout.leftCollapsed === "boolean") setCollapsed("left", savedLayout.leftCollapsed);
    if (typeof savedLayout.rightCollapsed === "boolean") setCollapsed("right", savedLayout.rightCollapsed);
  }

  function handlePointerDown(e: PointerEvent): void {
    const handle = e.target as HTMLDivElement;
    if (!handle.classList.contains("resize-handle")) return;
    const side = handle.dataset.side as Side;
    shell.classList.add("dragging");
    handle.setPointerCapture(e.pointerId);
    const moveHandler = (ev: PointerEvent): void => {
      const rect = shell.getBoundingClientRect();
      const px = ev.clientX - rect.left;
      if (side === "left") {
        setWidth("left", px);
      } else {
        setWidth("right", rect.width - px);
      }
      updateAriaValues(side);
    };
    const upHandler = (): void => {
      shell.classList.remove("dragging");
      handle.releasePointerCapture(e.pointerId);
      shell.removeEventListener("pointermove", moveHandler);
      shell.removeEventListener("pointerup", upHandler);
      shell.removeEventListener("pointercancel", upHandler);
    };
    shell.addEventListener("pointermove", moveHandler);
    shell.addEventListener("pointerup", upHandler);
    shell.addEventListener("pointercancel", upHandler);
  }

  function handleKeyDown(e: KeyboardEvent): void {
    const handle = e.target as HTMLDivElement;
    if (!handle.classList.contains("resize-handle")) return;
    const side = handle.dataset.side as Side;
    const step = e.shiftKey ? 64 : 16;

    if (e.key === "ArrowRight" && side === "left") {
      e.preventDefault();
      setWidth("left", width("left") + step);
    } else if (e.key === "ArrowLeft" && side === "left") {
      e.preventDefault();
      setWidth("left", width("left") - step);
    } else if (e.key === "ArrowLeft" && side === "right") {
      e.preventDefault();
      setWidth("right", width("right") + step);
    } else if (e.key === "ArrowRight" && side === "right") {
      e.preventDefault();
      setWidth("right", width("right") - step);
    } else if (e.key === "Home") {
      e.preventDefault();
      setWidth(side, MIN);
    } else if (e.key === "End") {
      e.preventDefault();
      setWidth(side, Math.round(window.innerWidth * 0.45));
    } else if (e.key === "Enter") {
      e.preventDefault();
      setCollapsed(side, !shell.hasAttribute(`data-${side}-collapsed`));
    }
    updateAriaValues(side);
    save(layout);
  }

  function handleDblClick(e: MouseEvent): void {
    const handle = e.target as HTMLDivElement;
    if (handle.classList.contains("resize-handle")) {
      const side = handle.dataset.side as Side;
      shell.style.removeProperty(`--${side}-size`);
      delete layout[side];
      save(layout);
      updateAriaValues(side);
    }
  }

  function handleCollapseClick(e: MouseEvent): void {
    const btn = e.target as HTMLButtonElement;
    if (btn.classList.contains("collapse-btn")) {
      const side = btn.dataset.side as Side;
      setCollapsed(side, !shell.hasAttribute(`data-${side}-collapsed`));
    }
  }

  load();
  applySavedLayout();

  const handles = [makeHandle("left"), makeHandle("right")];
  const buttons = [makeCollapse("left"), makeCollapse("right")];

  shell.appendChild(handles[0]);
  shell.appendChild(buttons[0]);
  shell.appendChild(handles[1]);
  shell.appendChild(buttons[1]);

  handles.forEach((h) => {
    h.addEventListener("pointerdown", handlePointerDown);
    h.addEventListener("keydown", handleKeyDown);
    h.addEventListener("dblclick", handleDblClick);
  });

  buttons.forEach((b) => {
    b.addEventListener("click", handleCollapseClick);
  });

  window.addEventListener("resize", () => {
    if (savedLayout) {
      if (typeof savedLayout.left === "number" && !shell.hasAttribute("data-left-collapsed")) setWidth("left", savedLayout.left);
      if (typeof savedLayout.right === "number" && !shell.hasAttribute("data-right-collapsed")) setWidth("right", savedLayout.right);
    }
  });
}

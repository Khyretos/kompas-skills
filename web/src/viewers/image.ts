// Picture viewer: zoom and pan, a checkerboard behind transparency, pixel-exact zoom for
// small sprites and an optional frame grid for sprite sheets. (The Coder's draft never
// assigned its image or canvas and broke dragging; Claude rewrote it.)

export function mountImage(el: HTMLElement, url: string, alt: string): () => void {
  const abort = new AbortController();
  const signal = abort.signal;
  const make = <K extends keyof HTMLElementTagNameMap>(tag: K, cls: string, text = "") => {
    const e = document.createElement(tag);
    e.className = cls;
    if (text) e.textContent = text;
    return e;
  };
  const tools = make("div", "img-tools");
  const stage = make("div", "img-stage");
  stage.tabIndex = 0;
  stage.setAttribute("aria-label", "Picture: drag to move, scroll to zoom");
  const layer = make("div", "img-layer");
  const img = make("img", "");
  img.alt = alt;
  img.draggable = false;
  const grid = make("canvas", "img-grid");
  layer.append(img, grid);
  stage.append(layer);
  el.replaceChildren(tools, stage);

  const button = (text: string, title: string, f: () => void) => {
    const b = make("button", "btn small", text);
    b.type = "button";
    b.title = title;
    b.addEventListener("click", f, { signal });
    tools.append(b);
  };
  const zoomOut = make("output", "img-zoom");
  const sizeOut = make("output", "img-size");
  const pixel = make("label", "asset-check");
  const pixelBox = make("input", "img-pixel");
  pixelBox.type = "checkbox";
  pixel.append(pixelBox, " Pixelated");
  const frames = make("label", "img-frames", "Frames ");
  const num = (cls: string, label: string) => {
    const i = make("input", cls);
    Object.assign(i, { type: "number", min: "1", max: "64", value: "1" });
    i.setAttribute("aria-label", label);
    i.addEventListener("input", draw, { signal });
    return i;
  };
  const cols = num("img-cols", "Columns"), rows = num("img-rows", "Rows");
  frames.append(cols, " × ", rows);

  let x = 0, y = 0, z = 1;
  const w = () => img.naturalWidth || 1, h = () => img.naturalHeight || 1;
  const apply = () => {
    layer.style.transform = `translate(${x}px, ${y}px) scale(${z})`;
    zoomOut.textContent = `${Math.round(z * 100)} %`;
  };
  const zoomAt = (factor: number, px: number, py: number) => {
    const nz = Math.min(32, Math.max(0.05, z * factor));
    x = px - ((px - x) / z) * nz;
    y = py - ((py - y) / z) * nz;
    z = nz;
    apply();
  };
  const fit = () => {
    const sw = stage.clientWidth, sh = stage.clientHeight;
    z = Math.min(1, sw / w(), sh / h());
    x = (sw - w() * z) / 2;
    y = (sh - h() * z) / 2;
    apply();
  };
  const centre = (factor: number) => zoomAt(factor, stage.clientWidth / 2, stage.clientHeight / 2);
  button("Fit", "Fit the picture (0)", fit);
  button("1:1", "Actual pixels", () => { const sw = stage.clientWidth, sh = stage.clientHeight; z = 1; x = (sw - w()) / 2; y = (sh - h()) / 2; apply(); });
  button("−", "Zoom out (-)", () => centre(1 / 1.15));
  button("+", "Zoom in (+)", () => centre(1.15));
  tools.append(zoomOut, sizeOut, pixel, frames);

  function draw(): void {
    const c = Number(cols.value) || 1, r = Number(rows.value) || 1;
    grid.width = w();
    grid.height = h();
    const ctx = grid.getContext("2d");
    if (!ctx) return;
    ctx.clearRect(0, 0, grid.width, grid.height);
    if (c <= 1 && r <= 1) return;
    ctx.strokeStyle = "rgba(243, 148, 31, 0.9)";
    ctx.lineWidth = Math.max(1, w() / 512);
    ctx.beginPath();
    for (let i = 1; i < c; i++) { ctx.moveTo((i * w()) / c, 0); ctx.lineTo((i * w()) / c, h()); }
    for (let i = 1; i < r; i++) { ctx.moveTo(0, (i * h()) / r); ctx.lineTo(w(), (i * h()) / r); }
    ctx.stroke();
  }

  pixelBox.addEventListener("change", () => { img.style.imageRendering = pixelBox.checked ? "pixelated" : ""; }, { signal });
  img.addEventListener("load", () => {
    sizeOut.textContent = `${img.naturalWidth} × ${img.naturalHeight} px`;
    pixelBox.checked = img.naturalWidth <= 256;
    img.style.imageRendering = pixelBox.checked ? "pixelated" : "";
    fit();
    draw();
    stage.dataset.state = "ready";
  }, { signal });
  img.addEventListener("error", () => {
    stage.replaceChildren(make("p", "img-error", "This picture can't be shown in the browser."));
  }, { signal });
  img.src = url;

  stage.addEventListener("wheel", (e) => {
    e.preventDefault();
    const r = stage.getBoundingClientRect();
    zoomAt(e.deltaY < 0 ? 1.15 : 1 / 1.15, e.clientX - r.left, e.clientY - r.top);
  }, { passive: false, signal });
  let drag: { id: number; px: number; py: number } | undefined;
  stage.addEventListener("pointerdown", (e) => {
    if (e.button !== 0) return;
    drag = { id: e.pointerId, px: e.clientX, py: e.clientY };
    stage.setPointerCapture(e.pointerId);
  }, { signal });
  stage.addEventListener("pointermove", (e) => {
    if (!drag || e.pointerId !== drag.id) return;
    x += e.clientX - drag.px;
    y += e.clientY - drag.py;
    drag.px = e.clientX;
    drag.py = e.clientY;
    apply();
  }, { signal });
  const end = (e: PointerEvent) => { if (drag?.id === e.pointerId) drag = undefined; };
  stage.addEventListener("pointerup", end, { signal });
  stage.addEventListener("pointercancel", end, { signal });
  stage.addEventListener("keydown", (e) => {
    const moves: Record<string, [number, number]> = { ArrowLeft: [40, 0], ArrowRight: [-40, 0], ArrowUp: [0, 40], ArrowDown: [0, -40] };
    if (e.key === "+" || e.key === "=") centre(1.15);
    else if (e.key === "-") centre(1 / 1.15);
    else if (e.key === "0") fit();
    else if (moves[e.key]) { x += moves[e.key][0]; y += moves[e.key][1]; apply(); }
    else return;
    e.preventDefault();
  }, { signal });

  return () => {
    abort.abort();
    img.removeAttribute("src");
    el.replaceChildren();
  };
}

import { html, type SafeHtml } from "../core/html";

export interface TlSample { at: string; usedMib: number | null; reservedMib: number; watts: number | null }
export interface TlJob { id: string; kind: string; what: string; state: string; startedAt: string; endedAt: string | null; error: string | null }
export interface TlEvent { at: string; kind: string; detail: string }
export interface TlGpu { gpu: string; machine: string; hours: number; samples: TlSample[]; jobs: TlJob[]; events: TlEvent[] }

const n = (v: number): string => String(Math.round(v * 10) / 10);

function line(pts: [number, number][]): string {
  return pts.length < 2 ? "" : pts.map(([x, y], i) => `${i ? "L" : "M"} ${n(x)} ${n(y)}`).join(" ");
}

/** "12.3 GB VRAM, 168 W" over the shown range (the watts line is scaled to its own peak). */
function peakText(g: TlGpu): string {
  const vram = Math.max(0, ...g.samples.map((s) => s.usedMib ?? 0));
  const watts = Math.max(0, ...g.samples.map((s) => s.watts ?? 0));
  return `${(vram / 1024).toFixed(1)} GB VRAM${watts ? `, ${Math.round(watts)} W` : ""}`;
}

export function renderTimeline(list: TlGpu[] | undefined, hours: 1 | 24, totalMib: Record<string, number>, now = Date.now()): SafeHtml {
  const W = hours === 1 ? 720 : 1440;
  const H = 140;
  const start = now - hours * 3600_000;
  const x = (t: number) => ((t - start) / (hours * 3600_000)) * W;

  function localTime(t: number): string {
    const d = new Date(t);
    return `${d.getHours().toString().padStart(2, "0")}:${d.getMinutes().toString().padStart(2, "0")}`;
  }

  function vramY(mib: number, max: number): number {
    return 110 - (mib / max) * 100;
  }

  function wattsY(w: number, maxW: number): number {
    return 110 - (w / maxW) * 100;
  }

  if (!list || list.length === 0) {
    return html`<section class="gpu-timeline" aria-labelledby="tl-h">
      <h2 id="tl-h" class="label">GPU timeline</h2>
      <div class="seg" role="group" aria-label="Time range">
        <button data-action="gpu-range" data-hours="1" aria-pressed="false">1 h</button>
        <button data-action="gpu-range" data-hours="24" aria-pressed="false">24 h</button>
      </div>
      <p class="muted small">${list === undefined ? "Loading…" : "No GPUs configured."}</p>
    </section>`;
  }

  const activeHours = hours === 1 ? 1 : 24;
  const activeBtn = html`<button data-action="gpu-range" data-hours="${activeHours}" aria-pressed="true">${activeHours} h</button>`;
  const inactiveBtn = html`<button data-action="gpu-range" data-hours="${activeHours === 1 ? 24 : 1}" aria-pressed="false">${activeHours === 1 ? 24 : 1} h</button>`;

  return html`<section class="gpu-timeline" aria-labelledby="tl-h">
    <h2 id="tl-h" class="label">GPU timeline</h2>
    <div class="seg" role="group" aria-label="Time range">
      ${activeBtn} ${inactiveBtn}
    </div>
    ${list.map((g) => {
      const gpuTotal = totalMib[g.gpu] ?? 1;
      const maxUsed = Math.max(...g.samples.map((s) => s.usedMib ?? 0), ...g.samples.map((s) => s.reservedMib));
      const maxW = Math.max(...g.samples.map((s) => s.watts ?? 0), 1);

      const vramPts = g.samples.filter((s) => s.usedMib !== null).map((s): [number, number] => [x(Date.parse(s.at)), vramY(s.usedMib as number, gpuTotal)]);
      const reservedPts = g.samples.map((s): [number, number] => [x(Date.parse(s.at)), vramY(s.reservedMib, gpuTotal)]);
      const wattsPts = g.samples.filter((s) => s.watts !== null).map((s): [number, number] => [x(Date.parse(s.at)), wattsY(s.watts as number, maxW)]);

      const jobBars = g.jobs.map((j) => {
        const sx = x(Date.parse(j.startedAt));
        const ex = j.endedAt ? x(Date.parse(j.endedAt)) : W;
        const width = Math.max(ex - sx, 2);
        const y = 116;
        const height = 12;
        return html`<g>
          <title>${j.kind}: ${j.what} (${j.state}${j.error ? ", " + j.error : ""})</title>
          <rect class="tl-job kind-${j.kind} ${j.state}" x="${n(sx)}" y="${y}" width="${n(width)}" height="${height}" rx="2" />
        </g>`;
      });

      const eventLines = g.events.map((e) => {
        const lx = x(Date.parse(e.at));
        return html`<g>
          <title>${e.kind}: ${e.detail}</title>
          <line class="tl-event" x1="${n(lx)}" x2="${n(lx)}" y1="6" y2="128" />
        </g>`;
      });

      const ticks = [];
      const stepMs = activeHours === 1 ? 15 * 60 * 1000 : 3 * 3600 * 1000;
      for (let t = start; t <= now; t += stepMs) {
        ticks.push(html`<text class="tl-tick" x="${n(x(t))}" y="138">${localTime(t)}</text>`);
      }

      const vramPath = vramPts.length >= 2 ? line([[vramPts[0][0], 110], ...vramPts, [vramPts[vramPts.length - 1][0], 110]]) + " Z" : "";
      const reservedPath = reservedPts.length >= 2 ? line(reservedPts) : "";
      const wattsPath = wattsPts.length >= 2 ? line(wattsPts) : "";

      return html`<figure class="tl-gpu">
        <figcaption><strong>${g.gpu}</strong> <span class="muted small">${g.machine} · peak ${peakText(g)}</span></figcaption>
        <div class="tl-scroll">
          <svg class="tl-svg" viewBox="0 0 ${W} ${H}" width="${W}" height="${H}" role="img" aria-label="${g.gpu}: peak ${Math.round(maxUsed)} MB VRAM, ${g.jobs.length} jobs, ${g.events.length} events in the last ${activeHours} h">
            <path class="tl-vram" d="${vramPath}" />
            <path class="tl-reserved" d="${reservedPath}" />
            <path class="tl-watts" d="${wattsPath}" />
            ${jobBars}
            ${eventLines}
            ${ticks}
          </svg>
        </div>
      </figure>`;
    })}
    <p class="muted small tl-legend">
      <span class="tl-key tl-key-vram">VRAM in use</span>
      <span class="tl-key tl-key-reserved">reserved</span>
      <span class="tl-key tl-key-watts">watts</span>
      <span class="tl-key tl-key-jobs">jobs</span>
      <span class="tl-key tl-key-events">events</span>
    </p>
  </section>`;
}

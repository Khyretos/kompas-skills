import { html, type SafeHtml } from "../core/html";

export interface TlSample {
  at: string;
  usedMib: number | null;
  reservedMib: number;
  watts: number | null;
}

export interface TlJob {
  id: string;
  kind: string;
  what: string;
  state: string;
  startedAt: string;
  endedAt: string | null;
  error: string | null;
}

export interface TlEvent {
  at: string;
  kind: string;
  detail: string;
}

export interface TlGpu {
  gpu: string;
  machine: string;
  hours: number;
  samples: TlSample[];
  jobs: TlJob[];
  events: TlEvent[];
}

const n = (v: number): number => Math.round(v * 10) / 10;

function parseTime(s: string): number {
  return Date.parse(s);
}

function formatTime(t: number): string {
  const d = new Date(t);
  return `${d.getHours().toString().padStart(2, "0")}:${d.getMinutes().toString().padStart(2, "0")}`;
}

function buildPath(samples: TlSample[], width: number, start: number, end: number, yScale: (mib: number) => number): string {
  if (!samples.length) return "";
  
  const points: string[] = [];
  let first = true;
  
  for (const s of samples) {
    if (s.usedMib === null) continue;
    
    const x = n((parseTime(s.at) - start) / (end - start) * width);
    const y = n(yScale(s.usedMib));
    
    if (first) {
      points.push(`M ${x} ${y}`);
      first = false;
    } else {
      points.push(`L ${x} ${y}`);
    }
  }
  
  if (points.length === 0) return "";
  
  // Close the path back to baseline
  const lastX = n((parseTime(samples[samples.length - 1].at) - start) / (end - start) * width);
  const lastY = n(yScale(samples[samples.length - 1].usedMib!));
  
  points.push(`L ${lastX} 110 L ${lastX} 110`);
  
  return `M ${points.join(" ")}`;
}

function renderTimeline(list: TlGpu[] | undefined, hours: 1 | 24, totalMib: Record<string, number>, now = Date.now()): SafeHtml {
  const width = hours === 1 ? 720 : 1440;
  const height = 140;
  const start = now - hours * 3600_000;
  const end = now;
  
  const header = html`
    <section class="gpu-timeline" aria-labelledby="tl-h">
      <h2 id="tl-h" class="label">GPU timeline</h2>
      <div class="seg" role="group" aria-label="Time range">
        <button 
          data-action="gpu-range" 
          data-hours="1" 
          aria-pressed="${String(list && list[0]?.hours === 1)}"
          class="${list && list[0]?.hours === 1 ? "" : "hidden"}"
        >1 h</button>
        <button 
          data-action="gpu-range" 
          data-hours="24" 
          aria-pressed="${String(list && list[0]?.hours === 24)}"
          class="${list && list[0]?.hours === 24 ? "" : "hidden"}"
        >24 h</button>
      </div>
  `;

  if (list === undefined) {
    return html`${header}<p class="muted small">Loading…</p>`;
  }

  if (list.length === 0) {
    return html`${header}<p class="muted small">No GPUs configured.</p>`;
  }

  const figures = list.map((g) => {
    const maxW = Math.max(...g.samples.map((s) => s.watts || 0), 1);
    const peakUsed = g.samples.reduce((max, s) => s.usedMib !== null && s.usedMib > max ? s.usedMib : max, 0);
    const peakReserved = g.samples.reduce((max, s) => s.reservedMib > max ? s.reservedMib : max, 0);
    const peak = Math.max(peakUsed, peakReserved);
    
    const vramTotal = totalMib[g.gpu] || peak || 1;
    
    const yScale = (mib: number) => 110 - (mib / vramTotal) * 100;
    
    const vramPoints = g.samples
      .filter((s) => s.usedMib !== null)
      .map((s) => {
        const x = n((parseTime(s.at) - start) / (end - start) * width);
        const y = n(yScale(s.usedMib));
        return `${x},${y}`;
      })
      .join(" ");
      
    const reservedPoints = g.samples
      .map((s) => {
        const x = n((parseTime(s.at) - start) / (end - start) * width);
        const y = n(yScale(s.reservedMib));
        return `${x},${y}`;
      })
      .join(" ");
      
    const wattsPoints = g.samples
      .filter((s) => s.watts !== null)
      .map((s) => {
        const x = n((parseTime(s.at) - start) / (end - start) * width);
        const y = n(110 - (s.watts / maxW) * 100);
        return `${x},${y}`;
      })
      .join(" ");
      
    const jobGroups = g.jobs.map((j) => {
      const jStart = parseTime(j.startedAt);
      const jEnd = j.endedAt ? parseTime(j.endedAt) : end;
      const jWidth = Math.max(2, n((jEnd - jStart) / (end - start) * width));
      const jX = n((jStart - start) / (end - start) * width);
      const jY = 116;
      const jH = 12;
      
      return html`
        <g>
          <title>${j.kind}: ${j.what} (${j.state}${j.error ? ", " + j.error : ""})</title>
          <rect 
            class="tl-job kind-${j.kind} ${j.state}" 
            x="${jX}" 
            y="${jY}" 
            width="${jWidth}" 
            height="${jH}" 
            rx="2"
          />
        </g>
      `;
    });
    
    const eventLines = g.events.map((e) => {
      const ex = n((parseTime(e.at) - start) / (end - start) * width);
      return html`
        <g>
          <title>${e.kind}: ${e.detail}</title>
          <line class="tl-event" x1="${ex}" x2="${ex}" y1="6" y2="128" />
        </g>
      `;
    });
    
    const tickInterval = hours === 1 ? 15 * 60_000 : 3 * 3600_000;
    const ticks = [];
    for (let t = start + tickInterval; t <= end; t += tickInterval) {
      const tx = n((t - start) / (end - start) * width);
      ticks.push(html`<text class="tl-tick" x="${tx}" y="138">${formatTime(t)}</text>`);
    }
    
    const ariaLabel = `a${g.gpu}: peak ${n(peak)} GB VRAM, ${g.jobs.length} jobs, ${g.events.length} events in the last ${hours} h`;
    
    return html`
      <figure class="tl-gpu">
        <figcaption><strong>${g.gpu}</strong> <span class="muted small">${g.machine}</span></figcaption>
        <div class="tl-scroll">
          <svg class="tl-svg" viewBox="0 0 ${width} ${height}" width="${width}" height="${height}" role="img" aria-label="${ariaLabel}">
            <path class="tl-vram" d="M ${vramPoints} L ${vramPoints.split(" ").pop() || "0,110"} 110 Z" />
            <path class="tl-reserved" d="M ${reservedPoints} L ${reservedPoints.split(" ").pop() || "0,110"} 110 Z" />
            <path class="tl-watts" d="M ${wattsPoints}" />
            ${jobGroups}
            ${eventLines}
            ${ticks}
          </svg>
        </div>
        <p class="muted small tl-legend">
          <span class="tl-key tl-key-vram">VRAM in use</span>
          <span class="tl-key tl-key-reserved">reserved</span>
          <span class="tl-key tl-key-watts">watts</span>
          <span class="tl-key tl-key-jobs">jobs</span>
          <span class="tl-key tl-key-events">events</span>
        </p>
      </figure>
    `;
  });

  return html`${header}${figures}`;
}

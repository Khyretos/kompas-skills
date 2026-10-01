// Machines tab: live load and power per connected computer, and today's totals.
import { html, SafeHtml } from "../core/html";
import type { DaySummary, MachineStats } from "../api/types";

const pct = (n: number) => `${Math.round(n * 100)}%`;
const fmtTokens = (n: number) => (n >= 1e6 ? `${(n / 1e6).toFixed(1)} M` : `${Math.round(n / 1000)}k`);

function sparkline(values: number[], fixedMax?: number): SafeHtml {
  if (values.length < 2) return html``;
  const w = 120, h = 32, max = fixedMax ?? (Math.max(...values) * 1.1 || 1), min = 0;
  const pts = values.map((v, i) => [(i / (values.length - 1)) * w, h - ((v - min) / (max - min)) * h]);
  const line = pts.map(([x, y]) => `${x.toFixed(1)},${y.toFixed(1)}`).join(" ");
  const [lx, ly] = pts[pts.length - 1];
  return new SafeHtml(
    `<svg class="spark" viewBox="0 0 ${w} ${h}" preserveAspectRatio="none" aria-hidden="true">` +
      `<polygon points="0,${h} ${line} ${w},${h}" class="spark-area"/>` +
      `<polyline points="${line}" class="spark-line"/>` +
      `<circle cx="${lx.toFixed(1)}" cy="${ly.toFixed(1)}" r="2.5" class="spark-dot"/></svg>`,
  );
}

function bar(value: number, label: string): SafeHtml {
  return html`<span class="meter" role="meter" aria-valuenow="${Math.round(value * 100)}" aria-valuemin="0" aria-valuemax="100" aria-label="${label}">
    <span style="width:${pct(value)}"></span></span>`;
}

function machine(m: MachineStats): SafeHtml {
  const cpuKind = m.historyKind === "cpu";
  const watts = m.history[m.history.length - 1];
  if (!m.online) {
    return html`<li class="machine off"><div class="machine-head"><span class="dot" aria-hidden="true"></span>
      <strong>${m.name}</strong><span class="muted small">${m.os} · offline</span></div></li>`;
  }
  return html`
    <li class="machine">
      <div class="machine-head">
        <span class="dot ok" aria-hidden="true"></span>
        <strong>${m.name}</strong><span class="muted small">${m.os}</span>
        <span class="watts">${cpuKind ? `${pct(m.cpu)} CPU` : watts === undefined ? "" : `${watts} W`}</span>
      </div>
      ${sparkline(m.history, cpuKind ? 1 : undefined)}
      ${m.busy ? html`<p class="busy">${cpuKind ? m.busy : `${m.busy}, so heavy tasks ask first.`}</p>` : ""}
      <dl class="stats">
        <div><dt>CPU</dt><dd>${bar(m.cpu, "CPU")}<span>${pct(m.cpu)}</span></dd></div>
        <div><dt>RAM</dt><dd>${bar(m.ramUsedGb / m.ramTotalGb, "RAM")}<span>${m.ramUsedGb}/${m.ramTotalGb} GB</span></dd></div>
        ${cpuKind ? "" : html`<div><dt>Kompanion</dt><dd>${bar(m.kompanionShare, "Kompanion share")}<span>${pct(m.kompanionShare)} of CPU</span></dd></div>`}
      </dl>
      ${m.gpus.map((g) => html`
        <div class="gpu">
          <div class="gpu-head"><strong>${g.name}</strong><span class="muted small">${g.use}</span></div>
          <dl class="stats">
            <div><dt>Load</dt><dd>${bar(g.load, `${g.name} load`)}<span>${pct(g.load)}</span></dd></div>
            <div><dt>VRAM</dt><dd>${bar(g.vramUsedGb / g.vramTotalGb, `${g.name} VRAM`)}<span>${g.vramUsedGb.toFixed(1)}/${g.vramTotalGb} GB</span></dd></div>
            <div><dt>Power</dt><dd><span class="num">${g.watts} W · ${g.tempC} °C</span></dd></div>
          </dl>
        </div>`)}
    </li>`;
}

export function renderMachines(machines: MachineStats[], day?: DaySummary): SafeHtml {
  return html`
    <div class="task-groups">
      ${day ? html`
        <section class="group">
          <h3 class="label">Today</h3>
          <dl class="today">
            <div><dt>Tasks</dt><dd>${day.tasks}</dd></div>
            <div><dt>Local tokens</dt><dd>${fmtTokens(day.localTokens)}</dd></div>
            <div><dt>Cloud tokens</dt><dd>${fmtTokens(day.cloudTokens)} <small>€${day.cloudCostEur.toFixed(2)}</small></dd></div>
            <div><dt>Energy</dt><dd>${day.energyKwh.toFixed(1)} kWh</dd></div>
          </dl>
        </section>` : ""}
      <section class="group">
        <h3 class="label">Computers</h3>
        <ul class="machines">${machines.map(machine)}</ul>
      </section>
    </div>`;
}

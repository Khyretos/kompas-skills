// Machines tab: live load and power per connected computer, and today's totals.
import { html, SafeHtml } from "../core/html";
import type { DaySummary, MachineStats } from "../api/types";
import { icon } from "./icons";

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
        ${m.id !== "server" ? html`<button class="icon-btn" data-action="unpair" data-id="${m.id}" aria-label="Unpair ${m.name}">${icon("trash")}</button>` : ""}
        <span class="watts">${cpuKind ? `${pct(m.cpu)} CPU` : watts === undefined ? "" : `${watts} W`}</span>
      </div>
      ${sparkline(m.history, cpuKind ? 1 : undefined)}
      ${m.busy ? html`<p class="busy">${cpuKind ? m.busy : `${m.busy}, so heavy tasks ask first.`}</p>` : ""}
      <dl class="stats">
        <div><dt>CPU</dt><dd>${bar(m.cpu, "CPU")}<span>${pct(m.cpu)}</span></dd></div>
        <div><dt>RAM</dt><dd>${bar(m.ramUsedGb / m.ramTotalGb, "RAM")}<span>${m.ramUsedGb}/${m.ramTotalGb} GB</span></dd></div>
        ${cpuKind ? "" : html`<div><dt>Kompanion</dt><dd>${bar(m.kompanionShare, "Kompanion share")}<span>${pct(m.kompanionShare)} of CPU</span></dd></div>`}
      </dl>
      ${m.diskTotalGb ? html`<dl class="stats">
        <div><dt>Disk</dt><dd>${bar((m.diskUsedGb ?? 0) / m.diskTotalGb, "Disk")}<span>${Math.round(m.diskUsedGb ?? 0)}/${Math.round(m.diskTotalGb)} GB</span></dd></div>
      </dl>` : ""}
      ${m.powerHistory && m.powerHistory.length > 1 ? html`
        <div class="power"><span class="muted small">GPU power ${Math.round(m.powerHistory[m.powerHistory.length - 1])} W</span>${sparkline(m.powerHistory)}</div>` : ""}
      ${m.gpus.map((g) => {
        const facts = [
          g.watts != null ? `${Math.round(g.watts)} W` : "",
          g.tempC != null ? `${Math.round(g.tempC)} °C` : "",
          g.coreMhz ? `${Math.round(g.coreMhz)} MHz` : "",
          g.fanRpm != null ? `fan ${Math.round(g.fanRpm)} rpm` : "",
        ].filter(Boolean).join(" · ");
        return html`
        <div class="gpu">
          <div class="gpu-head"><strong>${g.name}</strong><span class="muted small">${g.use || g.driver || ""}</span></div>
          <dl class="stats">
            <div><dt>Load</dt><dd>${g.load != null ? html`${bar(g.load, `${g.name} load`)}<span>${pct(g.load)}</span>` : html`<span class="muted">…</span>`}</dd></div>
            ${g.vramTotalGb ? html`<div><dt>VRAM</dt><dd>${bar((g.vramUsedGb ?? 0) / g.vramTotalGb, `${g.name} VRAM`)}<span>${(g.vramUsedGb ?? 0).toFixed(1)}/${g.vramTotalGb.toFixed(0)} GB</span></dd></div>` : ""}
            ${facts ? html`<div><dt>Now</dt><dd><span class="num">${facts}</span></dd></div>` : ""}
          </dl>
        </div>`;
      })}
    </li>`;
}

/** Refresh steps in seconds; 1 is "Live" (pushed by the server). */
export const REFRESH_STEPS = [1, 2, 5, 15, 30, 60, 300];
const stepLabel = (s: number) => (s === 1 ? "Live" : s < 60 ? `every ${s} s` : `every ${s / 60} min`);
const clock = (iso?: string) =>
  iso ? new Date(iso).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", second: "2-digit" }) : "not yet";

function pairResult(p: { id: string; name: string; token: string }, server: string): SafeHtml {
  const config = `server = "${server}"\nmachine_id = "${p.id}"\ntoken_file = "~/.config/kompanion-runner/token"`;
  return html`
    <div class="pair-result" role="status">
      <p><strong>${p.name} is paired.</strong> Copy the token now: it is shown only once.</p>
      <label class="label" for="pair-token">Token</label>
      <input id="pair-token" readonly value="${p.token}">
      <p class="small">On ${p.name}, save the token in <code>~/.config/kompanion-runner/token</code> (mode 600) and this as
        <code>~/.config/kompanion-runner/config.toml</code>:</p>
      <pre>${config}</pre>
      <p class="small">Then start <code>kompanion-runner</code> (see docs/runner-install.md). The computer shows up here within a minute.</p>
      <button class="btn small" data-action="pair-done">Done</button>
    </div>`;
}

export function renderMachines(machines: MachineStats[], day: DaySummary | undefined, refresh: number,
  pairing?: { id: string; name: string; token: string }): SafeHtml {
  const step = Math.max(0, REFRESH_STEPS.indexOf(refresh));
  const newest = machines.map((m) => m.sampledAt).filter(Boolean).sort().pop();
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
        <div class="refresh">
          <label for="machines-refresh">Refresh: <strong>${stepLabel(REFRESH_STEPS[step])}</strong></label>
          <input type="range" id="machines-refresh" min="0" max="${REFRESH_STEPS.length - 1}" step="1" value="${step}"
            aria-valuetext="${stepLabel(REFRESH_STEPS[step])}">
          <span class="muted small" aria-live="off">Updated ${clock(newest)}</span>
        </div>
        <ul class="machines">${machines.map(machine)}</ul>
        ${pairing ? pairResult(pairing, location.origin) : html`
          <form class="pair" id="pair-form">
            <label class="label" for="pair-name">Pair a computer</label>
            <div class="row">
              <input id="pair-name" name="name" placeholder="Computer name, e.g. soucouyant" maxlength="60" required>
              <button class="btn" type="submit">Pair</button>
            </div>
          </form>`}
      </section>
    </div>`;
}

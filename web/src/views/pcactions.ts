import { html, type SafeHtml } from "../core/html";
import type { PcAction } from "../api/client";
import { fromTool, renderOutput } from "../core/output";
import { kindOf, NEED_LABEL, styleOf, textOn, type CardStyle } from "../core/cardtypes";

export function renderPcPicker(
  machines: { id: string; name: string; online?: boolean }[],
  selected: string | undefined
): SafeHtml {
  if (machines.length === 0) {
    return html``;
  }

  const options = html`
    <option value="">no computer</option>
    ${machines.map((m) => {
      const text = m.online === false ? `${m.name} (offline)` : m.name;
      return html`<option value="${m.id}" ${selected === m.id ? "selected" : ""}>${text}</option>`;
    })}
  `;

  const picked = machines.find((m) => m.id === selected);
  return html`
    <div class="pc-pick">
      <label for="pc-machine">Computer</label>
      <select id="pc-machine">${options}</select>
      ${picked ? html`<span class="muted small">Kompanion may act on ${picked.name}; every step asks you first.</span>` : ""}
    </div>
  `;
}

const stateLabels: Record<string, string> = {
  pending: "waiting for you",
  approved: "starting…",
  always: "starting…",
  granting: "waiting for the computer to allow it…",
  running: "running…",
  denied: "declined",
  done: "done",
  failed: "failed",
  refused: "not allowed on that computer",
};

export function renderPcActions(actions: PcAction[], machineNames: Record<string, string>, cardStyle?: CardStyle): SafeHtml {
  if (actions.length === 0) {
    return html``;
  }

  const recent = actions.slice(-6);

  return html`
    <section class="pc-actions" aria-label="Steps on your computer">
      ${recent.map((a) => {
        const label = stateLabels[a.state] ?? a.state;
        const hasResult = !!a.result;
        const buttons = html`
          <button class="btn small primary" data-action="pc-decide" data-id="${a.id}" data-decision="approve">Approve</button>
          <button class="btn small" data-action="pc-decide" data-id="${a.id}" data-decision="always">Always allow (24 h)</button>
          <button class="btn small danger" data-action="pc-decide" data-id="${a.id}" data-decision="deny">Deny</button>
        `;

        return html`
          <article class="pc-action ${a.state} kind-${kindOf(a.tool)}">
            <header class="card-kind" style="background:${styleOf(kindOf(a.tool), cardStyle).color};color:${textOn(styleOf(kindOf(a.tool), cardStyle).color)}">
              <span>${styleOf(kindOf(a.tool), cardStyle).label}</span>
              <span class="need">${a.state === "pending" ? NEED_LABEL.approval : NEED_LABEL.automatic}</span>
            </header>
            <strong>${a.summary}</strong>
            <span class="muted small">on ${machineNames[a.machineId] ?? "a computer"}</span>
            ${a.needs && a.state === "pending" ? html`<span class="small">Needs: ${a.needs}</span>` : ""}
            <span class="chip ${a.state}">${label}</span>
            ${hasResult ? renderOutput(fromTool(a.tool, a.result ?? "", machineNames[a.machineId])) : ""}
            ${a.state === "pending" ? buttons : null}
          </article>
        `;
      })}
    </section>
  `;
}

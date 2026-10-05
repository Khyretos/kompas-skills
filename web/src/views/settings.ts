// Models and roles: which model fills which role. Skills belong to roles, so
// swapping a model here keeps everything Kompanion has learned.
import { html, type SafeHtml } from "../core/html";
import type { AppState } from "../state";
import type { AdminSettings, Role } from "../api/types";
import { icon } from "./icons";
import { renderAdmin } from "./admin";
import { DEFAULT_STYLE, KINDS, styleOf, textOn } from "../core/cardtypes";

const roleInfo: Record<Role, { name: string; text: string }> = {
  orchestrator: { name: "Orchestrator", text: "Talks with you, plans and splits the work." },
  worker: { name: "Worker", text: "Does the steps: code, files, commands." },
  reviewer: { name: "Reviewer and teacher", text: "Checks results and writes lessons into the skills." },
};

const DEFAULTS: AdminSettings = {
  appName: "", smtpHost: "", smtpPort: 587, smtpTls: "starttls", smtpUser: "", smtpFrom: "", smtpReplyTo: "",
  colorBrand: "#5c398e", colorLinkDark: "#f3941f", colorLinkLight: "#8f4700", colorAccent: "#f3941f",
};

const LANGS: [string, string][] = [["", "Detect"], ["en", "English"], ["es", "Spanish"], ["nl", "Dutch"]];

/** Item 7: the colour and label of each action type on step and approval cards, saved for this user. */
function cardSection(s: AppState): SafeHtml {
  return html`<section class="card-settings">
    <h3 class="label">Card colours</h3>
    <p class="muted small">Steps and approval cards get a coloured header by what they do. Text colour is picked for contrast.</p>
    <ul class="card-kinds">${KINDS.map((k) => {
      const st = styleOf(k, s.cardStyle);
      return html`<li>
        <span class="kind-tag" style="background:${st.color};color:${textOn(st.color)}">${st.label}</span>
        <label class="sr-only" for="card-label-${k}">Label for ${DEFAULT_STYLE[k].label}</label>
        <input id="card-label-${k}" data-kind="${k}" class="card-label" value="${st.label}" maxlength="30">
        <label class="sr-only" for="card-color-${k}">Colour for ${DEFAULT_STYLE[k].label}</label>
        <input id="card-color-${k}" data-kind="${k}" class="card-color" type="color" value="${st.color}">
      </li>`;
    })}</ul>
    <button class="btn small" type="button" data-action="card-reset">Back to the brand colours</button>
  </section>`;
}

/** W4: optional voice, per device. Off until switched on here. */
function voiceSection(s: AppState): SafeHtml {
  const p = s.voicePrefs;
  if (!s.voice) return html``;
  return html`<section class="voice-settings">
    <h3 class="label">Voice</h3>
    ${!s.voice.enabled ? html`<p class="muted small">Voice is off on this server.</p>` : html`
      <fieldset class="checks">
        <legend class="sr-only">Voice on this device</legend>
        <label><input type="checkbox" id="voice-input" ${p.input ? "checked" : ""}> Talk with the microphone button</label>
        <label><input type="checkbox" id="voice-read" ${p.readAloud ? "checked" : ""}> Read replies aloud</label>
      </fieldset>
      <div class="field"><label for="voice-lang">I speak</label>
        <select id="voice-lang">${LANGS.map(([v, l]) => html`<option value="${v}" ${v === p.lang ? "selected" : ""}>${l}</option>`)}</select></div>
      <div class="field"><label for="voice-voice">Reading voice</label>
        <select id="voice-voice">${s.voice.voices.map((v) => html`<option value="${v.id}" ${v.id === p.voice ? "selected" : ""}>${v.label}</option>`)}</select></div>
      ${p.lang === "nl" ? html`<p class="warn small">There is no Dutch reading voice yet: replies stay text only while "I speak" is Dutch. The microphone works in Dutch.</p>` : ""}
      <p class="muted small">Speech goes to Whisper and Kokoro on your own server and is never stored. These settings are kept on this device.</p>`}
  </section>`;
}

export function renderSettings(s: AppState): SafeHtml {
  const options = s.providers.flatMap((p) => p.models.map((m) => ({ p, m, value: `${p.id}::${m.id}` })));
  return html`
    <div class="sheet" role="dialog" aria-modal="true" aria-labelledby="settings-h">
      <div class="sheet-head">
        <h2 id="settings-h">Settings</h2>
        <button class="icon-btn" data-action="close-settings" aria-label="Close">${icon("close")}</button>
      </div>
      ${renderAdmin(s.admin?.settings ?? DEFAULTS, s.admin?.smtpPasswordSet ?? false, s.theme, s.isAdmin && !!s.admin)}
      ${s.notifications ? html`<form class="admin-form" id="notify-form">
        <h3 class="label">Notifications</h3>
        <div class="field">
          <label for="notify-email">Mail me at</label>
          <input type="email" id="notify-email" name="email" value="${s.notifications.email}" placeholder="you@example.com">
        </div>
        <fieldset class="checks">
          <legend class="sr-only">When</legend>
          <label><input type="checkbox" name="onNeedsInput" ${s.notifications.onNeedsInput ? "checked" : ""}> A task needs me</label>
          <label><input type="checkbox" name="onFailed" ${s.notifications.onFailed ? "checked" : ""}> A task failed</label>
          <label><input type="checkbox" name="onDone" ${s.notifications.onDone ? "checked" : ""}> A task is done</label>
          <label><input type="checkbox" name="dailySummary" ${s.notifications.dailySummary ? "checked" : ""}> A daily summary (08:00 UTC)</label>
        </fieldset>
        <p class="muted small">Mails name the task, its state and in one line what is needed, never its description or chat.</p>
        <p id="notify-msg" class="small" role="status"></p>
        <button class="btn primary" type="submit">Save</button>
      </form>` : ""}
      ${s.isAdmin ? html`<section>
        <h3 class="label">Connections</h3>
        <p>Windshift: <span class="chip ${s.windshift === "connected" ? "good" : ""}">${s.windshift ?? "not configured"}</span></p>
        ${s.windshiftWarning ? html`<p class="warn small" role="status">${s.windshiftWarning}</p>` : ""}
        <p class="muted small">Set in the server's compose file (WINDSHIFT_URL, WINDSHIFT_TOKEN); it can't be changed here.</p>
      </section>` : ""}
      <section>
        <h3 class="label">Roles</h3>
        <p class="muted">Any model can fill any role. Skills and lessons belong to the role, so switching a model keeps them.</p>
        <div class="roles">${(Object.keys(roleInfo) as Role[]).map((role) => {
          const current = s.roles.find((r) => r.role === role);
          const value = current ? `${current.providerId}::${current.modelId}` : "";
          return html`
            <div class="role">
              <label for="role-${role}"><strong>${roleInfo[role].name}</strong><small>${roleInfo[role].text}</small></label>
              <select id="role-${role}" data-role="${role}">
                ${value ? "" : html`<option value="" selected disabled>Not set</option>`}
                ${options.map((o) => html`<option value="${o.value}" ${o.value === value ? "selected" : ""}>${o.m.id} · ${o.p.name}</option>`)}
              </select>
            </div>`;
        })}</div>
      </section>
      ${voiceSection(s)}
      ${cardSection(s)}
      <section>
        <h3 class="label">Connected models</h3>
        <ul class="providers">${s.providers.map((p) => html`
          <li class="provider">
            <div class="provider-head">
              <strong>${p.name}</strong>
              <span class="chip ${p.local ? "good" : ""}">${p.local ? "Local" : "Cloud"}</span>
            </div>
            <small class="muted">${p.kind === "anthropic" ? "Anthropic API" : "OpenAI-compatible API"} · ${p.baseUrl}</small>
            ${p.error ? html`<p class="error small">Can't reach it: ${p.error}</p>` : ""}
            ${p.models.map((m) => html`
              <div class="model-row">
                <code>${m.id}</code>
                <span class="chips">
                  ${m.contextTokens ? html`<span class="chip">${Math.round(m.contextTokens / 1024)}k context</span>` : ""}
                  ${m.tokensPerSecond ? html`<span class="chip">${m.tokensPerSecond} tok/s</span>` : ""}
                  ${m.toolCalls === undefined ? html`<span class="chip">not tested yet</span>` : html`
                    <span class="chip ${m.toolCalls ? "good" : "bad"}">tool calls ${m.toolCalls ? "ok" : "no"}</span>
                    <span class="chip ${m.jsonSchema ? "good" : "bad"}">JSON schema ${m.jsonSchema ? "ok" : "no"}</span>`}
                </span>
              </div>`)}
          </li>`)}</ul>
        <button class="btn" data-action="add-provider" disabled title="Comes with the server">${icon("plus")} Add a model or API</button>
      </section>
    </div>`;
}

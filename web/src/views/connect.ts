// First screen: connect to a Kompanion server by link, or pick one found on
// the local network.
import { html, mount, onAction, $ } from "../core/html";
import type { KompanionApi } from "../api/client";
import type { Server } from "../api/types";
import { icon } from "./icons";
import logo from "../assets/kk-logo.svg";

export function showConnect(root: HTMLElement, api: KompanionApi, onConnected: (s: Server) => void): void {
  let found: Server[] | undefined;
  let error = "";
  let busy = false;

  const draw = () => {
    const list = found === undefined
      ? html`<p class="muted searching">Looking for Kompanion servers on this network…</p>`
      : found.length === 0
        ? html`<p class="muted">None found here. Paste a link instead.</p>`
        : html`<ul class="found">${found.map((s) => html`
            <li><button class="found-server" data-action="pick" data-url="${s.url}" ${busy ? "disabled" : ""}>
              ${icon("wifi")}
              <span><strong>${s.name}</strong><small>${s.url}</small></span>
              <span class="chip">v${s.version}</span>
            </button></li>`)}</ul>`;

    mount(root, html`
      <main class="connect">
        <div class="connect-card">
          <div class="brand">
            <img class="brand-logo" src="${logo}" alt="" width="44" height="44">
            <div><h1>Kreative Kompanion</h1><p class="muted">Connect to your Kompanion server</p></div>
          </div>
          <section aria-labelledby="found-h">
            <h2 id="found-h" class="label">On this network</h2>
            ${list}
          </section>
          <form class="link-form" id="link-form">
            <label class="label" for="server-url">Or paste a link</label>
            <div class="row">
              <input id="server-url" type="url" inputmode="url" autocomplete="url"
                placeholder="https://kompanion.example.com" required>
              <button class="btn primary" type="submit" ${busy ? "disabled" : ""}>Connect</button>
            </div>
            ${error ? html`<p class="error" role="alert">${error}</p>` : ""}
            <p class="hint">Works from anywhere: use your public link when you're away from home.</p>
          </form>
        </div>
      </main>`);
  };

  const connect = async (url: string) => {
    busy = true; error = ""; draw();
    try {
      onConnected(await api.connect(url));
    } catch (e) {
      busy = false;
      error = e instanceof Error ? e.message : String(e);
      draw();
    }
  };

  onAction(root, { pick: (el) => connect(el.dataset.url ?? "") });
  root.addEventListener("submit", (ev) => {
    ev.preventDefault();
    connect(($("#server-url", root) as HTMLInputElement).value);
  });

  draw();
  api.discover().then((s) => { found = s; if (!busy) draw(); });
}

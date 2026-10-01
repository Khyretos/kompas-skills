// First-run setup (create the first account with the code from the server
// log) and sign-in.
import { $, html, mount } from "../core/html";
import type { KompanionApi } from "../api/client";
import { icon } from "./icons";

export function showSignIn(root: HTMLElement, api: KompanionApi, setupNeeded: boolean, done: () => void): void {
  let error = "";
  let busy = false;

  const draw = () => {
    mount(root, html`
      <main class="connect">
        <form class="connect-card" id="signin">
          <div class="brand">
            <span class="brand-mark">${icon("spark")}</span>
            <div><h1>Kreative Kompanion</h1>
              <p class="muted">${setupNeeded ? "Create the first account" : "Sign in to continue"}</p></div>
          </div>
          ${setupNeeded ? html`
            <div class="field">
              <label class="label" for="setup-code">Setup code</label>
              <input id="setup-code" autocomplete="one-time-code" required>
              <p class="hint">The server prints this code in its log the first time it starts.</p>
            </div>` : ""}
          <div class="field">
            <label class="label" for="user-name">Name</label>
            <input id="user-name" autocomplete="username" required>
          </div>
          <div class="field">
            <label class="label" for="user-password">Password</label>
            <input id="user-password" type="password" required minlength="${setupNeeded ? 12 : 1}"
              autocomplete="${setupNeeded ? "new-password" : "current-password"}">
            ${setupNeeded ? html`<p class="hint">At least 12 characters.</p>` : ""}
          </div>
          ${error ? html`<p class="error" role="alert">${error}</p>` : ""}
          <button class="btn primary" type="submit" ${busy ? "disabled" : ""}>${setupNeeded ? "Create account" : "Sign in"}</button>
        </form>
      </main>`);
  };

  root.addEventListener("submit", async (ev) => {
    ev.preventDefault();
    // Read the fields before redrawing, which replaces them.
    const value = (id: string) => ($(`#${id}`, root) as HTMLInputElement | null)?.value ?? "";
    const code = setupNeeded ? value("setup-code").trim() : "";
    const name = value("user-name");
    const password = value("user-password");
    busy = true; error = ""; draw();
    try {
      if (setupNeeded) await api.setup(code, name, password);
      else await api.login(name, password);
      done();
    } catch (e) {
      busy = false;
      error = e instanceof Error ? e.message : String(e);
      draw();
    }
  });
  draw();
}

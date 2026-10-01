// First-run setup (create the first account with the code from the server
// log) and sign-in, with name and password and/or single sign-on.
import { $, html, mount } from "../core/html";
import type { KompanionApi } from "../api/client";
import type { ServerStatus } from "../api/types";
import logo from "../assets/kk-logo.svg";

const SSO_ERRORS: Record<string, string> = {
  "no-account": "Single sign-on worked, but no Kompanion account belongs to you yet. Ask the owner to add you.",
  failed: "Single sign-on didn't work. The server log has the details.",
};

export function showSignIn(root: HTMLElement, api: KompanionApi, status: ServerStatus, done: () => void): void {
  const setupNeeded = status.setupNeeded;
  const sso = status.signIn?.oidc ?? null;
  const password = setupNeeded || (status.signIn?.password ?? true);
  // The server sends problems from single sign-on back as ?signin=<reason>.
  const params = new URLSearchParams(location.search);
  let error = SSO_ERRORS[params.get("signin") ?? ""] ?? "";
  if (params.has("signin")) history.replaceState(null, "", location.pathname);
  let busy = false;

  const draw = () => {
    mount(root, html`
      <main class="connect">
        <form class="connect-card" id="signin">
          <div class="brand">
            <img class="brand-logo" src="${logo}" alt="" width="44" height="44">
            <div><h1>Kreative Kompanion</h1>
              <p class="muted">${setupNeeded ? "Create the first account" : "Sign in to continue"}</p></div>
          </div>
          ${sso && !setupNeeded ? html`
            <a class="btn primary sso" href="/api/auth/oidc/start"><img src="${logo}" alt="" width="20" height="20">Sign in with ${sso}</a>
            ${password ? html`<p class="hint">Or use your Kompanion name and password:</p>` : ""}` : ""}
          ${password ? html`
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
          <button class="btn ${sso && !setupNeeded ? "" : "primary"}" type="submit" ${busy ? "disabled" : ""}>${setupNeeded ? "Create account" : "Sign in"}</button>` : ""}
          ${error ? html`<p class="error" role="alert">${error}</p>` : ""}
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

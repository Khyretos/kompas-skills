// Appearance (every user) and the admin menu: app name, mail, colours.
// Drafted by qwen3:14b on soucouyant, reviewed.
import { html, type SafeHtml } from "../core/html";
import type { AdminSettings } from "../api/types";

export function renderAdmin(a: AdminSettings, smtpPasswordSet: boolean, theme: "system" | "light" | "dark", isAdmin: boolean): SafeHtml {
  return html`
    <section>
      <h3 class="label">Appearance</h3>
      <fieldset class="theme-pick">
        <legend class="sr-only">Theme</legend>
        <label>
          <input type="radio" name="theme" value="system" ${theme === "system" ? "checked" : ""}>
          System
        </label>
        <label>
          <input type="radio" name="theme" value="light" ${theme === "light" ? "checked" : ""}>
          Light
        </label>
        <label>
          <input type="radio" name="theme" value="dark" ${theme === "dark" ? "checked" : ""}>
          Dark
        </label>
      </fieldset>
      ${isAdmin ? html`
        <form id="admin-form" class="admin-form">
          <h3 class="label">General</h3>
          <div class="field">
            <label for="appName">App name</label>
            <input type="text" id="appName" name="appName" value="${a.appName}" maxlength="60" required>
          </div>

          <h3 class="label">Mail</h3>
          <div class="field">
            <label for="smtpHost">SMTP host</label>
            <input type="text" id="smtpHost" name="smtpHost" value="${a.smtpHost}">
          </div>
          <div class="field">
            <label for="smtpPort">SMTP port</label>
            <input type="number" id="smtpPort" name="smtpPort" value="${a.smtpPort}" min="1" max="65535">
          </div>
          <div class="field">
            <label for="smtpTls">SMTP TLS</label>
            <select id="smtpTls" name="smtpTls">
              <option value="starttls" ${a.smtpTls === "starttls" ? "selected" : ""}>STARTTLS (port 587)</option>
              <option value="tls" ${a.smtpTls === "tls" ? "selected" : ""}>TLS (port 465)</option>
              <option value="none" ${a.smtpTls === "none" ? "selected" : ""}>None</option>
            </select>
          </div>
          <div class="field">
            <label for="smtpUser">SMTP user</label>
            <input type="text" id="smtpUser" name="smtpUser" value="${a.smtpUser}" autocomplete="off">
          </div>
          <div class="field">
            <label for="smtpFrom">SMTP from</label>
            <input type="text" id="smtpFrom" name="smtpFrom" placeholder="Name &lt;address@example.com&gt;" value="${a.smtpFrom}">
          </div>
          <div class="field">
            <label for="smtpReplyTo">Replies go to</label>
            <input type="email" id="smtpReplyTo" name="smtpReplyTo" value="${a.smtpReplyTo}">
          </div>
          <p class="muted small">${smtpPasswordSet ? "The mail password is set in the server's .env (SMTP_PASSWORD)." : "No mail password set: add SMTP_PASSWORD to the server's .env if your mail server needs one."}</p>
          <div class="field">
            <label for="test-to">Send a test mail to</label>
            <div class="row">
              <input type="email" id="test-to" placeholder="you@example.com">
              <button type="button" class="btn" data-action="test-mail">Send test mail</button>
            </div>
          </div>

          <h3 class="label">Colours</h3>
          <p class="muted small">Colours that make text hard to read are refused (WCAG AA).</p>
          <div class="field">
            <label for="colorBrand">Brand: buttons, white text on it</label>
            <input type="color" id="colorBrand" name="colorBrand" value="${a.colorBrand}">
          </div>
          <div class="field">
            <label for="colorLinkDark">Links on dark</label>
            <input type="color" id="colorLinkDark" name="colorLinkDark" value="${a.colorLinkDark}">
          </div>
          <div class="field">
            <label for="colorLinkLight">Links on light</label>
            <input type="color" id="colorLinkLight" name="colorLinkLight" value="${a.colorLinkLight}">
          </div>
          <div class="field">
            <label for="colorAccent">Focus and highlights</label>
            <input type="color" id="colorAccent" name="colorAccent" value="${a.colorAccent}">
          </div>

          <p id="admin-msg" class="small" role="status"></p>
          <button class="btn primary" type="submit">Save</button>
        </form>` : "" }
    </section>`;
}

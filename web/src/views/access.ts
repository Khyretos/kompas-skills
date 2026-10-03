// Access tab: what each paired computer lets Kompanion do (grants.json on that
// PC, mirrored here), add or revoke a grant, and the history of what was used
// or refused. (gemma4 drafted it; Claude fixed the template and removed inline
// event handlers, which the CSP blocks.)
import { html, type SafeHtml } from "../core/html";

export interface GrantView { target: string; rights: string[]; grantedBy: string; grantedAt: string; expires: string | null; pending?: "add" | "revoke" }
export interface AccessEvent { at: string; kind: "granted" | "revoked" | "used" | "refused"; target: string | null; detail: string | null; machine: string }

const day = (iso: string) => (iso ? new Date(iso).toLocaleDateString() : "");

function grantRow(machineId: string, g: GrantView): SafeHtml {
  return html`
    <li class="grant-row ${g.pending ? "pending" : ""}">
      <code>${g.target}</code>
      <span class="grant-rights">${g.rights.map((r) => html`<span class="chip ${r === "root" ? "root" : ""}">${r}</span>`)}</span>
      ${g.pending ? html`<span class="muted small" role="status">${g.pending === "add" ? "waiting for the computer to apply it…" : "revoking, waiting for the computer…"}</span>` : html`<span class="muted small">by ${g.grantedBy}, ${day(g.grantedAt)}${g.expires ? `, expires ${day(g.expires)}` : ""}</span>`}
      <button class="btn small danger" data-action="grant-revoke" data-machine="${machineId}" data-target="${g.target}" ${g.pending ? "disabled" : ""}>Revoke</button>
    </li>`;
}

export function grantFromForm(form: HTMLFormElement): { target: string; rights: string[]; expiresHours?: number } {
  const fd = new FormData(form);
  const target = String(fd.get("target") ?? "").trim();
  const rights = Array.from(fd.getAll("rights")).map(String);
  const expiresStr = String(fd.get("expires") ?? "");
  let expiresHours: number | undefined;
  if (expiresStr !== "") {
    expiresHours = Number(expiresStr);
  }
  return { target, rights, expiresHours };
}

export function renderAccess(machines: { id: string; name: string }[], grants: Record<string, GrantView[]>, history: AccessEvent[]): SafeHtml {
  return html`
    <div class="task-groups">
      ${machines.length === 0 ? html`<p class="muted pad">Pair a computer in the Machines tab first.</p>` : ""}
      ${machines.map((m) => {
        const list = grants[m.id] ?? [];
        return html`
          <section class="group">
            <h3 class="label">${m.name}</h3>
            ${list.length ? html`<ul class="grants">${list.map((g) => grantRow(m.id, g))}</ul>`
              : html`<p class="muted small">No access granted on this computer.</p>`}
            <form class="grant-add" data-machine="${m.id}">
              <input name="target" placeholder="/home/you/projects/app" aria-label="Folder or system" required>
              <span class="grant-rights">
                <label><input type="checkbox" name="rights" value="read" checked> read</label>
                <label><input type="checkbox" name="rights" value="write"> write</label>
                <label><input type="checkbox" name="rights" value="shell"> shell</label>
              </span>
              <select name="expires" aria-label="Expires">
                <option value="1">1 hour</option>
                <option value="24">1 day</option>
                <option value="168">1 week</option>
                <option value="" selected>never</option>
              </select>
              <button class="btn small" type="submit">Grant</button>
            </form>
            <form class="grant-add" data-machine="${m.id}">
              <input type="hidden" name="target" value="system">
              <span class="muted small">System rights. Root actions still ask for your password on that PC.</span>
              <span class="grant-rights">
                <label><input type="checkbox" name="rights" value="packages"> packages</label>
                <label><input type="checkbox" name="rights" value="services"> user services</label>
                <label><input type="checkbox" name="rights" value="desktop"> desktop reload</label>
                <label><input type="checkbox" name="rights" value="root"> root (pkexec prompt on the PC)</label>
              </span>
              <select name="expires" aria-label="Expires">
                <option value="1">1 hour</option>
                <option value="24" selected>1 day</option>
                <option value="168">1 week</option>
                <option value="">never</option>
              </select>
              <button class="btn small" type="submit">Grant system access</button>
            </form>
          </section>`;
      })}
      <section class="group">
        <h3 class="label">History</h3>
        ${history.length ? html`<ul class="access-history">${history.slice(0, 50).map((e) => html`
          <li>
            <span class="muted small">${new Date(e.at).toLocaleString()} · ${e.machine}</span>
            <span class="chip ${e.kind}">${e.kind}</span>
            ${e.target ? html`<code>${e.target}</code>` : ""} ${e.detail ?? ""}
          </li>`)}
        </ul>` : html`<p class="muted small">Nothing yet.</p>`}
      </section>
    </div>`;
}

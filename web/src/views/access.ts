import { html, type SafeHtml } from "../core/html";

export interface GrantView {
  target: string;
  rights: string[];
  grantedBy: string;
  grantedAt: string;
  expires: string | null;
}

export interface AccessEvent {
  at: string;
  kind: "granted" | "revoked" | "used" | "refused";
  target: string | null;
  detail: string | null;
  machine: string;
}

export function renderAccess(
  machines: { id: string; name: string }[],
  grants: Record<string, GrantView[]>,
  history: AccessEvent[]
): SafeHtml {
  return html`
    <div class="task-groups">
      ${machines.map((m) => {
        const machineGrants = grants[m.id] ?? [];
        return html`
          <section class="group">
            <h3 class="label">${m.name}</h3>
            ${machineGrants.length > 0
              ? machineGrants.map((g) => html`
                  <div class="grant-row">
                    <code>${g.target}</code>
                    <div class="grant-rights">
                      ${g.rights.map((r) => html`<span class="chip">${r}</span>`)}
                    </div>
                    <div class="grant-meta">
                      <span class="muted small">by ${g.grantedBy}, ${new Date(g.grantedAt).toLocaleDateString()}</span>
                      ${g.expires ? html`<span class="muted small">expires ${g.expires}</span>` : ""}
                    </div>
                    <button
                      class="btn small danger"
                      data-action="grant-revoke"
                      data-machine="${m.id}"
                      data-target="${g.target}"
                    >
                      Revoke
                    </button>
                  </div>`
                )}
              : html`<p class="muted small">No access granted on this computer.</p>`}

            <form class="grant-add" data-machine="${m.id}">
              <input
                name="target"
                placeholder="/home/you/projects/app or system"
                required
              />
              <div class="grant-rights">
                <label>
                  <input
                    type="checkbox"
                    name="rights"
                    value="read"
                    checked
                    onchange="handleCheckboxChange(event)"
                  />
                  read
                </label>
                <label>
                  <input
                    type="checkbox"
                    name="rights"
                    value="write"
                    onchange="handleCheckboxChange(event)"
                  />
                  write
                </label>
                <label>
                  <input
                    type="checkbox"
                    name="rights"
                    value="shell"
                    onchange="handleCheckboxChange(event)"
                  />
                  shell
                </label>
              </div>
              <button class="btn" type="submit">Grant</button>
            </form>
          </section>`;
        });
      })}

      <section class="group">
        <h3 class="label">History</h3>
        ${history.length > 0
          ? history.slice(-50).reverse().map((e) => html`
              <div class="history-item">
                <span class="muted small">${new Date(e.at).toLocaleString()}</span>
                <span class="muted small">${e.machine}</span>
                <span class="chip ${e.kind}">${e.kind}</span>
                ${e.target ? html`<code class="history-target">${e.target}</code>` : ""}
                ${e.detail ? html`<span class="history-detail">${e.detail}</span>` : ""}
              </div>`
            )
          : html`<p class="muted small">Nothing yet.</p>`}
      </section>
    </div>`;
}

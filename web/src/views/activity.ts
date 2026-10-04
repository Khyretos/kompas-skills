import { html, type SafeHtml } from "../core/html";

export interface ActivityItem {
  at: string;
  kind: string;
  state?: string;
  text: string;
  machineId: string;
  machine: string;
  chatId?: string;
  chat?: string;
}

const stateMap = {
  pending: "waiting",
  done: "done",
  failed: "failed",
  refused: "not allowed",
  denied: "declined",
};

function getStateText(kind: string, state?: string): string {
  if (kind === "step") {
    if (state === "pending") return "waiting";
    if (state === "done") return "done";
    if (state === "failed") return "failed";
    if (state === "refused") return "not allowed";
    if (state === "denied") return "declined";
    return "running";
  }
  if (kind === "granted") return "granted";
  if (kind === "revoked") return "revoked";
  if (kind === "refused") return "refused";
  return kind;
}

export function renderActivity(
  items: ActivityItem[],
  filter: { machine?: string; chat?: string }
): SafeHtml {
  const machines = Array.from(new Set(items.map((i) => i.machineId))).sort();
  const chats = Array.from(new Set(items.filter((i) => i.chatId).map((i) => i.chatId))).sort();

  const filteredItems = items.filter((item) => {
    if (filter.machine && item.machineId !== filter.machine) return false;
    if (filter.chat && item.chatId !== filter.chat) return false;
    return true;
  });

  return html`
    <div class="activity-filters">
      <select id="activity-machine" aria-label="Computer">
        <option value="" ${!filter.machine ? 'selected' : ''}>All computers</option>
        ${machines.map((m) => html`<option value="${m}" ${filter.machine === m ? 'selected' : ''}>${m}</option>`)}
      </select>
      <select id="activity-chat" aria-label="Chat">
        <option value="" ${!filter.chat ? 'selected' : ''}>All chats</option>
        ${chats.map((c) => html`<option value="${c}" ${filter.chat === c ? 'selected' : ''}>${c}</option>`)}
      </select>
    </div>
    <ol class="activity">
      ${filteredItems.length === 0
        ? html`<p class="muted pad">Nothing yet.</p>`
        : filteredItems.map((item) => {
            const stateText = item.state ?? "";
            const chipClass = stateText || item.kind;
            const chipText = getStateText(item.kind, item.state);
            const hasChat = !!item.chatId;

            return html`
              <li class="activity-item ${item.kind} ${stateText}">
                <time datetime="${item.at}">${new Date(item.at).toLocaleString()}</time>
                <span class="chip ${chipClass}">${chipText}</span>
                ${item.text}
                <span class="muted small">${item.machine}</span>
                ${hasChat
                  ? html`<button class="linklike" data-action="open-chat" data-id="${item.chatId}">${item.chat || "chat"}</button>`
                  : null}
              </li>
            `;
          })}
    </ol>
  `;
}

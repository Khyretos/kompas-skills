import { html, type SafeHtml } from "../core/html";
import { icon } from "./icons";
import { fromTool, renderOutput } from "../core/output";

export interface ActivityItem {
  at: string;
  kind: string;
  state?: string;
  text: string;
  machineId: string;
  machine: string;
  chatId?: string;
  chat?: string;
  id?: string;
  tool?: Record<string, unknown>;
  result?: string | null;
  grant?: string | null;
  target?: string;
  detail?: string;
}

export interface ActivityFilter {
  machine?: string;
  chat?: string;
  failedOnly?: boolean;
}

const STATE: Record<string, string> = {
  done: "done",
  failed: "failed",
  refused: "not allowed",
  denied: "declined",
  pending: "waiting",
  approved: "running",
  always: "running",
  granting: "running",
  running: "running",
};

function kindIcon(it: ActivityItem): string {
  const toolName = String(it.tool?.tool ?? "");
  if (toolName === "shell") return "terminal";
  if (["read_file", "write_file", "edit_file", "list_dir"].includes(toolName)) return "edit";
  if (toolName === "package") return "box";
  if (toolName === "service") return "gear";
  if (toolName === "reload") return "spark";
  if (toolName === "system_info") return "pc";
  if (it.kind === "grant" || it.kind === "revoke") return "pin";
  return "chat";
}

export function ago(iso: string, now = Date.now()): string {
  const diff = Date.parse(iso) - now;
  const mins = Math.floor(diff / 60000);
  if (mins < 1) return "just now";
  if (mins < 60) return `${mins} min ago`;
  const hours = Math.floor(mins / 60);
  if (hours < 24) return `${hours} h ago`;
  return new Date(iso).toLocaleDateString();
}

function dayLabel(iso: string): string {
  const d = new Date(iso);
  const today = new Date();
  today.setHours(0, 0, 0, 0);
  const yesterday = new Date(today);
  yesterday.setDate(yesterday.getDate() - 1);

  if (d.getTime() === today.getTime()) return "Today";
  if (d.getTime() === yesterday.getTime()) return "Yesterday";
  return d.toLocaleDateString([], { weekday: "short", day: "numeric", month: "short" });
}

export function card(it: ActivityItem): SafeHtml {
  const stateClass = it.state ?? "";
  const stateText = STATE[it.state ?? ""] ?? it.state;
  const timeTitle = new Date(it.at).toLocaleString();

  if (it.kind === "step") {
    return html`<details class="act-card ${stateClass}">
      <summary>
        ${icon(kindIcon(it))}
        <span class="act-text">${it.text}</span>
        <span class="chip ${stateClass}">${stateText}</span>
        <span class="muted small">${it.machine}</span>
        <time datetime="${it.at}" title="${timeTitle}">${ago(it.at)}</time>
      </summary>
      ${it.grant ? html`<p class="act-grant muted small">${icon("pin")} ${it.grant}</p>` : ""}
      ${it.result ? renderOutput(fromTool(it.tool, it.result, it.machine)) : ""}
    </details>`;
  }

  const actionText =
    it.kind === "grant" ? "Allowed" : it.kind === "revoke" ? "Removed" : "Refused";
  const mainText = it.target ?? it.text;

  return html`<div class="act-card act-grant-card ${it.kind}">
    ${icon("pin")}
    <span>${actionText} ${mainText}</span>
    <span class="muted small">${it.detail ?? ""}</span>
    <span class="muted small">${it.machine}</span>
    <time datetime="${it.at}" title="${timeTitle}">${ago(it.at)}</time>
  </div>`;
}

export function renderActivity(items: ActivityItem[], filter: ActivityFilter): SafeHtml {
  const machines = Array.from(new Set(items.map((i) => i.machine))).sort();
  const chats = Array.from(new Set(items.filter((i) => i.chatId).map((i) => i.chatId))).sort();

  const filtered = items.filter((item) => {
    if (filter.machine && item.machine !== filter.machine) return false;
    if (filter.chat && item.chatId !== filter.chat) return false;
    if (filter.failedOnly) {
      const isFailedState = ["failed", "refused", "denied"].includes(item.state ?? "");
      const isRefusedKind = item.kind === "refused";
      if (!isFailedState && !isRefusedKind) return false;
    }
    return true;
  });

  if (filtered.length === 0) {
    return html`<p class="muted pad">Nothing here yet.</p>`;
  }

  const groupedByDay = groupByDay(filtered);

  return html`
    <div class="act-filters">
      <select id="activity-machine" class="small-select" aria-label="Computer">
        <option value="">All computers</option>
        ${machines.map((m) => html`<option value="${m}">${m}</option>`)}
      </select>
      <select id="activity-chat" class="small-select" aria-label="Chat">
        <option value="">All chats</option>
        ${chats.map((c) => html`<option value="${c}">${c}</option>`)}
      </select>
      <label class="act-toggle">
        <input type="checkbox" id="activity-failed" ${filter.failedOnly ? "checked" : ""}> Failed only
      </label>
    </div>
    ${groupedByDay.map((dayGroup) => {
      const dayItems = dayGroup.items;
      const dayChats = Array.from(new Set(dayItems.filter((i) => i.chatId).map((i) => i.chatId))).sort();
      const nonChatItems = dayItems.filter((i) => !i.chatId);

      return html`
        <section class="act-day">
          <h4 class="act-day-head">${dayLabel(dayGroup.day)}</h4>
          ${dayChats.map((chatId) => {
            const chatItems = dayItems.filter((i) => i.chatId === chatId);
            const firstItem = chatItems[0];
            return html`
              <details class="act-chat" open>
                <summary>
                  <button class="linklike" data-action="open-chat" data-id="${chatId}">
                    ${firstItem.chat || "Chat"}
                  </button>
                  <span class="muted small">${chatItems.length} step(s) on ${firstItem.machine}</span>
                </summary>
                ${chatItems.map(card)}
              </details>
            `;
          })}
          ${nonChatItems.map(card)}
        </section>
      `;
    })}
  `;
}

export function groupByDay(items: ActivityItem[]): { day: string; items: ActivityItem[] }[] {
  const map = new Map<string, ActivityItem[]>();
  for (const item of items) {
    const label = dayLabel(item.at);
    if (!map.has(label)) {
      map.set(label, []);
    }
    map.get(label)!.push(item);
  }
  return Array.from(map.entries()).map(([day, list]) => ({ day, items: list }));
}

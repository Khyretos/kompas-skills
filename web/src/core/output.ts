import { html, type SafeHtml } from "./html";
import { icon } from "../views/icons";

export type OutputKind = "terminal" | "code" | "diff" | "json" | "image" | "text";

export interface Output {
  kind: OutputKind;
  text: string;
  title?: string;
  machine?: string;
  lang?: string;
  src?: string;
}

export function splitExit(text: string): { body: string; exit: number | null } {
  const lines = text.split("\n");
  let lastLineIdx = -1;
  for (let i = lines.length - 1; i >= 0; i--) {
    if (lines[i].trim() === "") continue;
    lastLineIdx = i;
    break;
  }
  if (lastLineIdx < 0) return { body: text, exit: null };
  const lastLine = lines[lastLineIdx];
  const match = lastLine.match(/^exit:\s*(-?\d+)$/);
  if (!match) return { body: text, exit: null };
  const exitCode = parseInt(match[1], 10);
  const body = lines.slice(0, lastLineIdx).join("\n");
  return { body, exit: exitCode };
}

export function langOf(path: string): string {
  const ext = path.split(/[\\/]/).pop()?.split(".").pop();
  if (!ext) return "text";
  switch (ext) {
    case "rs":
      return "rust";
    case "ts":
      return "typescript";
    case "js":
      return "javascript";
    case "py":
      return "python";
    case "sh":
    case "bash":
      return "bash";
    case "toml":
      return "toml";
    case "json":
      return "json";
    case "yml":
    case "yaml":
      return "yaml";
    case "conf":
    case "ini":
    case "cfg":
      return "ini";
    case "css":
      return "css";
    case "html":
      return "html";
    case "md":
      return "markdown";
    default:
      return "text";
  }
}

export function fromTool(
  tool: Record<string, unknown> | undefined,
  result: string,
  machine?: string
): Output {
  const toolName = tool?.tool as string | undefined;
  if (toolName === "shell") {
    const command = result.trim();
    return {
      kind: "terminal",
      text: result,
      title: `$ ${command}`,
      machine,
    };
  }
  if (toolName === "package") {
    const manager = tool.manager as string | undefined ?? "";
    const action = tool.action as string | undefined ?? "";
    const names = Array.isArray(tool.names) ? tool.names : [];
    const cmd = `${manager} ${action} ${names.join(" ")}`;
    return {
      kind: "terminal",
      text: result,
      title: cmd,
      machine,
    };
  }
  if (toolName === "service") {
    const action = tool.action as string | undefined ?? "";
    const unit = tool.unit as string | undefined ?? "";
    const cmd = `systemctl --user ${action} ${unit}`;
    return {
      kind: "terminal",
      text: result,
      title: cmd,
      machine,
    };
  }
  if (toolName === "reload") {
    const what = tool.what as string | undefined ?? "";
    const cmd = `reload ${what}`;
    return {
      kind: "terminal",
      text: result,
      title: cmd,
      machine,
    };
  }
  if (toolName === "read_file") {
    const path = tool.path as string | undefined ?? "";
    return {
      kind: "code",
      text: result,
      title: path,
      lang: langOf(path),
      machine,
    };
  }
  if (toolName === "edit_file") {
    const path = tool.path as string | undefined ?? "";
    return {
      kind: "diff",
      text: result,
      title: path,
      machine,
    };
  }
  if (toolName === "system_info") {
    return {
      kind: "json",
      text: result,
      machine,
    };
  }
  if (toolName === "list_dir") {
    const path = tool.path as string | undefined ?? "";
    return {
      kind: "text",
      text: result,
      title: path,
      machine,
    };
  }
  return {
    kind: "text",
    text: result,
    machine,
  };
}

export function renderOutput(o: Output): SafeHtml {
  const { body, exit } = splitExit(o.text);
  const isTerminal = o.kind === "terminal";
  const hasTitle = !!o.title;
  const hasMachine = !!o.machine;

  const headerParts: Array<SafeHtml> = [];

  // Icon
  let iconHtml: SafeHtml;
  switch (o.kind) {
    case "terminal":
      iconHtml = icon("terminal");
      break;
    case "code":
      iconHtml = icon("code");
      break;
    case "diff":
      iconHtml = icon("diff");
      break;
    case "json":
      iconHtml = icon("braces");
      break;
    case "image":
      iconHtml = icon("image");
      break;
    case "text":
      iconHtml = html`<span></span>`;
      break;
  }

  // Label
  let labelHtml: SafeHtml;
  switch (o.kind) {
    case "terminal":
      labelHtml = html`<span class="output-kind">Terminal</span>`;
      break;
    case "code":
      labelHtml = html`<span class="output-kind">Code</span>`;
      break;
    case "diff":
      labelHtml = html`<span class="output-kind">Changes</span>`;
      break;
    case "json":
      labelHtml = html`<span class="output-kind">Data</span>`;
      break;
    case "image":
      labelHtml = html`<span class="output-kind">Image</span>`;
      break;
    case "text":
      labelHtml = html``;
      break;
  }

  // Title
  let titleHtml: SafeHtml | undefined;
  if (hasTitle) {
    titleHtml = html`<code class="output-title">${o.title}</code>`;
  }

  // Machine
  let machineHtml: SafeHtml | undefined;
  if (hasMachine) {
    machineHtml = html`<span class="muted small">on ${o.machine}</span>`;
  }

  // Exit chip
  let exitHtml: SafeHtml | undefined;
  if (isTerminal && exit !== null) {
    const exitClass = exit === 0 ? "ok" : "bad";
    exitHtml = html`<span class="chip exit ${exitClass}">exit ${exit}</span>`;
  }

  // Copy button
  const copyBtn = html`<button class="icon-btn output-copy" data-action="copy-text" data-text="${body}" aria-label="Copy">${icon("copy")}</button>`;

  const headParts = [iconHtml, labelHtml];
  if (hasTitle) headParts.push(titleHtml);
  if (hasMachine) headParts.push(machineHtml);
  if (exitHtml) headParts.push(exitHtml);
  headParts.push(copyBtn);

  const header = html`<figcaption class="output-head">${headParts}</figcaption>`;

  if (o.kind === "terminal") {
    return html`<figure class="output terminal">${header}<pre class="output-body term">${body}</pre></figure>`;
  }

  if (o.kind === "code") {
    const lang = o.lang ?? "text";
    return html`<figure class="output code">${header}<pre class="output-body"><code class="language-${lang}">${o.text}</code></pre></figure>`;
  }

  if (o.kind === "diff") {
    const lines = o.text.split("\n");
    const lineSpans = lines.map((line) => {
      let cls = "ctx";
      if (line.startsWith("+")) cls = "add";
      else if (line.startsWith("-")) cls = "del";
      else if (line.startsWith("@@")) cls = "hunk";
      return html`<span class="line ${cls}">${line}</span>`;
    });
    return html`<figure class="output diff">${header}<pre class="output-body diff">${lineSpans}</pre></figure>`;
  }

  if (o.kind === "json") {
    try {
      const parsed = JSON.parse(o.text);
      const pretty = JSON.stringify(parsed, null, 2);
      return html`<figure class="output json">${header}<details class="output-body json" open><summary>JSON</summary><pre>${pretty}</pre></details></figure>`;
    } catch {
      return html`<figure class="output json">${header}<details class="output-body json" open><summary>JSON</summary><pre>${o.text}</pre></details></figure>`;
    }
  }

  if (o.kind === "image") {
    const src = o.src ?? "";
    const alt = o.title ?? "image";
    return html`<figure class="output image">${header}<button class="output-image" data-action="lightbox" data-src="${src}"><img src="${src}" alt="${alt}" loading="lazy"></button></figure>`;
  }

  if (o.kind === "text") {
    if (hasTitle || hasMachine) {
      return html`<figure class="output text">${header}<p class="output-text">${o.text}</p></figure>`;
    }
    return html`<p class="output-text">${o.text}</p>`;
  }

  return html``;
}

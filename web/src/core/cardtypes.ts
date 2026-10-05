export type CardKind = "read" | "edit" | "run" | "network" | "system" | "git";

export interface KindStyle {
  label: string;
  color: string; // "#rrggbb"
}

export type CardStyle = Partial<Record<CardKind, KindStyle>>;

export const KINDS: CardKind[] = ["read", "edit", "run", "network", "system", "git"];

/** Defaults from the Kreative Kompas palette: purple first, orange second. */
export const DEFAULT_STYLE: Record<CardKind, KindStyle> = {
  read: { label: "Read", color: "#5c398e" },      // violet
  edit: { label: "Edit", color: "#f3941f" },      // orange
  run: { label: "Run command", color: "#cca9ff" }, // lilac
  network: { label: "Network", color: "#fde7cc" }, // orange tint
  system: { label: "Package / system", color: "#bf4eff" }, // neon (a background highlight, never text)
  git: { label: "Git", color: "#2c1f3f" },        // plum-2
};

export function kindOf(tool: Record<string, unknown> | undefined): CardKind {
  if (!tool) return "run";

  const toolName = String(tool.tool ?? "").trim();
  const command = String(tool.command ?? "").trim();

  if (toolName === "read_file" || toolName === "list_dir") return "read";
  if (toolName === "write_file" || toolName === "edit_file") return "edit";
  if (toolName === "package" || toolName === "service" || toolName === "reload" || toolName === "system_info") return "system";

  if (toolName === "shell") {
    if (command.startsWith("git ") || command === "git") return "git";
    const networkRegex = /\b(curl|wget|ssh|scp|rsync|ping|nc)\b/;
    if (networkRegex.test(command)) return "network";
    return "run";
  }

  return "run";
}

export function styleOf(kind: CardKind, custom: CardStyle | undefined): KindStyle {
  if (!custom) return DEFAULT_STYLE[kind];
  
  const customStyle = custom[kind];
  if (!customStyle) return DEFAULT_STYLE[kind];

  const isValidColor = /^#[0-9a-f]{6}$/i.test(customStyle.color);
  if (customStyle.label && isValidColor) {
    return customStyle;
  }

  return DEFAULT_STYLE[kind];
}

export function textOn(hex: string): string {
  let r = 0, g = 0, b = 0;
  if (hex.length === 7) {
    r = parseInt(hex.slice(1, 3), 16);
    g = parseInt(hex.slice(3, 5), 16);
    b = parseInt(hex.slice(5, 7), 16);
  } else {
    r = 12;
    g = 9;
    b = 23;
  }

  const channel = (v: number) => {
    const c = v / 255;
    return c <= 0.03928 ? c / 12.92 : Math.pow((c + 0.055) / 1.055, 2.4);
  };

  const L = 0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b);
  const nightL = 0.2126 * channel(12) + 0.7152 * channel(9) + 0.0722 * channel(23);
  const whiteL = 1.0;

  const contrastNight = (Math.max(L, nightL) + 0.05) / (Math.min(L, nightL) + 0.05);
  const contrastWhite = (Math.max(L, whiteL) + 0.05) / (Math.min(L, whiteL) + 0.05);

  return contrastNight >= contrastWhite ? "#0c0917" : "#ffffff";
}

export type CardNeed = "automatic" | "approval" | "input";

export const NEED_LABEL: Record<CardNeed, string> = {
  automatic: "automatic",
  approval: "needs your approval",
  input: "needs your input",
};

// The in-app viewers (milestone 6): one dialog, and per kind of file a module that loads
// only when it opens (three.js alone is ~700 KB). Everything runs in the browser on the
// original file the server streams; nothing is made on anyone's PC.
import type { AssetDetail, AssetsApi } from "../api/assets";

export type ViewerKind = "model" | "image" | "preview-image" | "video" | "audio" | "font" | "text" | "none";

const MODEL = new Set(["fbx", "obj", "gltf", "glb", "bvh", "stl", "ply", "dae"]);
const IMAGE = new Set(["png", "jpg", "jpeg", "gif", "webp", "bmp", "svg", "avif"]);
const VIDEO = new Set(["mp4", "m4v", "webm", "mov", "ogv"]);
const AUDIO = new Set(["mp3", "ogg", "oga", "wav", "flac", "m4a", "aac", "opus"]);
const FONT = new Set(["ttf", "otf", "woff", "woff2"]);
const TEXT = new Set(["txt", "md", "json", "gltf", "cs", "py", "lua", "shader", "hlsl", "glsl", "cginc", "vert", "frag", "mtl", "xml", "yaml", "yml", "csv", "ini", "cfg", "toml"]);
/** Bigger files open in the viewer only after asking (a 300 MB FBX takes a while). */
const BIG = 150 * 1024 * 1024;

export function viewerKind(a: { ext: string; container: string; preview: string | null; size: number }): ViewerKind {
  const ext = a.ext.toLowerCase();
  if (a.container.toLowerCase().endsWith(".unitypackage")) return a.preview === "image" ? "preview-image" : "none";
  if (MODEL.has(ext)) return "model";
  if (IMAGE.has(ext)) return "image";
  if (a.preview === "image") return "preview-image"; // tga, psd, dds…: the server's 1024 px preview
  if (VIDEO.has(ext)) return "video";
  if (AUDIO.has(ext)) return "audio";
  if (FONT.has(ext)) return "font";
  if (TEXT.has(ext) && a.size <= 8 * 1024 * 1024) return "text";
  return "none";
}

/** The button label for an asset's viewer. */
export function viewerLabel(kind: ViewerKind, category: string): string {
  if (kind === "model") return category === "animation" ? "Play the animation" : "View in 3D";
  if (kind === "font") return "Show the font";
  if (kind === "video") return "Play the video";
  if (kind === "text") return "Read the file";
  if (kind === "none") return "File details";
  return "Open the viewer";
}

const el = <K extends keyof HTMLElementTagNameMap>(tag: K, props: Partial<HTMLElementTagNameMap[K]> = {}, ...kids: (Node | string)[]) => {
  const e = Object.assign(document.createElement(tag), props);
  e.append(...kids);
  return e;
};

let open: { dialog: HTMLDialogElement; close: () => void } | undefined;

/** Opens the viewer for one asset in a dialog; closing it frees everything it loaded. */
export function openViewer(a: AssetDetail, api: AssetsApi, onClosed?: () => void): void {
  open?.close();
  const kind = viewerKind(a);
  const body = el("div", { className: "viewer-body" });
  const close = el("button", { type: "button", className: "icon-btn viewer-close", textContent: "×" });
  close.setAttribute("aria-label", "Close the viewer");
  const download = el("a", { className: "btn small", href: api.fileUrl(a), textContent: "Download", download: a.name });
  const head = el("header", { className: "viewer-head" },
    el("h2", { textContent: a.name }), el("small", { className: "muted", textContent: a.pack }), download, close);
  const dialog = el("dialog", { className: `asset-viewer kind-${kind}` }, head, body);
  dialog.setAttribute("aria-label", `Viewer: ${a.name}`);
  document.body.append(dialog);
  let cleanup: (() => void) | undefined;
  let closed = false;
  const finish = () => {
    if (closed) return;
    closed = true;
    try { cleanup?.(); } catch { /* already gone */ }
    dialog.remove();
    if (open?.dialog === dialog) open = undefined;
    onClosed?.();
  };
  close.addEventListener("click", () => dialog.close());
  dialog.addEventListener("close", finish);
  dialog.addEventListener("click", (ev) => { if (ev.target === dialog) dialog.close(); });
  open = { dialog, close: () => dialog.close() };
  dialog.showModal();
  void show(kind, a, api, body).then((c) => { if (closed) c?.(); else cleanup = c; }, (e: unknown) => {
    body.replaceChildren(unsupported(a, e instanceof Error ? e.message : String(e)));
  });
}

function unsupported(a: AssetDetail, why: string): HTMLElement {
  return el("div", { className: "viewer-none" },
    el("p", { className: "viewer-none-title", textContent: why }),
    el("p", { className: "muted", textContent: `.${a.ext} · ${a.container ? `${a.container} › ` : ""}${a.path}` }));
}

async function show(kind: ViewerKind, a: AssetDetail, api: AssetsApi, body: HTMLElement): Promise<(() => void) | undefined> {
  const url = api.fileUrl(a);
  switch (kind) {
    case "model": {
      if (a.size > BIG && !confirm(`${a.name} is ${Math.round(a.size / 1048576)} MB. Load it anyway?`)) {
        body.replaceChildren(unsupported(a, "Not loaded: the file is very big."));
        return undefined;
      }
      body.replaceChildren(el("p", { className: "muted", textContent: "Loading the 3D viewer…" }));
      const { mountModel } = await import("./model");
      return mountModel(body, {
        url, ext: a.ext.toLowerCase(), name: a.name, near: (name) => api.nearUrl(a.id, name),
        textures: async () => {
          const list = await api.list({ pack: a.packId, category: "texture" }, 0, 300);
          return list.items.filter((t) => t.container === a.container && /\.(png|jpe?g|tga)$/i.test(t.name))
            .map((t) => ({ name: t.name, url: api.fileUrl(t) }));
        },
      });
    }
    case "image":
    case "preview-image": {
      const { mountImage } = await import("./image");
      const c = mountImage(body, kind === "image" ? url : api.previewUrl(a, "l"), `${a.name}`);
      if (kind === "preview-image") body.append(el("p", { className: "muted viewer-note", textContent: `The browser can't open .${a.ext} files, so this is the server's 1024 px preview.` }));
      return c;
    }
    case "video": {
      const v = el("video", { controls: true, preload: "metadata", src: url, className: "viewer-video" });
      v.addEventListener("error", () => body.replaceChildren(unsupported(a, `This browser can't play this .${a.ext} video (its codec isn't supported). Download it to watch.`)));
      body.replaceChildren(v);
      return () => { v.pause(); v.removeAttribute("src"); v.load(); };
    }
    case "audio": {
      const au = el("audio", { controls: true, preload: "metadata", src: url, className: "viewer-audio" });
      au.addEventListener("error", () => body.replaceChildren(unsupported(a, `This browser can't play .${a.ext} files.`)));
      body.replaceChildren(au);
      return () => { au.pause(); au.removeAttribute("src"); au.load(); };
    }
    case "font": {
      const { mountFont } = await import("./font");
      return mountFont(body, url, a.name);
    }
    case "text": {
      const { mountText } = await import("./text");
      return mountText(body, url, a.ext);
    }
    default:
      body.replaceChildren(unsupported(a, a.container.toLowerCase().endsWith(".unitypackage")
        ? "Files inside Unity packages can't be previewed in the browser."
        : `No in-browser preview for .${a.ext} files.`));
      return undefined;
  }
}

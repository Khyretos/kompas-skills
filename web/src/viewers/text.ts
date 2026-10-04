// Read-only view of a text file (shaders, scripts, JSON, readmes), highlighted with
// highlight.js and cleaned by DOMPurify like the chat's code blocks.
import hljs from "highlight.js/lib/core";
import csharp from "highlight.js/lib/languages/csharp";
import cpp from "highlight.js/lib/languages/cpp";
import glsl from "highlight.js/lib/languages/glsl";
import json from "highlight.js/lib/languages/json";
import lua from "highlight.js/lib/languages/lua";
import markdown from "highlight.js/lib/languages/markdown";
import python from "highlight.js/lib/languages/python";
import xml from "highlight.js/lib/languages/xml";
import yaml from "highlight.js/lib/languages/yaml";
import ini from "highlight.js/lib/languages/ini";
import DOMPurify from "dompurify";

for (const [name, lang] of Object.entries({ csharp, cpp, glsl, json, lua, markdown, python, xml, yaml, ini })) hljs.registerLanguage(name, lang);

const LANG: Record<string, string> = {
  cs: "csharp", hlsl: "cpp", cginc: "cpp", shader: "cpp", glsl: "glsl", vert: "glsl", frag: "glsl", json: "json", gltf: "json",
  lua: "lua", md: "markdown", py: "python", xml: "xml", yaml: "yaml", yml: "yaml", ini: "ini", cfg: "ini", toml: "ini",
};
/** Bigger files are cut (the view says so). */
const MAX_BYTES = 1024 * 1024;

export async function mountText(el: HTMLElement, url: string, ext: string): Promise<() => void> {
  const res = await fetch(url, { credentials: "same-origin" });
  if (!res.ok) throw new Error(`The file couldn't be loaded (${res.status}).`);
  const buf = await res.arrayBuffer();
  let text = new TextDecoder("utf-8").decode(buf.slice(0, MAX_BYTES));
  const lang = LANG[ext.toLowerCase()];
  if (lang === "json") { try { text = JSON.stringify(JSON.parse(text), null, 2); } catch { /* shown as it is */ } }
  const pre = Object.assign(document.createElement("pre"), { className: "text-view" });
  const code = document.createElement("code");
  pre.append(code);
  if (lang && text.length < 400_000) {
    const html = hljs.highlight(text, { language: lang, ignoreIllegals: true }).value;
    code.append(DOMPurify.sanitize(html, { ALLOWED_TAGS: ["span"], ALLOWED_ATTR: ["class"], RETURN_DOM_FRAGMENT: true }));
    code.classList.add("hljs");
  } else {
    code.textContent = text;
  }
  el.replaceChildren(pre);
  if (buf.byteLength > MAX_BYTES) el.append(Object.assign(document.createElement("p"), { className: "muted", textContent: "Only the first 1 MB is shown." }));
  return () => el.replaceChildren();
}

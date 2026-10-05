// Chat markdown: `marked` turns text into HTML, DOMPurify removes anything
// unsafe, and the result is returned as a DOM fragment. Model output is
// untrusted: an XSS in an agent UI is remote code execution on your PCs
// (see OpenCode CVE-2026-22813, Open WebUI CVE-2025-46719).
import { Marked } from "marked";
import DOMPurify from "dompurify";
import { enhanceCodeBlocks } from "./codeblocks";

const md = new Marked({ gfm: true, breaks: true, async: false });

DOMPurify.addHook("uponSanitizeAttribute", (node, data) => {
  // Keep only "language-xyz" classes on code elements (for highlighting).
  if (data.attrName === "class") {
    data.keepAttr = node.tagName === "CODE" ? /^language-[\w+-]+$/.test(data.attrValue) :
      node.tagName === "SPAN" && /^(hljs-[\w-]+\s*)+$/.test(data.attrValue);
    data.forceKeepAttr = data.keepAttr;
  }
});

DOMPurify.addHook("afterSanitizeAttributes", (node) => {
  if (node.tagName === "A") {
    const href = node.getAttribute("href") ?? "";
    // Keep in-app hash links (#task=... or #chat=...) without target/rel; block everything else.
    if (/^#(task|chat)=[\w-]+$/.test(href)) {
      return;
    }
    if (!/^https?:\/\//i.test(href)) node.removeAttribute("href");
    node.setAttribute("target", "_blank");
    node.setAttribute("rel", "noopener noreferrer nofollow");
  }
});

const config = {
  ALLOWED_TAGS: ["p", "br", "strong", "em", "del", "code", "pre", "ul", "ol", "li", "blockquote",
    "h1", "h2", "h3", "h4", "h5", "h6", "a", "table", "thead", "tbody", "tr", "th", "td", "hr"],
  ALLOWED_ATTR: ["href", "target", "rel", "align"],
  RETURN_DOM_FRAGMENT: true as const,
};

/** Untrusted markdown in, safe DOM fragment out. */
export function renderMarkdown(src: string): DocumentFragment {
  const raw = md.parse(src) as string;
  const frag = DOMPurify.sanitize(raw, config);
  enhanceCodeBlocks(frag);
  return frag;
}

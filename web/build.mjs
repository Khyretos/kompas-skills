// Bundles src/main.ts with esbuild. `--serve` runs a dev server with live rebuilds.
// `--single` writes dist/preview.html with JS and CSS inlined (for sharing a preview).
import * as esbuild from "esbuild";
import { readFile, writeFile, mkdir, copyFile } from "node:fs/promises";

const serve = process.argv.includes("--serve");
const single = process.argv.includes("--single");

const options = {
  entryPoints: ["src/main.ts", "src/styles.css"],
  bundle: true,
  // Viewers (three.js and friends) load only when one opens: separate chunks.
  splitting: !single,
  chunkNames: "chunks/[name]-[hash]",
  outdir: "dist",
  format: "esm",
  target: "es2022",
  minify: !serve,
  sourcemap: serve,
  logLevel: "info",
  // Only the shareable preview runs on demo data.
  define: { __DEMO__: JSON.stringify(single) },
  loader: { ".svg": "dataurl" }, // logos as data: URLs (CSP allows img data:)
};

await mkdir("dist", { recursive: true });
await copyFile("index.html", "dist/index.html");

if (serve) {
  const ctx = await esbuild.context(options);
  await ctx.watch();
  const { port } = await ctx.serve({ servedir: "dist", port: 5173 });
  console.log(`Kompanion dev server: http://localhost:${port}`);
} else {
  await esbuild.build(options);
  if (single) {
    const js = await readFile("dist/main.js", "utf8");
    const css = await readFile("dist/styles.css", "utf8");
    // The shareable preview inlines everything, so it drops the app's CSP meta
    // (the host page applies its own policy).
    const html = (await readFile("index.html", "utf8"))
      .replace(/\s*<!-- Strict CSP[\s\S]*?-->\s*<meta http-equiv="Content-Security-Policy"[^>]*>/, "")
      .replace(/<link rel="stylesheet" href="styles.css">/, () => `<style>${css}</style>`)
      .replace(/<script type="module" src="main.js"><\/script>/, () => `<script type="module">${js}</script>`);
    await writeFile("dist/preview.html", html);
    console.log("Wrote dist/preview.html");
  }
}

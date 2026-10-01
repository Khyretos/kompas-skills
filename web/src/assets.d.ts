// esbuild inlines SVG files as data: URLs (see build.mjs).
declare module "*.svg" {
  const url: string;
  export default url;
}

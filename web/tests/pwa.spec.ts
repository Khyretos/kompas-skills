import { test, expect } from "@playwright/test";

// Apps 2: the web app installs as an app (manifest, icons, service worker file).
test("the app has a manifest with icons and a service worker file", async ({ page, request }) => {
  await page.goto("/?demo");
  await expect(page.locator('link[rel="manifest"]')).toHaveAttribute("href", "manifest.webmanifest");
  const manifest = await (await request.get("/manifest.webmanifest")).json();
  expect(manifest.name).toBe("Kreative Kompanion");
  expect(manifest.display).toBe("standalone");
  expect(manifest.icons.map((i: { sizes: string }) => i.sizes)).toEqual(["192x192", "512x512", "512x512"]);
  for (const icon of manifest.icons) expect((await request.get(`/${icon.src}`)).status()).toBe(200);
  const sw = await request.get("/sw.js");
  expect(sw.status()).toBe(200);
  expect(await sw.text()).toContain("kk-shell-v1");
  // Only over HTTPS: the dev server (http) registers no service worker.
  expect(await page.evaluate(async () => (await navigator.serviceWorker?.getRegistrations())?.length ?? 0)).toBe(0);
});

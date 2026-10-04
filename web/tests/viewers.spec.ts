// The viewer dialog in demo mode (real files are tested in tests-real/ against a server
// with the library): it opens from the details, shows a picture or a clear card, and
// closes without a reload. Heavy viewers are separate chunks, loaded only on demand.
import { test, expect } from "@playwright/test";

test.beforeEach(async ({ page }) => {
  await page.goto("/?demo");
  await page.click("button.found-server");
  await page.click('[data-action="assets"]');
  await expect(page.locator(".asset-card:not(.skeleton)").first()).toBeVisible();
});

test("a model opens the 3D viewer only on demand and closes cleanly", async ({ page }) => {
  const chunks: string[] = [];
  page.on("request", (r) => { if (r.url().includes("/chunks/model-")) chunks.push(r.url()); });
  await page.click('[data-action="asset-cat"][data-cat="3d-model"]');
  await page.locator("li.asset-cell button.asset-card").first().click();
  expect(chunks).toHaveLength(0);
  const open = page.locator('#asset-detail [data-action="asset-view"]');
  await expect(open).toHaveText("View in 3D");
  await open.click();
  const dlg = page.locator("dialog.asset-viewer");
  await expect(dlg).toBeVisible();
  // Demo mode has no files: the viewer says so instead of breaking.
  await expect(dlg.locator(".model-status, .viewer-none")).toContainText(/couldn't be shown|Not found|can't/, { timeout: 15_000 });
  await page.keyboard.press("Escape");
  await expect(dlg).toHaveCount(0);
  await expect(open).toBeFocused();
});

test("a picture opens in the zoomable viewer", async ({ page }) => {
  await page.click('[data-action="asset-cat"][data-cat="sprite"]');
  await page.locator("li.asset-cell button.asset-card").first().click();
  await page.locator('#asset-detail [data-action="asset-view"]').click();
  const dlg = page.locator("dialog.asset-viewer");
  await expect(dlg.locator(".img-stage[data-state=ready]")).toBeVisible();
  await expect(dlg.locator("output.img-zoom")).toContainText("%");
  await dlg.locator(".viewer-close").click();
  await expect(dlg).toHaveCount(0);
});

test("a file the browser can't show gets a clear card", async ({ page }) => {
  await page.click('[data-action="asset-cat"][data-cat="doc"]');
  await page.locator("li.asset-cell button.asset-card", { hasText: "LICENSE.pdf" }).click();
  await page.locator('#asset-detail [data-action="asset-view"]').click();
  await expect(page.locator("dialog.asset-viewer .viewer-none")).toContainText("No in-browser preview for .pdf files");
});

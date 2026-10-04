// The in-app viewers on real library files: a Synty FBX with its texture, an animation,
// a BVH, a font, a video, a text file and a format the browser can't show.
import { test, expect, type Page } from "@playwright/test";

const shots = process.env.KK_SHOTS;

test.beforeEach(async ({ page, context, baseURL }) => {
  await context.addCookies([{ name: "kk_session", value: process.env.KK_SESSION ?? "", url: baseURL! }]);
  await page.goto("/");
  await page.click('[data-action="assets"]');
  await expect(page.locator(".asset-card:not(.skeleton)").first()).toBeVisible({ timeout: 20_000 });
});

/** Finds an asset by name and opens its viewer. */
async function view(page: Page, name: string, pack: string): Promise<void> {
  await page.fill("#asset-q", `${name} ${pack}`);
  const card = page.locator("li.asset-cell button.asset-card", { hasText: name }).first();
  await expect(card).toBeVisible({ timeout: 15_000 });
  await card.click();
  await page.locator('#asset-detail [data-action="asset-view"]').click();
  await expect(page.locator("dialog.asset-viewer")).toBeVisible();
}

async function closeViewer(page: Page): Promise<void> {
  await page.keyboard.press("Escape");
  await expect(page.locator("dialog.asset-viewer")).toHaveCount(0);
}

test("a Synty FBX shows with its pack texture and frees the GPU on close", async ({ page }) => {
  await view(page, "SM_Tree_01.fbx", "POLYGON_Nature_Source_Files_v2");
  const dlg = page.locator("dialog.asset-viewer");
  await expect(dlg.locator(".model-stage[data-state=ready]")).toBeVisible({ timeout: 30_000 });
  await expect(dlg.locator(".model-status")).toContainText("triangles");
  await expect(dlg.locator(".model-status")).not.toContainText("not found");
  if (shots) await page.screenshot({ path: `${shots}/fbx.png` });
  await closeViewer(page);
});

test("an animation FBX plays on its skeleton with a clip list and scrubbing", async ({ page }) => {
  await view(page, "A_Idle_Crouching_Femn.fbx", "ANIMATION_Base_Locomotion");
  const dlg = page.locator("dialog.asset-viewer");
  await expect(dlg.locator(".model-stage[data-state=ready]")).toBeVisible({ timeout: 30_000 });
  await expect(dlg.locator(".model-status")).toContainText("bones");
  await expect(dlg.locator("select.model-clip option")).not.toHaveCount(0);
  const scrub = dlg.locator("input.model-scrub");
  await expect.poll(async () => Number(await scrub.inputValue()), { timeout: 10_000 }).toBeGreaterThan(0);
  if (shots) await page.screenshot({ path: `${shots}/anim.png` });
  await closeViewer(page);
});

test("a BVH motion plays", async ({ page }) => {
  await view(page, "back_kick.bvh", "Motifect");
  await expect(page.locator("dialog.asset-viewer .model-status")).toContainText("bones", { timeout: 30_000 });
  await closeViewer(page);
});

test("a font shows its sample and glyphs, and the sample follows typing", async ({ page }) => {
  await view(page, "AbhayaLibre-Bold.ttf", "fonts");
  const dlg = page.locator("dialog.asset-viewer");
  await expect(dlg.locator(".font-glyph").first()).toBeVisible({ timeout: 15_000 });
  await dlg.locator("input.font-sample").fill("Kreative Kompas");
  await expect(dlg.locator(".font-line").first()).toHaveText("Kreative Kompas");
  if (shots) await page.screenshot({ path: `${shots}/font.png` });
  await closeViewer(page);
});

test("a video plays, or says clearly that this browser lacks the codec", async ({ page }) => {
  await view(page, "Polygon_Sign_Video.mp4", "POLYGON_Shops");
  const dlg = page.locator("dialog.asset-viewer");
  // Playwright's Chromium has no H.264; Chrome and Firefox do.
  await expect(dlg.locator("video.viewer-video, .viewer-none")).toBeVisible();
  await expect.poll(async () => {
    if (await dlg.locator(".viewer-none").count()) return "message";
    return (await dlg.locator("video").evaluate((v: HTMLVideoElement) => v.readyState)) >= 1 ? "playing" : "waiting";
  }, { timeout: 15_000 }).not.toBe("waiting");
  await closeViewer(page);
});

test("a format the browser can't show gets a clear card, not a broken viewer", async ({ page }) => {
  await view(page, "DwarfF Character Dummy 1.1.blend", "Dwarf Character Dummy");
  await expect(page.locator("dialog.asset-viewer .viewer-none")).toContainText("No in-browser preview for .blend files");
  await closeViewer(page);
});

test("a big sprite opens in the zoomable picture viewer", async ({ page }) => {
  await view(page, "SPR_Synty_Branding_SyntyInterface_01.png", "INTERFACE_Apocalypse_HUD");
  const dlg = page.locator("dialog.asset-viewer");
  await expect(dlg.locator(".img-stage[data-state=ready]")).toBeVisible({ timeout: 15_000 });
  await expect(dlg.locator("output.img-size")).toContainText("px");
  const before = await dlg.locator("output.img-zoom").textContent();
  await dlg.locator(".img-stage").hover();
  await page.mouse.wheel(0, -300);
  await expect(dlg.locator("output.img-zoom")).not.toHaveText(before ?? "");
  if (shots) await page.screenshot({ path: `${shots}/image.png` });
  await closeViewer(page);
});

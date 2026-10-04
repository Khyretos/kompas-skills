// Assets section in demo mode: filtering, the detail panel and a live scan all
// update the page without a reload (Kees's standing rule).
import { test, expect, type Page } from "@playwright/test";

async function openAssets(page: Page): Promise<void> {
  await page.goto("/?demo");
  await page.click("button.found-server");
  await page.click('[data-action="assets"]');
  await expect(page.locator(".asset-card:not(.skeleton)").first()).toBeVisible();
}

test.describe("Assets", () => {
  let loads = 0;
  test.beforeEach(async ({ page }) => {
    loads = 0;
    page.on("load", () => loads++);
    await openAssets(page);
  });
  test.afterEach(() => expect(loads).toBe(1));

  test("only the visible cards are in the page", async ({ page }) => {
    await expect(page.locator("#asset-summary")).toContainText("files in");
    const total = Number((await page.locator('[data-action="asset-cat"][data-cat=""] span').textContent())?.replace(/,/g, ""));
    expect(total).toBeGreaterThan(500);
    expect(await page.locator(".asset-card").count()).toBeLessThan(total / 2);
    // Scrolling far down draws the cards that are now in view.
    await page.locator("#asset-scroll").evaluate((el) => { el.scrollTop = el.scrollHeight; });
    await expect(page.locator(".asset-card", { hasText: "PolygonNature_Texture_40" }).first()).toBeVisible();
  });

  test("a category chip and the search box filter at once", async ({ page }) => {
    await page.click('[data-action="asset-cat"][data-cat="music"]');
    await expect(page.locator('[data-action="asset-cat"][data-cat="music"]')).toHaveAttribute("aria-pressed", "true");
    await expect(page.locator(".asset-card:not(.skeleton)").first()).toContainText("Music");
    await expect(page.locator(".asset-card.cat-3d-model")).toHaveCount(0);
    await page.fill("#asset-q", "town");
    await expect(page.locator('[data-action="asset-cat"][data-cat="music"] span')).toHaveText("40");
    await expect(page.locator(".asset-card:not(.skeleton)").first()).toContainText("Town_01");
    await page.fill("#asset-q", "nothing-like-this");
    await expect(page.locator(".asset-empty")).toContainText("Nothing matches");
    await page.click('[data-action="asset-clear"]');
    await expect(page.locator('[data-action="asset-cat"][data-cat=""]')).toHaveAttribute("aria-pressed", "true");
  });

  test("a card opens its details with the licence state", async ({ page }) => {
    await page.fill("#asset-q", "fantasy action 01");
    await page.locator(".asset-card", { hasText: "Action_01" }).click();
    const panel = page.locator("#asset-detail");
    await expect(panel).toBeVisible();
    await expect(panel).toContainText("Not cleared to ship");
    await expect(panel).toContainText("LICENSE.pdf");
    // The pack link filters the grid to that pack.
    await panel.locator('[data-action="asset-pack"]').click();
    await expect(panel).toBeHidden();
    await expect(page.locator("#asset-pack")).not.toHaveValue("");
    await page.keyboard.press("Escape");
  });

  test("a scan shows progress and its result arrives live", async ({ page }) => {
    await expect(page.locator("#asset-pack option", { hasText: "Free Ambience Loops" })).toHaveCount(0);
    await page.click('[data-action="asset-scan"]');
    await expect(page.locator(".scan-live")).toBeVisible({ timeout: 300 });
    await expect(page.locator(".scan-live")).toBeHidden({ timeout: 5000 });
    await expect(page.locator("#asset-pack option", { hasText: "Free Ambience Loops" })).toHaveCount(1);
    await expect(page.locator("#asset-summary")).toContainText("just now");
  });

  test("going back to a chat leaves the Assets section", async ({ page }) => {
    await page.locator('[data-action="open-chat"]').first().click();
    await expect(page.locator("#assets")).toBeHidden();
    await expect(page.locator("#messages")).toBeVisible();
    await page.click('[data-action="assets"]');
    await expect(page.locator(".asset-card:not(.skeleton)").first()).toBeVisible();
  });

  test("sound cards show a waveform, their length and a play button", async ({ page }) => {
    await page.click('[data-action="asset-cat"][data-cat="music"]');
    const firstCard = page.locator("li.asset-cell").first();
    await expect(firstCard.locator("svg.wave")).toBeVisible();
    const durationText = await firstCard.locator(".asset-dur").textContent();
    expect(durationText).toMatch(/^(\d+:\d\d|\d+\.\d s)$/);
    await expect(firstCard.locator("button.asset-play")).toHaveAttribute("aria-label", /^Play /);
  });

  test("a picture's preview arrives live", async ({ page }) => {
    await page.click('[data-action="asset-cat"][data-cat="texture"]');
    const imgLocator = page.locator("li.asset-cell[data-id=\"243\"] .asset-thumb img");
    await expect(imgLocator).toHaveCount(0);
    await expect(imgLocator).toBeVisible({ timeout: 5000 });
  });

  test("a sound's details show a player and its length", async ({ page }) => {
    await page.click('[data-action="asset-cat"][data-cat="music"]');
    await page.locator("li.asset-cell").first().locator("button.asset-card").click();
    const panel = page.locator("#asset-detail");
    await expect(panel.locator("audio")).toBeAttached();
    await expect(panel).toContainText("Length");
  });

  test("a picture's details show the large preview and its size", async ({ page }) => {
    await page.click('[data-action="asset-cat"][data-cat="texture"]');
    await page.locator("li.asset-cell[data-id=\"241\"] button.asset-card").click();
    const panel = page.locator("#asset-detail");
    await expect(panel.locator(".asset-preview.is-image img")).toBeVisible();
    await expect(panel).toContainText("1024 × 1024 px");
  });

  test("the AI's other category is listed and accepting it updates the card live", async ({ page }) => {
    const chip = page.locator('[data-action="asset-review"]');
    await expect(chip).toBeVisible();
    const before = Number((await chip.locator("span").textContent())?.replace(/,/g, ""));
    expect(before).toBeGreaterThan(0);
    await chip.click();
    await expect(chip).toHaveAttribute("aria-pressed", "true");
    const first = page.locator("li.asset-cell").first();
    await expect(first.locator(".asset-flag")).toContainText("Music");
    const id = await first.getAttribute("data-id");
    await first.locator("button.asset-card").click();
    await page.locator('#asset-detail [data-action="asset-cat-accept"]').click();
    await expect(page.locator(`li.asset-cell[data-id="${id}"] .asset-flag`)).toHaveCount(0);
    await expect(chip.locator("span")).toHaveText(String(before - 1), { timeout: 5000 });
  });

  test("tags are added and removed in the details without a reload", async ({ page }) => {
    await page.click('[data-action="asset-cat"][data-cat="music"]');
    await page.locator("li.asset-cell button.asset-card").first().click();
    const tags = page.locator("#asset-tags");
    await expect(tags).toContainText("epic");
    await page.fill('#asset-tag-form input', "Boss Fight");
    await page.press('#asset-tag-form input', "Enter");
    await expect(tags.locator(".asset-tag", { hasText: "boss fight" })).toBeVisible({ timeout: 300 });
    await expect(tags.locator(".asset-tag.kees", { hasText: "boss fight" })).toBeVisible();
    await tags.locator('.asset-tag', { hasText: "epic" }).locator(".tag-x").click();
    await expect(tags.locator(".asset-tag", { hasText: "epic" })).toBeHidden();
  });

  test("a tag and Find similar filter the grid, each with a chip to undo it", async ({ page }) => {
    await page.click('[data-action="asset-cat"][data-cat="music"]');
    await page.locator("li.asset-cell button.asset-card").first().click();
    await page.locator('#asset-detail [data-action="asset-tag-filter"]', { hasText: "epic" }).click();
    const active = page.locator("#asset-active");
    await expect(active).toContainText("mood: epic");
    await expect(page.locator("li.asset-cell").first()).toBeVisible();
    await active.locator("button").click();
    await expect(active).toBeHidden();
    await page.locator("li.asset-cell button.asset-card").first().click();
    await page.locator('#asset-detail [data-action="asset-similar"]').click();
    await expect(active).toContainText("Like");
    await expect(page.locator("li.asset-cell .asset-card.cat-music").first()).toBeVisible();
  });

  test("an admin turns AI tagging on and Describe now tags the asset live", async ({ page }) => {
    await expect(page.locator("#asset-ai")).toContainText("off");
    await expect(page.locator("#asset-meaning")).toBeDisabled();
    await page.selectOption("#asset-ai-mode", "always");
    await expect(page.locator("#asset-ai")).toContainText("is on");
    await expect(page.locator("#asset-meaning")).toBeEnabled();
    await page.click('[data-action="asset-cat"][data-cat="texture"]');
    await page.locator("li.asset-cell button.asset-card").first().click();
    await expect(page.locator("#asset-tags .asset-tag")).toHaveCount(0);
    await page.locator('#asset-detail [data-action="asset-describe"]').click();
    await expect(page.locator("#asset-tags .asset-tag", { hasText: "epic" })).toBeVisible({ timeout: 5000 });
  });
});

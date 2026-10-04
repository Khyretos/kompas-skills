import { test, expect, type Page } from "@playwright/test";

async function openGames(page: Page): Promise<void> {
  await page.goto("/?demo");
  await page.click("button.found-server");
  await page.click('[data-action="assets"]');
  await expect(page.locator(".asset-card:not(.skeleton)").first()).toBeVisible();
  await page.click('[data-action="asset-tab"][data-tab="games"]');
  await expect(page.locator(".game-list")).toBeVisible();
}

test.describe("Games", () => {
  let loads = 0;
  test.beforeEach(async ({ page }) => {
    loads = 0;
    page.on("load", () => loads++);
    await openGames(page);
  });
  test.afterEach(() => expect(loads).toBe(1));

  test("AI tagging off hides picking buttons and shows instruction text", async ({ page }) => {
    const needs = page.locator("ol.needs li.need");
    await expect(needs.first().locator("[data-action='need-pick']")).not.toBeVisible();
    await expect(page.locator("#game-profile")).toContainText("Turn AI tagging on for drafts and picks.");
  });

  test("Turning AI on reveals Find assets buttons", async ({ page }) => {
    await page.selectOption("#asset-ai-mode", "always");
    const needs = page.locator("ol.needs li.need");
    await expect(needs.first().locator("[data-action='need-pick']")).toBeVisible();
  });

  test("Finding assets shows reasons live in pick items", async ({ page }) => {
    await page.selectOption("#asset-ai-mode", "always");
    const footstepsNeed = page.locator("li.need", { hasText: "footsteps on grass" });
    const searchBtn = footstepsNeed.locator("[data-action='need-pick']");
    await searchBtn.click();
    
    // The busy state shows at once (set before the server answers), so it's always there to
    // see. "Waiting…" turns into "Searching and picking…" only while the server works, which
    // in demo mode can be over before a re-render: asserting that label raced (CI flake).
    await expect(footstepsNeed.locator(".need-state")).toHaveText(/Waiting…|Searching and picking…/);
    await expect(searchBtn).toBeDisabled();
    await expect(footstepsNeed.locator("ul.picks li.pick").first()).toBeVisible();
    await expect(footstepsNeed.locator(".need-state")).toHaveText("");
    await expect(searchBtn).toHaveText("Pick again");
    await expect(footstepsNeed.locator(".pick-reason").first()).toContainText(/Matches "(footsteps|grass)"/);
  });

  test("Keeping a pick adds candidate class, kept chip, and updates count", async ({ page }) => {
    await page.selectOption("#asset-ai-mode", "always");
    const footstepsNeed = page.locator("li.need", { hasText: "footsteps on grass" });
    await footstepsNeed.locator("[data-action='need-pick']").click();
    await expect(footstepsNeed.locator("ul.picks li.pick").first()).toBeVisible();
    
    const firstPick = footstepsNeed.locator("ul.picks li.pick").first();
    await firstPick.locator("[data-action='pick-keep']").click();
    
    await expect(firstPick).toHaveClass(/candidate/);
    await expect(firstPick.locator(".chip.kept")).toHaveText("Kept");
    await expect(page.locator(".game-item[data-action='game-select'][aria-current='true']")).toContainText("1 kept");
  });

  test("Rejecting a pick removes it and shows rejection count", async ({ page }) => {
    await page.selectOption("#asset-ai-mode", "always");
    const pineNeed = page.locator("li.need", { hasText: "pine trees" });
    await pineNeed.locator("[data-action='need-pick']").click();
    await expect(pineNeed.locator("ul.picks li.pick").first()).toBeVisible();
    
    const name = (await pineNeed.locator("li.pick .pick-name strong").first().textContent()) ?? "";
    await pineNeed.locator("li.pick").first().locator("[data-action='pick-reject']").click();
    await expect(pineNeed.locator("li.pick", { hasText: name })).toHaveCount(0);
    await expect(pineNeed).toContainText("1 rejected");
  });

  test("Licence chips show OK to ship or Licence not linked based on pack", async ({ page }) => {
    await page.selectOption("#asset-ai-mode", "always");
    const pineNeed = page.locator("li.need", { hasText: "pine trees" });
    await pineNeed.locator("[data-action='need-pick']").click();
    await expect(pineNeed.locator("ul.picks li.pick").first()).toBeVisible();
    
    const firstPick = pineNeed.locator("ul.picks li.pick").first();
    await expect(firstPick.locator(".chip.ship-ok")).toHaveText("OK to ship");
    
    const footstepsNeed = page.locator("li.need", { hasText: "footsteps on grass" });
    await footstepsNeed.locator("[data-action='need-pick']").click();
    await expect(footstepsNeed.locator("ul.picks li.pick").first()).toBeVisible();
    
    const secondPick = footstepsNeed.locator("ul.picks li.pick").first();
    await expect(secondPick.locator(".chip.ship-no")).toHaveText("Licence not linked");
  });

  test("Adding a need creates a new list item with correct category", async ({ page }) => {
    await page.fill("#need-add input[name='text']", "New Sound Effect");
    await page.selectOption("#need-add select[name='category']", "music");
    await page.press("#need-add input[name='text']", "Enter");
    
    await expect(page.locator("ol.needs li.need")).toHaveCount(3);
    await expect(page.locator("ol.needs li.need").last().locator(".need-text")).toHaveText("New Sound Effect");
    await expect(page.locator("ol.needs li.need").last()).toContainText("Music");
  });

  test("Adding a game creates a new selected item", async ({ page }) => {
    await page.fill("#game-add input[name='name']", "Space Miner");
    await page.press("#game-add input[name='name']", "Enter");
    
    await expect(page.locator(".game-list .game-item")).toHaveCount(3);
    await expect(page.locator(".game-main h2")).toHaveText("Space Miner");
  });

  test("AI draft populates profile and needs with AI chips", async ({ page }) => {
    await page.selectOption("#asset-ai-mode", "always");
    const kkeItem = page.locator(".game-item", { hasText: "KKE Showcase" });
    await kkeItem.click();
    
    await expect(page.locator(".game-main h2")).toHaveText("KKE Showcase");
    await page.click("[data-action='game-draft']");
    await expect(page.locator(".game-note")).toContainText("The AI drafted this profile");
    
    await expect(page.locator("#game-profile input[name='genre']")).toHaveValue("Action adventure");
    await expect(page.locator("ol.needs li.need")).toHaveCount(3);
    await expect(page.locator("ol.needs li.need .chip.ai-chip")).toHaveCount(3);
    await expect(page.locator("ol.needs li.need", { hasText: "calm town music" })).toBeVisible();
  });

  test("Saving profile removes AI note and chips", async ({ page }) => {
    await page.selectOption("#asset-ai-mode", "always");
    const kkeItem = page.locator(".game-item", { hasText: "KKE Showcase" });
    await kkeItem.click();
    await expect(page.locator(".game-main h2")).toHaveText("KKE Showcase");
    await page.click("[data-action='game-draft']");
    await expect(page.locator(".game-note")).toContainText("The AI drafted this profile");
    
    await page.click("#game-profile button[type='submit']");
    
    await expect(page.locator(".game-note")).not.toBeVisible();
    await expect(page.locator(".chip.ai-chip")).toHaveCount(0);
  });

  test("Typing in profile input survives live updates from picking", async ({ page }) => {
    await page.selectOption("#asset-ai-mode", "always");
    const forestItem = page.locator(".game-item", { hasText: "Forest Walk" });
    await forestItem.click();
    
    await page.fill("#game-profile input[name='genre']", "Puzzle");
    await expect(page.locator("#game-profile input[name='genre']")).toHaveValue("Puzzle");
    
    const footstepsNeed = page.locator("li.need", { hasText: "footsteps on grass" });
    await footstepsNeed.locator("[data-action='need-pick']").click();
    await expect(footstepsNeed.locator("ul.picks li.pick").first()).toBeVisible();
    
    await expect(page.locator("#game-profile input[name='genre']")).toHaveValue("Puzzle");
  });

  // "Used in" (milestone 5). Drafted by the local Coder; selectors fixed by Claude.
  test('KKE Showcase scene summary shows missing asset code', async ({ page }) => {
    await page.locator('.game-list .game-item', { hasText: 'KKE Showcase' }).click();
    await expect(page.locator('.game-main h2')).toHaveText('KKE Showcase');
  
    const details = page.locator('.game-scenes details.game-missing').first();
    await expect(details).toBeVisible();
  
    await details.click();
    await expect(details.locator('summary')).toBeVisible();
    await expect(details.locator('ul li code')).toHaveText('SM_Prop_Lantern_01');
  });

  test('Show used assets in library and remove filter', async ({ page }) => {
    await page.locator('.game-list .game-item', { hasText: 'KKE Showcase' }).click();
    await expect(page.locator('.game-main h2')).toHaveText('KKE Showcase');
  
    await page.click('[data-action="game-show-used"]');
    await expect(page.locator('[data-action="asset-tab"][data-tab="library"]')).toHaveAttribute('aria-pressed', 'true');
    await expect(page.locator('li.asset-cell')).toHaveCount(6);
    await expect(page.locator('#asset-active')).toContainText('Used in KKE Showcase ×');
  
    await page.click('#asset-active [data-what="used"]');
    await expect(page.locator('#asset-active')).toBeHidden();
    await expect.poll(() => page.locator('li.asset-cell').count()).toBeGreaterThan(6);
  });

  test('Details panel links back to game via Used in row', async ({ page }) => {
    await page.locator('.game-list .game-item', { hasText: 'KKE Showcase' }).click();
    await expect(page.locator('.game-main h2')).toHaveText('KKE Showcase');
  
    await page.click('[data-action="game-show-used"]');
    await expect(page.locator('[data-action="asset-tab"][data-tab="library"]')).toHaveAttribute('aria-pressed', 'true');
  
    await page.locator('li.asset-cell button.asset-card').first().click();
    await expect(page.locator('#asset-detail')).toBeVisible();
  
    const usedInRow = page.locator('#asset-detail dt:has-text("Used in") + dd');
    await expect(usedInRow).toContainText('KKE Showcase');
    await expect(usedInRow.locator('small')).toHaveText('course_art.scene.json (2×)');
  
    await usedInRow.locator('[data-action="asset-used-game"]').click();
    await expect(page.locator('[data-action="asset-tab"][data-tab="games"]')).toHaveAttribute('aria-pressed', 'true');
    await expect(page.locator('.game-main h2')).toHaveText('KKE Showcase');
  });
});

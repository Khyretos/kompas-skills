import { test, expect, type Page } from "@playwright/test";

async function openDemo(page: Page): Promise<void> {
  await page.goto("/?demo");
  await page.click("button.found-server");
  await expect(page.locator("#left")).toBeVisible();
}

async function openCapabilities(page: Page): Promise<void> {
  await page.click('[data-action="capabilities"]');
  await expect(page.locator("#caps h1")).toHaveText("Capabilities");
}

test.describe("Capabilities", () => {
  test.beforeEach(async ({ page }) => {
    await openDemo(page);
  });

  test("shows models, computers, tools, MCP servers, indexes and skills as cards", async ({ page }) => {
    let loads = 0;
    page.on("load", () => { loads++; });
    await openCapabilities(page);
    for (const id of ["caps-models", "caps-computers", "caps-tools", "caps-mcp", "caps-indexes", "caps-skills"]) {
      await expect(page.locator(`#${id}`)).toBeVisible();
    }
    const down = page.locator("#caps li.cap", { hasText: "Ollama on soucouyant" });
    await expect(down.locator(".chip.state")).toHaveText("down");
    await expect(down.locator(".task-step")).toContainText("connection refused");
    await expect(page.locator("#caps li.cap", { hasText: "OVMS on kireserver" }).locator(".task-meta")).toContainText("orchestrator: Coder");
    await expect(page.locator("#caps-mcp").locator("..").locator("p.muted")).toHaveText("No MCP servers yet.");
    // The chat is hidden while the section is open; nothing reloaded.
    await expect(page.locator("main.center")).toBeHidden();
    expect(loads).toBe(0);
    await page.screenshot({ path: "test-results/capabilities.png", fullPage: true });
  });

  test("a revoked grant shows on the computer card", async ({ page }) => {
    await openCapabilities(page);
    const pc = page.locator("#caps li.cap", { hasText: "soucouyant" }).filter({ has: page.locator(".chip", { hasText: "online" }) });
    await expect(pc.locator(".task-step")).toContainText("/home/kees/projects/kompanion");
    // Revoke it from the Access tab, then come back.
    await page.click('[data-action="new-chat"]');
    await page.click('[data-action="tab"][data-tab="access"]');
    page.on("dialog", (d) => d.accept()); // "Revoke access to …?"
    const row = page.locator("li.grant-row", { hasText: "/home/kees/projects/kompanion" });
    await row.locator('[data-action="grant-revoke"]').click();
    await expect(row).toHaveCount(0);
    await openCapabilities(page);
    await expect(pc.locator(".task-step")).toHaveText("No grants: every step asks first");
  });

  test("a skill opens read-only and closes with Escape, × and an outside click", async ({ page }) => {
    await openCapabilities(page);
    const card = page.locator('#caps [data-action="open-skill"][data-id="worker/rust"]');
    const sheet = page.locator(".skill-sheet");

    await card.click();
    await expect(sheet.locator("h2")).toHaveText("Skill: worker/rust");
    await expect(sheet.locator(".skill-body ol li")).toHaveCount(2);
    await expect(sheet.locator("input, textarea")).toHaveCount(0);
    await page.keyboard.press("Escape");
    await expect(sheet).toHaveCount(0);
    await expect(card).toBeFocused();

    await card.click();
    await sheet.locator('button[aria-label="Close"]').click();
    await expect(sheet).toHaveCount(0);

    await card.click();
    await sheet.click({ position: { x: 5, y: 5 } });
    await expect(sheet).toHaveCount(0);
  });
});

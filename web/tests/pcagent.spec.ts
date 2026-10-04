import { test, expect, type Page } from "@playwright/test";

async function openDemo(page: Page): Promise<void> {
  await page.goto("/?demo");
  await page.click("button.found-server");
  await expect(page.locator("#left")).toBeVisible();
}

async function ask(page: Page): Promise<void> {
  await page.selectOption("#pc-machine", { label: "soucouyant" });
  await page.fill("#prompt", "install htop");
  await page.press("#prompt", "Enter");
}

test.describe("PC Agent approval cards", () => {
  test.beforeEach(async ({ page }) => {
    await openDemo(page);
    page.on("dialog", (d) => d.accept());
  });

  test("a step on a computer without grants shows a card that says what it needs", async ({ page }) => {
    await ask(page);
    
    const card = page.locator(".pc-action").last();
    await expect(card).toBeVisible();
    await expect(card.locator("text=install htop with paru")).toContainText("install htop with paru");
    await expect(card.locator("text=Needs: packages + root")).toContainText("Needs: packages + root");
  });

  test("Approve runs the step once and leaves no grant", async ({ page }) => {
    await ask(page);
    
    const card = page.locator(".pc-action").last();
    await card.locator('[data-decision="approve"]').click();
    // Finished steps leave the pinned area and show inline in the chat.
    await expect(page.locator("#messages details.step").last().locator(".chip")).toContainText("done");
    
    await page.click('[data-action="tab"][data-tab="access"]');
    const section = page.locator("section.group", { hasText: "soucouyant" });
    const count = await section.locator("li.grant-row", { hasText: "system" }).count();
    await expect(count).toBe(0);
  });

  test("Always allow leaves a 24 h grant", async ({ page }) => {
    await ask(page);
    
    const card = page.locator(".pc-action").last();
    await card.locator('[data-decision="always"]').click();
    
    await expect(page.locator("#messages details.step").last().locator(".chip")).toContainText("done");
    
    await page.click('[data-action="tab"][data-tab="access"]');
    const section = page.locator("section.group", { hasText: "soucouyant" });
    const row = section.locator("li.grant-row").filter({ hasText: "system" }).filter({ hasText: "packages" });
    await expect(row).toHaveCount(1);
  });

  test("Deny declines", async ({ page }) => {
    await ask(page);
    
    const card = page.locator(".pc-action").last();
    await card.locator('[data-decision="deny"]').click();
    
    await expect(page.locator("#messages details.step").last().locator(".chip")).toContainText("declined");
  });

  test("a finished step shows its output as a terminal block with the exit code", async ({ page }) => {
    await openDemo(page);
    await ask(page);
    const card = page.locator(".pc-action").last();
    await card.locator('[data-decision="approve"]').click();
    const step = page.locator("#messages details.step").last();
    await expect(step.locator(".chip")).toContainText("done");
    await step.locator("summary").click();
    const out = step.locator("figure.output.terminal");
    await expect(out).toBeVisible();
    await expect(out.locator(".output-title")).toContainText("paru install htop");
    await expect(out.locator(".chip.exit.ok")).toContainText("exit 0");
    await page.screenshot({ path: "test-results/output-terminal.png" });
  });

  test("the chat keeps its scroll position while cards wait and stats tick", async ({ page }) => {
    await openDemo(page);
    await ask(page);
    await expect(page.locator(".pc-action").last()).toBeVisible();
    const box = page.locator("#messages");
    await box.evaluate((el) => { el.scrollTop = Math.max(0, (el.scrollHeight - el.clientHeight) / 2); });
    const before = await box.evaluate((el) => el.scrollTop);
    await page.waitForTimeout(3500); // a few stats ticks
    expect(await box.evaluate((el) => el.scrollTop)).toBe(before);
  });
});

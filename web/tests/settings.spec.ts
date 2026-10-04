import { test, expect, type Page } from "@playwright/test";

async function openDemo(page: Page): Promise<void> {
  await page.goto("/?demo");
  await page.click("button.found-server");
  await expect(page.locator("#left")).toBeVisible();
}

test.describe("Settings window closes", () => {
  test.beforeEach(async ({ page }) => {
    await openDemo(page);
    page.on("dialog", (d) => d.accept());
  });

  test("click outside closes", async ({ page }) => {
    await page.click('[data-action="settings"]');
    await expect(page.locator("#settings")).toBeVisible();
    await page.locator("#settings").click({ position: { x: 5, y: 5 } });
    await expect(page.locator("#settings")).toBeHidden();
  });

  test("click inside does not close", async ({ page }) => {
    await page.click('[data-action="settings"]');
    await expect(page.locator("#settings")).toBeVisible();
    await page.click("#settings .sheet h2");
    await expect(page.locator("#settings")).toBeVisible();
  });

  test("a drag from inside to outside does not close", async ({ page }) => {
    await page.click('[data-action="settings"]');
    await expect(page.locator("#settings")).toBeVisible();
    
    const titleBox = await page.locator("#settings .sheet h2").boundingBox();
    if (!titleBox) throw new Error("Could not get bounding box of title");
    
    const center = { x: titleBox.x + titleBox.width / 2, y: titleBox.y + titleBox.height / 2 };
    
    await page.mouse.move(center.x, center.y);
    await page.mouse.down();
    await page.mouse.move(center.x + 5, center.y + 5);
    await page.mouse.up();
    
    await expect(page.locator("#settings")).toBeVisible();
  });

  test("× closes", async ({ page }) => {
    await page.click('[data-action="settings"]');
    await expect(page.locator("#settings")).toBeVisible();
    
    await page.click("#settings [data-action=\"close-settings\"]");
    await expect(page.locator("#settings")).toBeHidden();
    await expect(page.locator('[data-action="settings"]')).toBeFocused();
  });

  test("Escape closes", async ({ page }) => {
    await page.click('[data-action="settings"]');
    await expect(page.locator("#settings")).toBeVisible();
    
    await page.keyboard.press("Escape");
    await expect(page.locator("#settings")).toBeHidden();
  });

  test("unsaved edits ask first", async ({ page }) => {
    // The beforeEach handler accepts every dialog; this test answers them itself.
    page.removeAllListeners("dialog");
    await page.click('[data-action="settings"]');
    await expect(page.locator("#settings")).toBeVisible();
    await page.locator("#notify-email").fill("x@example.com");

    const asked: string[] = [];
    page.once("dialog", (d) => { asked.push(d.message()); void d.dismiss(); });
    await page.keyboard.press("Escape");
    await expect.poll(() => asked.length).toBe(1);
    await expect(page.locator("#settings")).toBeVisible();

    page.once("dialog", (d) => { asked.push(d.message()); void d.accept(); });
    await page.keyboard.press("Escape");
    await expect(page.locator("#settings")).toBeHidden();
    expect(asked).toEqual(["Close without saving your changes?", "Close without saving your changes?"]);
  });
});

import { test, expect } from "@playwright/test";

test.describe("Steering running tasks", () => {
  test.beforeEach(async ({ page }) => {
    await page.goto("/?demo");
    await page.click("button.found-server");
    await page.click('[data-action="project"][data-id="p-kk"]');
    await page.click('[data-action="open-thread"][data-project="p-kk"]');
  });

  test("pause and go on steer the running tasks", async ({ page }) => {
    // Send "pause"
    await page.fill("#prompt", "pause");
    await page.press("#prompt", "Enter");
    await expect(page.locator("#messages")).toContainText("Paused");
    await expect(page.locator("#messages")).toContainText("Add a --vk-validation flag");

    // Send "go on"
    await page.fill("#prompt", "go on");
    await page.press("#prompt", "Enter");
    await expect(page.locator("#messages")).toContainText("Going on with");
  });

  test("a sentence is a question, not a command", async ({ page }) => {
    await page.fill("#prompt", "wait, why did step 2 fail?");
    await page.press("#prompt", "Enter");
    await page.waitForTimeout(1500);
    await expect(page.locator("#messages")).not.toContainText("Paused");
  });

  test("a notification link opens the thread at its message", async ({ page }) => {
    // Open another chat first
    await page.click('[data-action="open-chat"][data-id="c-kk"]');

    // Go back to the thread and pause
    await page.click('[data-action="open-thread"][data-project="p-kk"]');
    await page.fill("#prompt", "pause");
    await page.press("#prompt", "Enter");
    await expect(page.locator("#messages")).toContainText("Paused");

    // Read the id of the last orchestrator message
    const msgId = (await page.locator("#messages article.msg.orchestrator").last().getAttribute("id"))!.replace("msg-", "");

    // Click the chat link again
    await page.click('[data-action="open-chat"][data-id="c-kk"]');

    // Navigate to the specific message using location.hash
    await page.evaluate((m) => { location.hash = `#chat=c-kk-thread&msg=${m}`; }, msgId);

    // Expect the message to be visible and have the search-hit class
    await expect(page.locator(`#msg-${msgId}`)).toBeVisible();
    await expect(page.locator(`#msg-${msgId}`)).toHaveClass(/search-hit/);
  });
});

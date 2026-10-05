import { test, expect } from "@playwright/test";

test.describe("Task search", () => {
  test.beforeEach(async ({ page }) => {
    await page.goto("/?demo");
    await page.click("button.found-server");
    await page.click('[data-action="project"][data-id="p-kk"]');
    await expect(page.locator("#task-filter")).toBeVisible();
  });

  test("typing filters the task list live, fuzzily", async ({ page }) => {
    const filter = page.locator("#task-filter");
    await filter.pressSequentially("shdr cmp");
    await expect(page.locator("#right .tasks .task-title")).toHaveText(["Profile shader compile times"]);

    await filter.fill("zzzz");
    await expect(page.locator("#right")).toContainText("No task matches");

    await filter.fill("");
    await expect(page.locator("#right .tasks .task-title")).toHaveCount(7);
  });

  test("the clear button empties the search and keeps the focus there", async ({ page }) => {
    const filter = page.locator("#task-filter");
    await filter.pressSequentially("shdr");
    await expect(page.locator(".search-clear")).toBeVisible();
    await page.locator(".search-clear").click();
    await expect(filter).toHaveValue("");
    await expect(filter).toBeFocused();
    await expect(page.locator(".search-clear")).toHaveCount(0);
    await expect(page.locator("#right .tasks .task-title")).toHaveCount(7);
  });

  test("a group closes, keeps its count, and stays closed after a reload", async ({ page }) => {
    const group = page.locator('#right details.group[data-group="Up next"]');
    await expect(group).toHaveAttribute("open", "");
    await group.locator("summary").click();
    await expect(group).not.toHaveAttribute("open", "");
    await expect(group.locator("summary .count")).toHaveText("3");

    await page.reload();
    await page.click("button.found-server");
    await page.click('[data-action="project"][data-id="p-kk"]');
    await expect(page.locator('#right details.group[data-group="Up next"]')).not.toHaveAttribute("open", "");
  });
});

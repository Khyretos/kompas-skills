import { test, expect, type Page } from "@playwright/test";

async function openDemo(page: Page): Promise<void> {
  await page.goto("/?demo");
  await page.click("button.found-server");
  await expect(page.locator("#left")).toBeVisible();
}

test.describe("Layout", () => {
  test.beforeEach(async ({ page }) => {
    await page.addInitScript(() => {
      try {
        if (!sessionStorage.getItem("kk-test")) {
          localStorage.removeItem("kk.layout");
          sessionStorage.setItem("kk-test", "1");
        }
      } catch {}
    });
    await openDemo(page);
    page.on("dialog", (d) => d.accept());
  });



  const viewports = [
    { width: 1280, height: 800 },
    { width: 1920, height: 1080 }
  ];

  for (const { width, height } of viewports) {
    test(`no header control is clipped at ${width}px`, async ({ page }) => {
      await page.setViewportSize({ width, height });
      
      const tabs = ["tasks", "machines", "access"];
      for (const tab of tabs) {
        await page.click('[data-action="tab"][data-tab="' + tab + '"]');
        
        const clipped = await page.evaluate(() => {
          const head = document.querySelector("#right .pane-head");
          if (!head) return ["no header"];
          const box = head.getBoundingClientRect();
          return [...head.querySelectorAll("button")]
            .filter((b) => b.offsetParent !== null)
            .filter((b) => b.scrollWidth > b.clientWidth + 1 || b.getBoundingClientRect().right > box.right + 1)
            .map((b) => b.textContent?.trim() ?? "?");
        });
        
        expect(clipped).toEqual([]);
      }
    });
  }

  test("dragging the right handle widens the tasks panel and it persists", async ({ page }) => {
    await page.setViewportSize({ width: 1600, height: 900 });
    
    const before = (await page.locator("#right").boundingBox())!.width;
    
    const handle = page.locator(".resize-handle[data-side=\"right\"]");
    const box = await handle.boundingBox();
    if (!box) throw new Error("Right resize handle not found");
    
    await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
    await page.mouse.down();
    await page.mouse.move(box.x - 120, box.y + box.height / 2, { steps: 6 });
    await page.mouse.up();
    
    const after = (await page.locator("#right").boundingBox())!.width;
    expect(after).toBeGreaterThan(before + 100);
    
    await page.reload();
    await openDemo(page);
    
    const afterReload = (await page.locator("#right").boundingBox())!.width;
    expect(afterReload).toBeGreaterThan(before + 100);
  });

  test("keyboard resizes and double-click resets", async ({ page }) => {
    await page.setViewportSize({ width: 1600, height: 900 });
    
    const start = (await page.locator("#left").boundingBox())!.width;
    
    await page.focus(".resize-handle[data-side=\"left\"]");
    await page.keyboard.press("ArrowRight");
    await page.keyboard.press("ArrowRight");
    await page.keyboard.press("ArrowRight");
    await page.keyboard.press("ArrowRight");
    
    const after = (await page.locator("#left").boundingBox())!.width;
    expect(after).toBeGreaterThan(start + 50);
    
    await page.dblclick(".resize-handle[data-side=\"left\"]");
    
    const reset = (await page.locator("#left").boundingBox())!.width;
    expect(Math.abs(reset - start)).toBeLessThan(2);
  });

  test("collapse hides and shows the projects panel", async ({ page }) => {
    await page.click(".collapse-btn[data-side=\"left\"]");
    
    await expect(page.locator("#left")).toBeHidden();
    await expect(page.locator(".collapse-btn[data-side=\"left\"]")).toHaveAttribute("aria-expanded", "false");
    
    await page.click(".collapse-btn[data-side=\"left\"]");
    
    await expect(page.locator("#left")).toBeVisible();
    await expect(page.locator(".collapse-btn[data-side=\"left\"]")).toHaveAttribute("aria-expanded", "true");
  });
});

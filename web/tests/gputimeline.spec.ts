import { test, expect } from "@playwright/test";

// M6-04: per GPU, what it did over time (VRAM, watts, jobs, events), 1 h or 24 h.
test("the GPU timeline shows VRAM, watts, jobs and events, and switches to 24 h", async ({ page }) => {
  await page.goto("/?demo");
  await page.click("button.found-server");
  await page.click('[data-action="capabilities"]');
  const a770 = page.locator(".tl-gpu", { hasText: "a770" });
  await expect(page.locator(".tl-gpu")).toHaveCount(2);
  await expect(a770.locator("figcaption")).toContainText("peak 14.5 GB VRAM, 168 W");
  expect(await a770.locator("path.tl-vram").getAttribute("d")).toMatch(/^M [\d.]+ 110 L .* Z$/);
  await expect(a770.locator("path.tl-watts")).toHaveCount(1);
  await expect(a770.locator("rect.tl-job")).toHaveCount(2);
  await expect(a770.locator("rect.tl-job.kind-asset title, rect.tl-job.kind-asset + title, g:has(rect.kind-asset) title").first()).toContainText("studio:comfyui");
  await expect(a770.locator("line.tl-event")).toHaveCount(1);
  await page.click('[data-action="gpu-range"][data-hours="24"]');
  await expect(page.locator('[data-action="gpu-range"][data-hours="24"]')).toHaveAttribute("aria-pressed", "true");
  await expect(a770.locator("svg.tl-svg")).toHaveAttribute("viewBox", /^0 0 1440 /);
});

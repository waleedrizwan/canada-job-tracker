// Smoke tests for the static site (static/index.html, which draws the treemap from data.json).
// They run against `wrangler dev` on port 8787, serving dist/, so build first: `npm test` does both.
// They use the bundled data, so they check shape rather than exact numbers.
const { test, expect } = require("@playwright/test");

// Fails the test on any console error or failed request.
function watchErrors(page) {
  const errors = [];
  page.on("console", (m) => m.type() === "error" && errors.push(m.text()));
  page.on("pageerror", (e) => errors.push(e.message));
  page.on("requestfailed", (r) => errors.push(`${r.url()}: ${r.failure()?.errorText}`));
  return errors;
}

// The page has loaded data.json and drawn the treemap.
async function drawn(page) {
  await expect(page.locator("#statTotalJobs")).not.toHaveText("—");
  await expect.poll(() => page.evaluate(() => rects.length)).toBeGreaterThan(0);
}

// Where a tile's centre is on the screen, after scrolling the treemap into view.
async function tileCentre(page, index) {
  await page.locator("canvas#canvas").scrollIntoViewIfNeeded();
  return page.evaluate((i) => {
    const r = rects[i];
    const box = canvas.getBoundingClientRect();
    return { x: box.left + r.rx + r.rw / 2, y: box.top + r.ry + r.rh / 2, url: r.url };
  }, index);
}

test("the page draws the treemap with its stats and layers", async ({ page }) => {
  const errors = watchErrors(page);
  await page.goto("/");
  await drawn(page);
  await expect(page).toHaveTitle("Canada Job Tracker");
  await expect(page.locator("h1")).toContainText("Canada Job Tracker");
  await expect(page.locator("#nOcc")).not.toHaveText("—");
  await expect(page.locator("#colorToggle button.active")).toHaveText("Median Wage");
  await expect(page.locator("#block2 h3")).toHaveText("Avg. wage");
  await expect(page.locator("#block3 .bar")).toHaveCount(5);
  await expect(page.locator("#block5 .hbar-row")).toHaveCount(6);
  await expect(page.locator("#block6 .hbar-row")).toHaveCount(6);
  await expect(page.locator("#legendLow")).toHaveText("$15/hr");
  expect(errors).toEqual([]);
});

test("switching layers recolors the treemap and changes its stats", async ({ page }) => {
  const errors = watchErrors(page);
  await page.goto("/");
  await drawn(page);
  await page.getByRole("button", { name: "Full-Time Share" }).click();
  await expect(page.locator("#colorToggle button.active")).toHaveText("Full-Time Share");
  await expect(page.locator("#block2 h3")).toHaveText("Avg. full-time");
  await expect(page.locator("#legendHigh")).toHaveText("Full-time");
  await page.getByRole("button", { name: "Training Level" }).click();
  await expect(page.locator("#block2 h3")).toHaveText("Avg. training");
  await expect(page.locator("#block4 .tier-name").first()).toHaveText("No formal / HS");
  expect(errors).toEqual([]);
});

test("hovering a tile shows its tooltip and clicking opens Job Bank", async ({ page, context }) => {
  const errors = watchErrors(page);
  await page.goto("/");
  await drawn(page);
  const { x, y, url } = await tileCentre(page, 0);
  await page.mouse.move(x, y);
  await expect(page.locator("#tooltip")).toHaveClass(/visible/);
  await expect(page.locator("#tooltip .tt-title")).not.toBeEmpty();
  await expect(page.locator("#tooltip .tt-stats")).toContainText("Postings (45 days)");

  const popup = context.waitForEvent("page");
  await page.mouse.click(x, y);
  expect((await popup).url()).toBe(url);
  expect(errors).toEqual([]);
});

test("fits a phone screen", async ({ browser }) => {
  const page = await browser.newPage({ viewport: { width: 390, height: 844 } });
  const errors = watchErrors(page);
  await page.goto("/");
  await drawn(page);
  const overflow = await page.evaluate(() => document.documentElement.scrollWidth - document.documentElement.clientWidth);
  expect(overflow).toBe(0);
  expect(errors).toEqual([]);
  await page.close();
});

// Smoke tests: the page loads, the browser renderer (WebAssembly) draws other views, and the
// phone layout fits. They use the bundled data, so they check shape rather than exact numbers.
const { test, expect } = require("@playwright/test");

// Fails the test on any console error or failed request.
function watchErrors(page) {
  const errors = [];
  page.on("console", (m) => m.type() === "error" && errors.push(m.text()));
  page.on("pageerror", (e) => errors.push(e.message));
  page.on("requestfailed", (r) => errors.push(`${r.url()}: ${r.failure()?.errorText}`));
  return errors;
}

// The renderer has loaded the data and drawn the view the URL asks for.
async function settled(page) {
  await page.waitForLoadState("networkidle");
  await expect(page.locator("html")).not.toHaveClass(/loading/);
}

test("the dashboard loads with its key content", async ({ page }) => {
  const errors = watchErrors(page);
  await page.goto("/");
  await settled(page);
  await expect(page).toHaveTitle("Canada Job Tracker");
  await expect(page.locator(".answer .lead")).toHaveText(/^[\d,]+ postings for [\d,]+ openings\. Half offer \$[\d.]+ an hour or more/);
  await expect(page.locator("svg.map path.region")).toHaveCount(13);
  await expect(page.locator("table.heat tbody tr")).toHaveCount(10);
  await expect(page.getByRole("heading", { name: "What Job Bank misses" })).toBeVisible();
  await expect(page.locator(".table-wrap tbody tr")).toHaveCount(25);
  await expect(page.locator(".pager .where")).toHaveText(/^Page 1 of [\d,]+$/);
  expect(errors).toEqual([]);
});

test("filters, chart links, the back button and the CSV work in the browser", async ({ page }) => {
  const errors = watchErrors(page);
  await page.goto("/");
  await settled(page);
  const everything = await page.locator(".answer .lead strong").first().innerText();

  // The sentence form: blank fields stay out of the URL.
  await page.selectOption("select[name=prov]", "BC");
  await page.fill("input[name=wage]", "30.50");
  await page.getByRole("button", { name: "Show jobs" }).click();
  await expect(page).toHaveURL("/?prov=BC&wage=30.50");
  await settled(page);
  await expect(page.locator(".answer .lead strong").first()).not.toHaveText(everything);
  await expect(page.locator(".table-wrap td.city").first()).toHaveText(/, BC$/);

  // A bar is a link that adds a filter.
  await page.locator(".chart-block", { hasText: "What kind of work?" }).locator("a.bar-row").first().click();
  await expect(page).toHaveURL(/cat=/);
  await settled(page);
  await expect(page.locator("select[name=cat] option[selected]")).toHaveCount(1);

  await page.goBack();
  await expect(page).toHaveURL("/?prov=BC&wage=30.50");

  const [download] = await Promise.all([page.waitForEvent("download"), page.locator("a.download").click()]);
  expect(download.suggestedFilename()).toMatch(/^canada-jobs-\d{4}-\d{2}-\d{2}\.csv$/);
  const csv = require("fs").readFileSync(await download.path(), "utf8");
  expect(csv.split("\n")[0]).toBe("id,title,occupation,category,skill_level,city,province,hours,term,wage_min_hourly,wage_max_hourly,openings,posted,source");
  expect(errors).toEqual([]);
});

test("a filter with no matches says so", async ({ page }) => {
  await page.goto("/?prov=NU&wage=290");
  await settled(page);
  await expect(page.locator(".answer")).toContainText("No postings match.");
  await expect(page.locator("a.download")).toHaveCount(0);
  await expect(page.getByRole("link", { name: "Start over" })).toBeVisible();
});

test("fits a phone screen", async ({ browser }) => {
  const page = await browser.newPage({ viewport: { width: 390, height: 844 } });
  const errors = watchErrors(page);
  await page.goto("/?prov=ON");
  await settled(page);
  const overflow = await page.evaluate(() => document.documentElement.scrollWidth - document.documentElement.clientWidth);
  expect(overflow).toBe(0);
  // Each posting shows its pay and date without scrolling sideways.
  const row = page.locator(".table-wrap tbody tr").first();
  for (const cell of ["td.wage", "td.date"]) {
    const box = await row.locator(cell).boundingBox();
    expect(box.x + box.width).toBeLessThanOrEqual(390);
  }
  expect(errors).toEqual([]);
  await page.close();
});

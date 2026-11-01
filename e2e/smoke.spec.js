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

test("the jobs tab shows a search and a grid of postings", async ({ page }) => {
  const errors = watchErrors(page);
  await page.goto("/");
  await settled(page);
  await expect(page).toHaveTitle("Canada Job Tracker");
  await expect(page.getByRole("link", { name: "Jobs", exact: true })).toHaveAttribute("aria-current", "page");
  await expect(page.locator("form.search input[name=q]")).toBeVisible();
  await expect(page.locator("details.advanced")).not.toHaveAttribute("open", "");
  await expect(page.locator("details.advanced select[name=prov]")).toBeHidden();
  await expect(page.locator(".cards .card")).toHaveCount(48);
  await expect(page.locator(".job").first().locator(".card-pay")).toHaveText(/^\$[\d.]+|Pay not listed/);
  await expect(page.locator(".pager .where")).toHaveText(/^Page 1 of [\d,]+$/);
  expect(errors).toEqual([]);
});

test("searching, the pager, the tabs, the back button and the CSV work in the browser", async ({ page }) => {
  const errors = watchErrors(page);
  await page.goto("/");
  await settled(page);
  const count = page.locator(".count strong");
  const everything = await count.innerText();

  await page.fill("input[name=q]", "nurse");
  await page.getByRole("button", { name: "Search" }).click();
  await expect(page).toHaveURL("/?q=nurse");
  await settled(page);
  await expect(count).not.toHaveText(everything);
  await expect(page.locator(".count")).toContainText("matching “nurse”");

  // Advanced filters open under the search and narrow it further; blank ones stay out of the URL.
  await page.locator("details.advanced summary").click();
  await page.selectOption("details.advanced select[name=prov]", "BC");
  await page.getByRole("button", { name: "Apply filters" }).click();
  await expect(page).toHaveURL("/?q=nurse&prov=BC");
  await settled(page);
  await expect(page.locator("details.advanced summary .badge")).toHaveText("1");
  await expect(page.locator(".card-loc").first()).toHaveText(/(, BC|British Columbia)$/);
  await page.locator("details.advanced summary").click();
  await page.getByRole("link", { name: "Clear filters" }).click();
  await expect(page).toHaveURL("/?q=nurse");
  await settled(page);

  await page.locator(".pager a[rel=next]").click();
  await expect(page).toHaveURL("/?q=nurse&page=2#results");
  await settled(page);
  await expect(page.locator(".pager .where")).toHaveText(/^Page 2 of /);

  // The heatmap tab keeps the search; a square opens its jobs, shown as removable chips.
  await page.getByRole("link", { name: "Heatmap" }).click();
  await expect(page).toHaveURL("/?view=heatmap&q=nurse");
  await settled(page);
  await expect(page.locator("table.heat tbody tr")).toHaveCount(10);
  await expect(page.locator("svg.map path.region")).toHaveCount(13);
  await page.locator("table.heat td a").first().click();
  await expect(page).toHaveURL(/^[^?]*\?q=nurse&cat=.+&prov=/);
  await settled(page);
  await expect(page.locator(".chips a")).toHaveCount(2);

  await page.goBack();
  await expect(page).toHaveURL("/?view=heatmap&q=nurse");

  // The charts tab keeps the dropdown filters, and filtering stays on it.
  await page.getByRole("link", { name: "Charts" }).click();
  await settled(page);
  await expect(page.locator(".kpi")).toHaveCount(6);
  await page.selectOption("select[name=prov]", "BC");
  await page.getByRole("button", { name: "Apply filters" }).click();
  await expect(page).toHaveURL("/?q=nurse&prov=BC&view=charts");
  await settled(page);
  await expect(page.getByRole("heading", { name: "Job Bank vs StatCan vacancies" })).toBeVisible();

  await page.getByRole("link", { name: "Jobs", exact: true }).click();
  await settled(page);
  const [download] = await Promise.all([page.waitForEvent("download"), page.locator("a.download").click()]);
  expect(download.suggestedFilename()).toMatch(/^canada-jobs-\d{4}-\d{2}-\d{2}\.csv$/);
  const csv = require("fs").readFileSync(await download.path(), "utf8");
  expect(csv.split("\n")[0]).toBe("id,title,occupation,category,skill_level,city,province,hours,term,wage_min_hourly,wage_max_hourly,openings,posted,source");
  expect(csv.split("\n").slice(1, -1).every((l) => l.includes(",BC,"))).toBe(true);
  expect(errors).toEqual([]);
});

test("a search with no matches says so", async ({ page }) => {
  await page.goto("/?q=zzzzqqq");
  await settled(page);
  await expect(page.locator(".results")).toContainText("No jobs match.");
  await expect(page.locator(".job")).toHaveCount(0);
  await expect(page.locator("a.download")).toHaveCount(0);
  // The charts tab: a wage that isn't one of the choices still shows as selected.
  await page.goto("/?view=charts&prov=NU&wage=290");
  await settled(page);
  await expect(page.locator(".summary")).toContainText("No postings match.");
  await expect(page.getByRole("link", { name: "Clear 2 filters" })).toBeVisible();
  await expect(page.locator("select[name=wage] option[selected]")).toHaveText("$290+ an hour");
});

test("fits a phone screen", async ({ browser }) => {
  const page = await browser.newPage({ viewport: { width: 390, height: 844 } });
  const errors = watchErrors(page);
  for (const url of ["/?prov=ON", "/?view=heatmap", "/?view=charts"]) {
    await page.goto(url);
    await settled(page);
    const overflow = await page.evaluate(() => document.documentElement.scrollWidth - document.documentElement.clientWidth);
    expect(overflow, url).toBe(0);
  }
  // Cards stack in one column, each fully on screen.
  await page.goto("/?prov=ON");
  await settled(page);
  const box = await page.locator(".job").first().boundingBox();
  expect(box.x).toBeGreaterThanOrEqual(0);
  expect(box.x + box.width).toBeLessThanOrEqual(390);
  expect(errors).toEqual([]);
  await page.close();
});

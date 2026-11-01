# QA report: `qa-sweep` branch

Date: 2026-10-22. Everything is on the `qa-sweep` branch. Nothing was pushed, merged or deployed, and nothing was written to any external service.

## What was tested

**How:** headless Chromium driven by Playwright (no visible browser). Each view was checked at **1440×900** and **390×844**, and key sections in **light and dark** colour schemes. Every view logged console errors and warnings, failed requests, HTTP errors and horizontal overflow, and each page section was screenshotted and reviewed.

**Static site** (`npm run build`, then `npm run dev` on port 8787; this is how the live site works):

- Views: default, each province, combined filters, map shaded by wage, text search, every sort column in both directions, first, last and out-of-range pages, and the empty state.
- Bad input: `?page=abc`, `?page=99999`, `?prov=XX`, `?month=garbage`, `?wage=-5`, `?sort=bogus&dir=sideways`, `?wage=%ZZ&cat=%`, and an unknown path (`/nope/xyz`).
- The sentence form (category, province, wage, month) and all seven "More filters" fields.
- Links: category, occupation and city bars; map provinces; the ranked province list; heat-grid cells; the map shading toggle; toggling a bar off again; Start over; next and previous page; browser back and forward.
- CSV: the download link, and a direct `/export.csv?…` URL.
- Wage field input: decimals, over the maximum, and zero.
- Keyboard tab order through the form.
- Data failure: the dataset forced to return 500. Slow data: the dataset delayed 3 s.
- External links: Job Bank, StatCan ×2, Natural Earth, GitHub, open.canada.ca, and a Job Bank search link.
- Colour contrast: computed for every text and background pair in both themes.

**Server version** (`cargo run`, run on port 8787 with `REFRESH_HOURS=0` so it never contacted the data sources):

- Routes: `/`, `/export.csv`, `/healthz`, `/styles.css`, unknown paths, malformed queries.
- The no-JavaScript flow in a browser at both widths: form, bar link, paging, CSV.

**Live site** (read only): loaded two views and checked console and network. There were no errors. It runs the code from before these fixes.

## Issues found

### Fixed

| # | Issue | Where | Commit |
|---|---|---|---|
| 1 | Bar charts weren't proportional. The fill was capped at "track minus room for the label", so the longest bars were all clipped to the same length. Every province in the ranked list looked equal (Yukon 100.1 vs Nunavut 25.1), and the top 3 occupations looked alike. | `styles.css` | c9709f3 |
| 2 | The map section was squeezed into 900px: `.map-block { max-width: none }` lost to the more specific `.chart-block.wide`. | `styles.css` | c9709f3 |
| 3 | The ON and YT map labels straddled borders, and NL sat over the sea. In dark mode the ON label was dark text on the dark page, so it was invisible. | `charts.rs` | eb24281 |
| 4 | **Wrong shading:** with a strict filter, provinces with *zero* postings got the *darkest* shade. Tied quantile edges each counted as "at or above". | `charts.rs` `Bins::step` | 0d2c953 |
| 5 | Per-capita rates under 1 all printed as "0.0", so the map legend read "0–0" five times. | `views.rs` | 0d2c953 |
| 6 | Phone: the postings table showed only title and occupation. City, pay and date were hidden behind a sideways scroll. Each posting is now a short block on phones. | `styles.css`, `views.rs` | 653efe7 |
| 7 | Phone: the heat grid's row names took half the screen and scrolled away with the cells. They now wrap and stay pinned. | `styles.css` | 653efe7 |
| 8 | Phone: chart axis text rendered at about 8px, where the font's slashed zero read as a 6 ("20k" looked like "26k"). | `styles.css` | 653efe7 |
| 9 | Postings with no city showed just "Not given", so the province was lost. They now show "Not given, BC". | `views.rs` | 653efe7 |
| 10 | Pay ranges mixed formats ("$16.60–$28"). | `views.rs` | 653efe7 |
| 11 | The CSV had no province column, so rows with no city couldn't be placed. Cells containing a carriage return also weren't quoted. | `views.rs` | 0ef679f |
| 12 | Copy: "Half offer *more than* $25.00" is wrong for a median. It now reads "Half offer $25.00 an hour or more". | `views.rs` | 0ef679f |
| 13 | Copy: the heat-grid hint said "darker means higher", which is backwards in dark mode. It now points to the key. | `views.rs` | 0ef679f |
| 14 | "Previous" to page 1 added `page=1` to the URL. | `views.rs` | 0ef679f |
| 15 | Text sorting was case-sensitive, so lower-case titles sorted after every capitalised one. | `query.rs` | 8d60752 |
| 16 | Submitting the form put every blank field in the URL (`?cat=Health&prov=&wage=&month=&q=…`). This is the link people share. | `app.js` | fab4973 |
| 17 | **Silent wrong data:** if the dataset failed to load, a link like `/?prov=ON` showed all-Canada figures with nothing to say so. There's now a notice with a retry link. | `app.js` | 56ddd51 |
| 18 | While the data loaded, only the summary was dimmed. The charts and table showed the default view's numbers at full strength. | `styles.css` | 56ddd51 |
| 19 | The wage field rejected cents. `step="1"` made the browser refuse 17.50 ("nearest valid values are 17 and 18"). | `views.rs` | b109f74 |
| 20 | With no matches, the link offered to "Download these 0". | `views.rs` | b109f74 |
| 21 | Contrast below WCAG AA: muted text was 3.8:1 (now 5.1:1). White figures on the light theme's 4th shade were 4.0:1 (now 4.5:1). Light figures on the dark theme's 4th shade were 2.9:1 (now 5.9:1). | `styles.css` | 5b4951d |
| 22 | Server: a repeated parameter (`?page=1&page=2`) returned a plain-text 400 "Failed to deserialize query string". It now shows the default view, as the browser build does. | `main.rs` | 26ae99f |
| 23 | Server: unknown paths returned an empty 404. They now get a short page linking to the dashboard. | `main.rs`, `views.rs` | 26ae99f |
| 24 | No favicon: every page load requested a missing `/favicon.ico`. There's now an inline icon. | `views.rs` | 26ae99f |
| 25 | The stylesheet header still said "Ontario Job Tracker". | `styles.css` | c9709f3 |
| 26 | README: "Both sources" (four are listed). It had no screenshots, test instructions or architecture section, and the static-site run instructions were buried under Deploying. | `README.md` | d0c1f43 |

Each fix was re-checked in the browser after it was made. A final sweep of all views at both widths showed no console errors, failed requests or overflow.

### Not fixed

| Issue | Why |
|---|---|
| In the **server version** (no JavaScript), submitting the form still puts blank fields in the URL. | That's how a plain HTML GET form works. Fixing it needs JavaScript or a redirect, which goes against the no-JS server design. The static site (live) is fixed. |
| On the **static site**, unknown paths such as `/nope` show the dashboard with status 200. | This comes from `"not_found_handling": "single-page-application"` in `wrangler.jsonc`, which every view URL relies on. A real 404 would need Worker code; the README says no Worker code runs. |
| Text sort is case-insensitive but not locale-aware: "Électricien" sorts after "Z", and titles that start with quotes sort first. | Proper collation needs an ICU-style dependency, which would bloat the WebAssembly. |
| With extreme filters (say a $290 minimum), the map legend's ranges bunch up near zero (for example "0.0–0.0"). | This is a cosmetic edge case: the colours are now right (#4), and the ranked list shows exact values. |
| Map labels are about 10px on a phone. | They're readable, and the ranked list beside the map gives the same information. |
| Odd values in the source data: a $300/h cap, 999 openings on one posting, a $201 NT Health median, some French titles. | These are Job Bank's data, not code bugs. The footer already explains the French titles. |
| `cargo clippy` reports 3 lints: `query.rs:136` and `views.rs:403` (collapsible `if`), and `views.rs:23` (`is_multiple_of`). | They're in code this sweep didn't touch, so I kept the diff focused. Each is a one-line fix. |
| The open.canada.ca source link timed out from Playwright's request client. | Not a dead link: it returns 200 to a browser user agent in 0.1 s. |

## Test coverage added

Run everything with `npm test`. It runs `cargo test`, builds the site, then runs the Playwright tests. All pass: 44 Rust unit tests, 1 real-data test and 4 browser tests.

**Rust** (`cargo test`, no network):

- **`src/fetch.rs`** (9 tests): wage conversion from hour, day, week, bi-weekly, month and year, including the posting's own weekly hours. Out-of-range and malformed pay. Placeholder values. Title-casing. StatCan quarters. Parsing a full Job Bank row (original title preferred, NOC to category and TEER, Toronto merging, maximum raised to minimum, partner source) and a sparse one. Skipping rows without a province or date. Text interning.
- **`src/data.rs`** (6): wage midpoints, months, dictionary lookups, a JSON round trip with openings clamped, and rejecting bad columns or dates. Also adds a small `sample()` dataset used by the other modules' tests.
- **`src/query.rs`** (11): parameter cleanup, links, paging and sort defaults, toggling, month parsing, every filter, minimum wage, case-insensitive search across title and occupation, unknown values, sort order with missing wages last, case-insensitive text sort.
- **`src/charts.rs`** (7): number formatting, axis maxima, colour steps, the tie regression from #4, empty and non-finite input, bar scaling and links.
- **`src/views.rs`** (10): number and wage formatting, median, CSV header, quoting and province, file name, page summary, empty state, filters kept in the form, singular wording, out-of-range page, 404 page.
- **`src/regions.rs`** (1): codes are unique, and lookups by Job Bank name work.
- **`tests/real_data.rs`** (1): loads the bundled `data/jobs.json` and renders 8 views (including the edge cases above) plus a CSV, to catch panics on real data.

**Browser** (`e2e/smoke.spec.js`, Playwright with Chromium, against `wrangler dev` on 8787; `playwright.config.js` starts the server or reuses a running one):

1. The dashboard loads with its summary, map (13 regions), heat grid, benchmark chart, table and pager, and no console errors.
2. The form gives a clean URL and the view updates. A chart bar adds a filter, back works, and the CSV downloads with the right header.
3. The empty state explains itself and hides the download.
4. At 390px there's no horizontal overflow, and pay and date are visible for each posting.

## For you to do by hand

1. **Review and merge `qa-sweep` into `main`.** The live site deploys automatically from `main` through Workers Builds, so none of these fixes are live yet.
2. **On another machine,** run `npx playwright install chromium` once before `npm test`.
3. **Cloudflare builds** will now also install `@playwright/test` as a dev dependency. It doesn't download browsers on install, so the only cost is a slightly longer `npm install`.
4. **If `npm install` on a fresh clone warns about install scripts and `wrangler dev` then fails,** run `npm install-scripts approve workerd`. npm 11 showed this warning here, but wrangler still worked because `workerd` was already installed.
5. **Optional:** nothing runs the tests in CI yet. Adding `cargo test` to a GitHub Actions workflow would be a small follow-up. The browser tests need the WebAssembly build and Chromium, so they're slower to set up in CI.

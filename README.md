# Ontario Job Tracker

A dashboard for exploring Ontario job postings: filter by category, city, pay, work mode, employment type, seniority and posting age, and see the results as charts and a sortable, exportable table.

Plain HTML, CSS and JavaScript. No build step, no dependencies.

## Run it

```bash
npm run dev
```

Then open http://localhost:5173. (`npm run dev` just runs `python3 -m http.server 5173`; any static server works. Opening `index.html` directly from disk won't, because the page fetches `data/jobs.json`.)

## Data

The repo ships with **synthetic sample data** so the dashboard works out of the box. A banner says so whenever sample data is loaded.

- `npm run sample` regenerates the sample (`node scripts/generate-sample.mjs [count]`).
- `npm run fetch` pulls real postings from [TheirStack](https://theirstack.com) into `data/jobs.json`.

### Loading real postings from TheirStack

1. Put your key in a `.env` file (gitignored):
   ```
   THEIRSTACK_API_KEY=your_key_here
   ```
2. Check how many postings match before spending credits (fetches a single job):
   ```bash
   node scripts/fetch-theirstack.mjs --count
   ```
3. Fetch. Each job costs one credit, so `MAX_JOBS` defaults to 500:
   ```bash
   MAX_JOBS=2000 MAX_AGE_DAYS=60 npm run fetch
   ```

Optional settings: `ONTARIO_LOCATION_ID` (TheirStack's location id for Ontario; without it the script matches the location text "Ontario" and double-checks each job's province) and `CAD_PER_USD` (used only when a posting lists pay in USD).

TheirStack doesn't tag postings with an industry, so the script assigns categories from title keywords (`CATEGORY_RULES` in the script). Expect some misses; anything unmatched becomes "Other".

### Data schema

`data/jobs.json` is `{ generatedAt, sample, source, jobs: [...] }`, where each job is:

| field | example |
|---|---|
| `id` | `"S00001"` |
| `title`, `company` | `"Registered Nurse"`, `"Trillium Health"` |
| `category` | `"Healthcare"` |
| `city` | `"Toronto"` |
| `workMode` | `"On-site"` / `"Hybrid"` / `"Remote"` |
| `employmentType` | `"Full-time"` |
| `seniority` | `"Entry"` / `"Mid"` / `"Senior"` / `"Executive"` |
| `salaryMin`, `salaryMax` | annual CAD, or `null` |
| `postedAt`, `closedAt` | `"2026-10-01"`, `null` if still open |
| `source` | `"Indeed"` |
| `url` | link to the posting, or `null` |

Any source that can produce this shape will work with the dashboard.

## How to read it

Postings measure what's advertised online, which is not the same as who is actually hiring:

- **Stale and ghost postings.** Some live postings have no hiring activity behind them. By default the dashboard shows postings from the last 30 days that haven't been marked closed. The "Older than 30 days" figure shows how much of the full set falls outside that window.
- **Jobs that are never posted.** Internal fills, referrals and agency hires don't appear anywhere.
- **Source mix.** Most postings come from big job boards, with their biases. The Source mix chart shows the breakdown.
- **Location.** Remote and multi-location roles are fuzzy and may be loosely tied to Ontario.

For a sanity check, compare category totals with Statistics Canada's Job Vacancy and Wage Survey for Ontario.

## Features

- Filters: search, category, city, work mode, type, seniority, minimum pay, posted within, hide closed, pay listed only
- Filters are saved in the URL, so a filtered view can be bookmarked or shared
- Click a bar in the category, median pay or city charts to filter by it; click again to clear
- KPIs: matching postings, median pay, share with pay disclosed, distinct employers, remote/hybrid share, share older than 30 days
- Sortable table with pagination and CSV export of the filtered rows
- Light and dark mode

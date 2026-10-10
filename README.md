# Ontario Job Tracker

A dashboard for exploring job postings across Ontario: filter by occupation, city, offered wage, skill level, hours, term and month, and see the results as charts and a sortable, exportable table. It also compares the postings with Statistics Canada's vacancy estimates, to show which kinds of work the postings over- or under-count.

Plain HTML, CSS and JavaScript. No build step, no dependencies.

## Run it locally

```bash
npm run dev
```

Then open http://localhost:5173. Any static file server works. Opening `index.html` directly from disk won't, because the page fetches `data/jobs.json`.

## Data sources

Both sources are official, free and need no API key.

| Source | What we use | Updated |
|---|---|---|
| [Job Postings Advertised on Canada's National Job Bank](https://open.canada.ca/data/en/dataset/ea639e28-c0fc-48bf-b5dd-b8899bd43072) (ESDC, Open Government Licence – Canada) | Every Ontario posting from the latest 3 monthly files: title, NOC 2021 occupation, city, offered wage, hours, term, number of openings, first posting date | Monthly, early in the following month |
| [StatCan Job Vacancy and Wage Survey, table 14-10-0444-01](https://www150.statcan.gc.ca/t1/tbl1/en/tv.action?pid=1410044401) | Estimated Ontario vacancies and average offered hourly wage by broad occupational category | Quarterly |

To refresh the data:

```bash
npm run data
```

This downloads about 140 MB of CSVs, keeps the Ontario rows, and writes about 3 MB of JSON (about 740 KB gzipped) to `data/jobs.json`. Set `MONTHS=6` to keep more history.

A GitHub Action (`.github/workflows/refresh-data.yml`) runs the same script every Monday and commits `data/jobs.json` when there's new data. If Netlify is linked to the repo, each of those commits redeploys the site.

### How the data is shaped

- **Category** is the NOC 2021 broad occupational category (first digit of the code). **Skill level** is the TEER category (second digit).
- **Wages** are converted to hourly: annual ÷ 2,080, daily ÷ 8, and weekly or monthly using the posting's stated hours (40 a week if none are given). Values outside $10–$300/h are treated as missing.
- **Toronto** includes North York, Etobicoke, Scarborough, York and East York, which Job Bank reports separately.
- `data/jobs.json` is stored by column, with a lookup table for each repeated text field. `js/app.js` expands it on load.

## Caveats

Postings measure what's advertised, not all hiring:

- **Job Bank is one slice of the market.** Employers using the Temporary Foreign Worker program must advertise there, so food service, caregiving and trades are over-represented, and tech and professional roles are under-represented. The "Postings vs. estimated vacancies" chart shows how far off each category is.
- **No employer names and no closing dates** are published, so you can't see who is hiring or tell filled jobs from open ones.
- **Partner postings** (about 60%) come from other job boards and have fewer fields filled in (no experience, education or telework details).
- **Jobs that are never posted** (internal fills, referrals, agency hires) don't appear anywhere. StatCan's survey counts them; postings data can't.

## Deploying

The site is static. `netlify.toml` publishes the repo root, with no build command.

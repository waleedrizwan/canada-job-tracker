# Canada Job Tracker

A dashboard for exploring job postings across Canada: filter by province, city, occupation, offered wage, skill level, hours, term and month. Results appear as a map of Canada, a heat grid of pay by field and province, charts, and a sortable, exportable table. It also compares the postings with Statistics Canada's vacancy estimates, to show which kinds of work the postings over- or under-count.

Written in Rust. The server renders every page, including the SVG charts, so the browser gets plain HTML and CSS with **no JavaScript**. All page state lives in the URL, so any view can be bookmarked or shared. Bars in the charts are links that apply a filter, and hovering a bar shows its numbers.

## Run it locally

```bash
cargo run --release
```

Then open http://localhost:8080. The repo includes a current `data/jobs.json`, so it works straight away.

| Env var | Default | |
|---|---|---|
| `PORT` | `8080` | |
| `DATA_PATH` | `data/jobs.json` | where data is loaded from and saved to |
| `MONTHS` | `3` | how many monthly Job Bank files to keep |
| `REFRESH_HOURS` | `6` | how often to check for new releases (`0` turns it off) |

## Data sources

Both sources are official, free and need no API key.

| Source | What we use | Updated |
|---|---|---|
| [Job Postings Advertised on Canada's National Job Bank](https://open.canada.ca/data/en/dataset/ea639e28-c0fc-48bf-b5dd-b8899bd43072) (ESDC, Open Government Licence – Canada) | Every posting from the latest monthly files: title, NOC 2021 occupation, province, city, offered wage, hours, term, number of openings, first posting date | Monthly, early in the following month |
| [StatCan Job Vacancy and Wage Survey, table 14-10-0444-01](https://www150.statcan.gc.ca/t1/tbl1/en/tv.action?pid=1410044401) | Estimated vacancies and average offered hourly wage for Canada and each province/territory, by broad occupational category | Quarterly |
| [StatCan population estimates, table 17-10-0009-01](https://www150.statcan.gc.ca/t1/tbl1/en/tv.action?pid=1710000901) | Population by province/territory, for postings per 10,000 people on the map | Quarterly |
| [Natural Earth](https://www.naturalearthdata.com/) 1:50m admin-1 boundaries (public domain) | Province and territory outlines, converted once into `src/canada_map.rs` by `tools/gen_map.py` | Static |

**The data stays up to date by itself.** The server checks both sources every `REFRESH_HOURS`. When Job Bank publishes a new month or StatCan a new quarter, the server downloads it in the background and switches over without restarting.

To fetch data by hand (for example, to update the copy bundled in the repo):

```bash
cargo run --release -- fetch
```

This streams about 140 MB of UTF-16 CSVs (about 158,000 postings for three months). Memory peaks around 85 MB because each file is processed as it downloads and repeated text is stored once.

### How the data is shaped

- **Category** is the NOC 2021 broad occupational category (first digit of the code). **Skill level** is the TEER category (second digit).
- **Wages** are converted to hourly: annual ÷ 2,080, daily ÷ 8, and weekly or monthly using the posting's stated hours (40 a week if none are given). Values outside $10–$300/h are treated as missing.
- **Cities** carry their province ("Richmond, BC") so same-named places stay separate. **Toronto** includes North York, Etobicoke, Scarborough, York and East York, which Job Bank reports separately.
- **Map and heat-grid colours** use quantile steps, so each shade covers about the same number of provinces or cells and one outlier can't wash out the rest. Heat-grid cells need at least 10 postings with pay.
- In memory, each posting is packed into about 40 bytes, with repeated text stored once. The server uses about 85 MB, even mid-refresh, and renders a page in 5–15 ms.

## Code layout

| File | |
|---|---|
| `src/main.rs` | HTTP server (Axum), routes, background refresh |
| `src/fetch.rs` | Downloads and parses Job Bank and StatCan data |
| `src/data.rs` | On-disk JSON format and the compact in-memory dataset |
| `src/query.rs` | URL parameters, filtering, sorting |
| `src/views.rs` | The page (Maud templates) and CSV export |
| `src/charts.rs` | Bar charts (HTML), histograms (SVG), the Canada map and the heat grid |
| `src/regions.rs` | Provinces and territories, with each data source's codes |
| `src/canada_map.rs` | Generated province outlines (don't edit; rerun `tools/gen_map.py`) |
| `static/styles.css` | Stylesheet, built into the binary |

## Deploying (Fly.io)

```bash
fly launch --copy-config --no-deploy   # first time only
fly deploy
```

`fly.toml` runs one small machine in Toronto (`yyz`) that stops when idle and starts on the next request. The Dockerfile builds a release binary into a small distroless image (no shell or package manager).

To use your own domain:

```bash
fly certs add yourdomain.ca
```

Then add the DNS records it prints at your registrar.

## Caveats

Postings measure what's advertised, not all hiring:

- **Job Bank is one slice of the market.** Employers using the Temporary Foreign Worker program must advertise there, so food service, caregiving and trades are over-represented, and tech and professional roles are under-represented. The "Postings vs. estimated vacancies" chart shows how far off each category is.
- **No employer names and no closing dates** are published, so you can't see who is hiring or tell filled jobs from open ones.
- **Partner postings** (about 60%) come from other job boards and have fewer fields filled in.
- **Jobs that are never posted** (internal fills, referrals, agency hires) don't appear anywhere. StatCan's survey counts them; postings data can't.

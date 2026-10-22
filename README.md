# Canada Job Tracker

**Live at https://canada-job-tracker.w2riz.workers.dev**, hosted free on Cloudflare.

A dashboard for exploring job postings across Canada: filter by province, city, occupation, offered wage, skill level, hours, term and month. Results appear as a map of Canada, a heat grid of pay by field and province, charts, and a sortable, exportable table. It also compares the postings with Statistics Canada's vacancy estimates, to show which kinds of work the postings over- or under-count.

Written in Rust. The same Rust code renders every page, including the SVG charts, in one of two places:

- **On a server** (`cargo run`). The browser gets plain HTML and CSS with **no JavaScript**.
- **In the browser**, compiled to WebAssembly. This is how the live site works: it's a static site, so it costs nothing to host (see [Deploying](#deploying)).

All page state lives in the URL, so any view can be bookmarked or shared. Bars in the charts are links that apply a filter, and hovering a bar shows its numbers.

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

**The data stays up to date by itself.** The server checks both sources every `REFRESH_HOURS`. When Job Bank publishes a new month or StatCan a new quarter, the server downloads it in the background and switches over without restarting. For the live site, a daily GitHub Actions job does the same and redeploys (see [Deploying](#deploying)).

To fetch data by hand (for example, to update the copy bundled in the repo):

```bash
cargo run --release -- fetch    # always download
cargo run --release -- update   # download only if there's something new
```

This streams about 140 MB of UTF-16 CSVs (about 158,000 postings for three months). Memory peaks around 85 MB because each file is processed as it downloads and repeated text is stored once.

### How the data is shaped

- **Category** is the NOC 2021 broad occupational category (first digit of the code). **Skill level** is the TEER category (second digit).
- **Wages** are converted to hourly: annual ÷ 2,080, daily ÷ 8, and weekly or monthly using the posting's stated hours (40 a week if none are given). Values outside $10–$300/h are treated as missing.
- **Cities** carry their province ("Richmond, BC") so same-named places stay separate. **Toronto** includes North York, Etobicoke, Scarborough, York and East York, which Job Bank reports separately.
- **Map and heat-grid colours** use quantile steps, so each shade covers about the same number of provinces or cells and one outlier can't wash out the rest. Heat-grid cells need at least 10 postings with pay.
- In memory, each posting is packed into about 40 bytes, with repeated text stored once. The server uses about 85 MB, even mid-refresh, and renders a page in 5–15 ms. In the browser, loading the dataset takes about 0.1 s on a recent laptop and each view renders in 10–25 ms.

## Code layout

| File | |
|---|---|
| `src/main.rs` | HTTP server (Axum), routes, background refresh, and the `fetch`, `update` and `site` commands |
| `src/lib.rs` | The code shared by the server and the WebAssembly build |
| `src/wasm.rs` | WebAssembly exports: load the dataset, render a page or CSV |
| `src/site.rs` | Builds the static site in `dist/` |
| `src/fetch.rs` | Downloads and parses Job Bank and StatCan data |
| `src/data.rs` | On-disk JSON format and the compact in-memory dataset |
| `src/query.rs` | URL parameters, filtering, sorting |
| `src/views.rs` | The page (Maud templates) and CSV export |
| `src/charts.rs` | Bar charts (HTML), histograms (SVG), the Canada map and the heat grid |
| `src/regions.rs` | Provinces and territories, with each data source's codes |
| `src/canada_map.rs` | Generated province outlines (don't edit; rerun `tools/gen_map.py`) |
| `static/styles.css` | Stylesheet, built into the binary |
| `static/app.js` | Static site only: loads the WebAssembly renderer and handles links, forms and downloads in the browser |
| `tools/build-site.sh` | Compiles the WebAssembly and builds the static site |
| `wrangler.jsonc` | Cloudflare config for the static site |
| `.github/workflows/deploy.yml` | Daily data check, and deploys to Cloudflare |

## Deploying

### Cloudflare (the live site)

The live site, https://canada-job-tracker.w2riz.workers.dev, is static files on Cloudflare Workers. No server code runs, so it's free. `tools/build-site.sh` builds `dist/`, which contains:

- `index.html`: the default view, rendered ahead of time so it shows straight away
- the dataset (11 MB; about 2.7 MB as sent, compressed)
- the WebAssembly renderer (600 KB; about 200 KB as sent)
- `static/app.js`

The browser downloads the dataset once. After that, `app.js` renders each view with the same Rust code the server uses, so filters, chart links, paging, the back button and CSV downloads work as they do on the server, without page reloads. Every URL is served `index.html`, and `app.js` draws the view the URL asks for. Without JavaScript, only the default view shows.

To deploy by hand, you need Rust, Node and a Cloudflare account:

```bash
rustup target add wasm32-unknown-unknown   # first time only
npm install
npx wrangler login                          # first time only
npm run deploy                              # builds dist/, then uploads it
```

To try the static site locally, run `npm run build`, then `npm run dev`, then open http://localhost:8787.

**Data updates.** A static site can't refresh itself, so `.github/workflows/deploy.yml` does it:

- Every day, it runs `cargo run --release -- update`.
- If Job Bank or StatCan has published something new, it commits the new `data/jobs.json` and redeploys.
- Pushes to `main` redeploy too.

The workflow needs a Cloudflare API token:

1. In the Cloudflare dashboard, open **My Profile → API Tokens → Create Token** and use the **Edit Cloudflare Workers** template.
2. Save it as a repository secret named `CLOUDFLARE_API_TOKEN` (Settings → Secrets and variables → Actions, or `gh secret set CLOUDFLARE_API_TOKEN`).

Until the secret exists, the workflow skips deploying and says so.

### Fly.io

To run the server version instead:

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

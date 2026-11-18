# Canada Job Tracker

**Live at https://canada-job-tracker.w2riz.workers.dev**

A dashboard for exploring job postings across Canada. It uses postings from the Government of Canada's Job Bank and compares them with Statistics Canada's job vacancy estimates, so you can see which kinds of work the postings over- or under-count.

The live site is a treemap: each tile is an occupation from the last 45 days of postings, and the buttons above it change what the tile colours show (median wage, wage against Statistics Canada, full-time share or training level). Clicking a tile opens a Job Bank search for that occupation.

The data is refreshed daily when Job Bank or Statistics Canada publish something new.

## Data sources

- [Job Postings Advertised on Canada's National Job Bank](https://open.canada.ca/data/en/dataset/ea639e28-c0fc-48bf-b5dd-b8899bd43072), from ESDC, under the Open Government Licence – Canada
- [Statistics Canada Job Vacancy and Wage Survey, table 14-10-0444-01](https://www150.statcan.gc.ca/t1/tbl1/en/tv.action?pid=1410044401)

## Caveats

- Job Bank is only one part of the job market, so some kinds of work are over-represented and others under-represented.
- Postings don't show employers or closing dates, and they don't include jobs that are never advertised.

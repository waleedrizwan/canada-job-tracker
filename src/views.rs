//! The dashboard page and CSV export.

use std::collections::BTreeMap;

use chrono::{Datelike, Duration, NaiveDate};
use maud::{DOCTYPE, Markup, html};

use crate::charts::{self, Bar, Bin, HeatCell, Pair, Region, compact};
use crate::data::{Dataset, Dict, Job};
use crate::query::{self, Params, toggle};
use crate::regions::{self, REGIONS};

const PAGE_SIZE: usize = 48;
const HOURS_PER_YEAR: f64 = 2080.0;
const MONTHS: [&str; 12] = [
    "January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December",
];

fn thousands(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}
fn pct(n: usize, d: usize) -> String {
    if d == 0 { "–".into() } else { format!("{}%", (n as f64 / d as f64 * 100.0).round()) }
}
fn pct1(v: f64) -> String {
    format!("{:.1}%", v * 100.0)
}
fn wage(v: f64) -> String {
    format!("${v:.2}")
}
fn wage_short(v: f32) -> String {
    if v.fract() == 0.0 { format!("${v}") } else { format!("${v:.2}") }
}
/// An offered range, with cents on both ends if either has them ("$16.60–$28.00", not "$16.60–$28").
fn wage_range(lo: f32, hi: f32) -> String {
    if lo == hi {
        wage_short(lo)
    } else if lo.fract() == 0.0 && hi.fract() == 0.0 {
        format!("${lo}–${hi}")
    } else {
        format!("${lo:.2}–${hi:.2}")
    }
}
/// Postings per 10,000 people. Rates under 1 keep two decimals, so they don't all show as "0.0".
fn per_10k(v: f64) -> String {
    if v > 0.0 && v < 1.0 { format!("{v:.2}") } else { format!("{v:.1}") }
}
fn annual(h: f64) -> String {
    format!("≈ ${}/yr", compact(h * HOURS_PER_YEAR))
}
fn month_name((y, m): (i32, u32)) -> String {
    format!("{} {y}", MONTHS[(m - 1) as usize])
}
fn median(v: &mut [f32]) -> Option<f64> {
    if v.is_empty() {
        return None;
    }
    v.sort_by(|a, b| a.total_cmp(b));
    let mid = v.len() / 2;
    Some(if v.len() % 2 == 1 { v[mid] as f64 } else { (v[mid - 1] as f64 + v[mid] as f64) / 2.0 })
}

/// Counts postings per dictionary id, largest first.
fn count_by(rows: &[&Job], dict: &Dict, key: impl Fn(&Job) -> usize) -> Vec<(usize, usize)> {
    let mut counts = vec![0usize; dict.values.len()];
    for j in rows {
        counts[key(j)] += 1;
    }
    let mut out: Vec<(usize, usize)> = counts.into_iter().enumerate().filter(|(_, c)| *c > 0).collect();
    out.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| dict.get(a.0).cmp(dict.get(b.0))));
    out
}

/// A labelled dropdown filter. `options` are (value, shown text) pairs; the blank option means "any".
fn dropdown(name: &str, label: &str, any: &str, options: &[(String, String)], current: &Option<String>) -> Markup {
    html! {
        div.filter.col-6.col-md-4.col-lg-3 {
            label.form-label.small.fw-semibold for=(format!("f-{name}")) { (label) }
            select.form-select #(format!("f-{name}")) name=(name) class=[current.is_some().then_some("border-primary")] {
                option value="" { (any) }
                @for (value, text) in options {
                    option value=(value) selected[current.as_deref() == Some(value.as_str())] { (text) }
                }
            }
        }
    }
}

/// Options whose value is also their label.
fn same(values: Vec<String>) -> Vec<(String, String)> {
    values.into_iter().map(|v| (v.clone(), v)).collect()
}

/// Minimum-wage choices, plus the current value if a link asked for one that isn't listed.
fn wage_options(current: &Option<String>) -> Vec<(String, String)> {
    let mut v: Vec<(String, String)> = [17, 20, 25, 30, 35, 40, 50, 60, 75].iter().map(|w| (w.to_string(), format!("${w}+ an hour"))).collect();
    if let Some(c) = current {
        if !v.iter().any(|(k, _)| k == c) {
            let shown = c.parse::<f32>().map(wage_short).unwrap_or_else(|_| c.clone());
            v.push((c.clone(), format!("{shown}+ an hour")));
            v.sort_by(|a, b| a.0.parse::<f32>().unwrap_or(0.0).total_cmp(&b.0.parse::<f32>().unwrap_or(0.0)));
        }
    }
    v
}

/// One chart in a Bootstrap card. `cols` is its grid column classes.
fn panel(cols: &str, title: &str, hint: Markup, body: Markup) -> Markup {
    html! {
        div class=(cols) {
            section.panel.card.h-100 {
                div.card-body {
                    h2.h5.card-title { (title) }
                    p.small.text-body-secondary.mb-3 { (hint) }
                    (body)
                }
            }
        }
    }
}
const HALF: &str = "col-12 col-xl-6";
const FULL: &str = "col-12";

fn sorted_values(rows: Vec<(usize, usize)>, dict: &Dict, alphabetical: bool) -> Vec<String> {
    let mut v: Vec<String> = rows.into_iter().map(|(i, _)| dict.get(i).to_string()).collect();
    if alphabetical {
        v.sort();
    }
    v
}

/// The tabs, by their `view` value.
const TABS: [(Option<&str>, &str); 3] = [(None, "Jobs"), (Some("heatmap"), "Heatmap"), (Some("charts"), "Charts")];

pub fn page(ds: &Dataset, p: &Params) -> Markup {
    let (title, content) = match p.view.as_deref() {
        Some("heatmap") => ("Heatmap · Canada Job Tracker", heatmap_view(ds, p)),
        Some("charts") => ("Charts · Canada Job Tracker", charts_view(ds, p)),
        _ => ("Canada Job Tracker", jobs_view(ds, p)),
    };
    let span = match (ds.months.last(), ds.months.first()) {
        (Some(a), Some(b)) if a != b => format!("{} to {}", a.split(' ').next().unwrap_or(a), b),
        (_, Some(b)) => b.clone(),
        _ => String::new(),
    };
    let updated = format!("{} {}, {}", MONTHS[ds.as_of.month0() as usize], ds.as_of.day(), ds.as_of.year());

    html! {
        (DOCTYPE)
        html lang="en-CA" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { (title) }
                meta name="description" content="Who's hiring across Canada, for what, and what it pays. Every job posted on the Government of Canada's Job Bank, by province, compared with Statistics Canada's vacancy survey.";
                link rel="stylesheet" href=(format!("/styles.css?v={}", crate::styles_version()));
                link rel="icon" href=(FAVICON);
            }
            body {
                header.topbar.bg-body-tertiary.border-bottom.mb-4 {
                    div.container.py-3.d-flex.flex-wrap.align-items-center.justify-content-between.gap-3 {
                        div {
                            a.navbar-brand.fw-bold.fs-4 href="/" { "Canada Job Tracker" }
                            div.small.text-body-secondary { "Job Bank postings, " (span) ". Latest posting " (updated) "." }
                        }
                        nav aria-label="Views" {
                            ul.nav.nav-pills {
                                @for (view, label) in TABS {
                                    li.nav-item {
                                        @if p.view.as_deref() == view {
                                            a.nav-link.active href=(p.link(|q| q.view = view.map(str::to_string))) aria-current="page" { (label) }
                                        } @else {
                                            a.nav-link href=(p.link(|q| q.view = view.map(str::to_string))) { (label) }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                main.container.pb-5 { (content) }
                footer.container.border-top.py-4.small.text-body-secondary {
                    div.row.g-4 {
                        div.col-md-6 {
                            h3.h6.text-body { "Where this comes from" }
                            p.mb-0 {
                                "Postings: " a href=(ds.source_url) target="_blank" rel="noopener" { "Job Postings Advertised on Canada's National Job Bank" }
                                ", Employment and Social Development Canada, under the Open Government Licence – Canada. "
                                "Vacancy estimates: " a href="https://www150.statcan.gc.ca/t1/tbl1/en/tv.action?pid=1410044401" target="_blank" rel="noopener" { "Statistics Canada, Job Vacancy and Wage Survey, table 14-10-0444-01" } ". "
                                "Population: " a href="https://www150.statcan.gc.ca/t1/tbl1/en/tv.action?pid=1710000901" target="_blank" rel="noopener" { "Statistics Canada, table 17-10-0009-01" } ". "
                                "Map boundaries: " a href="https://www.naturalearthdata.com/" target="_blank" rel="noopener" { "Natural Earth" } ", public domain."
                            }
                        }
                        div.col-md-6 {
                            h3.h6.text-body { "What to keep in mind" }
                            p.mb-0 {
                                "Job Bank is one slice of the market. Employers hiring through the Temporary Foreign Worker program must advertise there, so food service, caregiving, farm work and trades show up more, and office and tech jobs less. "
                                "Employer names and closing dates aren't published. Wages are converted to hourly at 40 hours a week unless the posting says otherwise. "
                                "Some Quebec postings have French titles. Toronto includes North York, Etobicoke, Scarborough, York and East York."
                            }
                        }
                    }
                    p.mt-4.mb-0 { "An independent project, not affiliated with the Government of Canada. " a href="https://github.com/waleedrizwan/canada-job-tracker" { "Source code" } }
                }
            }
        }
    }
}

/// A link to page `n` of the job cards, scrolled to the results.
fn page_link(p: &Params, n: usize) -> String {
    let mut q = p.clone();
    q.page = (n > 1).then(|| n.to_string());
    match serde_urlencoded::to_string(&q) {
        Ok(qs) if !qs.is_empty() => format!("/?{qs}#results"),
        _ => "/#results".into(),
    }
}

/// The page numbers to offer: the first, the last and the current page's neighbours, with `None`
/// for each gap.
fn page_numbers(cur: usize, pages: usize) -> Vec<Option<usize>> {
    let mut out = Vec::new();
    for n in 1..=pages {
        if n == 1 || n == pages || n.abs_diff(cur) <= 1 {
            out.push(Some(n));
        } else if out.last() != Some(&None) {
            out.push(None);
        }
    }
    out
}

/// The filters other than the search, as (label, link that removes it), so the jobs tab can show
/// what a heatmap or chart link narrowed it to.
fn chips(p: &Params) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut add = |value: &Option<String>, show: &dyn Fn(&str) -> String, clear: fn(&mut Params)| {
        if let Some(v) = value {
            out.push((show(v), p.link(clear)));
        }
    };
    let same = |v: &str| v.to_string();
    add(&p.cat, &same, |q| q.cat = None);
    add(&p.prov, &|v| regions::name(v).to_string(), |q| q.prov = None);
    add(&p.city, &same, |q| q.city = None);
    add(&p.occ, &same, |q| q.occ = None);
    add(&p.wage, &|v| format!("{}+ an hour", v.parse::<f32>().map(wage_short).unwrap_or_else(|_| v.to_string())), |q| q.wage = None);
    add(&p.month, &|v| query::parse_month(v).map(month_name).unwrap_or_else(|| v.to_string()), |q| q.month = None);
    add(&p.level, &same, |q| q.level = None);
    add(&p.hours, &same, |q| q.hours = None);
    add(&p.term, &same, |q| q.term = None);
    add(&p.src, &same, |q| q.src = None);
    out
}

/// One posting as a card. Its view-transition name lets it glide to its new place when a search
/// changes the grid (see styles.css).
fn card(ds: &Dataset, j: &Job) -> Markup {
    let title = ds.titles.get(j.title as usize);
    let city = ds.cities.get(j.city as usize);
    let loc = if city == "Not given" { regions::name(ds.provinces.get(j.province as usize)).to_string() } else { city.to_string() };
    let search = format!(
        "https://www.jobbank.gc.ca/jobsearch/jobsearch?{}",
        serde_urlencoded::to_string([("searchstring", title), ("locationstring", loc.as_str())]).unwrap_or_default()
    );
    let term = ds.terms.get(j.term as usize);
    html! {
        div.col {
            article.job.card.h-100.shadow-sm style=(format!("view-transition-name: job-{}", j.id)) {
                div.card-body {
                    p.card-pay.fs-5.fw-semibold.text-success.mb-1 {
                        @match (j.wage_min, j.wage_max) {
                            (Some(a), b) => { (wage_range(a, b.unwrap_or(a))) small.fw-normal.text-body-secondary { " an hour" } }
                            _ => { span.fs-6.text-body-secondary { "Pay not listed" } }
                        }
                    }
                    h3.card-title.h6.mb-1 {
                        a.stretched-link.link-body-emphasis.link-underline-opacity-0 href=(search) target="_blank" rel="noopener" title="Find current postings like this on Job Bank" { (title) }
                    }
                    p.card-text.small.text-body-secondary.mb-2 { (ds.occupations.get(j.occupation as usize)) }
                    div.d-flex.flex-wrap.gap-1 {
                        span.badge.text-bg-light.border { (ds.hours.get(j.hours as usize)) }
                        @if term != "Unspecified" { span.badge.text-bg-light.border { (term) } }
                        @if j.vacancies > 1 { span.badge.text-bg-light.border { (j.vacancies) " openings" } }
                    }
                }
                div.card-footer.d-flex.justify-content-between.small.text-body-secondary {
                    span.card-loc.text-truncate.me-2 { (loc) }
                    span.text-nowrap { (format!("{} {}", &MONTHS[j.posted.month0() as usize][..3], j.posted.day())) }
                }
            }
        }
    }
}

/// The default tab: a title search and a grid of postings, newest first.
fn jobs_view(ds: &Dataset, p: &Params) -> Markup {
    let mut rows = query::filter(ds, p);
    let n = rows.len();
    query::sort(ds, &mut rows, p.sort_key(), p.descending());
    let pages = n.div_ceil(PAGE_SIZE).max(1);
    let cur = p.page().min(pages);
    let slice = &rows[(cur - 1) * PAGE_SIZE..(cur * PAGE_SIZE).min(n)];
    let chips = chips(p);
    // The search keeps any other filters, so searching within a heatmap square stays in it.
    let mut keep = p.clone();
    keep.q = None;
    keep.page = None;
    let hidden: Vec<(String, String)> = serde_urlencoded::to_string(&keep).ok().and_then(|qs| serde_urlencoded::from_str(&qs).ok()).unwrap_or_default();
    let export_link = p.link(|_| {}).replacen('/', "/export.csv", 1);

    html! {
        form.search.mb-3 method="get" action="/" role="search" {
            label.visually-hidden for="q" { "Job title" }
            div.input-group.input-group-lg {
                input #q.form-control type="search" name="q" placeholder="Search job titles, like nurse or electrician" value=(p.q.as_deref().unwrap_or("")) autocomplete="off";
                button.btn.btn-primary type="submit" { "Search" }
            }
            @for (k, v) in &hidden { input type="hidden" name=(k) value=(v); }
        }
        @if !chips.is_empty() {
            div.chips.d-flex.flex-wrap.gap-2.mb-3 aria-label="Filters" {
                @for (label, href) in &chips {
                    a.btn.btn-sm.btn-outline-secondary href=(href) title=(format!("Remove {label}")) { (label) span aria-hidden="true" { " ×" } }
                }
            }
        }
        section #results.results {
            div.results-head.d-flex.flex-wrap.justify-content-between.align-items-baseline.gap-2.mb-3 {
                p.count.mb-0 aria-live="polite" {
                    strong { (thousands(n as u64)) } (if n == 1 { " job" } else { " jobs" })
                    @if let Some(q) = &p.q { " matching “" (q) "”" }
                    @if n > PAGE_SIZE { span.text-body-secondary { " · newest first" } }
                }
                @if n > 0 { a.download.small href=(export_link) download { "Download " (thousands(n as u64)) " as CSV" } }
            }
            @if n == 0 {
                div.alert.alert-light.border {
                    p.mb-1 { "No jobs match. Try a shorter or different job title." }
                    a.alert-link href="/" { "Show all jobs" }
                }
            } @else {
                div.cards.row.row-cols-1.row-cols-sm-2.row-cols-lg-3.row-cols-xl-4.g-3 { @for j in slice { (card(ds, j)) } }
                @if pages > 1 {
                    nav.pager.mt-4 aria-label="Pages" {
                        ul.pagination.justify-content-center.flex-wrap.mb-1 {
                            @if cur > 1 {
                                li.page-item { a.page-link href=(page_link(p, cur - 1)) rel="prev" { "Previous" } }
                            } @else {
                                li.page-item.disabled { span.page-link { "Previous" } }
                            }
                            @for n in page_numbers(cur, pages) {
                                @match n {
                                    Some(n) if n == cur => { li.page-item.active aria-current="page" { span.page-link { (thousands(n as u64)) } } }
                                    Some(n) => { li.page-item { a.page-link href=(page_link(p, n)) { (thousands(n as u64)) } } }
                                    None => { li.page-item.disabled { span.page-link { "…" } } }
                                }
                            }
                            @if cur < pages {
                                li.page-item { a.page-link href=(page_link(p, cur + 1)) rel="next" { "Next" } }
                            } @else {
                                li.page-item.disabled { span.page-link { "Next" } }
                            }
                        }
                        p.where.text-center.small.text-body-secondary { "Page " (thousands(cur as u64)) " of " (thousands(pages as u64)) }
                    }
                }
            }
        }
    }
}

/// The heatmap tab: the map of Canada and pay by field and province. Clicking either opens the jobs.
fn heatmap_view(ds: &Dataset, p: &Params) -> Markup {
    // ---- Map: provinces compared under every filter except the province itself
    let mut no_prov = p.clone();
    no_prov.prov = None;
    let map_rows = query::filter(ds, &no_prov);
    let by_wage = p.map.as_deref() == Some("wage");
    let mut per_prov: Vec<(usize, Vec<f32>)> = vec![(0, Vec::new()); ds.provinces.values.len()];
    for j in &map_rows {
        let e = &mut per_prov[j.province as usize];
        e.0 += 1;
        if let Some(w) = j.wage() {
            e.1.push(w);
        }
    }
    let population = ds.benchmark.as_ref().map(|b| &b.population);
    let region_stats: Vec<(&regions::Region, usize, Option<f64>, Option<f64>)> = REGIONS
        .iter()
        .map(|r| {
            let (count, mut w) = ds.provinces.find(r.code).map(|i| per_prov[i].clone()).unwrap_or_default();
            let med = if w.len() >= 5 { median(&mut w) } else { None };
            let rate = population.and_then(|pop| pop.get(r.code)).filter(|p| **p > 0.0).map(|pop| count as f64 / pop * 10_000.0);
            (r, count, rate, med)
        })
        .collect();
    let map_value = |rate: Option<f64>, med: Option<f64>| if by_wage { med } else { rate };
    let map_bins = charts::Bins::new(region_stats.iter().filter_map(|(_, _, r, m)| map_value(*r, *m)));
    let fmt_map = |v: f64| if by_wage { wage(v) } else { per_10k(v) };
    let map_regions: Vec<Region> = region_stats
        .iter()
        .map(|(r, count, rate, med)| {
            let mut tip = format!("{}\n{} postings", r.name, thousands(*count as u64));
            if let Some(rt) = rate {
                tip.push_str(&format!("\n{} per 10,000 people", per_10k(*rt)));
            }
            if let Some(m) = med {
                tip.push_str(&format!("\nMedian offer {}/h", wage(*m)));
            }
            Region {
                code: r.code,
                step: map_value(*rate, *med).map(|v| map_bins.step(v)),
                tip,
                href: p.link(|q| {
                    q.view = None;
                    q.prov = Some(r.code.to_string());
                }),
                active: p.prov.as_deref() == Some(r.code),
            }
        })
        .collect();
    let mut map_ranked: Vec<Bar> = region_stats
        .iter()
        .filter_map(|(r, count, rate, med)| {
            let v = map_value(*rate, *med)?;
            Some(Bar {
                label: r.name.to_string(),
                value: v,
                display: fmt_map(v),
                tip: format!("{}\n{} postings", r.name, thousands(*count as u64)),
                href: Some(p.link(|q| {
                    q.view = None;
                    q.prov = Some(r.code.to_string());
                })),
                active: p.prov.as_deref() == Some(r.code),
            })
        })
        .collect();
    map_ranked.sort_by(|a, b| b.value.total_cmp(&a.value));
    let map_legend = map_bins.labels(|v| if by_wage { format!("${v:.0}") } else if v >= 10.0 { format!("{v:.0}") } else { per_10k(v) });

    // ---- Heat grid: median offer by field × province, under every filter except those two
    let mut no_pc = p.clone();
    no_pc.prov = None;
    no_pc.cat = None;
    let heat_rows = query::filter(ds, &no_pc);
    let (ncat, nprov) = (ds.categories.values.len(), ds.provinces.values.len());
    let mut grid: Vec<Vec<f32>> = vec![Vec::new(); ncat * nprov];
    for j in &heat_rows {
        if let Some(w) = j.wage() {
            grid[j.category as usize * nprov + j.province as usize].push(w);
        }
    }
    let mut medians: Vec<Vec<Option<(f64, usize)>>> = Vec::new();
    let mut heat_cats: Vec<(usize, String)> =
        ds.categories.values.iter().enumerate().filter(|(_, c)| *c != "Unclassified").map(|(i, c)| (i, c.clone())).collect();
    heat_cats.sort_by(|a, b| a.1.cmp(&b.1));
    for (ci, _) in &heat_cats {
        medians.push(
            REGIONS
                .iter()
                .map(|r| {
                    let pi = ds.provinces.find(r.code)?;
                    let cell = &mut grid[ci * nprov + pi];
                    let n = cell.len();
                    (n >= 10).then(|| (median(cell).unwrap_or(0.0), n))
                })
                .collect(),
        );
    }
    let heat_bins = charts::Bins::new(medians.iter().flatten().flatten().map(|(m, _)| *m));
    let heat_legend = heat_bins.labels(|v| format!("${v:.0}"));
    let heat: Vec<(String, Vec<HeatCell>)> = heat_cats
        .iter()
        .zip(&medians)
        .map(|((_, cat), row)| {
            let cells = REGIONS
                .iter()
                .zip(row)
                .map(|(r, cell)| match cell {
                    Some((m, n)) => HeatCell {
                        step: Some(heat_bins.step(*m)),
                        display: format!("${:.0}", m),
                        tip: format!("{cat} in {}: median {}/h from {} postings", r.name, wage(*m), thousands(*n as u64)),
                        href: p.link(|q| {
                            q.view = None;
                            q.cat = Some(cat.clone());
                            q.prov = Some(r.code.to_string());
                        }),
                    },
                    None => HeatCell { step: None, display: String::new(), tip: format!("{cat} in {}: fewer than 10 postings with pay", r.name), href: String::new() },
                })
                .collect();
            (cat.clone(), cells)
        })
        .collect();
    let heat_cols: Vec<(&str, &str)> = REGIONS.iter().map(|r| (r.code, r.name)).collect();

    html! {
        div.dashboard.row.g-3 {
            (panel(FULL, "Median offer by field and province",
                html! { "Click a square to see those jobs. A dash means fewer than 10 postings listed pay." },
                html! {
                    (charts::heat_grid(&heat_cols, &heat, "Median hourly offer by field and province"))
                    (charts::steps_legend(&heat_legend))
                }))
            div.col-12 {
                section.panel.card.map-block {
                    div.card-body {
                        div.d-flex.flex-wrap.justify-content-between.align-items-start.gap-2.mb-3 {
                            div {
                                h2.h5.card-title { @if by_wage { "Median offer by province" } @else { "Postings per 10,000 people" } }
                                p.small.text-body-secondary.mb-0 {
                                    @if by_wage { "Median hourly offer in each province and territory. " } @else { "Postings for every 10,000 residents, so big provinces don't win by size alone. " }
                                    "Click a province to see its jobs."
                                }
                            }
                            nav.btn-group.btn-group-sm aria-label="Shade the map by" {
                                @if by_wage { a.btn.btn-outline-secondary href=(p.link(|q| q.map = None)) { "Per person" } } @else { span.btn.btn-secondary.active aria-current="true" { "Per person" } }
                                @if by_wage { span.btn.btn-secondary.active aria-current="true" { "Median offer" } } @else { a.btn.btn-outline-secondary href=(p.link(|q| q.map = Some("wage".into()))) { "Median offer" } }
                            }
                        }
                        div.row.g-4.align-items-start {
                            div.col-lg-7 {
                                (charts::canada_map(&map_regions, "Map of Canada shaded by province"))
                                (charts::steps_legend(&map_legend))
                            }
                            div.map-ranked.col-lg-5 {
                                (charts::hbar(&map_ranked, "Provinces and territories ranked"))
                            }
                        }
                    }
                }
            }
        }
    }
}

/// One summary figure on the charts tab.
fn kpi(label: &str, value: Markup, sub: Option<String>) -> Markup {
    html! {
        div.kpi.col-6.col-md-4.col-xl-2 {
            dl.card.h-100.mb-0 {
                div.card-body {
                    dt.small.fw-semibold.text-body-secondary { (label) }
                    dd.mb-0 {
                        span.kpi-value.d-block.fs-3.fw-bold { (value) }
                        @if let Some(sub) = sub { span.d-block.small.text-body-secondary { (sub) } }
                    }
                }
            }
        }
    }
}

/// The charts tab: dropdown filters, summary figures and the rest of the charts.
fn charts_view(ds: &Dataset, p: &Params) -> Markup {
    let rows = query::filter(ds, p);
    let all: Vec<&Job> = ds.jobs.iter().collect();
    let n = rows.len();

    // ---- KPIs
    let mut wages: Vec<f32> = rows.iter().filter_map(|j| j.wage()).collect();
    let med = median(&mut wages);
    let vacancies: u64 = rows.iter().map(|j| j.vacancies as u64).sum();
    let full_time_id = ds.hours.find("Full time");
    let full_time = rows.iter().filter(|j| Some(j.hours as usize) == full_time_id).count();
    let unspecified_term = ds.terms.find("Unspecified");
    let with_term = rows.iter().filter(|j| Some(j.term as usize) != unspecified_term).count();
    let permanent_id = ds.terms.find("Permanent");
    let permanent = rows.iter().filter(|j| Some(j.term as usize) == permanent_id).count();
    let direct_id = ds.sources.find("Posted on Job Bank");
    let direct = rows.iter().filter(|j| Some(j.source as usize) == direct_id).count();

    // ---- Bar charts by dimension
    let count_tip = |label: &str, c: usize| format!("{label}\n{} postings ({} of the total)", thousands(c as u64), pct(c, n));
    let bars = |dict: &Dict, key: fn(&Job) -> usize, limit: usize, current: &Option<String>, set: Option<fn(&mut Params, &str)>| -> Vec<Bar> {
        count_by(&rows, dict, key)
            .into_iter()
            .take(limit)
            .map(|(i, c)| {
                let label = dict.get(i).to_string();
                Bar {
                    href: set.map(|f| p.link(|q| f(q, &label))),
                    active: current.as_deref() == Some(label.as_str()),
                    tip: count_tip(&label, c),
                    display: thousands(c as u64),
                    value: c as f64,
                    label,
                }
            })
            .collect()
    };
    let by_category = bars(&ds.categories, |j| j.category as usize, usize::MAX, &p.cat, Some(|q, v| toggle(&mut q.cat, v)));
    let by_occupation = bars(&ds.occupations, |j| j.occupation as usize, 12, &p.occ, Some(|q, v| toggle(&mut q.occ, v)));
    let by_city = bars(&ds.cities, |j| j.city as usize, 12, &p.city, Some(|q, v| toggle(&mut q.city, v)));
    let mut by_level = bars(&ds.levels, |j| j.level as usize, usize::MAX, &p.level, Some(|q, v| toggle(&mut q.level, v)));
    by_level.sort_by(|a, b| a.label.cmp(&b.label));
    let by_source = bars(&ds.sources, |j| j.source as usize, usize::MAX, &p.src, Some(|q, v| toggle(&mut q.src, v)));

    // ---- Median wage by category
    let mut cat_wages: Vec<Vec<f32>> = vec![Vec::new(); ds.categories.values.len()];
    for j in &rows {
        if let Some(w) = j.wage() {
            cat_wages[j.category as usize].push(w);
        }
    }
    let mut wage_bars: Vec<Bar> = cat_wages
        .iter_mut()
        .enumerate()
        .filter(|(_, w)| w.len() >= 5)
        .map(|(i, w)| {
            let count = w.len();
            let m = median(w).unwrap_or(0.0);
            let label = ds.categories.get(i).to_string();
            Bar {
                href: Some(p.link(|q| toggle(&mut q.cat, &label))),
                active: p.cat.as_deref() == Some(label.as_str()),
                tip: format!("{label}\nMedian {}/h {}\n{} postings", wage(m), annual(m), thousands(count as u64)),
                display: wage(m),
                value: m,
                label,
            }
        })
        .collect();
    wage_bars.sort_by(|a, b| b.value.total_cmp(&a.value));

    // ---- Wage histogram: $2 bins from $16 to $80/h, open-ended at both ends
    let (lo, hi, step) = (16.0f32, 80.0f32, 2.0f32);
    let nbins = ((hi - lo) / step) as usize;
    let mut hist = vec![0usize; nbins];
    for w in &wages {
        hist[(((w - lo) / step).floor().max(0.0) as usize).min(nbins - 1)] += 1;
    }
    let wage_bins: Vec<Bin> = hist
        .iter()
        .enumerate()
        .map(|(i, &c)| {
            let b = lo + i as f32 * step;
            let range = if i == 0 { format!("Under ${}", b + step) } else if i == nbins - 1 { format!("${b}+") } else { format!("${b}–${}", b + step) };
            Bin { label: format!("${b}"), value: c as f64, tip: format!("{range}/h\n{} postings", thousands(c as u64)) }
        })
        .collect();

    // ---- New postings per week (weeks start Monday)
    let mut weeks: BTreeMap<NaiveDate, usize> = BTreeMap::new();
    for j in &rows {
        let monday = j.posted - Duration::days(j.posted.weekday().num_days_from_monday() as i64);
        *weeks.entry(monday).or_default() += 1;
    }
    let week_bins: Vec<Bin> = weeks
        .iter()
        .map(|(d, &c)| {
            let label = format!("{} {}", &MONTHS[d.month0() as usize][..3], d.day());
            Bin { tip: format!("Week of {label}\n{} new postings", thousands(c as u64)), label, value: c as f64 }
        })
        .collect();
    let week_every = week_bins.len().div_ceil(7).max(1);

    // ---- Benchmark: share of postings vs share of StatCan vacancies, for the chosen province or Canada
    let bench_region = p.prov.clone().filter(|c| regions::by_code(c).is_some()).unwrap_or_else(|| "CA".into());
    let bench_place = if bench_region == "CA" { "Canada" } else { regions::name(&bench_region) };
    let bench_params = Params { month: p.month.clone(), prov: p.prov.clone(), ..Default::default() };
    let bench_rows = query::filter(ds, &bench_params);
    let unclassified = ds.categories.find("Unclassified");
    let classified = bench_rows.iter().filter(|j| Some(j.category as usize) != unclassified).count().max(1);
    let bench_counts = count_by(&bench_rows, &ds.categories, |j| j.category as usize);
    let bench_cats = ds.benchmark.as_ref().and_then(|b| b.by_region.get(&bench_region));
    let bench_total = bench_cats.and_then(|c| c.get("Total")).and_then(|t| t.vacancies);
    let pairs: Vec<Pair> = bench_cats.map_or_else(Vec::new, |cats| {
        let total: f64 = cats.iter().filter(|(k, _)| *k != "Total").filter_map(|(_, v)| v.vacancies).sum();
        let mut v: Vec<Pair> = cats
            .iter()
            .filter(|(k, e)| *k != "Total" && e.vacancies.is_some())
            .map(|(label, e)| {
                let posted = bench_counts.iter().find(|(i, _)| ds.categories.get(*i) == label).map_or(0, |(_, c)| *c);
                let a = posted as f64 / classified as f64;
                let vac = e.vacancies.unwrap_or(0.0);
                let bshare = if total > 0.0 { vac / total } else { 0.0 };
                let ratio = if bshare > 0.0 { a / bshare } else { 0.0 };
                let verdict = if ratio > 1.15 { "Over-represented" } else if ratio < 0.85 { "Under-represented" } else { "Roughly in line" };
                let mut tip = format!(
                    "{label}\nJob Bank: {} of postings\nStatCan: {} of vacancies ({})\n{verdict} on Job Bank ({ratio:.2}×)",
                    pct1(a), pct1(bshare), thousands(vac as u64)
                );
                if let Some(w) = e.wage {
                    tip.push_str(&format!("\nStatCan avg offered wage: {}/h", wage(w)));
                }
                Pair { label: label.clone(), a, b: bshare, tip }
            })
            .collect();
        v.sort_by(|x, y| y.b.total_cmp(&x.b));
        v
    });

    // ---- Filter options
    let months: Vec<(String, String)> = {
        let mut m: Vec<(i32, u32)> = ds.jobs.iter().map(|j| j.month()).collect::<std::collections::BTreeSet<_>>().into_iter().collect();
        m.reverse();
        m.into_iter().map(|ym| (format!("{}-{:02}", ym.0, ym.1), month_name(ym))).collect()
    };
    let opts = |dict: &Dict, key: fn(&Job) -> usize, alpha: bool| sorted_values(count_by(&all, dict, key), dict, alpha);
    let cities: Vec<String> = {
        let prov_id = p.prov.as_deref().and_then(|c| ds.provinces.find(c));
        let pool: Vec<&Job> = match prov_id {
            Some(pi) => ds.jobs.iter().filter(|j| j.province as usize == pi).collect(),
            None => all.clone(),
        };
        let mut c: Vec<String> = count_by(&pool, &ds.cities, |j| j.city as usize)
            .into_iter()
            .take(if prov_id.is_some() { usize::MAX } else { 300 })
            .map(|(i, _)| ds.cities.get(i).to_string())
            .filter(|c| c != "Not given")
            .collect();
        if let Some(cur) = &p.city {
            if !c.contains(cur) && cur != "Not given" {
                c.push(cur.clone());
            }
        }
        c.sort();
        c.push("Not given".into());
        c
    };
    let active_filters = [&p.q, &p.cat, &p.prov, &p.city, &p.occ, &p.wage, &p.month, &p.level, &p.hours, &p.term, &p.src].iter().filter(|f| f.is_some()).count();
    let provinces: Vec<(String, String)> = REGIONS.iter().map(|r| (r.code.to_string(), r.name.to_string())).collect();
    let clear_link = Params { view: p.view.clone(), map: p.map.clone(), ..Default::default() }.link(|_| {});

    html! {
        form.filters.card.card-body.mb-4 method="get" action="/" aria-label="Filter postings" {
            div.row.g-3.align-items-end {
                div.filter.col-12.col-lg-6 {
                    label.form-label.small.fw-semibold for="f-q" { "Search" }
                    input #f-q.form-control type="search" name="q" placeholder="Job title or occupation" value=(p.q.as_deref().unwrap_or("")) class=[p.q.is_some().then_some("border-primary")];
                }
                (dropdown("cat", "Category", "All categories", &same(opts(&ds.categories, |j| j.category as usize, true)), &p.cat))
                (dropdown("prov", "Province or territory", "All of Canada", &provinces, &p.prov))
                (dropdown("city", if p.prov.is_some() { "City" } else { "City (busiest 300)" }, "Any city", &same(cities), &p.city))
                (dropdown("occ", "Occupation", "Any occupation", &same(opts(&ds.occupations, |j| j.occupation as usize, true)), &p.occ))
                (dropdown("wage", "Minimum hourly offer", "Any wage", &wage_options(&p.wage), &p.wage))
                (dropdown("month", "Month posted", "Any month", &months, &p.month))
                (dropdown("level", "Training needed", "Any training", &same(opts(&ds.levels, |j| j.level as usize, true)), &p.level))
                (dropdown("hours", "Hours", "Any hours", &same(opts(&ds.hours, |j| j.hours as usize, false)), &p.hours))
                (dropdown("term", "Term", "Any term", &same(opts(&ds.terms, |j| j.term as usize, false)), &p.term))
                (dropdown("src", "Advertised on", "Anywhere", &same(opts(&ds.sources, |j| j.source as usize, false)), &p.src))
                @if let Some(s) = &p.sort { input type="hidden" name="sort" value=(s); }
                @if let Some(d) = &p.dir { input type="hidden" name="dir" value=(d); }
                @if let Some(m) = &p.map { input type="hidden" name="map" value=(m); }
                input type="hidden" name="view" value="charts";
                div.col-12.d-flex.align-items-center.gap-2 {
                    button.btn.btn-primary type="submit" { "Apply filters" }
                    @if active_filters > 0 {
                        a.reset.btn.btn-link href=(clear_link) { "Clear " (active_filters) (if active_filters == 1 { " filter" } else { " filters" }) }
                    }
                }
            }
        }

        section.summary.mb-4 aria-live="polite" aria-label="Summary" {
            @if n == 0 {
                p.alert.alert-light.border { "No postings match. Try a lower minimum wage, a different month, or all of Canada." }
            } @else {
                div.kpis.row.g-3 {
                    (kpi("Postings", html! { (thousands(n as u64)) }, None))
                    (kpi("Openings", html! { (thousands(vacancies)) }, None))
                    @if let Some(m) = med { (kpi("Median hourly offer", html! { (wage(m)) }, Some(format!("{} full time", annual(m))))) }
                    @else { (kpi("Median hourly offer", html! { "–" }, Some("No pay listed".into()))) }
                    (kpi("Full time", html! { (pct(full_time, n)) }, Some("of postings".into())))
                    (kpi("Permanent", html! { @if with_term > 0 { (pct(permanent, with_term)) } @else { "–" } }, Some("of those stating a term".into())))
                    (kpi("Posted on Job Bank", html! { (pct(direct, n)) }, Some("the rest from partner boards".into())))
                }
            }
        }

        div.dashboard.row.g-3 {
            (panel(HALF, "Postings by category", html! { "Click a bar to filter." }, charts::hbar(&by_category, "Postings by category")))
            (panel(HALF, "Median offer by category", html! { "Middle of each posted range, fields with 5+ postings." }, charts::hbar(&wage_bars, "Median offered wage by category")))
            (panel(HALF, "Top occupations", html! { "The 12 most-posted occupations." }, charts::hbar(&by_occupation, "Most-posted occupations")))
            (panel(HALF, "Top cities", html! { "The 12 cities with the most postings." }, charts::hbar(&by_city, "Postings by city")))
            (panel(HALF, "Wage distribution", html! { "Postings at each hourly offer, in $2 steps." }, charts::vbar(&wage_bins, 4, "Offered wage distribution")))
            (panel(HALF, "New postings per week", html! { "Weeks start on Monday." }, charts::vbar(&week_bins, week_every, "New postings per week")))
            (panel(HALF, "Training required", html! { "TEER levels from Canada's occupation system." }, charts::hbar(&by_level, "Training required")))
            (panel(HALF, "Where postings were advertised", html! { "Job Bank also republishes partner job boards." }, charts::hbar(&by_source, "Where postings were advertised")))
            @if let (Some(b), false) = (&ds.benchmark, pairs.is_empty()) {
                (panel(FULL, "Job Bank vs StatCan vacancies",
                    html! {
                        "Each field's share of Job Bank postings in " (bench_place) ", next to its share of the "
                        (thousands(bench_total.unwrap_or(0.0) as u64))
                        " vacancies employers reported to Statistics Canada in " (b.period) "."
                    },
                    html! {
                        (charts::hbar_pair(&pairs, ["Job Bank postings", &format!("StatCan vacancies, {}", b.period)], pct1, "Job Bank postings vs StatCan vacancies"))
                        p.small.text-body-secondary.mt-3.mb-0 {
                            "A longer blue bar means Job Bank over-represents that work; a shorter one means most of those jobs are advertised elsewhere. "
                            "Follows the chosen province and month only."
                        }
                    }))
            }

        }
    }
}

/// A pine square with a highlighter bar, inline so it costs no request.
const FAVICON: &str = "data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 32 32'%3E%3Crect width='32' height='32' rx='6' fill='%230e7a57'/%3E%3Crect x='7' y='19' width='18' height='5' fill='%23ffe86b'/%3E%3C/svg%3E";

/// The server's page for an unknown address. (The static site shows the dashboard for every path.)
pub fn not_found() -> Markup {
    html! {
        (DOCTYPE)
        html lang="en-CA" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { "Page not found · Canada Job Tracker" }
                link rel="stylesheet" href=(format!("/styles.css?v={}", crate::styles_version()));
                link rel="icon" href=(FAVICON);
            }
            body {
                header.topbar.bg-body-tertiary.border-bottom.mb-4 {
                    div.container.py-3 { a.navbar-brand.fw-bold.fs-4 href="/" { "Canada Job Tracker" } }
                }
                main.container {
                    h1.h4 { "There's no page at this address." }
                    p { a href="/" { "Go to the job search" } }
                }
            }
        }
    }
}

/// The download name for `csv`.
pub fn csv_filename(ds: &Dataset) -> String {
    format!("canada-jobs-{}.csv", ds.as_of)
}

/// The filtered postings as CSV, in table order.
pub fn csv(ds: &Dataset, p: &Params) -> String {
    let mut rows = query::filter(ds, p);
    query::sort(ds, &mut rows, p.sort_key(), p.descending());
    let cell = |s: &str| if s.contains([',', '"', '\n', '\r']) { format!("\"{}\"", s.replace('"', "\"\"")) } else { s.to_string() };
    let opt = |v: Option<f32>| v.map(|v| v.to_string()).unwrap_or_default();
    let mut out = String::from("id,title,occupation,category,skill_level,city,province,hours,term,wage_min_hourly,wage_max_hourly,openings,posted,source\n");
    for j in rows {
        let fields = [
            j.id.to_string(),
            cell(ds.titles.get(j.title as usize)),
            cell(ds.occupations.get(j.occupation as usize)),
            cell(ds.categories.get(j.category as usize)),
            cell(ds.levels.get(j.level as usize)),
            cell(ds.cities.get(j.city as usize)),
            cell(ds.provinces.get(j.province as usize)),
            cell(ds.hours.get(j.hours as usize)),
            cell(ds.terms.get(j.term as usize)),
            opt(j.wage_min),
            opt(j.wage_max),
            j.vacancies.to_string(),
            j.posted.to_string(),
            cell(ds.sources.get(j.source as usize)),
        ];
        out.push_str(&fields.join(","));
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::sample;

    fn params(qs: &str) -> Params {
        serde_urlencoded::from_str::<Params>(qs).unwrap().normalize()
    }

    #[test]
    fn formats_numbers() {
        assert_eq!(thousands(0), "0");
        assert_eq!(thousands(999), "999");
        assert_eq!(thousands(1_000), "1,000");
        assert_eq!(thousands(158_166), "158,166");
        assert_eq!(thousands(1_234_567), "1,234,567");
        assert_eq!(pct(1, 3), "33%");
        assert_eq!(pct(1, 0), "–");
        assert_eq!(per_10k(25.06), "25.1");
        assert_eq!(per_10k(0.0153), "0.02");
        assert_eq!(per_10k(0.0), "0.0");
    }

    #[test]
    fn formats_wage_ranges() {
        assert_eq!(wage_range(17.0, 23.0), "$17–$23");
        assert_eq!(wage_range(16.6, 28.0), "$16.60–$28.00");
        assert_eq!(wage_range(25.53, 25.53), "$25.53");
        assert_eq!(wage_range(32.0, 32.0), "$32");
    }

    #[test]
    fn median_of_odd_even_and_empty() {
        assert_eq!(median(&mut [3.0, 1.0, 2.0]), Some(2.0));
        assert_eq!(median(&mut [4.0, 1.0, 2.0, 3.0]), Some(2.5));
        assert_eq!(median(&mut []), None);
    }

    #[test]
    fn csv_has_header_province_and_quoting() {
        let ds = sample();
        let out = csv(&ds, &params("sort=posted&dir=desc"));
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines[0], "id,title,occupation,category,skill_level,city,province,hours,term,wage_min_hourly,wage_max_hourly,openings,posted,source");
        assert_eq!(lines.len(), 6);
        assert!(lines[1].starts_with("1,Cook,Cooks,"));
        // A title with a comma is quoted; a posting with no city keeps its province.
        let nights = lines.iter().find(|l| l.starts_with("4,")).unwrap();
        assert!(nights.contains(r#""Line Cook, Nights""#), "{nights}");
        assert!(nights.contains("Not given,BC,"), "{nights}");
        assert_eq!(csv(&ds, &params("prov=XX")).lines().count(), 1, "just the header when nothing matches");
    }

    #[test]
    fn csv_filename_uses_data_date() {
        assert_eq!(csv_filename(&sample()), "canada-jobs-2026-09-30.csv");
    }

    #[test]
    fn jobs_tab_shows_a_card_per_posting() {
        let ds = sample();
        let html = page(&ds, &params("")).into_string();
        assert!(html.contains("<title>Canada Job Tracker</title>"));
        assert!(html.contains(r#"<a class="nav-link active" href="/" aria-current="page">Jobs</a>"#), "the jobs tab is current");
        assert!(html.contains("<strong>5</strong> jobs"), "posting count");
        assert_eq!(html.matches(r#"<article class="job card"#).count(), 5);
        assert!(html.contains("view-transition-name: job-1"), "each card can be animated on its own");
        assert!(html.contains(r#"<span class="card-loc text-truncate me-2">British Columbia</span>"#), "a posting with no city shows its province");
        assert!(html.contains(r#"$17–$19<small class="fw-normal text-body-secondary"> an hour</small>"#));
        assert!(html.contains("Download 5 as CSV"));
        assert!(!html.contains("Page 1 of"), "no pager when everything fits");
    }

    #[test]
    fn search_keeps_other_filters_and_shows_them_as_chips() {
        let html = page(&sample(), &params("q=nurse&cat=Health&prov=ON")).into_string();
        assert!(html.contains("<strong>1</strong> job matching “nurse”"), "one posting, singular");
        assert!(html.contains(r#"<input type="hidden" name="cat" value="Health">"#));
        assert!(html.contains(r#"<input type="hidden" name="prov" value="ON">"#));
        assert!(!html.contains(r#"type="hidden" name="q""#));
        assert!(html.contains(r#"href="/?q=nurse&amp;prov=ON" title="Remove Health">"#), "removing a chip keeps the rest");
        assert!(html.contains(">Ontario<"), "provinces show by name");
    }

    #[test]
    fn page_with_no_matches_explains_and_hides_download() {
        let html = page(&sample(), &params("q=astronaut")).into_string();
        assert!(html.contains("No jobs match."));
        assert!(!html.contains("Download "));
        assert!(!html.contains("job card"));
        let charts = page(&sample(), &params("view=charts&prov=ON&wage=290")).into_string();
        assert!(charts.contains("No postings match."));
        assert!(charts.contains(r#"<a class="reset btn btn-link" href="/?view=charts">Clear 2 filters</a>"#), "clearing stays on the tab");
    }

    #[test]
    fn heatmap_tab_links_to_the_jobs() {
        let html = page(&sample(), &params("view=heatmap")).into_string();
        assert!(html.contains("<title>Heatmap · Canada Job Tracker</title>"));
        assert!(html.contains(r#"aria-current="page">Heatmap</a>"#));
        assert!(html.contains("Median offer by field and province"));
        assert!(html.contains(r#"svg class="map""#));
        assert!(html.contains(r#"href="/?prov=ON""#), "a province opens its jobs");
        assert!(!html.contains("job card"));
    }

    #[test]
    fn page_keeps_filters_in_the_form() {
        let html = page(&sample(), &params("view=charts&prov=BC&wage=20&hours=Part+time")).into_string();
        assert!(html.contains(r#"<option value="BC" selected>"#));
        assert!(html.contains(r#"<option value="20" selected>$20+ an hour</option>"#));
        assert!(html.contains(r#"<option value="Part time" selected>"#));
        assert!(html.contains(r#"<input type="hidden" name="view" value="charts">"#), "filtering stays on the tab");
        assert!(html.contains("Clear 3 filters"));
        assert!(html.contains(r#">Postings</dt><dd class="mb-0"><span class="kpi-value d-block fs-3 fw-bold">1</span>"#));
    }

    #[test]
    fn unlisted_minimum_wage_stays_selected() {
        let html = page(&sample(), &params("view=charts&wage=22.5")).into_string();
        assert!(html.contains(r#"<option value="22.5" selected>$22.50+ an hour</option>"#));
        let listed = wage_options(&Some("22.5".into()));
        assert_eq!(listed.iter().position(|(v, _)| v == "22.5"), Some(2), "kept in order between $20 and $25");
    }

    #[test]
    fn not_found_page_links_home() {
        let html = not_found().into_string();
        assert!(html.contains("<title>Page not found · Canada Job Tracker</title>"));
        assert!(html.contains(r#"<a href="/">Go to the job search</a>"#));
    }

    #[test]
    fn page_numbers_show_the_ends_and_neighbours() {
        assert_eq!(page_numbers(1, 1), [Some(1)]);
        assert_eq!(page_numbers(1, 3), [Some(1), Some(2), Some(3)]);
        assert_eq!(page_numbers(1, 100), [Some(1), Some(2), None, Some(100)]);
        assert_eq!(page_numbers(50, 100), [Some(1), None, Some(49), Some(50), Some(51), None, Some(100)]);
        assert_eq!(page_numbers(3, 100), [Some(1), Some(2), Some(3), Some(4), None, Some(100)], "no gap of one page");
    }

    #[test]
    fn pages_through_the_cards() {
        let p = params("q=cook&page=3");
        assert_eq!(page_link(&p, 2), "/?q=cook&page=2#results");
        assert_eq!(page_link(&p, 1), "/?q=cook#results", "page 1 has no page number");
        let html = page(&sample(), &params("page=9")).into_string();
        assert_eq!(html.matches(r#"<article class="job card"#).count(), 5, "an out-of-range page shows the last");
        assert!(!html.contains("rel=\"next\"") && !html.contains("rel=\"prev\""));
    }
}

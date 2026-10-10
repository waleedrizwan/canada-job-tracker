//! The dashboard page and CSV export.

use std::collections::BTreeMap;

use chrono::{Datelike, Duration, NaiveDate};
use maud::{DOCTYPE, Markup, html};

use crate::charts::{self, Bar, Bin, Pair, compact};
use crate::data::{Dataset, Dict, Job};
use crate::query::{self, Params, toggle};

const PAGE_SIZE: usize = 25;
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

fn select(name: &str, label: &str, any: &str, options: &[String], current: &Option<String>) -> Markup {
    html! {
        label {
            span { (label) }
            select name=(name) {
                option value="" { (any) }
                @for o in options {
                    option value=(o) selected[current.as_deref() == Some(o.as_str())] { (o) }
                }
            }
        }
    }
}

fn sorted_values(rows: Vec<(usize, usize)>, dict: &Dict, alphabetical: bool) -> Vec<String> {
    let mut v: Vec<String> = rows.into_iter().map(|(i, _)| dict.get(i).to_string()).collect();
    if alphabetical {
        v.sort();
    }
    v
}

pub fn page(ds: &Dataset, p: &Params) -> Markup {
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
    let kpis = [
        ("Postings", thousands(n as u64), format!("of {} total", thousands(ds.jobs.len() as u64))),
        ("Openings advertised", thousands(vacancies), "a single posting can hire several people".into()),
        ("Median offered wage", med.map_or("–".into(), |m| format!("{}/h", wage(m))), med.map_or("no wage data".into(), annual)),
        ("Full time", pct(full_time, n), "of postings".into()),
        ("Permanent", pct(permanent, with_term), "of postings that state a term".into()),
        ("Posted on Job Bank", pct(direct, n), "the rest come from partner boards".into()),
    ];

    // ---- Bar charts by dimension
    let count_tip = |label: &str, c: usize| format!("{label}\n{} postings · {}", thousands(c as u64), pct(c, n));
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

    // ---- Benchmark: share of postings vs share of StatCan vacancies, by category
    let month_only = Params { month: p.month.clone(), ..Default::default() };
    let bench_rows = query::filter(ds, &month_only);
    let unclassified = ds.categories.find("Unclassified");
    let classified = bench_rows.iter().filter(|j| Some(j.category as usize) != unclassified).count().max(1);
    let bench_counts = count_by(&bench_rows, &ds.categories, |j| j.category as usize);
    let pairs: Vec<Pair> = ds.benchmark.as_ref().map_or_else(Vec::new, |b| {
        let total: f64 = b.by_category.iter().filter(|(k, _)| *k != "Total").filter_map(|(_, v)| v.vacancies).sum();
        let mut v: Vec<Pair> = b
            .by_category
            .iter()
            .filter(|(k, _)| *k != "Total")
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

    // ---- Table
    let mut sorted = rows.clone();
    query::sort(ds, &mut sorted, p.sort_key(), p.descending());
    let pages = n.div_ceil(PAGE_SIZE).max(1);
    let cur = p.page().min(pages);
    let slice = &sorted[(cur - 1) * PAGE_SIZE..(cur * PAGE_SIZE).min(n)];
    let sort_link = |key: &str| {
        let desc = if p.sort_key() == key { !p.descending() } else { matches!(key, "wage" | "vacancies" | "posted") };
        p.link(|q| {
            q.sort = Some(key.to_string());
            q.dir = Some(if desc { "desc" } else { "asc" }.into());
        })
    };
    let page_link = |n: usize| {
        let mut q = p.clone();
        q.page = Some(n.to_string());
        format!("/?{}", serde_urlencoded::to_string(&q).unwrap_or_default())
    };
    let export_link = p.link(|_| {}).replacen('/', "/export.csv", 1);

    // ---- Filter options
    let months: Vec<(String, String)> = {
        let mut m: Vec<(i32, u32)> = ds.jobs.iter().map(|j| j.month()).collect::<std::collections::BTreeSet<_>>().into_iter().collect();
        m.reverse();
        m.into_iter().map(|ym| (format!("{}-{:02}", ym.0, ym.1), month_name(ym))).collect()
    };
    let opts = |dict: &Dict, key: fn(&Job) -> usize, alpha: bool| sorted_values(count_by(&all, dict, key), dict, alpha);
    let mut cities = opts(&ds.cities, |j| j.city as usize, true);
    if let Some(i) = cities.iter().position(|c| c == "Unspecified") {
        let u = cities.remove(i);
        cities.push(u);
    }
    let headers = [
        ("title", "Job title", false), ("occupation", "Occupation", false), ("city", "City", false), ("hours", "Hours", false),
        ("term", "Term", false), ("wage", "Wage", true), ("vacancies", "Openings", true), ("posted", "Posted", false),
    ];
    let as_of = format!("{} {}, {}", &MONTHS[ds.as_of.month0() as usize][..3], ds.as_of.day(), ds.as_of.year());

    html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { "Ontario Job Tracker" }
                meta name="description" content="Explore job postings across Ontario by occupation, city and offered wage, using Government of Canada Job Bank open data and Statistics Canada vacancy estimates.";
                link rel="stylesheet" href=(format!("/styles.css?v={}", crate::styles_version()));
            }
            body {
                header.top {
                    div {
                        h1 { a href="/" { "Ontario Job Tracker" } }
                        p.sub {
                            (thousands(ds.jobs.len() as u64)) " postings across Ontario, "
                            (ds.months.last().map_or("", String::as_str)) " to " (ds.months.first().map_or("", String::as_str))
                            " · Government of Canada Job Bank · latest posting " (as_of)
                        }
                    }
                }

                form.filters method="get" action="/" aria-label="Filters" {
                    label.f-search {
                        span { "Search" }
                        input type="search" name="q" placeholder="Job title or occupation" value=(p.q.as_deref().unwrap_or(""));
                    }
                    (select("cat", "Category", "All categories", &opts(&ds.categories, |j| j.category as usize, true), &p.cat))
                    (select("occ", "Occupation", "All occupations", &opts(&ds.occupations, |j| j.occupation as usize, true), &p.occ))
                    (select("city", "City", "All cities", &cities, &p.city))
                    (select("level", "Skill level", "Any", &opts(&ds.levels, |j| j.level as usize, true), &p.level))
                    (select("hours", "Hours", "Any", &opts(&ds.hours, |j| j.hours as usize, false), &p.hours))
                    (select("term", "Term", "Any", &opts(&ds.terms, |j| j.term as usize, false), &p.term))
                    label {
                        span { "Min wage ($/h)" }
                        input.num type="number" name="wage" min="0" max="300" step="1" placeholder="Any" value=(p.wage.as_deref().unwrap_or(""));
                    }
                    label {
                        span { "Posted" }
                        select name="month" {
                            option value="" { "All months" }
                            @for (v, name) in &months {
                                option value=(v) selected[p.month.as_deref() == Some(v.as_str())] { (name) }
                            }
                        }
                    }
                    (select("src", "Source", "All sources", &opts(&ds.sources, |j| j.source as usize, false), &p.src))
                    @if let Some(s) = &p.sort { input type="hidden" name="sort" value=(s); }
                    @if let Some(d) = &p.dir { input type="hidden" name="dir" value=(d); }
                    div.actions {
                        button.primary type="submit" { "Apply" }
                        a.button.ghost href="/" { "Reset" }
                    }
                }

                section.kpis aria-label="Summary" {
                    @for (label, value, note) in &kpis {
                        div.kpi { div.label { (label) } div.value { (value) } div.note { (note) } }
                    }
                }

                main.grid {
                    article.card {
                        h2 { "Postings by category" }
                        p.hint { "NOC 2021 broad occupational category · click a bar to filter" }
                        div.chart { (charts::hbar(&by_category, "Postings by category")) }
                    }
                    article.card {
                        h2 { "Median offered wage by category" }
                        p.hint { "Hourly, midpoint of the posted range · click a bar to filter" }
                        div.chart { (charts::hbar(&wage_bars, "Median offered wage by category")) }
                    }
                    article.card {
                        h2 { "Most-posted occupations" }
                        p.hint { "Top 12 NOC unit groups · click a bar to filter" }
                        div.chart { (charts::hbar(&by_occupation, "Most-posted occupations")) }
                    }
                    article.card {
                        h2 { "Postings by city" }
                        p.hint { "Top 12 · click a bar to filter" }
                        div.chart { (charts::hbar(&by_city, "Postings by city")) }
                    }
                    article.card {
                        h2 { "Offered wage distribution" }
                        p.hint { "Hourly midpoint, $2 bins · hover a bar for counts" }
                        div.chart { (charts::vbar(&wage_bins, 4, "Offered wage distribution")) }
                    }
                    article.card {
                        h2 { "New postings per week" }
                        p.hint { "By first posting date" }
                        div.chart { (charts::vbar(&week_bins, week_every, "New postings per week")) }
                    }
                    article.card {
                        h2 { "Skill level required" }
                        p.hint { "NOC TEER category · click a bar to filter" }
                        div.chart { (charts::hbar(&by_level, "Skill level required")) }
                    }
                    article.card {
                        h2 { "Where postings come from" }
                        p.hint { "Job Bank also republishes postings from partner job boards · click a bar to filter" }
                        div.chart { (charts::hbar(&by_source, "Posting source")) }
                    }
                    @if let Some(b) = &ds.benchmark {
                        article.card.wide {
                            h2 { "Postings vs. estimated vacancies" }
                            p.hint {
                                "Share of the Ontario total in each category · StatCan estimates for " (b.period) " ("
                                (thousands(b.by_category.get("Total").and_then(|t| t.vacancies).unwrap_or(0.0) as u64)) " vacancies)"
                            }
                            div.chart { (charts::hbar_pair(&pairs, ["Job Bank postings", &format!("StatCan vacancies, {}", b.period)], pct1, "Job Bank postings vs StatCan vacancies")) }
                            p.note-box {
                                "Statistics Canada surveys employers for the number of jobs they are actively trying to fill, whether or not those jobs are advertised. "
                                "Where Job Bank's share is higher than StatCan's, Job Bank over-represents that kind of work. Where it's lower, most of those openings are "
                                "advertised elsewhere or filled without a public posting. This chart reflects all Ontario postings in the selected month and ignores the other filters."
                            }
                        }
                    }
                }

                section.card.table-card {
                    div.table-head {
                        h2 { "Postings " span.muted { "· " (thousands(n as u64)) } }
                        a.button.ghost href=(export_link) { "Export CSV" }
                    }
                    div.table-wrap {
                        table {
                            thead { tr {
                                @for (key, label, num) in headers {
                                    @let active = p.sort_key() == key;
                                    th class=(format!("{}{}", if num { "num " } else { "" }, if active { if p.descending() { "sorted" } else { "sorted asc" } } else { "" })) {
                                        a href=(sort_link(key)) { (label) }
                                    }
                                }
                            } }
                            tbody {
                                @for j in slice {
                                    @let title = ds.titles.get(j.title as usize);
                                    @let city = ds.cities.get(j.city as usize);
                                    @let loc = if city == "Unspecified" { "Ontario".to_string() } else { format!("{city}, ON") };
                                    @let search = format!("https://www.jobbank.gc.ca/jobsearch/jobsearch?{}",
                                        serde_urlencoded::to_string([("searchstring", title), ("locationstring", loc.as_str())]).unwrap_or_default());
                                    tr {
                                        td.title { a href=(search) target="_blank" rel="noopener" title="Search Job Bank for current postings like this" { (title) } }
                                        td { (ds.occupations.get(j.occupation as usize)) }
                                        td { (city) }
                                        td { (ds.hours.get(j.hours as usize)) }
                                        td { (ds.terms.get(j.term as usize)) }
                                        td.num {
                                            @match (j.wage_min, j.wage_max) {
                                                (Some(a), Some(b)) if a != b => { (wage_short(a)) "–" (wage_short(b)) }
                                                (Some(a), _) => { (wage_short(a)) }
                                                _ => { "–" }
                                            }
                                        }
                                        td.num { (j.vacancies) }
                                        td { (j.posted) }
                                    }
                                }
                                @if slice.is_empty() {
                                    tr { td.muted colspan="8" { "No postings match these filters" } }
                                }
                            }
                        }
                    }
                    nav.pager aria-label="Pages" {
                        @if cur > 1 { a.button.ghost href=(page_link(cur - 1)) { "‹ Prev" } } @else { span.button.ghost.disabled { "‹ Prev" } }
                        span { "Page " (thousands(cur as u64)) " of " (thousands(pages as u64)) }
                        @if cur < pages { a.button.ghost href=(page_link(cur + 1)) { "Next ›" } } @else { span.button.ghost.disabled { "Next ›" } }
                    }
                }

                footer.foot {
                    p {
                        strong { "Sources. " }
                        "Postings come from " a href=(ds.source_url) target="_blank" rel="noopener" { "Job Postings Advertised on Canada's National Job Bank" }
                        " (Employment and Social Development Canada, Open Government Licence – Canada). Vacancy estimates come from Statistics Canada's "
                        a href="https://www150.statcan.gc.ca/t1/tbl1/en/tv.action?pid=1410044401" target="_blank" rel="noopener" { "Job Vacancy and Wage Survey, table 14-10-0444-01" }
                        ". The data refreshes automatically when new releases come out."
                    }
                    p {
                        strong { "Caveats. " }
                        "Job Bank captures only part of the market. It over-represents employers hiring through the Temporary Foreign Worker program "
                        "(who must advertise there) and under-represents professional and tech roles. The data has no employer names and doesn't record when a job was filled. "
                        "Wages are converted to hourly at 40 hours a week unless the posting gives its hours. Toronto includes North York, Etobicoke, Scarborough, York and East York."
                    }
                    p { "Built in Rust. No JavaScript. " a href="https://github.com/waleedrizwan/ontario-job-tracker" { "Source code" } }
                }
            }
        }
    }
}

/// The filtered postings as CSV, in table order.
pub fn csv(ds: &Dataset, p: &Params) -> String {
    let mut rows = query::filter(ds, p);
    query::sort(ds, &mut rows, p.sort_key(), p.descending());
    let cell = |s: &str| if s.contains([',', '"', '\n']) { format!("\"{}\"", s.replace('"', "\"\"")) } else { s.to_string() };
    let opt = |v: Option<f32>| v.map(|v| v.to_string()).unwrap_or_default();
    let mut out = String::from("id,title,occupation,category,skill_level,city,hours,term,wage_min_hourly,wage_max_hourly,openings,posted,source\n");
    for j in rows {
        let fields = [
            j.id.to_string(),
            cell(ds.titles.get(j.title as usize)),
            cell(ds.occupations.get(j.occupation as usize)),
            cell(ds.categories.get(j.category as usize)),
            cell(ds.levels.get(j.level as usize)),
            cell(ds.cities.get(j.city as usize)),
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

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
        ("term", "Term", false), ("wage", "Hourly offer", true), ("vacancies", "Openings", true), ("posted", "Posted", false),
    ];
    let more_open = p.q.is_some() || p.occ.is_some() || p.level.is_some() || p.hours.is_some() || p.term.is_some() || p.src.is_some();
    let filtered_at_all = more_open || p.cat.is_some() || p.city.is_some() || p.wage.is_some() || p.month.is_some();
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
                title { "Ontario Job Tracker" }
                meta name="description" content="Who's hiring in Ontario, for what, and what it pays. Every Ontario job posted on the Government of Canada's Job Bank, compared with Statistics Canada's vacancy survey.";
                link rel="preconnect" href="https://fonts.googleapis.com";
                link rel="preconnect" href="https://fonts.gstatic.com" crossorigin;
                link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Atkinson+Hyperlegible+Next:ital,wght@0,400;0,500;0,700;0,800;1,400&display=swap";
                link rel="stylesheet" href=(format!("/styles.css?v={}", crate::styles_version()));
            }
            body {
                header.masthead {
                    a.wordmark href="/" { "Ontario Job Tracker" }
                    p.about {
                        "Every Ontario job posted on Canada's Job Bank from " (span) ". Updated when the government publishes new data; latest posting " (updated) "."
                    }
                }

                form.ask method="get" action="/" aria-label="Choose which jobs to show" {
                    p.sentence {
                        "Show me "
                        label.field {
                            span.sr { "Category" }
                            select name="cat" {
                                option value="" { "all kinds of" }
                                @for o in &opts(&ds.categories, |j| j.category as usize, true) {
                                    option value=(o) selected[p.cat.as_deref() == Some(o.as_str())] { (o.to_lowercase()) }
                                }
                            }
                        }
                        " jobs in "
                        label.field {
                            span.sr { "City" }
                            select name="city" {
                                option value="" { "all of Ontario" }
                                @for o in &cities {
                                    option value=(o) selected[p.city.as_deref() == Some(o.as_str())] { (o) }
                                }
                            }
                        }
                        " paying at least $"
                        label.field.money {
                            span.sr { "Minimum hourly wage" }
                            input type="number" name="wage" min="0" max="300" step="1" placeholder="any" value=(p.wage.as_deref().unwrap_or(""));
                        }
                        " an hour, posted "
                        label.field {
                            span.sr { "Month posted" }
                            select name="month" {
                                option value="" { "any month" }
                                @for (v, name) in &months {
                                    option value=(v) selected[p.month.as_deref() == Some(v.as_str())] { "in " (name) }
                                }
                            }
                        }
                        "."
                    }
                    details.more open[more_open] {
                        summary { "More filters" }
                        div.more-grid {
                            label.stack {
                                span { "Job title or occupation contains" }
                                input type="search" name="q" placeholder="e.g. nurse, electrician" value=(p.q.as_deref().unwrap_or(""));
                            }
                            (select("occ", "Occupation", "Any occupation", &opts(&ds.occupations, |j| j.occupation as usize, true), &p.occ))
                            (select("level", "Training needed", "Any", &opts(&ds.levels, |j| j.level as usize, true), &p.level))
                            (select("hours", "Hours", "Any", &opts(&ds.hours, |j| j.hours as usize, false), &p.hours))
                            (select("term", "Term", "Any", &opts(&ds.terms, |j| j.term as usize, false), &p.term))
                            (select("src", "Advertised on", "Anywhere", &opts(&ds.sources, |j| j.source as usize, false), &p.src))
                        }
                    }
                    @if let Some(s) = &p.sort { input type="hidden" name="sort" value=(s); }
                    @if let Some(d) = &p.dir { input type="hidden" name="dir" value=(d); }
                    div.ask-actions {
                        button type="submit" { "Show jobs" }
                        @if filtered_at_all { a.start-over href="/" { "Start over" } }
                    }
                }

                section.answer aria-live="polite" {
                    @if n == 0 {
                        p { "No postings match. Try a lower minimum wage, a different month, or all of Ontario." }
                    } @else {
                        p.lead {
                            strong { (thousands(n as u64)) } (if n == 1 { " posting" } else { " postings" }) " for "
                            strong { (thousands(vacancies)) } (if vacancies == 1 { " opening" } else { " openings" }) "."
                            @if let Some(m) = med {
                                " Half offer more than " strong { (wage(m)) " an hour" } ", or about $" (thousands(((m * HOURS_PER_YEAR) / 1000.0).round() as u64 * 1000)) " a year full time."
                            }
                        }
                        p.detail {
                            (pct(full_time, n)) " are full time"
                            @if with_term > 0 { ", and " (pct(permanent, with_term)) " of those that state a term are permanent" }
                            ". " (pct(direct, n)) " were posted directly on Job Bank; the rest were copied from partner job boards."
                        }
                    }
                }

                main.charts {
                    section.chart-block {
                        h2 { "What kind of work?" }
                        p.hint { "Click a bar to see only that kind of job." }
                        (charts::hbar(&by_category, "Postings by category"))
                    }
                    section.chart-block {
                        h2 { "What does it pay?" }
                        p.hint { "Median hourly offer in each field, using the middle of each posted range." }
                        (charts::hbar(&wage_bars, "Median offered wage by category"))
                    }
                    section.chart-block {
                        h2 { "Which jobs come up most?" }
                        p.hint { "The 12 most-posted occupations." }
                        (charts::hbar(&by_occupation, "Most-posted occupations"))
                    }
                    section.chart-block {
                        h2 { "Where?" }
                        p.hint { "The 12 places with the most postings." }
                        (charts::hbar(&by_city, "Postings by city"))
                    }
                    section.chart-block {
                        h2 { "How is pay spread out?" }
                        p.hint { "Number of postings at each hourly offer, in $2 steps. Hover a bar for the count." }
                        (charts::vbar(&wage_bins, 4, "Offered wage distribution"))
                    }
                    section.chart-block {
                        h2 { "When were they posted?" }
                        p.hint { "New postings each week." }
                        (charts::vbar(&week_bins, week_every, "New postings per week"))
                    }
                    section.chart-block {
                        h2 { "How much training do they ask for?" }
                        p.hint { "Training, education and experience levels (TEER) from Canada's occupation system." }
                        (charts::hbar(&by_level, "Training required"))
                    }
                    section.chart-block {
                        h2 { "Where were they advertised?" }
                        p.hint { "Job Bank also republishes postings from partner job boards." }
                        (charts::hbar(&by_source, "Where postings were advertised"))
                    }
                    @if let Some(b) = &ds.benchmark {
                        section.chart-block.wide {
                            h2 { "What Job Bank misses" }
                            p.hint {
                                "Each field's share of Job Bank postings, next to its share of the "
                                (thousands(b.by_category.get("Total").and_then(|t| t.vacancies).unwrap_or(0.0) as u64))
                                " jobs Ontario employers told Statistics Canada they were trying to fill in " (b.period) "."
                            }
                            (charts::hbar_pair(&pairs, ["Job Bank postings", &format!("StatCan vacancies, {}", b.period)], pct1, "Job Bank postings vs StatCan vacancies"))
                            p.note {
                                "Where the green bar is longer, Job Bank over-represents that work. Where it's shorter, most of those jobs are advertised elsewhere or filled without a public posting. "
                                "This comparison covers all of Ontario for the chosen month and ignores the other filters."
                            }
                        }
                    }
                }

                section.listing {
                    div.listing-head {
                        h2 { "The postings" }
                        a.download href=(export_link) download { "Download these " (thousands(n as u64)) " as a spreadsheet (CSV)" }
                    }
                    div.table-wrap {
                        table {
                            thead { tr {
                                @for (key, label, num) in headers {
                                    @let active = p.sort_key() == key;
                                    th class=(format!("{}{}", if num { "num " } else { "" }, if active { if p.descending() { "sorted" } else { "sorted asc" } } else { "" }))
                                        aria-sort=(if active { if p.descending() { "descending" } else { "ascending" } } else { "none" }) {
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
                                        td.title { a href=(search) target="_blank" rel="noopener" title="Find current postings like this on Job Bank" { (title) } }
                                        td { (ds.occupations.get(j.occupation as usize)) }
                                        td { (if city == "Unspecified" { "Not given" } else { city }) }
                                        td { (ds.hours.get(j.hours as usize)) }
                                        td { (ds.terms.get(j.term as usize)) }
                                        td.num {
                                            @match (j.wage_min, j.wage_max) {
                                                (Some(a), Some(b)) if a != b => { (wage_short(a)) "–" (wage_short(b)) }
                                                (Some(a), _) => { (wage_short(a)) }
                                                _ => { "Not given" }
                                            }
                                        }
                                        td.num { (j.vacancies) }
                                        td.date { (format!("{} {}", &MONTHS[j.posted.month0() as usize][..3], j.posted.day())) }
                                    }
                                }
                                @if slice.is_empty() {
                                    tr { td.empty-row colspan="8" { "No postings match these filters." } }
                                }
                            }
                        }
                    }
                    nav.pager aria-label="Pages" {
                        @if cur > 1 { a href=(page_link(cur - 1)) rel="prev" { "Previous 25" } } @else { span {} }
                        span.where { "Page " (thousands(cur as u64)) " of " (thousands(pages as u64)) }
                        @if cur < pages { a href=(page_link(cur + 1)) rel="next" { "Next 25" } } @else { span {} }
                    }
                }

                footer.colophon {
                    div {
                        h3 { "Where this comes from" }
                        p {
                            "Postings: " a href=(ds.source_url) target="_blank" rel="noopener" { "Job Postings Advertised on Canada's National Job Bank" }
                            ", Employment and Social Development Canada, under the Open Government Licence – Canada. "
                            "Vacancy estimates: " a href="https://www150.statcan.gc.ca/t1/tbl1/en/tv.action?pid=1410044401" target="_blank" rel="noopener" { "Statistics Canada, Job Vacancy and Wage Survey, table 14-10-0444-01" } "."
                        }
                    }
                    div {
                        h3 { "What to keep in mind" }
                        p {
                            "Job Bank is one slice of the market. Employers hiring through the Temporary Foreign Worker program must advertise there, so food service, caregiving and trades show up more, and office and tech jobs less. "
                            "Employer names and closing dates aren't published. Wages are converted to hourly at 40 hours a week unless the posting says otherwise. "
                            "Toronto includes North York, Etobicoke, Scarborough, York and East York."
                        }
                    }
                    p.made { "An independent project, not affiliated with the Government of Canada. " a href="https://github.com/waleedrizwan/ontario-job-tracker" { "Source code" } }
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

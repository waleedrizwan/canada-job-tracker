//! Downloads the official sources and builds a `RawFile`:
//!
//! 1. Job Bank open data (ESDC): every posting advertised on Canada's National Job Bank,
//!    published monthly on open.canada.ca, from the latest `months` files.
//! 2. StatCan Job Vacancy and Wage Survey, table 14-10-0444: estimated vacancies and average
//!    offered hourly wage by region and broad occupational category, as a benchmark.
//! 3. StatCan population estimates, table 17-10-0009, for per-capita rates on the map.
//!
//! The Job Bank files are ~45 MB of UTF-16 each; they're decoded and parsed as they stream in,
//! so memory stays flat regardless of file size.

use std::collections::HashMap;

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};

use crate::data::{BenchEntry, Benchmark, COLS, DICT_COLS, RawFile, RawRow};
use crate::regions::{self, REGIONS};

const DATASET: &str = "ea639e28-c0fc-48bf-b5dd-b8899bd43072";
const HOURS_PER_YEAR: f64 = 2080.0;

/// NOC 2021 broad occupational categories (first digit of the 5-digit code).
pub const BROAD: [&str; 10] = [
    "Senior management",
    "Business, finance & admin",
    "Sciences & tech",
    "Health",
    "Education, law & social services",
    "Arts, culture & recreation",
    "Sales & service",
    "Trades & transport",
    "Natural resources & agriculture",
    "Manufacturing & utilities",
];
/// TEER (second digit): the training, education, experience and responsibilities a job needs.
const TEER: [&str; 6] = [
    "TEER 0 · Management",
    "TEER 1 · University degree",
    "TEER 2 · College / apprenticeship (2+ yrs)",
    "TEER 3 · College / apprenticeship (<2 yrs)",
    "TEER 4 · High school",
    "TEER 5 · No formal education",
];
/// Toronto's former municipalities, which Job Bank reports separately.
const TORONTO: [&str; 5] = ["North York", "Etobicoke", "Scarborough", "York", "East York"];

pub struct MonthFile {
    pub name: String,
    pub url: String,
}

fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent("canada-job-tracker (+https://github.com/waleedrizwan/canada-job-tracker)")
        .build()
        .expect("http client")
}

/// The latest `months` English monthly Job Bank files, newest first.
pub async fn job_bank_files(months: usize) -> Result<Vec<MonthFile>> {
    let url = format!("https://open.canada.ca/data/api/action/package_show?id={DATASET}");
    let v: Value = client().get(url).send().await?.error_for_status()?.json().await?;
    let mut res: Vec<&Value> = v["result"]["resources"]
        .as_array()
        .context("no resources in Job Bank dataset")?
        .iter()
        .filter(|r| {
            let u = r["url"].as_str().unwrap_or("");
            u.contains("-en-") && u.to_lowercase().ends_with(".csv")
        })
        .collect();
    res.sort_by(|a, b| b["created"].as_str().cmp(&a["created"].as_str()));
    Ok(res
        .into_iter()
        .take(months)
        .map(|r| {
            let name = r["name"]["en"].as_str().or(r["name"].as_str()).unwrap_or("");
            MonthFile {
                name: name.split(" Job Postings").next().unwrap_or(name).trim().to_string(),
                url: r["url"].as_str().unwrap_or("").to_string(),
            }
        })
        .collect())
}

#[derive(Default)]
struct Interner {
    index: HashMap<String, u32>,
    values: Vec<String>,
}
impl Interner {
    fn id(&mut self, s: &str) -> u32 {
        if let Some(&i) = self.index.get(s) {
            return i;
        }
        let i = self.values.len() as u32;
        self.values.push(s.to_string());
        self.index.insert(s.to_string(), i);
        i
    }
}

/// Distinct values for each text column; rows refer to them by index.
#[derive(Default)]
struct Dicts(HashMap<&'static str, Interner>);
impl Dicts {
    fn id(&mut self, col: &'static str, v: &str) -> u32 {
        self.0.entry(col).or_default().id(v)
    }
}

fn na(v: Option<&str>) -> Option<&str> {
    v.map(str::trim).filter(|v| !v.is_empty() && *v != "NA" && !v.starts_with('*'))
}

fn title_case(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut start = true;
    for c in s.chars() {
        out.extend(if start { c.to_uppercase().collect::<Vec<_>>() } else { vec![c] });
        start = !c.is_alphanumeric();
    }
    out
}

/// Converts a posted pay figure to hourly, using the posting's weekly hours when given.
fn hourly(per: Option<&str>, value: Option<&str>, hours_per: Option<&str>, hours_max: Option<&str>) -> Option<f32> {
    let v: f64 = value?.parse().ok().filter(|v: &f64| *v > 0.0)?;
    let weekly = match (hours_per, hours_max.and_then(|h| h.parse::<f64>().ok())) {
        (Some("Week"), Some(h)) if h > 0.0 => h,
        _ => 40.0,
    };
    let h = match per? {
        "Hour" => v,
        "Day" => v / 8.0,
        "Week" => v / weekly,
        "Bi-weekly" => v / (weekly * 2.0),
        "Month" => v * 12.0 / (weekly * 52.0),
        "Year" => v / HOURS_PER_YEAR,
        _ => return None,
    };
    (10.0..=300.0).contains(&h).then(|| ((h * 100.0).round() / 100.0) as f32)
}

struct Columns(HashMap<String, usize>);
impl Columns {
    fn get<'a>(&self, fields: &[&'a str], name: &str) -> Option<&'a str> {
        na(self.0.get(name).and_then(|&i| fields.get(i).copied()))
    }
}

/// Parses one Job Bank row straight into the compact on-disk form.
fn parse_line(cols: &Columns, line: &str, d: &mut Dicts) -> Option<RawRow> {
    let f: Vec<&str> = line.split('\t').collect();
    let region = regions::by_job_bank_name(cols.get(&f, "Province/Territory")?)?;
    let g = |name: &str| cols.get(&f, name);
    let noc = g("NOC21 Code").unwrap_or("");
    let digit = |i: usize| noc.as_bytes().get(i).filter(|b| b.is_ascii_digit()).map(|b| (b - b'0') as usize);

    let per = g("Salary Per");
    let (hp, hm) = (g("Hours Per"), g("Hours Maximum"));
    let lo = hourly(per, g("Salary Minimum"), hp, hm);
    let hi = hourly(per, g("Salary Maximum"), hp, hm).or(lo);
    let hi = match (lo, hi) {
        (Some(l), Some(h)) if h < l => Some(l),
        _ => hi,
    };
    let posted = g("First Posting Date")?.replace('/', "-");
    // Cities carry their province ("Richmond, BC") so same-named places stay apart.
    let city = match g("City") {
        Some(c) if region.code == "ON" && TORONTO.contains(&c) => d.id("city", "Toronto, ON"),
        Some(c) => d.id("city", &format!("{c}, {}", region.code)),
        None => d.id("city", "Not given"),
    };
    let hours = match g("Employment Type") {
        Some(t) if t.starts_with("Part time leading") => "Part time → full time",
        Some(t) => t,
        None => "Unspecified",
    };
    let term = g("Employment Term").unwrap_or("Unspecified");
    let title = match g("Original Job Title") {
        Some(t) => d.id("title", t),
        None => d.id("title", &title_case(g("Job Title").unwrap_or("Untitled"))),
    };

    Some((
        g("WIC Job Location Snapshot ID").and_then(|v| v.parse().ok()).unwrap_or(0),
        title,
        d.id("occupation", g("NOC21 Code Name").unwrap_or("Unclassified")),
        d.id("category", digit(0).map_or("Unclassified", |n| BROAD[n])),
        d.id("level", digit(1).and_then(|n| TEER.get(n).copied()).unwrap_or("Unclassified")),
        city,
        d.id("province", region.code),
        d.id("employmentType", hours),
        d.id("term", term.strip_suffix(" employment").unwrap_or(term)),
        lo,
        hi,
        posted,
        g("Vacancy Count").and_then(|v| v.parse().ok()).filter(|&v| v > 0).unwrap_or(1),
        d.id("source", if g("External Indicator") == Some("1") { "Partner job boards" } else { "Posted on Job Bank" }),
    ))
}

/// Streams one monthly file, returning its postings.
async fn load_month(file: &MonthFile, dicts: &mut Dicts) -> Result<Vec<RawRow>> {
    let mut resp = client().get(&file.url).send().await?.error_for_status()?;
    // BOM sniffing picks UTF-16LE; the files are tab-separated with no quoting.
    let mut decoder = encoding_rs::UTF_16LE.new_decoder();
    let mut buf = String::new();
    let mut cols: Option<Columns> = None;
    let mut out = Vec::new();

    let mut handle = |line: &str, cols: &mut Option<Columns>| {
        let line = line.trim_end_matches('\r');
        match cols {
            None => {
                let header = line.split('\t').enumerate().map(|(i, h)| (h.split_whitespace().collect::<Vec<_>>().join(" "), i));
                *cols = Some(Columns(header.collect()));
            }
            Some(c) => {
                if let Some(p) = parse_line(c, line, dicts) {
                    out.push(p);
                }
            }
        }
    };

    let mut done = false;
    while !done {
        let chunk = resp.chunk().await?;
        done = chunk.is_none();
        let bytes = chunk.as_deref().unwrap_or(&[]);
        buf.reserve(decoder.max_utf8_buffer_length(bytes.len()).unwrap_or(bytes.len() * 3));
        let (_, read, _) = decoder.decode_to_string(bytes, &mut buf, done);
        debug_assert_eq!(read, bytes.len());

        let cut = if done { buf.len() } else { buf.rfind('\n').map_or(0, |i| i + 1) };
        for line in buf[..cut].split('\n').filter(|l| !l.is_empty()) {
            handle(line, &mut cols);
        }
        buf.drain(..cut);
    }

    let Some(c) = cols else { bail!("{} was empty", file.name) };
    for need in ["Province/Territory", "First Posting Date", "NOC21 Code", "Salary Per"] {
        if !c.0.contains_key(need) {
            bail!("{} is missing column {need:?}", file.name);
        }
    }
    Ok(out)
}

/// Fetches StatCan series in batches (the API takes a list of table/coordinate pairs).
async fn statcan(coords: &[(u32, String)]) -> Result<Vec<Value>> {
    let mut out = Vec::new();
    for chunk in coords.chunks(100) {
        let body: Vec<Value> = chunk.iter().map(|(pid, c)| json!({ "productId": pid, "coordinate": c, "latestN": 1 })).collect();
        let data: Vec<Value> = client()
            .post("https://www150.statcan.gc.ca/t1/wds/rest/getDataFromCubePidCoordAndLatestNPeriods")
            .json(&body)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        out.extend(data);
    }
    Ok(out)
}

fn quarter(ref_per: &str) -> String {
    let year = ref_per.get(..4).unwrap_or("?");
    let month = ref_per.get(5..7).and_then(|m| m.parse::<u32>().ok()).unwrap_or(1);
    format!("Q{} {year}", (month - 1) / 3 + 1)
}

/// StatCan JVWS (table 14-10-0444) for Canada and each province/territory by broad category,
/// plus the latest population estimates (table 17-10-0009).
pub async fn benchmark() -> Result<Benchmark> {
    const JVWS: u32 = 14100444;
    const POP: u32 = 17100009;
    const VACANCIES: u32 = 1;
    const WAGE: u32 = 5;
    let geos: Vec<(&str, u32, u32)> = std::iter::once(("CA", 1, 1)).chain(REGIONS.iter().map(|r| (r.code, r.jvws_geo, r.pop_geo))).collect();

    // NOC member 1 is the total; members 2..=11 are broad categories 0..=9.
    let mut coords: Vec<(u32, String)> = Vec::new();
    for (_, g, _) in &geos {
        for m in 1..=11 {
            for s in [VACANCIES, WAGE] {
                coords.push((JVWS, format!("{g}.{m}.{s}.0.0.0.0.0.0.0")));
            }
        }
    }
    for (_, _, pg) in &geos {
        coords.push((POP, format!("{pg}.0.0.0.0.0.0.0.0.0")));
    }
    let data = statcan(&coords).await?;

    let mut by_region: HashMap<String, HashMap<String, BenchEntry>> = HashMap::new();
    let mut population: HashMap<String, f64> = HashMap::new();
    let (mut period, mut pop_period) = (String::new(), String::new());
    for item in &data {
        if item["status"] != "SUCCESS" {
            continue;
        }
        let obj = &item["object"];
        let pid = obj["productId"].as_u64().unwrap_or(0) as u32;
        let coord: Vec<u32> = obj["coordinate"].as_str().unwrap_or("").split('.').filter_map(|p| p.parse().ok()).collect();
        let Some(point) = obj["vectorDataPoint"].get(0) else { continue };
        let ref_per = point["refPer"].as_str().unwrap_or("").to_string();
        let value = point["value"].as_f64();
        if pid == POP {
            let Some(&(code, _, _)) = geos.iter().find(|(_, _, pg)| Some(pg) == coord.first()) else { continue };
            if let Some(v) = value {
                population.insert(code.to_string(), v);
            }
            pop_period = ref_per;
            continue;
        }
        let (Some(&g), Some(&m), Some(&s)) = (coord.first(), coord.get(1), coord.get(2)) else { continue };
        let Some(&(code, _, _)) = geos.iter().find(|(_, jg, _)| *jg == g) else { continue };
        period = ref_per;
        let label = if m == 1 { "Total" } else { BROAD[(m - 2) as usize] };
        let e = by_region.entry(code.to_string()).or_default().entry(label.to_string()).or_default();
        if s == VACANCIES {
            e.vacancies = value;
        } else {
            e.wage = value;
        }
    }
    if by_region.is_empty() {
        bail!("StatCan returned no data");
    }
    Ok(Benchmark {
        source: "Statistics Canada, Job Vacancy and Wage Survey, table 14-10-0444-01; population estimates, table 17-10-0009-01".into(),
        period: quarter(&period),
        by_region,
        population,
        population_period: quarter(&pop_period),
    })
}

/// Fetches everything and builds the file the server loads.
pub async fn fetch_all(months: usize) -> Result<RawFile> {
    let files = job_bank_files(months).await?;
    if files.is_empty() {
        bail!("no Job Bank files found");
    }
    let mut dicts = Dicts::default();
    let mut rows: Vec<RawRow> = Vec::new();
    for f in &files {
        let month = load_month(f, &mut dicts).await.with_context(|| format!("loading {}", f.name))?;
        eprintln!("  {}: {} postings", f.name, month.len());
        rows.extend(month);
    }
    rows.sort_by(|a, b| b.11.cmp(&a.11));

    let bench = benchmark().await;
    match &bench {
        Ok(b) => eprintln!(
            "  StatCan JVWS {}: {:.0} vacancies in Canada; population for {} regions ({})",
            b.period,
            b.by_region.get("CA").and_then(|c| c.get("Total")).and_then(|t| t.vacancies).unwrap_or(0.0),
            b.population.len(),
            b.population_period
        ),
        Err(e) => eprintln!("  StatCan benchmark unavailable: {e:#}"),
    }

    Ok(RawFile {
        as_of: rows.first().map(|r| r.11.clone()).unwrap_or_default(),
        source: "Job Bank open data (Government of Canada)".into(),
        source_url: format!("https://open.canada.ca/data/en/dataset/{DATASET}"),
        months: files.into_iter().map(|f| f.name).collect(),
        benchmark: bench.ok(),
        cols: COLS.iter().map(|s| s.to_string()).collect(),
        dicts: DICT_COLS.iter().map(|&c| (c.to_string(), dicts.0.remove(c).unwrap_or_default().values)).collect(),
        rows,
    })
}

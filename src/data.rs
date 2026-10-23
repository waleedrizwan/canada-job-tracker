//! The dataset: the on-disk JSON format and the compact in-memory form the server queries.

use std::collections::HashMap;

use anyhow::{Context, Result, bail};
use chrono::{Datelike, NaiveDate};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Default, Debug)]
pub struct BenchEntry {
    pub vacancies: Option<f64>,
    pub wage: Option<f64>,
}

/// StatCan figures: Job Vacancy and Wage Survey by region ("CA" or a province code) and broad
/// category ("Total" for all), plus population by region for per-capita rates.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Benchmark {
    pub source: String,
    pub period: String,
    #[serde(rename = "byRegion")]
    pub by_region: HashMap<String, HashMap<String, BenchEntry>>,
    #[serde(default)]
    pub population: HashMap<String, f64>,
    #[serde(default, rename = "populationPeriod")]
    pub population_period: String,
}

/// One posting as stored on disk: strings are indexes into `RawFile::dicts`.
/// Column order matches `COLS`.
pub type RawRow = (u32, u32, u32, u32, u32, u32, u32, u32, u32, Option<f32>, Option<f32>, String, u32, u32);

pub const COLS: [&str; 14] = [
    "id", "title", "occupation", "category", "level", "city", "province", "employmentType", "term", "wageMin", "wageMax", "postedAt", "vacancies", "source",
];
pub const DICT_COLS: [&str; 9] = ["title", "occupation", "category", "level", "city", "province", "employmentType", "term", "source"];

#[derive(Serialize, Deserialize)]
pub struct RawFile {
    #[serde(rename = "asOf")]
    pub as_of: String,
    pub source: String,
    #[serde(rename = "sourceUrl")]
    pub source_url: String,
    /// Job Bank monthly files included, newest first, e.g. "September 2026".
    pub months: Vec<String>,
    pub benchmark: Option<Benchmark>,
    pub cols: Vec<String>,
    pub dicts: HashMap<String, Vec<String>>,
    pub rows: Vec<RawRow>,
}

/// A posting, packed small: ~40 bytes each.
pub struct Job {
    pub id: u32,
    pub title: u32,
    pub occupation: u16,
    pub category: u8,
    pub level: u8,
    pub city: u16,
    pub province: u8,
    pub hours: u8,
    pub term: u8,
    pub source: u8,
    pub vacancies: u16,
    pub wage_min: Option<f32>,
    pub wage_max: Option<f32>,
    pub posted: NaiveDate,
}

impl Job {
    /// Midpoint of the offered hourly range.
    pub fn wage(&self) -> Option<f32> {
        self.wage_min.map(|lo| (lo + self.wage_max.unwrap_or(lo)) / 2.0)
    }
    pub fn month(&self) -> (i32, u32) {
        (self.posted.year(), self.posted.month())
    }
}

/// A string column's distinct values, plus lowercase copies for search.
pub struct Dict {
    pub values: Vec<String>,
    pub lower: Vec<String>,
}

impl Dict {
    fn new(values: Vec<String>) -> Self {
        let lower = values.iter().map(|v| v.to_lowercase()).collect();
        Dict { values, lower }
    }
    pub fn get(&self, i: impl Into<usize>) -> &str {
        &self.values[i.into()]
    }
    pub fn find(&self, value: &str) -> Option<usize> {
        self.values.iter().position(|v| v == value)
    }
}

pub struct Dataset {
    pub as_of: NaiveDate,
    pub source_url: String,
    pub months: Vec<String>,
    pub benchmark: Option<Benchmark>,
    pub titles: Dict,
    pub occupations: Dict,
    pub categories: Dict,
    pub levels: Dict,
    pub cities: Dict,
    pub provinces: Dict,
    pub hours: Dict,
    pub terms: Dict,
    pub sources: Dict,
    pub jobs: Vec<Job>,
}

impl Dataset {
    pub fn from_raw(mut raw: RawFile) -> Result<Self> {
        if raw.cols != COLS {
            bail!("unexpected column layout {:?}", raw.cols);
        }
        let mut dict = |name: &str| Dict::new(raw.dicts.remove(name).unwrap_or_default());
        let (titles, occupations, categories, levels, cities, provinces, hours, terms, sources) = (
            dict("title"), dict("occupation"), dict("category"), dict("level"),
            dict("city"), dict("province"), dict("employmentType"), dict("term"), dict("source"),
        );
        let jobs = raw
            .rows
            .into_iter()
            .map(|(id, title, occ, cat, level, city, prov, hrs, term, wmin, wmax, posted, vac, src)| {
                Ok(Job {
                    id,
                    title,
                    occupation: occ as u16,
                    category: cat as u8,
                    level: level as u8,
                    city: city as u16,
                    province: prov as u8,
                    hours: hrs as u8,
                    term: term as u8,
                    source: src as u8,
                    vacancies: vac.min(u16::MAX as u32) as u16,
                    wage_min: wmin,
                    wage_max: wmax,
                    posted: NaiveDate::parse_from_str(&posted, "%Y-%m-%d").with_context(|| format!("bad date {posted}"))?,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(Dataset {
            as_of: NaiveDate::parse_from_str(&raw.as_of, "%Y-%m-%d")?,
            source_url: raw.source_url,
            months: raw.months,
            benchmark: raw.benchmark,
            titles, occupations, categories, levels, cities, provinces, hours, terms, sources,
            jobs,
        })
    }

    pub fn load(path: &str) -> Result<Self> {
        let bytes = std::fs::read(path).with_context(|| format!("reading {path}"))?;
        Self::from_json(&bytes).with_context(|| format!("parsing {path}"))
    }

    pub fn from_json(bytes: &[u8]) -> Result<Self> {
        Self::from_raw(serde_json::from_slice(bytes)?)
    }
}

/// A tiny dataset for tests: five postings across two provinces.
#[cfg(test)]
pub(crate) fn sample() -> Dataset {
    let dict = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    let dicts = HashMap::from([
        ("title".to_string(), dict(&["Cook", "registered nurse", "Electrician", "Line Cook, Nights"])),
        ("occupation".to_string(), dict(&["Cooks", "Registered nurses", "Electricians"])),
        ("category".to_string(), dict(&["Sales & service", "Health", "Trades & transport"])),
        ("level".to_string(), dict(&["TEER 3 · College / apprenticeship (<2 yrs)", "TEER 1 · University degree", "TEER 2 · College / apprenticeship (2+ yrs)"])),
        ("city".to_string(), dict(&["Toronto, ON", "Vancouver, BC", "Not given"])),
        ("province".to_string(), dict(&["ON", "BC"])),
        ("employmentType".to_string(), dict(&["Full time", "Part time"])),
        ("term".to_string(), dict(&["Permanent", "Unspecified"])),
        ("source".to_string(), dict(&["Posted on Job Bank", "Partner job boards"])),
    ]);
    let row = |id, title, occ, prov_city: (u32, u32), hours, term, wage: (Option<f32>, Option<f32>), posted: &str, vac, src| -> RawRow {
        (id, title, occ, occ, occ, prov_city.1, prov_city.0, hours, term, wage.0, wage.1, posted.to_string(), vac, src)
    };
    let rows = vec![
        row(1, 0, 0, (0, 0), 0, 0, (Some(17.0), Some(19.0)), "2026-09-30", 2, 0),
        row(2, 1, 1, (1, 1), 1, 1, (Some(40.0), Some(50.0)), "2026-09-15", 1, 1),
        row(3, 2, 2, (0, 0), 0, 0, (Some(35.0), None), "2026-08-20", 1, 0),
        row(4, 3, 0, (1, 2), 0, 1, (None, None), "2026-08-03", 3, 1),
        row(5, 1, 1, (0, 0), 0, 0, (Some(45.5), Some(45.5)), "2026-07-10", 1, 0),
    ];
    let raw = RawFile {
        as_of: "2026-09-30".into(),
        source: "test".into(),
        source_url: "https://example.org/data".into(),
        months: vec!["September 2026".into(), "August 2026".into(), "July 2026".into()],
        benchmark: None,
        cols: COLS.iter().map(|s| s.to_string()).collect(),
        dicts,
        rows,
    };
    Dataset::from_raw(raw).expect("sample dataset")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wage_is_midpoint_of_range() {
        let ds = sample();
        assert_eq!(ds.jobs[0].wage(), Some(18.0));
        assert_eq!(ds.jobs[2].wage(), Some(35.0), "a missing maximum means a single figure");
        assert_eq!(ds.jobs[3].wage(), None);
    }

    #[test]
    fn month_comes_from_posting_date() {
        assert_eq!(sample().jobs[1].month(), (2026, 9));
    }

    #[test]
    fn dict_lookups() {
        let ds = sample();
        assert_eq!(ds.provinces.find("BC"), Some(1));
        assert_eq!(ds.provinces.find("QC"), None);
        assert_eq!(ds.titles.lower[1], "registered nurse");
        assert_eq!(ds.titles.get(2usize), "Electrician");
    }

    #[test]
    fn json_round_trip() {
        let raw = RawFile {
            as_of: "2026-01-31".into(),
            source: "s".into(),
            source_url: "u".into(),
            months: vec!["January 2026".into()],
            benchmark: None,
            cols: COLS.iter().map(|s| s.to_string()).collect(),
            dicts: HashMap::from([("title".to_string(), vec!["A".to_string()])]),
            rows: vec![(9, 0, 0, 0, 0, 0, 0, 0, 0, Some(20.0), None, "2026-01-05".into(), 70_000, 0)],
        };
        let ds = Dataset::from_json(&serde_json::to_vec(&raw).unwrap()).unwrap();
        assert_eq!(ds.jobs.len(), 1);
        assert_eq!(ds.jobs[0].vacancies, u16::MAX, "openings are clamped to fit");
        assert_eq!(ds.as_of.to_string(), "2026-01-31");
    }

    #[test]
    fn rejects_unexpected_columns() {
        let raw = RawFile {
            as_of: "2026-01-31".into(),
            source: String::new(),
            source_url: String::new(),
            months: vec![],
            benchmark: None,
            cols: vec!["id".into()],
            dicts: HashMap::new(),
            rows: vec![],
        };
        assert!(Dataset::from_raw(raw).is_err());
    }

    #[test]
    fn rejects_bad_dates() {
        let mut raw: RawFile = serde_json::from_value(serde_json::json!({
            "asOf": "2026-01-31", "source": "", "sourceUrl": "", "months": [], "benchmark": null,
            "cols": COLS, "dicts": {}, "rows": []
        }))
        .unwrap();
        raw.rows.push((1, 0, 0, 0, 0, 0, 0, 0, 0, None, None, "31/01/2026".into(), 1, 0));
        assert!(Dataset::from_raw(raw).is_err());
    }
}

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

/// StatCan Job Vacancy and Wage Survey figures for Ontario, keyed by broad category ("Total" for all).
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Benchmark {
    pub source: String,
    pub period: String,
    #[serde(rename = "byCategory")]
    pub by_category: HashMap<String, BenchEntry>,
}

/// One posting as stored on disk: strings are indexes into `RawFile::dicts`.
/// Column order matches `COLS`.
pub type RawRow = (String, u32, u32, u32, u32, u32, u32, u32, Option<f32>, Option<f32>, String, u32, u32);

pub const COLS: [&str; 13] = [
    "id", "title", "occupation", "category", "level", "city", "employmentType", "term", "wageMin", "wageMax", "postedAt", "vacancies", "source",
];
pub const DICT_COLS: [&str; 8] = ["title", "occupation", "category", "level", "city", "employmentType", "term", "source"];

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
        let (titles, occupations, categories, levels, cities, hours, terms, sources) = (
            dict("title"), dict("occupation"), dict("category"), dict("level"),
            dict("city"), dict("employmentType"), dict("term"), dict("source"),
        );
        let jobs = raw
            .rows
            .into_iter()
            .map(|(id, title, occ, cat, level, city, hrs, term, wmin, wmax, posted, vac, src)| {
                Ok(Job {
                    id: id.parse().unwrap_or(0),
                    title,
                    occupation: occ as u16,
                    category: cat as u8,
                    level: level as u8,
                    city: city as u16,
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
            titles, occupations, categories, levels, cities, hours, terms, sources,
            jobs,
        })
    }

    pub fn load(path: &str) -> Result<Self> {
        let bytes = std::fs::read(path).with_context(|| format!("reading {path}"))?;
        let raw: RawFile = serde_json::from_slice(&bytes).with_context(|| format!("parsing {path}"))?;
        Self::from_raw(raw)
    }
}

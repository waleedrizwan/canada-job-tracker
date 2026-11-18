//! Builds the static site hosted on Cloudflare into dist/: the page in static/index.html, which
//! draws a treemap of occupations in the browser, and data.json, the occupations it draws,
//! aggregated from the dataset over the last `RECENT_DAYS` days of postings.

use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;
use serde::Serialize;

use crate::data::Dataset;
use crate::query::{RECENT_DAYS, recent_since};

const PAGE: &str = include_str!("../static/index.html");

/// Everything the page shows that isn't a tile.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Meta {
    pub as_of: String,
    pub since: String,
    pub days: i64,
    pub postings: usize,
    pub occupations: usize,
}

/// One occupation: a tile on the treemap, sized by its postings.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Tile {
    pub title: String,
    pub category: String,
    pub jobs: usize,
    /// Median of the hourly wage midpoints, where a wage is listed.
    pub wage: Option<f64>,
    /// Percent above (+) or below (−) the StatCan average wage for the category.
    pub vs_statcan: Option<f64>,
    /// Share of postings listed as full time, 0 to 1.
    pub full_time: f64,
    /// Average training level on a 0 (no formal education) to 5 (management) scale.
    pub training: Option<f64>,
    /// A Job Bank search for the occupation.
    pub url: String,
}

#[derive(Serialize)]
pub struct Data {
    pub meta: Meta,
    pub tiles: Vec<Tile>,
}

#[derive(Default)]
struct Group {
    jobs: usize,
    wages: Vec<f32>,
    full_time: usize,
    training_sum: f64,
    training_n: usize,
    categories: HashMap<u8, usize>,
}

/// Aggregates the postings from the last `RECENT_DAYS` days, one tile per occupation.
pub fn data(ds: &Dataset) -> Data {
    let since = recent_since(ds);
    let mut groups: HashMap<u16, Group> = HashMap::new();
    let mut postings = 0;
    for j in ds.jobs.iter().filter(|j| j.posted >= since) {
        postings += 1;
        let g = groups.entry(j.occupation).or_default();
        g.jobs += 1;
        *g.categories.entry(j.category).or_default() += 1;
        if let Some(w) = j.wage() {
            g.wages.push(w);
        }
        if ds.hours.get(j.hours as usize) == "Full time" {
            g.full_time += 1;
        }
        if let Some(teer) = teer_level(ds.levels.get(j.level as usize)) {
            // TEER 0 is management and TEER 5 is no formal education; the scale runs the other way.
            g.training_sum += (5 - teer) as f64;
            g.training_n += 1;
        }
    }

    let mut tiles: Vec<Tile> = groups
        .into_iter()
        .map(|(occupation, mut g)| {
            let title = ds.occupations.get(occupation as usize).to_string();
            // The category most of the occupation's postings sit in (ties go to the lower index).
            let category_index = g.categories.iter().max_by_key(|(c, n)| (**n, std::cmp::Reverse(**c))).map(|(c, _)| *c).unwrap_or(0);
            let category = ds.categories.get(category_index as usize).to_string();
            let wage = median(&mut g.wages);
            let vs_statcan = match (wage, benchmark_wage(ds, &category)) {
                (Some(w), Some(b)) if b > 0.0 => Some((w / b - 1.0) * 100.0),
                _ => None,
            };
            Tile {
                url: search_url(&title),
                title,
                category,
                jobs: g.jobs,
                wage,
                vs_statcan,
                full_time: g.full_time as f64 / g.jobs as f64,
                training: (g.training_n > 0).then(|| g.training_sum / g.training_n as f64),
            }
        })
        .collect();
    tiles.sort_by(|a, b| b.jobs.cmp(&a.jobs).then_with(|| a.title.cmp(&b.title)));

    Data {
        meta: Meta {
            as_of: ds.as_of.to_string(),
            since: since.to_string(),
            days: RECENT_DAYS,
            postings,
            occupations: tiles.len(),
        },
        tiles,
    }
}

/// The hourly wage StatCan reports for a broad category, Canada-wide.
fn benchmark_wage(ds: &Dataset, category: &str) -> Option<f64> {
    ds.benchmark.as_ref()?.by_region.get("CA")?.get(category)?.wage
}

/// The number in a level such as "TEER 3 · College / apprenticeship (<2 yrs)", or None.
fn teer_level(level: &str) -> Option<u32> {
    level.strip_prefix("TEER ")?.chars().next()?.to_digit(10).filter(|n| *n <= 5)
}

fn search_url(occupation: &str) -> String {
    format!(
        "https://www.jobbank.gc.ca/jobsearch/jobsearch?{}",
        serde_urlencoded::to_string([("searchstring", occupation)]).unwrap_or_default()
    )
}

fn median(v: &mut [f32]) -> Option<f64> {
    if v.is_empty() {
        return None;
    }
    v.sort_by(|a, b| a.total_cmp(b));
    let mid = v.len() / 2;
    Some(if v.len() % 2 == 1 { v[mid] as f64 } else { (v[mid - 1] as f64 + v[mid] as f64) / 2.0 })
}

/// Writes the page and its data into `out`, replacing the previous build's assets.
pub fn build(ds: &Dataset, out: &Path) -> Result<()> {
    std::fs::create_dir_all(out)?;
    // Files from the Bootstrap build that this page doesn't use.
    for old in ["assets", "styles.css", "_headers"] {
        let path = out.join(old);
        if path.is_dir() {
            std::fs::remove_dir_all(&path)?;
        } else if path.exists() {
            std::fs::remove_file(&path)?;
        }
    }
    std::fs::write(out.join("index.html"), PAGE)?;
    std::fs::write(out.join("data.json"), serde_json::to_string(&data(ds))?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bundled() -> Dataset {
        Dataset::from_json(include_bytes!("../data/jobs.json")).unwrap()
    }

    #[test]
    fn every_recent_posting_is_in_exactly_one_tile() {
        let ds = bundled();
        let data = data(&ds);
        let in_tiles: usize = data.tiles.iter().map(|t| t.jobs).sum();
        assert_eq!(in_tiles, data.meta.postings);
        assert!(data.meta.postings > 0, "the bundled data has recent postings");
        assert!(data.tiles.windows(2).all(|w| w[0].jobs >= w[1].jobs), "largest tiles first");
    }

    #[test]
    fn tile_values_stay_in_their_ranges() {
        let data = data(&bundled());
        for t in &data.tiles {
            assert!((0.0..=1.0).contains(&t.full_time), "{}", t.title);
            if let Some(v) = t.training {
                assert!((0.0..=5.0).contains(&v), "{}", t.title);
            }
            if let Some(w) = t.wage {
                assert!(w > 0.0, "{}", t.title);
            }
            assert!(t.url.starts_with("https://www.jobbank.gc.ca/jobsearch/jobsearch?searchstring="), "{}", t.url);
        }
    }

    #[test]
    fn reads_teer_levels() {
        assert_eq!(teer_level("TEER 0 · Management"), Some(0));
        assert_eq!(teer_level("TEER 5 · No formal education"), Some(5));
        assert_eq!(teer_level("Unclassified"), None);
    }

    #[test]
    fn medians_of_odd_even_and_empty() {
        assert_eq!(median(&mut [3.0, 1.0, 2.0]), Some(2.0));
        assert_eq!(median(&mut [4.0, 1.0, 2.0, 3.0]), Some(2.5));
        assert_eq!(median(&mut []), None);
    }

    #[test]
    fn writes_the_page_and_data() {
        let dir = std::env::temp_dir().join(format!("site-test-{}", std::process::id()));
        build(&bundled(), &dir).unwrap();
        let html = std::fs::read_to_string(dir.join("index.html")).unwrap();
        assert!(html.contains("<title>Canada Job Tracker</title>"));
        let json: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(dir.join("data.json")).unwrap()).unwrap();
        assert!(json["tiles"].as_array().unwrap().len() > 0);
        assert!(json["tiles"][0].get("vsStatcan").is_some(), "camelCase keys, as the page reads them");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}

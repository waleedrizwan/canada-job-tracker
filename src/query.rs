//! URL query parameters and the filtering they drive.

use serde::{Deserialize, Serialize};

use crate::data::{Dataset, Dict, Job};

/// Every piece of page state lives in the query string, so any view can be bookmarked or shared.
#[derive(Deserialize, Serialize, Clone, Default)]
pub struct Params {
    /// Which tab: the job cards (default), "heatmap" or "charts".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub view: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub q: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cat: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub occ: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prov: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub city: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub level: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hours: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub term: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wage: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub month: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub src: Option<String>,
    /// What the map is shaded by: postings per capita (default) or "wage".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub map: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sort: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dir: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page: Option<String>,
}

impl Params {
    /// Treats blank form fields as unset.
    pub fn normalize(mut self) -> Self {
        for f in [
            &mut self.view, &mut self.q, &mut self.cat, &mut self.occ, &mut self.prov, &mut self.city, &mut self.map, &mut self.level, &mut self.hours, &mut self.term,
            &mut self.wage, &mut self.month, &mut self.src, &mut self.sort, &mut self.dir, &mut self.page,
        ] {
            if f.as_deref().is_some_and(|v| v.trim().is_empty()) {
                *f = None;
            }
        }
        if !matches!(self.view.as_deref(), Some("heatmap" | "charts")) {
            self.view = None;
        }
        if self.wage.as_deref().and_then(|w| w.parse::<f32>().ok()).is_none_or(|w| w <= 0.0) {
            self.wage = None;
        }
        self
    }

    /// A link to this view with some changes applied. Always returns to page 1.
    pub fn link(&self, change: impl FnOnce(&mut Params)) -> String {
        let mut p = self.clone();
        p.page = None;
        change(&mut p);
        match serde_urlencoded::to_string(&p) {
            Ok(qs) if !qs.is_empty() => format!("/?{qs}"),
            _ => "/".into(),
        }
    }

    pub fn page(&self) -> usize {
        self.page.as_deref().and_then(|p| p.parse().ok()).filter(|&p| p >= 1).unwrap_or(1)
    }
    pub fn sort_key(&self) -> &str {
        self.sort.as_deref().unwrap_or("posted")
    }
    pub fn descending(&self) -> bool {
        match self.dir.as_deref() {
            Some("asc") => false,
            Some("desc") => true,
            _ => self.sort.is_none(),
        }
    }
}

/// Toggles a filter value: set it, or clear it if it's already set to that value.
pub fn toggle(slot: &mut Option<String>, value: &str) {
    *slot = if slot.as_deref() == Some(value) { None } else { Some(value.to_string()) };
}

enum Sel {
    Any,
    Is(usize),
    Never,
}

fn sel(dict: &Dict, v: &Option<String>) -> Sel {
    match v {
        None => Sel::Any,
        Some(v) => dict.find(v).map_or(Sel::Never, Sel::Is),
    }
}

impl Sel {
    fn ok(&self, id: usize) -> bool {
        match self {
            Sel::Any => true,
            Sel::Is(i) => *i == id,
            Sel::Never => false,
        }
    }
}

pub fn parse_month(m: &str) -> Option<(i32, u32)> {
    let (y, mo) = m.split_once('-')?;
    Some((y.parse().ok()?, mo.parse().ok()?))
}

/// Postings matching every filter.
pub fn filter<'a>(ds: &'a Dataset, p: &Params) -> Vec<&'a Job> {
    // Match the search against each distinct title/occupation once, not once per posting.
    let q = p.q.as_deref().map(str::to_lowercase);
    let hit = |d: &Dict| -> Option<Vec<bool>> { q.as_ref().map(|q| d.lower.iter().map(|v| v.contains(q.as_str())).collect()) };
    let (title_hit, occ_hit) = (hit(&ds.titles), hit(&ds.occupations));

    let (cat, occ, prov, city, level, hours, term, src) = (
        sel(&ds.categories, &p.cat), sel(&ds.occupations, &p.occ), sel(&ds.provinces, &p.prov), sel(&ds.cities, &p.city), sel(&ds.levels, &p.level),
        sel(&ds.hours, &p.hours), sel(&ds.terms, &p.term), sel(&ds.sources, &p.src),
    );
    let min_wage: Option<f32> = p.wage.as_deref().and_then(|w| w.parse().ok());
    let month = p.month.as_deref().and_then(parse_month);

    ds.jobs
        .iter()
        .filter(|j| {
            if let (Some(t), Some(o)) = (&title_hit, &occ_hit) {
                if !t[j.title as usize] && !o[j.occupation as usize] {
                    return false;
                }
            }
            cat.ok(j.category as usize)
                && occ.ok(j.occupation as usize)
                && prov.ok(j.province as usize)
                && city.ok(j.city as usize)
                && level.ok(j.level as usize)
                && hours.ok(j.hours as usize)
                && term.ok(j.term as usize)
                && src.ok(j.source as usize)
                && min_wage.is_none_or(|m| j.wage().is_some_and(|w| w >= m))
                && month.is_none_or(|m| j.month() == m)
        })
        .collect()
}

/// Sorts for the table. Missing wages always sort last.
pub fn sort(ds: &Dataset, rows: &mut [&Job], key: &str, desc: bool) {
    use std::cmp::Ordering;
    // Case-insensitive, so "technicien agricole" sorts with "Technician", not after "Z".
    let by_str = |d: &Dict, a: usize, b: usize| d.lower[a].cmp(&d.lower[b]).then_with(|| d.get(a).cmp(d.get(b)));
    rows.sort_by(|a, b| {
        let o = match key {
            "title" => by_str(&ds.titles, a.title as usize, b.title as usize),
            "occupation" => by_str(&ds.occupations, a.occupation as usize, b.occupation as usize),
            "city" => by_str(&ds.cities, a.city as usize, b.city as usize),
            "hours" => by_str(&ds.hours, a.hours as usize, b.hours as usize),
            "term" => by_str(&ds.terms, a.term as usize, b.term as usize),
            "vacancies" => a.vacancies.cmp(&b.vacancies),
            "wage" => match (a.wage(), b.wage()) {
                (Some(x), Some(y)) => x.partial_cmp(&y).unwrap_or(Ordering::Equal),
                (Some(_), None) => return Ordering::Less,
                (None, Some(_)) => return Ordering::Greater,
                (None, None) => Ordering::Equal,
            },
            _ => a.posted.cmp(&b.posted),
        };
        if desc { o.reverse() } else { o }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::sample;

    fn params(qs: &str) -> Params {
        serde_urlencoded::from_str::<Params>(qs).unwrap().normalize()
    }
    fn ids(rows: &[&Job]) -> Vec<u32> {
        rows.iter().map(|j| j.id).collect()
    }

    #[test]
    fn normalize_drops_blank_and_bad_values() {
        let p = params("cat=&prov=%20&wage=abc&q=cook");
        assert!(p.cat.is_none() && p.prov.is_none() && p.wage.is_none());
        assert_eq!(p.q.as_deref(), Some("cook"));
        assert!(params("wage=0").wage.is_none());
        assert!(params("wage=-5").wage.is_none());
        assert_eq!(params("wage=17.5").wage.as_deref(), Some("17.5"));
        assert_eq!(params("view=heatmap").view.as_deref(), Some("heatmap"));
        assert!(params("view=nope").view.is_none(), "an unknown tab shows the jobs");
    }

    #[test]
    fn link_resets_page_and_keeps_other_filters() {
        let p = params("prov=ON&page=4");
        assert_eq!(p.link(|q| q.cat = Some("Health".into())), "/?cat=Health&prov=ON");
        assert_eq!(Params::default().link(|_| {}), "/");
    }

    #[test]
    fn page_and_sort_defaults() {
        assert_eq!(params("page=abc").page(), 1);
        assert_eq!(params("page=0").page(), 1);
        assert_eq!(params("page=3").page(), 3);
        let p = params("");
        assert_eq!(p.sort_key(), "posted");
        assert!(p.descending(), "newest first by default");
        assert!(!params("sort=title").descending(), "a chosen column sorts ascending unless told otherwise");
        assert!(params("sort=wage&dir=desc").descending());
    }

    #[test]
    fn toggle_sets_and_clears() {
        let mut slot = None;
        toggle(&mut slot, "ON");
        assert_eq!(slot.as_deref(), Some("ON"));
        toggle(&mut slot, "BC");
        assert_eq!(slot.as_deref(), Some("BC"));
        toggle(&mut slot, "BC");
        assert_eq!(slot, None);
    }

    #[test]
    fn parses_months() {
        assert_eq!(parse_month("2026-09"), Some((2026, 9)));
        assert_eq!(parse_month("garbage"), None);
        assert_eq!(parse_month("2026-xx"), None);
    }

    #[test]
    fn filters_by_each_field() {
        let ds = sample();
        assert_eq!(filter(&ds, &params("")).len(), 5);
        assert_eq!(ids(&filter(&ds, &params("prov=BC"))), [2, 4]);
        assert_eq!(ids(&filter(&ds, &params("cat=Health"))), [2, 5]);
        assert_eq!(ids(&filter(&ds, &params("city=Toronto%2C+ON"))), [1, 3, 5]);
        assert_eq!(ids(&filter(&ds, &params("hours=Part+time"))), [2]);
        assert_eq!(ids(&filter(&ds, &params("term=Unspecified"))), [2, 4]);
        assert_eq!(ids(&filter(&ds, &params("src=Partner+job+boards"))), [2, 4]);
        assert_eq!(ids(&filter(&ds, &params("month=2026-08"))), [3, 4]);
        assert_eq!(ids(&filter(&ds, &params("prov=ON&cat=Health"))), [5]);
    }

    #[test]
    fn minimum_wage_uses_midpoint_and_skips_unpaid() {
        let ds = sample();
        // Midpoints: 18, 45, 35, none, 45.5
        assert_eq!(ids(&filter(&ds, &params("wage=35"))), [2, 3, 5]);
        assert_eq!(ids(&filter(&ds, &params("wage=45.25"))), [5]);
    }

    #[test]
    fn search_matches_title_or_occupation_case_insensitively() {
        let ds = sample();
        assert_eq!(ids(&filter(&ds, &params("q=NURSE"))), [2, 5]);
        assert_eq!(ids(&filter(&ds, &params("q=cook"))), [1, 4], "title \"Cook\" and occupation \"Cooks\"");
        assert!(filter(&ds, &params("q=astronaut")).is_empty());
    }

    #[test]
    fn unknown_values_match_nothing() {
        let ds = sample();
        assert!(filter(&ds, &params("prov=XX")).is_empty());
        assert!(filter(&ds, &params("cat=Nope")).is_empty());
    }

    #[test]
    fn sorts_and_keeps_missing_wages_last() {
        let ds = sample();
        let mut rows = filter(&ds, &params(""));
        sort(&ds, &mut rows, "wage", false);
        assert_eq!(ids(&rows), [1, 3, 2, 5, 4]);
        sort(&ds, &mut rows, "wage", true);
        assert_eq!(ids(&rows), [5, 2, 3, 1, 4], "missing wages stay last when descending");
        sort(&ds, &mut rows, "posted", true);
        assert_eq!(ids(&rows), [1, 2, 3, 4, 5]);
        sort(&ds, &mut rows, "vacancies", true);
        assert_eq!(ids(&rows)[0], 4);
    }

    #[test]
    fn text_sort_ignores_case() {
        let ds = sample();
        let mut rows = filter(&ds, &params(""));
        sort(&ds, &mut rows, "title", false);
        let titles: Vec<&str> = rows.iter().map(|j| ds.titles.get(j.title as usize)).collect();
        assert_eq!(titles, ["Cook", "Electrician", "Line Cook, Nights", "registered nurse", "registered nurse"]);
    }
}

//! URL query parameters and the filtering they drive.

use serde::{Deserialize, Serialize};

use crate::data::{Dataset, Dict, Job};

/// Every piece of page state lives in the query string, so any view can be bookmarked or shared.
#[derive(Deserialize, Serialize, Clone, Default)]
pub struct Params {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub q: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cat: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub occ: Option<String>,
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
            &mut self.q, &mut self.cat, &mut self.occ, &mut self.city, &mut self.level, &mut self.hours, &mut self.term,
            &mut self.wage, &mut self.month, &mut self.src, &mut self.sort, &mut self.dir, &mut self.page,
        ] {
            if f.as_deref().is_some_and(|v| v.trim().is_empty()) {
                *f = None;
            }
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

    let (cat, occ, city, level, hours, term, src) = (
        sel(&ds.categories, &p.cat), sel(&ds.occupations, &p.occ), sel(&ds.cities, &p.city), sel(&ds.levels, &p.level),
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
    let by_str = |d: &Dict, a: usize, b: usize| d.get(a).cmp(d.get(b));
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

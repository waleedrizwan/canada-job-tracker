//! Server-rendered charts: horizontal bars as HTML, histograms as SVG. Hover text uses
//! native tooltips (title) and clickable bars are plain links, so no JavaScript is needed.

use maud::{Markup, html};

pub struct Bar {
    pub label: String,
    pub value: f64,
    pub display: String,
    pub tip: String,
    pub href: Option<String>,
    pub active: bool,
}

pub struct Bin {
    pub label: String,
    pub value: f64,
    pub tip: String,
}

pub struct Pair {
    pub label: String,
    pub a: f64,
    pub b: f64,
    pub tip: String,
}

fn empty() -> Markup {
    html! { div.empty { "No postings match these filters" } }
}

fn vbar_path(x: f64, y: f64, w: f64, h: f64, r: f64) -> String {
    let r = r.min(h).min(w / 2.0);
    format!("M{x:.1},{:.1}v-{:.1}a{r},{r} 0 0 1 {r},-{r}h{:.1}a{r},{r} 0 0 1 {r},{r}v{:.1}z", y + h, h - r, w - 2.0 * r, h - r)
}

fn nice_max(v: f64) -> f64 {
    if v <= 0.0 {
        return 1.0;
    }
    let p = 10f64.powf(v.log10().floor());
    [1.0, 2.0, 2.5, 5.0, 10.0].iter().map(|m| m * p).find(|m| *m >= v).unwrap_or(v)
}

pub fn compact(v: f64) -> String {
    let trim = |s: String| s.trim_end_matches(".0").to_string();
    if v >= 1e6 {
        format!("{}M", trim(format!("{:.1}", v / 1e6)))
    } else if v >= 1e3 {
        format!("{}k", trim(format!("{:.1}", v / 1e3)))
    } else {
        format!("{}", v.round())
    }
}

/// Horizontal bars, one row per category. Plain HTML so labels stay readable at any width.
pub fn hbar(rows: &[Bar], aria: &str) -> Markup {
    if rows.is_empty() {
        return empty();
    }
    let max = rows.iter().map(|r| r.value).fold(0.0, f64::max);
    let any_active = rows.iter().any(|r| r.active);
    let row = |r: &Bar| {
        let pct = if max > 0.0 { (r.value / max * 100.0).max(0.6) } else { 0.6 };
        html! {
            span.label { (r.label) }
            span.track {
                span.fill style=(format!("width:{pct:.2}%")) {}
                span.value { (r.display) }
            }
        }
    };
    html! {
        ol.bars aria-label=(aria) {
            @for r in rows {
                li class=(if r.active { "active" } else if any_active { "dim" } else { "" }) {
                    @if let Some(href) = &r.href {
                        a.bar-row href=(href) title=(r.tip) { (row(r)) }
                    } @else {
                        div.bar-row title=(r.tip) { (row(r)) }
                    }
                }
            }
        }
    }
}

/// Vertical bars in order (histograms, time series).
pub fn vbar(bins: &[Bin], label_every: usize, aria: &str) -> Markup {
    if bins.iter().all(|b| b.value == 0.0) {
        return empty();
    }
    let (w, h) = (520.0, 230.0);
    let (mt, mr, mb, ml) = (14.0, 8.0, 30.0, 44.0);
    let (pw, ph) = (w - ml - mr, h - mt - mb);
    let max = nice_max(bins.iter().map(|b| b.value).fold(0.0, f64::max));
    let step = pw / bins.len() as f64;
    let gap = 2.0;
    let bw = (step - gap).max(1.0);
    let every = label_every.max(1);

    html! {
        svg viewBox=(format!("0 0 {w} {h}")) role="img" aria-label=(aria) {
            g.axis {
                @for i in 0..=4 {
                    @let v = max / 4.0 * i as f64;
                    @let y = mt + ph - v / max * ph;
                    line class=(if i == 0 { "baseline" } else { "gridline" }) x1=(ml) x2=(w - mr) y1=(y) y2=(y) {}
                    text x=(ml - 6.0) y=(y + 4.0) text-anchor="end" { (compact(v)) }
                }
                @for (i, b) in bins.iter().enumerate() {
                    @if i % every == 0 {
                        text x=(ml + i as f64 * step + step / 2.0) y=(h - 10.0) text-anchor="middle" { (b.label) }
                    }
                }
            }
            @for (i, b) in bins.iter().enumerate() {
                @let bh = (b.value / max * ph).max(if b.value > 0.0 { 2.0 } else { 0.0 });
                g.row {
                    title { (b.tip) }
                    rect.hit x=(ml + i as f64 * step) y=(mt) width=(step) height=(ph) {}
                    @if bh > 0.0 {
                        path.bar d=(vbar_path(ml + i as f64 * step + gap / 2.0, mt + ph - bh, bw, bh, 4.0)) {}
                    }
                }
            }
        }
    }
}

/// Two series per row on one scale, with a legend. Plain HTML, like `hbar`.
pub fn hbar_pair(rows: &[Pair], names: [&str; 2], fmt: fn(f64) -> String, aria: &str) -> Markup {
    if rows.is_empty() {
        return empty();
    }
    let max = rows.iter().flat_map(|r| [r.a, r.b]).fold(0.0, f64::max);
    let w = |v: f64| if max > 0.0 { (v / max * 100.0).max(0.6) } else { 0.6 };
    html! {
        div.legend {
            @for (k, name) in names.iter().enumerate() {
                span { span class=(format!("key s{}", k + 1)) {} (name) }
            }
        }
        ol.bars.pairs aria-label=(aria) {
            @for r in rows {
                li { div.bar-row title=(r.tip) {
                    span.label { (r.label) }
                    span.stack {
                        @for (k, v) in [r.a, r.b].into_iter().enumerate() {
                            span.track {
                                span class=(format!("fill s{}", k + 1)) style=(format!("width:{:.2}%", w(v))) {}
                                span.value { (fmt(v)) span.sr { " " (names[k]) } }
                            }
                        }
                    }
                } }
            }
        }
    }
}

/// Number of colour steps on the map and heat grid.
pub const STEPS: usize = 5;

/// Quantile bins: each colour step holds about the same number of values, so one
/// outlier can't wash everything else into the lightest step.
pub struct Bins {
    edges: Vec<f64>,
}

impl Bins {
    pub fn new(values: impl Iterator<Item = f64>) -> Self {
        let mut v: Vec<f64> = values.filter(|x| x.is_finite()).collect();
        v.sort_by(f64::total_cmp);
        if v.is_empty() {
            return Bins { edges: Vec::new() };
        }
        let n = v.len();
        let mut edges: Vec<f64> = (0..=STEPS).map(|k| v[(k * n / STEPS).min(n - 1)]).collect();
        edges[STEPS] = v[n - 1];
        Bins { edges }
    }

    pub fn step(&self, v: f64) -> usize {
        if self.edges.is_empty() {
            return STEPS / 2;
        }
        (1..STEPS).filter(|&k| v >= self.edges[k]).count()
    }

    /// One label per step, e.g. "$17–$24".
    pub fn labels(&self, fmt: impl Fn(f64) -> String) -> Vec<String> {
        if self.edges.is_empty() {
            return Vec::new();
        }
        (0..STEPS).map(|k| format!("{}–{}", fmt(self.edges[k]), fmt(self.edges[k + 1]))).collect()
    }
}

pub struct Region {
    pub code: &'static str,
    pub step: Option<usize>,
    pub tip: String,
    pub href: String,
    pub active: bool,
}

/// Map label anchors, placed by eye. The three Maritime provinces are too small to label.
const LABELS: [(&str, f64, f64); 10] = [
    ("BC", 76.0, 470.0), ("AB", 152.0, 520.0), ("SK", 271.0, 515.0), ("MB", 332.0, 520.0), ("ON", 470.0, 625.0),
    ("QC", 590.0, 545.0), ("NL", 700.0, 478.0), ("NU", 382.0, 300.0), ("NT", 192.0, 335.0), ("YT", 122.0, 300.0),
];

/// Choropleth of Canada. Each province is a link; hover shows its numbers.
pub fn canada_map(regions: &[Region], aria: &str) -> Markup {
    use crate::canada_map::{PROVINCES, VIEW_H, VIEW_W};
    let find = |code: &str| regions.iter().find(|r| r.code == code);
    html! {
        svg.map viewBox=(format!("0 0 {VIEW_W} {VIEW_H}")) role="img" aria-label=(aria) {
            @for (code, d, _, _) in PROVINCES {
                @if let Some(r) = find(code) {
                    a href=(r.href) {
                        title { (r.tip) }
                        path class=(format!("region {}{}", r.step.map_or("nodata".to_string(), |s| format!("q{s}")), if r.active { " active" } else { "" })) d=(d) {}
                    }
                }
            }
            @for (code, x, y) in LABELS {
                @let dark = find(code).and_then(|r| r.step).is_some_and(|s| s >= 3);
                text class=(if dark { "map-label on-dark" } else { "map-label" }) x=(x) y=(y) text-anchor="middle" { (code) }
            }
        }
    }
}

pub struct HeatCell {
    pub step: Option<usize>,
    pub display: String,
    pub tip: String,
    pub href: String,
}

/// A grid of linked cells shaded by value: rows × columns.
pub fn heat_grid(cols: &[(&str, &str)], rows: &[(String, Vec<HeatCell>)], aria: &str) -> Markup {
    html! {
        div.heat-wrap {
            table.heat aria-label=(aria) {
                thead { tr {
                    th {}
                    @for (code, name) in cols { th scope="col" title=(name) { abbr title=(name) { (code) } } }
                } }
                tbody {
                    @for (label, cells) in rows {
                        tr {
                            th scope="row" { (label) }
                            @for c in cells {
                                td class=(c.step.map_or("nodata".to_string(), |s| format!("q{s}"))) {
                                    @if c.step.is_some() {
                                        a href=(c.href) title=(c.tip) { (c.display) }
                                    } @else {
                                        span title=(c.tip) { "–" }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Legend for the shared colour steps.
pub fn steps_legend(labels: &[String]) -> Markup {
    html! {
        div.steps-legend {
            @for (i, l) in labels.iter().enumerate() {
                span { span class=(format!("swatch q{i}")) {} (l) }
            }
            span { span.swatch.nodata {} "Too few postings" }
        }
    }
}

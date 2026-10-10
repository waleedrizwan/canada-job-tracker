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

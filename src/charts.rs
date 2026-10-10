//! Server-rendered SVG charts. Hover text uses SVG <title> (native tooltips) and
//! clickable bars are plain links, so no JavaScript is needed.

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

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() > n { format!("{}…", s.chars().take(n - 1).collect::<String>()) } else { s.to_string() }
}

/// Rounded on the data end only, square on the baseline.
fn hbar_path(x: f64, y: f64, w: f64, h: f64, r: f64) -> String {
    let r = r.min(w).min(h / 2.0);
    format!("M{x:.1},{y:.1}h{:.1}a{r},{r} 0 0 1 {r},{r}v{:.1}a{r},{r} 0 0 1 -{r},{r}h-{:.1}z", w - r, h - 2.0 * r, w - r)
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

/// Horizontal bars, one row per category.
pub fn hbar(rows: &[Bar], aria: &str) -> Markup {
    if rows.is_empty() {
        return empty();
    }
    let (w, label_w, value_w, row_h, bar_h) = (520.0, 160.0, 64.0, 26.0, 16.0);
    let plot_w = w - label_w - value_w;
    let h = rows.len() as f64 * row_h + 4.0;
    let max = rows.iter().map(|r| r.value).fold(0.0, f64::max);
    let any_active = rows.iter().any(|r| r.active);

    let row = |i: usize, r: &Bar| {
        let y = i as f64 * row_h + 2.0;
        let bw = if max > 0.0 { (r.value / max * plot_w).max(2.0) } else { 2.0 };
        let dim = any_active && !r.active;
        html! {
            g.row {
                title { (r.tip) }
                rect.hit x="0" y=(y) width=(w) height=(row_h) {}
                text.lbl x=(label_w - 10.0) y=(y + row_h / 2.0 + 4.0) text-anchor="end" { (truncate(&r.label, 24)) }
                path class=(if dim { "bar dim" } else { "bar" }) d=(hbar_path(label_w, y + (row_h - bar_h) / 2.0, bw, bar_h, 4.0)) {}
                text.val x=(label_w + bw + 6.0) y=(y + row_h / 2.0 + 4.0) { (r.display) }
            }
        }
    };
    html! {
        svg viewBox=(format!("0 0 {w} {h}")) role="img" aria-label=(aria) {
            @for (i, r) in rows.iter().enumerate() {
                @if let Some(href) = &r.href {
                    a href=(href) { (row(i, r)) }
                } @else {
                    (row(i, r))
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

/// Two series per row on one scale, with a legend.
pub fn hbar_pair(rows: &[Pair], names: [&str; 2], fmt: fn(f64) -> String, aria: &str) -> Markup {
    if rows.is_empty() {
        return empty();
    }
    let (w, label_w, value_w, bar_h, pair_gap) = (760.0, 220.0, 60.0, 10.0, 2.0);
    let row_h = bar_h * 2.0 + pair_gap + 12.0;
    let legend_h = 26.0;
    let plot_w = w - label_w - value_w;
    let h = legend_h + rows.len() as f64 * row_h;
    let max = rows.iter().flat_map(|r| [r.a, r.b]).fold(0.0, f64::max);
    let legend_x = [label_w, label_w + 30.0 + names[0].len() as f64 * 6.6];

    html! {
        svg viewBox=(format!("0 0 {w} {h}")) role="img" aria-label=(aria) {
            @for (k, name) in names.iter().enumerate() {
                rect class=(format!("bar s{}", k + 1)) x=(legend_x[k]) y="4" width="12" height="12" rx="3" {}
                text.lbl x=(legend_x[k] + 18.0) y="14" { (name) }
            }
            @for (i, r) in rows.iter().enumerate() {
                @let y = legend_h + i as f64 * row_h;
                g.row.pair {
                    title { (r.tip) }
                    rect.hit x="0" y=(y - 4.0) width=(w) height=(row_h) {}
                    text.lbl x=(label_w - 10.0) y=(y + bar_h + 4.0) text-anchor="end" { (truncate(&r.label, 34)) }
                    @for (k, v) in [r.a, r.b].into_iter().enumerate() {
                        @let by = y + k as f64 * (bar_h + pair_gap);
                        @let bw = if max > 0.0 { (v / max * plot_w).max(2.0) } else { 2.0 };
                        path class=(format!("bar s{}", k + 1)) d=(hbar_path(label_w, by, bw, bar_h, 3.0)) {}
                        text.val x=(label_w + bw + 6.0) y=(by + bar_h - 1.0) { (fmt(v)) }
                    }
                }
            }
        }
    }
}

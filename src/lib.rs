//! Canada Job Tracker: the dataset, filtering and page rendering, for the server (src/main.rs)
//! and the static site (src/site.rs).

mod canada_map;
pub mod charts;
pub mod data;
#[cfg(not(target_arch = "wasm32"))]
pub mod fetch;
pub mod query;
pub mod regions;
pub mod site;
pub mod views;

/// Bootstrap 5.3.3 (MIT, static/vendor), then the site's own few rules, served as one stylesheet.
pub const STYLES: &str = concat!(include_str!("../static/vendor/bootstrap.min.css"), "\n", include_str!("../static/styles.css"));

/// Changes whenever the stylesheet does, so browsers can cache it forever.
pub fn styles_version() -> &'static str {
    use std::hash::{DefaultHasher, Hash, Hasher};
    static V: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    V.get_or_init(|| {
        let mut h = DefaultHasher::new();
        STYLES.hash(&mut h);
        format!("{:x}", h.finish())
    })
}

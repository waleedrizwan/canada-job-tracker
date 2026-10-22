//! Canada Job Tracker: the dataset, filtering and page rendering. Shared by the server
//! (src/main.rs) and the WebAssembly build that renders pages in the browser (src/wasm.rs).

mod canada_map;
pub mod charts;
pub mod data;
#[cfg(not(target_arch = "wasm32"))]
pub mod fetch;
pub mod query;
pub mod regions;
#[cfg(not(target_arch = "wasm32"))]
pub mod site;
pub mod views;
#[cfg(target_arch = "wasm32")]
mod wasm;

pub const STYLES: &str = include_str!("../static/styles.css");

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

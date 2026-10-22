//! Builds the static version of the site, hosted on Cloudflare: the default view rendered ahead
//! of time, plus the dataset, the WebAssembly build (src/wasm.rs) and static/app.js, which
//! render every other view in the browser.

use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::Path;

use anyhow::Result;

use crate::data::Dataset;
use crate::query::Params;
use crate::{STYLES, views};

const APP_JS: &str = include_str!("../static/app.js");

/// Cloudflare's headers file. Asset names change with their contents, so they can be cached forever.
const HEADERS: &str = "\
/assets/*
  Cache-Control: public, max-age=31536000, immutable
/styles.css
  Cache-Control: public, max-age=31536000, immutable
";

pub fn build(ds: &Dataset, data: &[u8], wasm: &[u8], out: &Path) -> Result<()> {
    let assets = out.join("assets");
    if assets.exists() {
        std::fs::remove_dir_all(&assets)?;
    }
    std::fs::create_dir_all(&assets)?;
    let data_url = write_hashed(&assets, "jobs", "json", data)?;
    let wasm_url = write_hashed(&assets, "app", "wasm", wasm)?;
    let js_url = write_hashed(&assets, "app", "js", APP_JS.as_bytes())?;

    let script = format!(r#"<script type="module" src="{js_url}" data-wasm="{wasm_url}" data-data="{data_url}"></script>"#);
    let page = views::page(ds, &Params::default().normalize()).into_string().replacen("</head>", &format!("{script}</head>"), 1);
    std::fs::write(out.join("index.html"), page)?;
    std::fs::write(out.join("styles.css"), STYLES)?;
    std::fs::write(out.join("_headers"), HEADERS)?;
    Ok(())
}

/// Writes `assets/{name}.{hash}.{ext}` and returns its URL.
fn write_hashed(dir: &Path, name: &str, ext: &str, bytes: &[u8]) -> Result<String> {
    let mut h = DefaultHasher::new();
    bytes.hash(&mut h);
    let file = format!("{name}.{:016x}.{ext}", h.finish());
    std::fs::write(dir.join(&file), bytes)?;
    Ok(format!("/assets/{file}"))
}

//! Canada Job Tracker: a server-rendered dashboard of Canadian job postings.
//!
//!   canada-job-tracker           serve the dashboard (PORT, default 8080)
//!   canada-job-tracker fetch     download fresh data into DATA_PATH and exit
//!
//! Env: DATA_PATH (default data/jobs.json), MONTHS (Job Bank months to keep, default 3),
//! REFRESH_HOURS (how often the server checks for new releases, default 6; 0 disables).

mod charts;
mod data;
mod fetch;
mod query;
mod regions;
mod views;
mod canada_map;

use std::sync::{Arc, RwLock};
use std::time::Duration;

use anyhow::{Context, Result};
use axum::extract::{Query, State};
use axum::http::{HeaderValue, header};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use tower_http::compression::CompressionLayer;

use data::{Dataset, RawFile};
use query::Params;

const STYLES: &str = include_str!("../static/styles.css");

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

#[derive(Clone)]
struct App {
    data: Arc<RwLock<Arc<Dataset>>>,
    data_path: String,
    months: usize,
}

impl App {
    fn current(&self) -> Arc<Dataset> {
        self.data.read().expect("dataset lock").clone()
    }
}

fn env_or<T: std::str::FromStr>(key: &str, default: T) -> T {
    std::env::var(key).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

/// Writes atomically so a crash mid-write never leaves a half file behind.
fn save(path: &str, raw: &RawFile) -> Result<()> {
    let tmp = format!("{path}.tmp");
    if let Some(dir) = std::path::Path::new(path).parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(&tmp, serde_json::to_vec(raw)?)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let data_path = env_or("DATA_PATH", "data/jobs.json".to_string());
    let months = env_or("MONTHS", 3usize);

    if std::env::args().nth(1).as_deref() == Some("fetch") {
        eprintln!("Fetching Job Bank and StatCan data…");
        let raw = fetch::fetch_all(months).await?;
        save(&data_path, &raw)?;
        eprintln!("Wrote {} postings (latest {}) to {data_path}", raw.rows.len(), raw.as_of);
        return Ok(());
    }

    let ds = Dataset::load(&data_path).context("no data yet; run `cargo run --release -- fetch` first")?;
    eprintln!("Loaded {} postings ({} to {})", ds.jobs.len(), ds.months.last().map_or("?", |s| s), ds.months.first().map_or("?", |s| s));
    let app = App { data: Arc::new(RwLock::new(Arc::new(ds))), data_path, months };

    let refresh_hours = env_or("REFRESH_HOURS", 6u64);
    if refresh_hours > 0 {
        tokio::spawn(refresh_loop(app.clone(), Duration::from_secs(refresh_hours * 3600)));
    }

    let router = Router::new()
        .route("/", get(index))
        .route("/export.csv", get(export))
        .route("/styles.css", get(styles))
        .route("/healthz", get(|| async { "ok" }))
        .layer(CompressionLayer::new())
        .with_state(app);

    let port = env_or("PORT", 8080u16);
    let listener = tokio::net::TcpListener::bind(("0.0.0.0", port)).await?;
    eprintln!("Listening on http://localhost:{port}");
    axum::serve(listener, router).with_graceful_shutdown(shutdown()).await?;
    Ok(())
}

async fn index(State(app): State<App>, Query(p): Query<Params>) -> Response {
    let p = p.normalize();
    let ds = app.current();
    let html = views::page(&ds, &p).into_string();
    ([(header::CACHE_CONTROL, "public, max-age=60")], Html(html)).into_response()
}

async fn export(State(app): State<App>, Query(p): Query<Params>) -> Response {
    let ds = app.current();
    let body = views::csv(&ds, &p.normalize());
    let name = format!("attachment; filename=\"ontario-jobs-{}.csv\"", ds.as_of);
    (
        [
            (header::CONTENT_TYPE, HeaderValue::from_static("text/csv; charset=utf-8")),
            (header::CONTENT_DISPOSITION, HeaderValue::from_str(&name).unwrap_or(HeaderValue::from_static("attachment"))),
        ],
        body,
    )
        .into_response()
}

async fn styles() -> Response {
    (
        [(header::CONTENT_TYPE, "text/css; charset=utf-8"), (header::CACHE_CONTROL, "public, max-age=31536000, immutable")],
        STYLES,
    )
        .into_response()
}

/// Checks for a new Job Bank month or StatCan quarter and reloads when either appears.
async fn refresh_loop(app: App, every: Duration) {
    loop {
        if let Err(e) = refresh_once(&app).await {
            eprintln!("Refresh failed: {e:#}");
        }
        tokio::time::sleep(every).await;
    }
}

async fn refresh_once(app: &App) -> Result<()> {
    let current = app.current();
    let latest: Vec<String> = fetch::job_bank_files(app.months).await?.into_iter().map(|f| f.name).collect();
    let bench_period = fetch::benchmark().await.ok().map(|b| b.period);
    let have_period = current.benchmark.as_ref().map(|b| b.period.clone());
    if latest == current.months && (bench_period.is_none() || bench_period == have_period) {
        return Ok(());
    }
    eprintln!("New data available ({latest:?}, StatCan {bench_period:?}); refreshing…");
    let raw = fetch::fetch_all(app.months).await?;
    if let Err(e) = save(&app.data_path, &raw) {
        eprintln!("Couldn't save refreshed data: {e:#}");
    }
    let ds = Dataset::from_raw(raw)?;
    eprintln!("Refreshed: {} postings, latest {}", ds.jobs.len(), ds.as_of);
    *app.data.write().expect("dataset lock") = Arc::new(ds);
    Ok(())
}

async fn shutdown() {
    let ctrl_c = async { tokio::signal::ctrl_c().await.ok(); };
    #[cfg(unix)]
    let term = async {
        if let Ok(mut s) = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            s.recv().await;
        }
    };
    #[cfg(not(unix))]
    let term = std::future::pending::<()>();
    tokio::select! { _ = ctrl_c => {}, _ = term => {} }
}

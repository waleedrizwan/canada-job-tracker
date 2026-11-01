//! Renders views from the bundled data/jobs.json, to catch panics on real data that the small
//! fixtures in the unit tests can't (odd titles, empty categories, provinces with few postings).

use canada_job_tracker::data::Dataset;
use canada_job_tracker::query::Params;
use canada_job_tracker::views;

fn params(qs: &str) -> Params {
    serde_urlencoded::from_str::<Params>(qs).unwrap().normalize()
}

#[test]
fn renders_views_from_the_bundled_data() {
    let ds = Dataset::load(concat!(env!("CARGO_MANIFEST_DIR"), "/data/jobs.json")).expect("data/jobs.json loads");
    assert!(ds.jobs.len() > 1_000, "the bundled dataset has postings");
    assert!(ds.benchmark.is_some(), "the bundled dataset has StatCan figures");

    // Every tab, under filters that match many postings, a few, or none.
    for qs in ["", "prov=NU", "map=wage&prov=QC", "cat=Health&wage=40", "q=nurse&sort=wage&dir=asc", "prov=NU&wage=290", "page=99999", "month=garbage"] {
        let jobs = views::page(&ds, &params(qs)).into_string();
        assert!(jobs.contains(r#"<form class="search mb-3""#), "{qs}: search");
        assert!(jobs.contains("job card") || jobs.contains("No jobs match."), "{qs}: cards");
        let heat = views::page(&ds, &params(&format!("view=heatmap&{qs}"))).into_string();
        assert!(heat.contains("svg class=\"map\""), "{qs}: map");
        assert!(heat.contains(r#"<table class="heat""#), "{qs}: heat grid");
        let charts = views::page(&ds, &params(&format!("view=charts&{qs}"))).into_string();
        assert!(charts.contains(r#"<form class="filters card card-body mb-4""#), "{qs}: filters");
    }
    let charts = views::page(&ds, &params("view=charts")).into_string();
    assert!(charts.contains("Job Bank vs StatCan vacancies"), "benchmark chart shows with StatCan data");
    let first = views::page(&ds, &params("")).into_string();
    assert_eq!(first.matches(r#"<article class="job card"#).count(), 48, "a full page of cards");

    let csv = views::csv(&ds, &params("prov=PE"));
    let rows = csv.lines().count() - 1;
    assert!(rows > 0 && rows < ds.jobs.len());
    assert!(csv.lines().skip(1).all(|l| l.contains(",PE,")), "every PE row carries its province");
}

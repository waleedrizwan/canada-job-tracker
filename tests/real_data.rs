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

    for qs in ["", "prov=NU", "map=wage&prov=QC", "cat=Health&wage=40", "q=nurse&sort=wage&dir=asc", "prov=NU&wage=290", "page=99999", "month=garbage"] {
        let html = views::page(&ds, &params(qs)).into_string();
        assert!(html.contains("Where in Canada?"), "{qs}: map section");
        assert!(html.contains("The postings"), "{qs}: table");
    }
    let all = views::page(&ds, &params("")).into_string();
    assert!(all.contains("What Job Bank misses"), "benchmark chart shows with StatCan data");

    let csv = views::csv(&ds, &params("prov=PE"));
    let rows = csv.lines().count() - 1;
    assert!(rows > 0 && rows < ds.jobs.len());
    assert!(csv.lines().skip(1).all(|l| l.contains(",PE,")), "every PE row carries its province");
}

use launcher_core::{Package, PackageSource, SortOrder, TREND_DAYS, sort_packages};
use time::{Date, Month};
use url::Url;

fn today() -> Date {
    Date::from_calendar_date(2026, Month::November, 4).unwrap()
}

fn package(id: &str, dependencies: &[&str], release_dates: &[&str]) -> Package {
    let url = Url::parse("https://example.com/").unwrap();
    let mut package = Package::simple(id, id, url.clone(), PackageSource::Web(url));
    package.dependencies = dependencies.iter().map(|id| (*id).to_owned()).collect();
    package.release_dates = release_dates
        .iter()
        .map(|date| (*date).to_owned())
        .collect();
    package
}

fn ids(catalog: &[Package], order: SortOrder) -> Vec<String> {
    let mut shown: Vec<&Package> = catalog.iter().collect();
    sort_packages(&mut shown, order, catalog, today());
    shown.iter().map(|package| package.id.clone()).collect()
}

#[test]
fn popularity_counts_each_dependent_package_once() {
    let catalog = [
        package("w", &[], &[]),
        package("y", &[], &[]),
        package("x", &["y", "y"], &[]),
        package("z", &["w"], &[]),
    ];
    assert_eq!(ids(&catalog, SortOrder::Popularity), ["w", "y", "x", "z"]);
}

#[test]
fn popularity_ranks_the_most_depended_package_first() {
    let catalog = [
        package("a", &[], &[]),
        package("b", &[], &[]),
        package("c", &["b"], &[]),
        package("d", &["b", "a"], &[]),
    ];
    assert_eq!(ids(&catalog, SortOrder::Popularity), ["b", "a", "c", "d"]);
}

#[test]
fn trend_counts_releases_inside_the_window() {
    let catalog = [
        package("old", &[], &["2026-01-01", "2025-12-01"]),
        package("one", &[], &["2026-10-01"]),
        package("two", &[], &["2026-11-01", "2026-10-20", "2026-01-01"]),
        package("future", &[], &["2026-11-05"]),
    ];
    assert_eq!(
        ids(&catalog, SortOrder::Trend),
        ["two", "one", "future", "old"]
    );
}

#[test]
fn trend_window_starts_trend_days_before_today() {
    assert_eq!(TREND_DAYS, 90);
    let catalog = [
        package("inside", &[], &["2026-08-06"]),
        package("outside", &[], &["2026-08-05"]),
    ];
    assert_eq!(ids(&catalog, SortOrder::Trend), ["inside", "outside"]);
}

#[test]
fn newest_uses_the_first_release_and_updated_uses_the_latest() {
    let catalog = [
        package("a", &[], &["2026-09-01", "2024-01-01"]),
        package("b", &[], &["2026-02-01", "2025-06-01"]),
        package("none", &[], &[]),
    ];
    assert_eq!(ids(&catalog, SortOrder::Newest), ["b", "a", "none"]);
    assert_eq!(ids(&catalog, SortOrder::Updated), ["a", "b", "none"]);
}

#[test]
fn ties_fall_back_to_name_then_id() {
    let catalog = [package("b", &[], &[]), package("a", &[], &[])];
    for order in [
        SortOrder::Popularity,
        SortOrder::Trend,
        SortOrder::Newest,
        SortOrder::Updated,
    ] {
        assert_eq!(ids(&catalog, order), ["a", "b"]);
    }
}

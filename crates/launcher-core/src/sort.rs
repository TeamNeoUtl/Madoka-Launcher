//! Orderings of catalog packages. Every key derives from catalog data:
//! `Package::dependencies` and `Package::release_dates`.

use std::collections::{HashMap, HashSet};

use time::{Date, Duration, OffsetDateTime};

use crate::Package;

/// A release within this many days up to `today` counts for the trend.
pub const TREND_DAYS: i64 = 90;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SortOrder {
    /// Packages that most other catalog packages depend on come first.
    #[default]
    Popularity,
    /// Packages with the most releases in the last `TREND_DAYS` days come first.
    Trend,
    /// Packages with the newest first release come first.
    Newest,
    /// Packages with the newest latest release come first.
    Updated,
}

/// Current UTC date.
#[must_use]
pub fn today() -> Date {
    OffsetDateTime::now_utc().date()
}

/// Sorts `shown` in place. Packages without a key come last.
/// Ties fall back to the other key, then to the lowercase name, then to the id.
/// `catalog` is the full list in which popularity counts dependents.
pub fn sort_packages(shown: &mut [&Package], order: SortOrder, catalog: &[Package], today: Date) {
    let dependents = dependents(catalog);
    let popularity = |package: &Package| {
        dependents
            .get(package.id.as_str())
            .copied()
            .unwrap_or_default()
    };
    let window_end = iso(today);
    let window_start = iso(today
        .checked_sub(Duration::days(TREND_DAYS))
        .unwrap_or(Date::MIN));
    let trend = |package: &Package| {
        package
            .release_dates
            .iter()
            .filter(|date| {
                date.as_str() >= window_start.as_str() && date.as_str() <= window_end.as_str()
            })
            .count()
    };
    shown.sort_by(|a, b| {
        let primary = match order {
            SortOrder::Popularity => popularity(b)
                .cmp(&popularity(a))
                .then_with(|| trend(b).cmp(&trend(a))),
            SortOrder::Trend => trend(b)
                .cmp(&trend(a))
                .then_with(|| popularity(b).cmp(&popularity(a))),
            SortOrder::Newest => b.first_release().cmp(&a.first_release()),
            SortOrder::Updated => b.latest_release().cmp(&a.latest_release()),
        };
        primary
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
            .then_with(|| a.id.cmp(&b.id))
    });
}

/// Number of catalog packages that list each id in `dependencies`.
fn dependents(catalog: &[Package]) -> HashMap<&str, usize> {
    let mut counts = HashMap::new();
    for package in catalog {
        let unique: HashSet<&str> = package.dependencies.iter().map(String::as_str).collect();
        for id in unique {
            *counts.entry(id).or_insert(0) += 1;
        }
    }
    counts
}

fn iso(date: Date) -> String {
    format!(
        "{:04}-{:02}-{:02}",
        date.year(),
        u8::from(date.month()),
        date.day()
    )
}

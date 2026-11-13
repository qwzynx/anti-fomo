//! What the reader asked to have scraped, and which sources that makes due.
//!
//! Scraping used to be unconditional: every source, every launch, whatever the
//! reader cared about. This is the other half of asking first — the answer to
//! "what do you want?", and the arithmetic that turns it into "so these
//! sources, now". All of it is pure, so the rules that decide what costs a
//! network request are testable without an app or a clock.

use std::collections::HashMap;

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

use crate::db::{SourceRun, Wanted};
use crate::models::{Item, ItemType};
use crate::scrapers::Category;

/// The settings key the preferences are stored under, as JSON.
pub const KEY: &str = "scrape_prefs";

/// How long a source that *failed* waits before it is tried again — shorter
/// than any category's freshness window, because a failure means nothing is
/// fresh. Not zero: a source that is down should not be hammered by every
/// refresh, and a reader pressing Refresh is already the manual override.
pub const FAILURE_RETRY_MINUTES: i64 = 15;

/// What the reader wants collected. Every field defaults to "yes", so an
/// absent or partial stored value — a first launch, or a setting written
/// before a field existed — scrapes everything, which is what the app did
/// before it asked.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct ScrapePrefs {
    pub internships: bool,
    pub jobs: bool,
    pub news: bool,
    pub events: bool,
    /// Sources switched off one by one, by `source_name`. A source in an
    /// enabled category can still be off; the reverse cannot happen.
    pub disabled_sources: Vec<String>,
}

impl Default for ScrapePrefs {
    fn default() -> Self {
        ScrapePrefs {
            internships: true,
            jobs: true,
            news: true,
            events: true,
            disabled_sources: Vec::new(),
        }
    }
}

impl ScrapePrefs {
    /// The kinds of row wanted, as the store's purge understands them.
    pub fn wanted(&self) -> Wanted {
        Wanted {
            internships: self.internships,
            jobs: self.jobs,
            news: self.news,
            events: self.events,
        }
    }

    /// Whether anything in this category is worth a request. Opportunities need
    /// only one of the two reader-facing kinds, since the same employer boards
    /// serve both.
    pub fn category_enabled(&self, category: Category) -> bool {
        match category {
            Category::News => self.news,
            Category::Events => self.events,
            Category::Opportunities => self.internships || self.jobs,
        }
    }

    pub fn source_enabled(&self, category: Category, name: &str) -> bool {
        self.category_enabled(category) && !self.disabled_sources.iter().any(|s| s == name)
    }

    /// An internship is a listing a source filed as one, or any role whose
    /// title said "Intern" (the employer boards file everything as a job).
    /// The store's `purge_unwanted` applies the same rule in SQL.
    pub fn is_internship(item: &Item) -> bool {
        item.item_type == ItemType::Internship || item.seniority.as_deref() == Some("Intern")
    }

    /// Whether a row the scrape returned is one the reader asked for. Applied
    /// before anything is stored, which is what keeps an unwanted kind out of
    /// the ranker and out of the description-fetch budget instead of merely
    /// hiding it on screen.
    ///
    /// It checks the source as well as the kind, because the preferences are
    /// read *after* the fetch: a source switched off while a forty-second
    /// refresh was already in flight would otherwise land its rows, and the
    /// sweep never visits a source that is not being scraped, so they would
    /// stay for good.
    pub fn admits(&self, item: &Item) -> bool {
        if self
            .disabled_sources
            .iter()
            .any(|s| s == &item.source_platform)
        {
            return false;
        }
        match item.item_type {
            ItemType::Article => self.news,
            ItemType::Event => self.events,
            ItemType::Job | ItemType::Internship => {
                if Self::is_internship(item) {
                    self.internships
                } else {
                    self.jobs
                }
            }
        }
    }
}

/// The sources a refresh should run: those the reader wants and whose last run
/// has gone stale. `force` — the Refresh button — overrides staleness but never
/// the reader's choices: a switched-off source is not fetched by a manual
/// refresh either.
///
/// A source never run is due; one whose last run worked is due once its
/// category's window has passed; one whose last run failed is due after a
/// shorter wait. A dormant source counts as having worked.
pub fn due(
    sources: &[(Category, &'static str)],
    runs: &[SourceRun],
    prefs: &ScrapePrefs,
    now: DateTime<Utc>,
    force: bool,
) -> Vec<&'static str> {
    let last: HashMap<&str, &SourceRun> = runs.iter().map(|r| (r.source.as_str(), r)).collect();
    sources
        .iter()
        .filter(|(category, name)| prefs.source_enabled(*category, name))
        .filter(|(category, name)| {
            force
                || match last.get(name) {
                    None => true,
                    Some(run) => {
                        let wait = if run.ok {
                            category.ttl()
                        } else {
                            Duration::minutes(FAILURE_RETRY_MINUTES)
                        };
                        now - run.last_run_at >= wait
                    }
                }
        })
        .map(|(_, name)| *name)
        .collect()
}

/// The sources a change of preferences makes due right away, so that changing
/// what you want refreshes *only* what newly became wanted.
///
/// A source switched on is due. So is every opportunity source when internships
/// or jobs are newly wanted even though the category was already running: those
/// scrapers were fetching everything and the ingest filter was dropping the
/// kind that was off, so the rows are not in the cache to be revealed — they
/// have to be fetched again.
pub fn made_due(
    old: &ScrapePrefs,
    new: &ScrapePrefs,
    sources: &[(Category, &'static str)],
) -> Vec<&'static str> {
    let widened = (!old.internships && new.internships) || (!old.jobs && new.jobs);
    sources
        .iter()
        .filter(|(category, name)| new.source_enabled(*category, name))
        .filter(|(category, name)| {
            !old.source_enabled(*category, name)
                || (widened && *category == Category::Opportunities)
        })
        .map(|(_, name)| *name)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(ty: ItemType, seniority: Option<&str>) -> Item {
        let mut item = Item::new("Role", "S", ty, "https://x.test/1");
        item.seniority = seniority.map(String::from);
        item
    }

    fn run(source: &str, ago: Duration, ok: bool) -> SourceRun {
        let at = Utc::now() - ago;
        SourceRun {
            source: source.into(),
            category: "opportunities".into(),
            last_run_at: at,
            ok,
            last_ok_at: ok.then_some(at),
            last_count: 10,
            last_error: (!ok).then(|| "boom".to_string()),
        }
    }

    const SOURCES: &[(Category, &str)] = &[
        (Category::News, "Hacker News"),
        (Category::Opportunities, "Greenhouse"),
        (Category::Opportunities, "Simplify"),
        (Category::Events, "Luma"),
    ];

    // --- what a row is ---

    #[test]
    fn everything_is_wanted_until_the_reader_says_otherwise() {
        let prefs = ScrapePrefs::default();
        for ty in [
            ItemType::Job,
            ItemType::Internship,
            ItemType::Article,
            ItemType::Event,
        ] {
            assert!(prefs.admits(&item(ty, None)));
        }
        // A stored value from before a field existed fills in with "yes".
        let partial: ScrapePrefs = serde_json::from_str(r#"{"news": false}"#).unwrap();
        assert!(!partial.news && partial.jobs && partial.internships && partial.events);
        // And a corrupt one is no preference at all, not a crash.
        assert!(serde_json::from_str::<ScrapePrefs>("not json").is_err());
    }

    #[test]
    fn an_internship_is_filed_as_one_or_titled_as_one() {
        assert!(ScrapePrefs::is_internship(&item(
            ItemType::Internship,
            None
        )));
        assert!(ScrapePrefs::is_internship(&item(
            ItemType::Job,
            Some("Intern")
        )));
        assert!(!ScrapePrefs::is_internship(&item(
            ItemType::Job,
            Some("New grad")
        )));
        assert!(!ScrapePrefs::is_internship(&item(ItemType::Job, None)));
    }

    #[test]
    fn internships_and_jobs_are_admitted_independently() {
        let internships_only = ScrapePrefs {
            jobs: false,
            ..ScrapePrefs::default()
        };
        assert!(internships_only.admits(&item(ItemType::Internship, None)));
        assert!(internships_only.admits(&item(ItemType::Job, Some("Intern"))));
        assert!(!internships_only.admits(&item(ItemType::Job, Some("Senior"))));
        assert!(!internships_only.admits(&item(ItemType::Job, None)));

        let jobs_only = ScrapePrefs {
            internships: false,
            ..ScrapePrefs::default()
        };
        assert!(!jobs_only.admits(&item(ItemType::Internship, None)));
        assert!(!jobs_only.admits(&item(ItemType::Job, Some("Intern"))));
        assert!(jobs_only.admits(&item(ItemType::Job, None)));
    }

    /// The race: the plan was made before the source was switched off, so its
    /// scraper ran anyway, and its rows arrive after the preference changed.
    #[test]
    fn rows_from_a_source_switched_off_mid_refresh_are_refused() {
        let mut job = item(ItemType::Job, None);
        job.source_platform = "Greenhouse".into();
        let picky = ScrapePrefs {
            disabled_sources: vec!["Greenhouse".into()],
            ..ScrapePrefs::default()
        };
        assert!(!picky.admits(&job), "a switched-off source's row got in");
        assert!(ScrapePrefs::default().admits(&job));

        let mut elsewhere = item(ItemType::Job, None);
        elsewhere.source_platform = "Simplify".into();
        assert!(picky.admits(&elsewhere), "only the named source is refused");
    }

    #[test]
    fn news_and_events_switch_off_on_their_own() {
        let quiet = ScrapePrefs {
            news: false,
            events: false,
            ..ScrapePrefs::default()
        };
        assert!(!quiet.admits(&item(ItemType::Article, None)));
        assert!(!quiet.admits(&item(ItemType::Event, None)));
        assert!(quiet.admits(&item(ItemType::Job, None)));
    }

    #[test]
    fn opportunities_need_only_one_of_the_two_kinds_and_a_disabled_source_stays_off() {
        let one = ScrapePrefs {
            jobs: false,
            ..ScrapePrefs::default()
        };
        assert!(one.category_enabled(Category::Opportunities));
        let neither = ScrapePrefs {
            jobs: false,
            internships: false,
            ..ScrapePrefs::default()
        };
        assert!(!neither.category_enabled(Category::Opportunities));

        let picky = ScrapePrefs {
            disabled_sources: vec!["Greenhouse".into()],
            ..ScrapePrefs::default()
        };
        assert!(!picky.source_enabled(Category::Opportunities, "Greenhouse"));
        assert!(picky.source_enabled(Category::Opportunities, "Simplify"));
    }

    // --- what is due ---

    #[test]
    fn a_source_never_run_is_due_and_a_fresh_one_is_not() {
        let prefs = ScrapePrefs::default();
        let now = Utc::now();
        let runs = [run("Greenhouse", Duration::minutes(10), true)];
        let due = due(SOURCES, &runs, &prefs, now, false);
        assert!(
            !due.contains(&"Greenhouse"),
            "ten minutes is inside the six-hour window"
        );
        assert!(due.contains(&"Hacker News"), "never run");
        assert!(due.contains(&"Luma"));
    }

    #[test]
    fn a_source_is_due_once_its_own_categorys_window_has_passed() {
        let prefs = ScrapePrefs::default();
        let now = Utc::now();
        let mut runs = [run("Hacker News", Duration::minutes(45), true)];
        runs[0].category = "news".into();
        // 45 minutes is stale for news (30) but would be fresh for a role (6 h).
        assert!(due(SOURCES, &runs, &prefs, now, false).contains(&"Hacker News"));
        let mut roles = [run("Greenhouse", Duration::minutes(45), true)];
        roles[0].category = "opportunities".into();
        assert!(!due(SOURCES, &roles, &prefs, now, false).contains(&"Greenhouse"));
        let old = [run("Greenhouse", Duration::hours(7), true)];
        assert!(due(SOURCES, &old, &prefs, now, false).contains(&"Greenhouse"));
    }

    #[test]
    fn a_failed_source_is_retried_after_a_short_wait_not_the_long_one() {
        let prefs = ScrapePrefs::default();
        let now = Utc::now();
        let just_failed = [run("Greenhouse", Duration::minutes(5), false)];
        assert!(!due(SOURCES, &just_failed, &prefs, now, false).contains(&"Greenhouse"));
        let a_while_ago = [run("Greenhouse", Duration::minutes(20), false)];
        assert!(
            due(SOURCES, &a_while_ago, &prefs, now, false).contains(&"Greenhouse"),
            "twenty minutes would still be fresh had it worked, but it did not"
        );
    }

    #[test]
    fn a_manual_refresh_overrides_freshness_but_never_the_readers_choices() {
        let prefs = ScrapePrefs {
            disabled_sources: vec!["Simplify".into()],
            events: false,
            ..ScrapePrefs::default()
        };
        let now = Utc::now();
        let fresh = [
            run("Greenhouse", Duration::minutes(1), true),
            run("Simplify", Duration::minutes(1), true),
        ];
        let forced = due(SOURCES, &fresh, &prefs, now, true);
        assert_eq!(forced, ["Hacker News", "Greenhouse"]);
    }

    #[test]
    fn nothing_is_due_when_nothing_is_wanted() {
        let prefs = ScrapePrefs {
            internships: false,
            jobs: false,
            news: false,
            events: false,
            disabled_sources: vec![],
        };
        assert!(due(SOURCES, &[], &prefs, Utc::now(), true).is_empty());
    }

    // --- what a change of mind makes due ---

    #[test]
    fn switching_a_source_on_makes_just_that_source_due() {
        let old = ScrapePrefs {
            disabled_sources: vec!["Luma".into()],
            ..ScrapePrefs::default()
        };
        let new = ScrapePrefs::default();
        assert_eq!(made_due(&old, &new, SOURCES), ["Luma"]);
    }

    /// The scrapers were running all along and the ingest filter was dropping
    /// the unwanted kind, so the rows are not in the cache to be revealed.
    #[test]
    fn newly_wanting_jobs_reruns_the_opportunity_sources_that_were_already_running() {
        let old = ScrapePrefs {
            jobs: false,
            ..ScrapePrefs::default()
        };
        let new = ScrapePrefs::default();
        assert_eq!(made_due(&old, &new, SOURCES), ["Greenhouse", "Simplify"]);
    }

    #[test]
    fn narrowing_what_is_wanted_makes_nothing_due() {
        let old = ScrapePrefs::default();
        let new = ScrapePrefs {
            jobs: false,
            news: false,
            disabled_sources: vec!["Luma".into()],
            ..ScrapePrefs::default()
        };
        assert!(made_due(&old, &new, SOURCES).is_empty());
        assert!(made_due(&old, &old, SOURCES).is_empty());
    }
}

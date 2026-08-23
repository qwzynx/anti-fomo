//! Job Bank is the Government of Canada's official posting board. Every other
//! job source in this app is US-centric — the GitHub repos are overwhelmingly
//! American — so this is what makes the hub useful to a student who actually
//! wants to work in Canada.
//!
//! There is no public API, but the search results page is plain server-rendered
//! HTML with stable class names, so no browser is needed.

use anyhow::Result;
use async_trait::async_trait;
use chrono::{NaiveDate, TimeZone, Utc};
use scraper::{ElementRef, Html, Selector};
use std::sync::LazyLock;

use super::{collapse_ws, Scraper};
use crate::models::{Item, ItemType};

pub struct JobBank {
    pub name: &'static str,
    /// A `searchstring` for the public job search.
    pub query: &'static str,
    pub item_type: ItemType,
}

/// One pass, typed as a job rather than an internship, and that is deliberate.
/// Job Bank's search ignores qualifier words — "software developer intern" and
/// "junior software developer" both return the same generic developer listings
/// (measured: 24 of 25 results identical) — so there is no honest way to make
/// this an internship feed. It is Canadian developer roles, labelled as such.
pub const JOB_BANK: JobBank = JobBank {
    name: "Job Bank Canada",
    query: "software+developer",
    item_type: ItemType::Job,
};

const BASE: &str = "https://www.jobbank.gc.ca";
/// `sort=D` is newest first; `fperiod=1` limits to the last 30 days so the
/// scrape doesn't drag in months of stale postings.
const SEARCH: &str = "https://www.jobbank.gc.ca/jobsearch/jobsearch?sort=D&fperiod=1&searchstring=";
/// The results page serves 25 postings and takes a `page` parameter; measured,
/// consecutive pages share no postings at all. Five passes is ~125 Canadian
/// roles, which is what makes this competitive with the US-centric sources.
const MAX_PAGES: usize = 5;

static ARTICLE: LazyLock<Selector> = LazyLock::new(|| Selector::parse("article").unwrap());
static LINK: LazyLock<Selector> = LazyLock::new(|| Selector::parse("a").unwrap());
static TITLE: LazyLock<Selector> = LazyLock::new(|| Selector::parse(".noctitle").unwrap());
static BUSINESS: LazyLock<Selector> = LazyLock::new(|| Selector::parse("li.business").unwrap());
static LOCATION: LazyLock<Selector> = LazyLock::new(|| Selector::parse("li.location").unwrap());
static DATE: LazyLock<Selector> = LazyLock::new(|| Selector::parse("li.date").unwrap());
static SALARY: LazyLock<Selector> = LazyLock::new(|| Selector::parse("li.salary").unwrap());

/// Job Bank stamps a rotating `;jsessionid=…` into every result href. Left in
/// place it would defeat the URL-keyed dedupe entirely — each refresh would
/// look like a brand new set of postings and the cache would grow without
/// bound — so the path is reduced to its stable posting-id form.
fn canonical_url(href: &str) -> String {
    let path = href.split(['?', '#']).next().unwrap_or(href);
    let path = match path.split_once(";jsessionid=") {
        Some((before, _)) => before,
        None => path,
    };
    if path.starts_with("http") {
        path.to_string()
    } else {
        format!("{BASE}{path}")
    }
}

/// Job Bank prints "August 12, 2026". Anything unrecognised falls back to now,
/// which is what every other date-less source in the pipeline does.
fn parse_posted(raw: &str) -> Option<chrono::DateTime<Utc>> {
    let date = NaiveDate::parse_from_str(raw.trim(), "%B %d, %Y").ok()?;
    Utc.from_utc_datetime(&date.and_hms_opt(12, 0, 0)?).into()
}

fn text_of(article: ElementRef, sel: &Selector) -> Option<String> {
    let raw = collapse_ws(&article.select(sel).next()?.text().collect::<String>());
    // The location cell carries an invisible "Location" label for screen readers.
    let cleaned = raw.trim_start_matches("Location").trim();
    (!cleaned.is_empty()).then(|| cleaned.to_string())
}

#[async_trait]
impl Scraper for JobBank {
    fn source_name(&self) -> &'static str {
        self.name
    }

    async fn fetch(&self, client: &reqwest::Client) -> Result<Vec<Item>> {
        let mut all = Vec::new();
        // Sequential rather than concurrent: five polite requests to a
        // government server, and an exhausted search should stop paging rather
        // than fire the remaining requests regardless.
        for page in 1..=MAX_PAGES {
            let items = self.fetch_page(client, page).await;
            match items {
                // Page 1 failing is a real failure; a later page failing just
                // ends the walk with whatever came before it.
                Err(e) if page == 1 => return Err(e),
                Err(e) => {
                    log::warn!("{}: page {page} failed: {e}", self.name);
                    break;
                }
                Ok(items) if items.is_empty() => break,
                Ok(items) => all.extend(items),
            }
        }
        Ok(all)
    }
}

impl JobBank {
    async fn fetch_page(&self, client: &reqwest::Client, page: usize) -> Result<Vec<Item>> {
        let body = client
            .get(format!("{SEARCH}{}&page={page}", self.query))
            .send()
            .await?
            .text()
            .await?;
        let doc = Html::parse_document(&body);

        Ok(doc
            .select(&ARTICLE)
            .filter_map(|article| {
                let href = article
                    .select(&LINK)
                    .next()
                    .and_then(|a| a.value().attr("href"))?;
                let title = text_of(article, &TITLE)?;
                let company = text_of(article, &BUSINESS).unwrap_or_else(|| "Employer".into());
                let location = text_of(article, &LOCATION);

                let mut parts = Vec::new();
                if let Some(loc) = location.as_deref() {
                    parts.push(format!("Location: {loc}"));
                }
                if let Some(salary) = text_of(article, &SALARY) {
                    parts.push(salary);
                }

                let ts = text_of(article, &DATE)
                    .as_deref()
                    .and_then(parse_posted)
                    .unwrap_or_else(Utc::now);

                // The salary cell is the one place a Job Bank listing states
                // pay, and it does so on almost every posting — far more often
                // than a private employer's page does.
                let salary_text = text_of(article, &SALARY);
                let pay = salary_text.as_deref().and_then(crate::pay::parse);

                Some(
                    Item {
                        // Job Bank titles are lowercase NOC occupation names
                        // ("developer, software"), which the keyword classifier
                        // handles poorly; the query already constrains the field.
                        discipline: Some("Software Engineering".to_string()),
                        company: (company != "Employer").then(|| company.clone()),
                        salary_min: pay.as_ref().and_then(|p| p.min),
                        salary_max: pay.as_ref().and_then(|p| p.max),
                        // Job Bank is a Canadian federal board; an unqualified
                        // "$" on it is not US dollars.
                        salary_currency: pay
                            .as_ref()
                            .map(|p| p.currency.clone().unwrap_or_else(|| "CAD".to_string())),
                        salary_period: pay.as_ref().and_then(|p| p.period.clone()),
                        ..Item::new(
                            format!("{title} at {company}"),
                            self.name,
                            self.item_type,
                            canonical_url(href),
                        )
                    }
                    .with_content(parts.join(" · "))
                    .with_timestamp(ts)
                    .with_location(location),
                )
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Datelike;

    #[test]
    fn strips_the_rotating_session_id() {
        // Two scrapes of the same posting differ only by session; both must
        // reduce to one URL or the dedupe and the cache both break.
        let a = canonical_url(
            "/jobsearch/jobposting/50062282;jsessionid=DB3F9881.jobsearch76?source=search",
        );
        let b = canonical_url(
            "/jobsearch/jobposting/50062282;jsessionid=AAAAAAAA.jobsearch74?source=searchresults",
        );
        assert_eq!(a, b);
        assert_eq!(a, "https://www.jobbank.gc.ca/jobsearch/jobposting/50062282");
    }

    #[test]
    fn handles_the_tfw_posting_path_and_absolute_urls() {
        assert_eq!(
            canonical_url("/jobsearch/jobpostingtfw/49549988;jsessionid=X?source=searchresults"),
            "https://www.jobbank.gc.ca/jobsearch/jobpostingtfw/49549988"
        );
        assert_eq!(
            canonical_url("https://www.jobbank.gc.ca/jobsearch/jobposting/1"),
            "https://www.jobbank.gc.ca/jobsearch/jobposting/1"
        );
    }

    #[test]
    fn distinct_postings_stay_distinct() {
        assert_ne!(
            canonical_url("/jobsearch/jobposting/1;jsessionid=X"),
            canonical_url("/jobsearch/jobposting/2;jsessionid=X")
        );
    }

    #[test]
    fn parses_the_posted_date() {
        let d = parse_posted("August 12, 2026").unwrap();
        assert_eq!((d.year(), d.month(), d.day()), (2026, 8, 12));
        assert!(parse_posted("yesterday").is_none());
    }
}

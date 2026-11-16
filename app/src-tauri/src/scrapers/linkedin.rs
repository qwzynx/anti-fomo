//! LinkedIn's job search, read through the guest endpoint that its own
//! logged-out search page pages through. Plain server-rendered HTML, ten cards
//! a request, no account and no browser.
//!
//! **Software roles in Canada, and nothing else.** LinkedIn carries every job
//! there is, and the reader asked for it on that condition: the searches below
//! are all software phrases scoped to Canada, and [`is_software`] then reads
//! each title, because LinkedIn's search is fuzzy and a query for "software
//! engineer" also returns a mechanical engineer and a software sales rep.
//!
//! Measured against the live endpoint before any number here was picked:
//!
//! - **The filters do nothing for a guest.** `f_E` (experience) and `f_JT`
//!   (job type) return the same cards as no filter at all, so the early-career
//!   corner is reached through the keywords — "software engineer intern" is
//!   overwhelmingly interns, "software engineer" overwhelmingly senior.
//! - **Depth.** An early-career query is still on topic 150 results in; a
//!   broad one has said what it has to say within fifty. Hence different page
//!   counts per query.
//! - **Rate.** At one request every 1.5 s about one in five answers 429, and
//!   the same request a few seconds later succeeds. So the requests go one at
//!   a time, paced, and a 429 is waited out rather than treated as an answer.
//!
//! Many of these postings also exist on the employer's own board, under a
//! different URL. The employer's copy is the better one, and the read path
//! hides a LinkedIn row it duplicates ([`super::hide_mirrored`]).

use anyhow::Result;
use async_trait::async_trait;
use chrono::{NaiveDate, TimeZone, Utc};
use scraper::{ElementRef, Html, Selector};
use std::collections::HashSet;
use std::sync::LazyLock;
use std::time::Duration;

use super::{collapse_ws, Scraper, Sink};
use crate::models::{Item, ItemType};
use crate::rank::{classify_seniority, DEFAULT_MAJOR};

pub struct LinkedIn;

pub const NAME: &str = "LinkedIn";

const SEARCH: &str = "https://www.linkedin.com/jobs-guest/jobs/api/seeMoreJobPostings/search";
/// LinkedIn's own id for Canada, which is what its search page sends.
const CANADA_GEO: &str = "101174742";
/// Posted in the last thirty days. Anything older is either filled or has
/// already reached the cache through an earlier refresh.
const POSTED_WITHIN: &str = "r2592000";
/// Cards per request. Fixed by LinkedIn; a shorter page is the last one.
const PAGE: usize = 10;

/// The searches, each with how many pages it is worth. Early-career phrases
/// first and deepest: they are what the aggregators miss and what this reader
/// is looking for. The broad phrases are there for the Canadian roles that no
/// one titled "junior", and stop early because past a few pages they are all
/// senior.
const QUERIES: &[(&str, usize)] = &[
    ("software engineer intern", 10),
    ("software developer co-op", 10),
    ("new grad software engineer", 8),
    ("junior software developer", 8),
    ("software engineer", 5),
    ("software developer", 5),
    ("full stack developer", 3),
    ("backend developer", 3),
    ("frontend developer", 3),
];

/// Between requests. Faster than this the 429s stop being occasional.
const PAUSE: Duration = Duration::from_millis(1500);
/// After a 429, multiplied by the attempt number.
const RATE_LIMIT_WAIT: Duration = Duration::from_secs(4);
const RATE_LIMIT_TRIES: u32 = 3;

static CARD: LazyLock<Selector> = LazyLock::new(|| Selector::parse("[data-entity-urn]").unwrap());
static TITLE: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse(".base-search-card__title").unwrap());
static COMPANY: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse(".base-search-card__subtitle").unwrap());
static LOCATION: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse(".job-search-card__location").unwrap());
static DATE: LazyLock<Selector> = LazyLock::new(|| Selector::parse("time[datetime]").unwrap());
static LINK: LazyLock<Selector> = LazyLock::new(|| Selector::parse("a[href]").unwrap());

#[async_trait]
impl Scraper for LinkedIn {
    fn source_name(&self) -> &'static str {
        NAME
    }

    async fn fetch(&self, client: &reqwest::Client) -> Result<Vec<Item>> {
        super::collect_batches(self, client).await
    }

    /// One query at a time, each handed over when it ends. Not concurrent:
    /// the whole point of the pacing is that LinkedIn sees one reader.
    async fn fetch_batches(&self, client: &reqwest::Client, sink: Sink<'_>) -> Result<()> {
        let mut seen: HashSet<String> = HashSet::new();
        let mut failed = 0;
        let mut last_error = None;
        let mut first_request = true;
        for (query, pages) in QUERIES {
            match walk(client, query, *pages, &mut first_request).await {
                Ok(items) => {
                    // The same posting answers several queries; it is sent once.
                    let fresh: Vec<Item> = items
                        .into_iter()
                        .filter(|item| seen.insert(item.url.clone()))
                        .collect();
                    if !fresh.is_empty() {
                        sink(fresh);
                    }
                }
                Err(e) => {
                    log::warn!("{NAME}: '{query}' failed: {e}");
                    failed += 1;
                    last_error = Some(e);
                }
            }
        }
        // One search failing is thin coverage; every one failing is the network.
        if failed == QUERIES.len() {
            return Err(last_error.unwrap_or_else(|| anyhow::anyhow!("no search answered")));
        }
        Ok(())
    }
}

/// One query, a page at a time, until a short page or the page budget.
async fn walk(
    client: &reqwest::Client,
    query: &str,
    pages: usize,
    first_request: &mut bool,
) -> Result<Vec<Item>> {
    let mut all = Vec::new();
    for page in 0..pages {
        if !std::mem::take(first_request) {
            tokio::time::sleep(PAUSE).await;
        }
        let body = match get_page(client, query, page * PAGE).await {
            Ok(body) => body,
            // The first page failing is a real failure of this query; a later
            // one ends the walk with what came before it.
            Err(e) if page == 0 => return Err(e),
            Err(e) => {
                log::warn!("{NAME}: '{query}' page {page} failed: {e}");
                break;
            }
        };
        let (items, cards) = parse_cards(&body);
        all.extend(items);
        if cards < PAGE {
            break;
        }
    }
    Ok(all)
}

/// GETs one page of results, waiting out a 429.
async fn get_page(client: &reqwest::Client, query: &str, start: usize) -> Result<String> {
    let url = reqwest::Url::parse_with_params(
        SEARCH,
        [
            ("keywords", query),
            ("location", "Canada"),
            ("geoId", CANADA_GEO),
            ("f_TPR", POSTED_WITHIN),
            ("start", start.to_string().as_str()),
        ],
    )?;
    for attempt in 1..=RATE_LIMIT_TRIES {
        let response = client
            .get(url.clone())
            .header("Accept-Language", "en-US,en;q=0.9")
            .send()
            .await?;
        if response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
            if attempt < RATE_LIMIT_TRIES {
                tokio::time::sleep(RATE_LIMIT_WAIT * attempt).await;
                continue;
            }
            anyhow::bail!("rate limited");
        }
        return Ok(response.error_for_status()?.text().await?);
    }
    unreachable!("the last attempt returns or bails")
}

/// The software postings on one page, and how many cards the page held in
/// all — which is what says whether there is another page, since a page of
/// ten sales roles is still a full page.
fn parse_cards(html: &str) -> (Vec<Item>, usize) {
    let doc = Html::parse_document(html);
    let mut cards = 0;
    let mut items = Vec::new();
    for card in doc.select(&CARD) {
        cards += 1;
        if let Some(item) = to_item(card) {
            items.push(item);
        }
    }
    (items, cards)
}

fn text_of(card: ElementRef, sel: &Selector) -> Option<String> {
    let text = collapse_ws(&card.select(sel).next()?.text().collect::<String>());
    (!text.is_empty()).then_some(text)
}

fn to_item(card: ElementRef) -> Option<Item> {
    let id = card
        .value()
        .attr("data-entity-urn")
        .and_then(|urn| urn.rsplit(':').next())
        .filter(|id| !id.is_empty() && id.bytes().all(|b| b.is_ascii_digit()))
        .map(str::to_string)
        .or_else(|| {
            let href = card.select(&LINK).next()?.value().attr("href")?;
            job_id(href).map(str::to_string)
        })?;
    let title = text_of(card, &TITLE)?;
    if !is_software(&title) {
        return None;
    }
    let company = text_of(card, &COMPANY);
    let location = text_of(card, &LOCATION);
    let timestamp = card
        .select(&DATE)
        .next()
        .and_then(|t| t.value().attr("datetime"))
        .and_then(parse_date)
        // Every card measured carries a date; one that does not is treated as
        // the far end of the search window rather than as brand new.
        .unwrap_or_else(|| Utc::now() - chrono::Duration::days(30));

    let seniority = level(&title);
    let item_type = if seniority.as_deref() == Some("Intern") {
        ItemType::Internship
    } else {
        ItemType::Job
    };
    let content = [company.as_deref(), location.as_deref()]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" · ");

    let mut item = Item::new(title, NAME, item_type, canonical_url(&id))
        .with_content(content)
        .with_timestamp(timestamp)
        .with_location(location);
    if let Some(company) = company {
        item = item.with_company(company);
    }
    // Every row here passed `is_software`, which is a stricter test than the
    // keyword classifier would apply to a title alone.
    item.discipline = Some(DEFAULT_MAJOR.to_string());
    item.seniority = seniority;
    Some(item)
}

/// LinkedIn prints the day it was posted, `2026-11-14`.
fn parse_date(raw: &str) -> Option<chrono::DateTime<Utc>> {
    let date = NaiveDate::parse_from_str(raw.trim(), "%Y-%m-%d").ok()?;
    Some(Utc.from_utc_datetime(&date.and_hms_opt(12, 0, 0)?))
}

/// The stable form of a posting's address. A card links through whichever
/// country subdomain served the search, with a slug and three tracking
/// parameters that change on every request; left in, each refresh would look
/// like a new set of postings to a cache keyed by URL.
pub fn canonical_url(id: &str) -> String {
    format!("https://www.linkedin.com/jobs/view/{id}")
}

/// The posting id in a LinkedIn job link: `/jobs/view/{id}` or
/// `/jobs/view/{slug}-{id}`, on any LinkedIn host. `None` for anything that is
/// not a single posting.
pub fn job_id(url: &str) -> Option<&str> {
    let rest = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))?;
    let (host, path) = rest.split_once('/')?;
    if host != "linkedin.com" && !host.ends_with(".linkedin.com") {
        return None;
    }
    let path = path.split(['?', '#']).next()?;
    let segment = path.strip_prefix("jobs/view/")?.trim_end_matches('/');
    let id = segment.rsplit('-').next()?;
    (!id.is_empty() && !id.contains('/') && id.bytes().all(|b| b.is_ascii_digit())).then_some(id)
}

/// A title, lowercased, with every run of punctuation turned into one space
/// and a space either side, so a phrase can be matched as whole words:
/// "Back-End" reads as " back end ", and " sales " is not inside "Salesforce".
fn words(title: &str) -> String {
    let spaced: String = title
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect();
    format!(" {} ", collapse_ws(&spaced))
}

/// Phrases that make a title a software role. Some French, because a good
/// share of Canadian postings are Québécois and LinkedIn returns them for an
/// English query.
const SOFTWARE: &[&str] = &[
    " software ",
    " developer ",
    " developers ",
    " développeur ",
    " développeuse ",
    " developpeur ",
    " developpeuse ",
    " programmer ",
    " programmeur ",
    " programmeuse ",
    " logiciel ",
    " logiciels ",
    " full stack ",
    " fullstack ",
    " backend ",
    " back end ",
    " frontend ",
    " front end ",
    " web engineer",
    " mobile engineer",
    " ui engineer",
    " ios ",
    " android ",
    " sdet ",
    " devops ",
    " site reliability ",
    " sre ",
    " swe ",
    " machine learning engineer",
    " ml engineer",
    " ai engineer",
    " computer science ",
];

/// Phrases that make a title something else, whatever software words it also
/// carries: the software company's sales team, the person who recruits its
/// developers, the one who manages the product.
const NOT_SOFTWARE: &[&str] = &[
    " sales ",
    " account executive ",
    " account manager ",
    " marketing ",
    " recruiter ",
    " recruiting ",
    " talent acquisition ",
    " product manager ",
    " project manager ",
    " program manager ",
    " customer success ",
    " business development ",
    " business developer ",
    " d affaires ",
    " technician ",
    " technicien ",
    " technicienne ",
    " support specialist ",
    " implementation specialist ",
    " instructor ",
    " teacher ",
    " tutor ",
    " trainer ",
];

/// Whether a job title is a software role. Read on the title alone: it is all
/// a card carries, and it is what the reader sees.
pub fn is_software(title: &str) -> bool {
    let text = words(title);
    SOFTWARE.iter().any(|w| text.contains(w)) && !NOT_SOFTWARE.iter().any(|w| text.contains(w))
}

/// The level off the title, with the French words for an internship that
/// [`classify_seniority`] does not know.
fn level(title: &str) -> Option<String> {
    classify_seniority(title).or_else(|| {
        let text = words(title);
        [
            " stage ",
            " stagiaire ",
            " stagiaires ",
            " coop ",
            " coopératif ",
        ]
        .iter()
        .any(|w| text.contains(w))
        .then(|| "Intern".to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Datelike;

    /// One card as the endpoint serves it, trimmed of its tracking attributes.
    const PAGE_HTML: &str = r#"<!DOCTYPE html>
      <li>
        <div class="base-card relative w-full base-search-card job-search-card"
             data-entity-urn="urn:li:jobPosting:4455797536">
          <a class="base-card__full-link" href="https://ca.linkedin.com/jobs/view/software-engineer-intern-at-acme-4455797536?position=1&amp;pageNum=0&amp;refId=x&amp;trackingId=y">
            <span class="sr-only">Software Engineer Intern (Summer 2027)</span>
          </a>
          <div class="base-search-card__info">
            <h3 class="base-search-card__title">
              Software Engineer Intern (Summer 2027)
            </h3>
            <h4 class="base-search-card__subtitle">
              <a class="hidden-nested-link" href="https://www.linkedin.com/company/acme">Acme Robotics</a>
            </h4>
            <div class="base-search-card__metadata">
              <span class="job-search-card__location">Toronto, Ontario, Canada</span>
              <time class="job-search-card__listdate" datetime="2026-11-14">2 days ago</time>
            </div>
          </div>
        </div>
      </li>
      <li>
        <div class="base-card base-search-card job-search-card" data-entity-urn="urn:li:jobPosting:4400000001">
          <h3 class="base-search-card__title">Software Sales Representative</h3>
          <h4 class="base-search-card__subtitle">Acme Robotics</h4>
          <span class="job-search-card__location">Toronto, Ontario, Canada</span>
          <time class="job-search-card__listdate--new" datetime="2026-11-15">1 day ago</time>
        </div>
      </li>"#;

    #[test]
    fn a_card_becomes_a_posting_at_its_stable_address() {
        let (items, cards) = parse_cards(PAGE_HTML);
        assert_eq!(cards, 2, "both cards count towards the page being full");
        assert_eq!(items.len(), 1, "the sales role is not a software role");
        let item = &items[0];
        assert_eq!(item.url, "https://www.linkedin.com/jobs/view/4455797536");
        assert_eq!(item.title, "Software Engineer Intern (Summer 2027)");
        assert_eq!(item.company.as_deref(), Some("Acme Robotics"));
        assert_eq!(item.location.as_deref(), Some("Toronto, Ontario, Canada"));
        assert_eq!(item.item_type, ItemType::Internship);
        assert_eq!(item.seniority.as_deref(), Some("Intern"));
        assert_eq!(item.discipline.as_deref(), Some(DEFAULT_MAJOR));
        let d = item.timestamp;
        assert_eq!((d.year(), d.month(), d.day()), (2026, 11, 14));
        assert_eq!(
            item.content_text,
            "Acme Robotics · Toronto, Ontario, Canada"
        );
    }

    #[test]
    fn the_id_is_read_out_of_any_linkedin_job_link() {
        assert_eq!(
            job_id("https://ca.linkedin.com/jobs/view/software-engineer-at-acme-4455797536?position=1&refId=x"),
            Some("4455797536")
        );
        assert_eq!(
            job_id("https://www.linkedin.com/jobs/view/4455797536/"),
            Some("4455797536")
        );
        assert_eq!(job_id("https://www.linkedin.com/company/acme"), None);
        assert_eq!(
            job_id("https://www.linkedin.com/jobs/search?keywords=x"),
            None
        );
        assert_eq!(job_id("https://notlinkedin.com/jobs/view/123"), None);
        assert_eq!(
            job_id("https://www.linkedin.com/jobs/view/a-slug-only"),
            None
        );
    }

    #[test]
    fn software_titles_are_told_from_the_rest() {
        for title in [
            "Software Engineer Intern (Summer 2027)",
            "Développeur Full-Stack junior",
            "Stagiaire en développement de logiciels (été 2027)",
            "Back-End Developer Co-op",
            "iOS Engineer",
            "Senior Software Engineer-C#",
            "Salesforce Developer",
            "Applied AI Engineer Intern",
            "Computer Science Co-Op",
            "Programmer Analyst Intern (12 months)",
            "EverPro - Senior Web Engineer",
        ] {
            assert!(is_software(title), "{title} should be software");
        }
        for title in [
            "Software Sales Representative",
            "Mechanical Engineer",
            "Technical Recruiter, Software",
            "Software Product Manager",
            "Développeur d'affaires",
            "Business Developer",
            "Field Service Technician - Software Systems",
            "Cloud Engineer Intern",
        ] {
            assert!(!is_software(title), "{title} should not be software");
        }
    }

    #[test]
    fn french_internships_are_internships() {
        assert_eq!(
            level("Stagiaire, Développement logiciel (Hiver 2027)").as_deref(),
            Some("Intern")
        );
        assert_eq!(
            level("Stage coopératif - Hiver 2027: Développement").as_deref(),
            Some("Intern")
        );
        assert_eq!(level("Développeur logiciel"), None);
    }

    #[test]
    fn a_page_with_no_cards_ends_the_walk() {
        let (items, cards) = parse_cards("<!DOCTYPE html>");
        assert!(items.is_empty());
        assert_eq!(cards, 0);
    }
}

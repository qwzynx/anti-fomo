//! The two community internship repos (Pitt CSC and Simplify) publish their
//! listings as HTML `<table>` blocks inside README.md. Same shape, different
//! title convention, so one parser covers both.

use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use regex::Regex;
use scraper::{Html, Selector};
use std::sync::LazyLock;

use super::{collapse_ws, Scraper};
use crate::location::clean_location;
use crate::models::{Item, ItemType};

/// Rows are appended newest-first, so the first 150 are the freshest.
const MAX_ROWS: usize = 150;
/// Marks a row that reuses the company from the row above it.
const CONTINUATION: &str = "↳";

#[derive(Clone, Copy)]
pub enum TitleStyle {
    /// "Internship at {company}" — Pitt CSC
    CompanyOnly,
    /// "{role} at {company}" — Simplify
    RoleAtCompany,
}

pub struct GithubInternships {
    pub name: &'static str,
    pub url: &'static str,
    pub title_style: TitleStyle,
    pub item_type: ItemType,
}

pub const PITT_CSC: GithubInternships = GithubInternships {
    name: "Pitt CSC Repo",
    url: "https://raw.githubusercontent.com/pittcsc/Summer2026-Internships/dev/README.md",
    title_style: TitleStyle::CompanyOnly,
    item_type: ItemType::Internship,
};

pub const SIMPLIFY: GithubInternships = GithubInternships {
    name: "Simplify",
    url: "https://raw.githubusercontent.com/SimplifyJobs/Summer2026-Internships/dev/README.md",
    title_style: TitleStyle::RoleAtCompany,
    item_type: ItemType::Internship,
};

/// Same maintainers and the same table shape as [`SIMPLIFY`], but full-time
/// graduate roles rather than internships — the postings a final-year student
/// is actually applying to.
pub const NEW_GRAD: GithubInternships = GithubInternships {
    name: "New Grad Positions",
    url: "https://raw.githubusercontent.com/SimplifyJobs/New-Grad-Positions/dev/README.md",
    title_style: TitleStyle::RoleAtCompany,
    item_type: ItemType::Job,
};

/// The tables carry a fifth "Age" cell holding a relative age like `0d`, `13d`
/// or `1mo`. Parsing it matters: without it every row is stamped `Utc::now()`,
/// so a listing posted a month ago claims to be brand new and the recency term
/// lets these three large repos monopolise the top of the feed.
static AGE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(\d+)\s*(h|d|w|mo|y|yr)$").unwrap());

fn parse_age(raw: &str, now: DateTime<Utc>) -> Option<DateTime<Utc>> {
    let caps = AGE.captures(raw.trim())?;
    let n: i64 = caps[1].parse().ok()?;
    let span = match &caps[2] {
        "h" => Duration::hours(n),
        "d" => Duration::days(n),
        "w" => Duration::weeks(n),
        "mo" => Duration::days(n * 30),
        _ => Duration::days(n * 365),
    };
    Some(now - span)
}

static ROW: LazyLock<Selector> = LazyLock::new(|| Selector::parse("tr").unwrap());
static CELL: LazyLock<Selector> = LazyLock::new(|| Selector::parse("td").unwrap());
static LINK: LazyLock<Selector> = LazyLock::new(|| Selector::parse("a").unwrap());

#[async_trait]
impl Scraper for GithubInternships {
    fn source_name(&self) -> &'static str {
        self.name
    }

    async fn fetch(&self, client: &reqwest::Client) -> Result<Vec<Item>> {
        let body = client.get(self.url).send().await?.text().await?;
        let doc = Html::parse_document(&body);

        // One clock for the whole table so every row's age resolves against the
        // same instant.
        let now = Utc::now();
        let mut items: Vec<Item> = Vec::new();
        // Tracked separately from `items` because rows without an application
        // link are skipped, and a continuation row still refers to the company
        // of the row above it in the table, not the last row we kept.
        let mut last_company = String::new();

        for row in doc.select(&ROW).take(MAX_ROWS) {
            let cells: Vec<_> = row.select(&CELL).collect();
            if cells.len() < 4 {
                continue; // header row, or a table with a different shape
            }

            let mut company = match cells[0].select(&LINK).next() {
                Some(a) => collapse_ws(&a.text().collect::<String>()),
                None => collapse_ws(&cells[0].text().collect::<String>()),
            };
            if company == CONTINUATION {
                company = if last_company.is_empty() {
                    "Unknown".to_string()
                } else {
                    last_company.clone()
                };
            }
            last_company = company.clone();

            let role = collapse_ws(&cells[1].text().collect::<String>());
            let location = clean_location(&cells[2].inner_html());

            // The application cell holds the apply button; rows without one are
            // closed or placeholder listings.
            let Some(href) = cells[3]
                .select(&LINK)
                .next()
                .and_then(|a| a.value().attr("href"))
            else {
                continue;
            };

            let (title, content) = match self.title_style {
                TitleStyle::CompanyOnly => (
                    format!("Internship at {company}"),
                    format!("Role: {role} · Location: {location}"),
                ),
                TitleStyle::RoleAtCompany => (
                    format!("{role} at {company}"),
                    format!("Location: {location}"),
                ),
            };

            items.push(
                Item {
                    // These repos are software-internship-only, so skip the
                    // keyword classifier and label them directly.
                    discipline: Some("Software Engineering".to_string()),
                    ..Item::new(title, self.name, self.item_type, href)
                }
                .with_content(content)
                .with_timestamp(
                    cells
                        .get(4)
                        .map(|c| collapse_ws(&c.text().collect::<String>()))
                        .and_then(|age| parse_age(&age, now))
                        .unwrap_or(now),
                )
                .with_location(Some(location)),
            );
        }

        Ok(items)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_age_column_units() {
        let now = Utc::now();
        assert_eq!(parse_age("0d", now).unwrap(), now);
        assert_eq!(parse_age("13d", now).unwrap(), now - Duration::days(13));
        assert_eq!(parse_age("1mo", now).unwrap(), now - Duration::days(30));
        assert_eq!(parse_age(" 6h ", now).unwrap(), now - Duration::hours(6));
        assert_eq!(parse_age("2w", now).unwrap(), now - Duration::weeks(2));
    }

    #[test]
    fn unparseable_ages_fall_back_to_now() {
        let now = Utc::now();
        // The caller substitutes `now` on None; what matters is not guessing.
        assert!(parse_age("", now).is_none());
        assert!(parse_age("recently", now).is_none());
        assert!(parse_age("d", now).is_none());
    }

    #[test]
    fn an_old_listing_sorts_behind_a_fresh_one() {
        let now = Utc::now();
        assert!(parse_age("1mo", now).unwrap() < parse_age("2d", now).unwrap());
    }
}

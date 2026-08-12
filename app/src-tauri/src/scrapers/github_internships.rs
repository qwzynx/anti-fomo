//! The two community internship repos (Pitt CSC and Simplify) publish their
//! listings as HTML `<table>` blocks inside README.md. Same shape, different
//! title convention, so one parser covers both.

use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;
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
}

pub const PITT_CSC: GithubInternships = GithubInternships {
    name: "Pitt CSC Repo",
    url: "https://raw.githubusercontent.com/pittcsc/Summer2026-Internships/dev/README.md",
    title_style: TitleStyle::CompanyOnly,
};

pub const SIMPLIFY: GithubInternships = GithubInternships {
    name: "Simplify",
    url: "https://raw.githubusercontent.com/SimplifyJobs/Summer2026-Internships/dev/README.md",
    title_style: TitleStyle::RoleAtCompany,
};

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
                    ..Item::new(title, self.name, ItemType::Internship, href)
                }
                .with_content(content)
                .with_timestamp(Utc::now())
                .with_location(Some(location)),
            );
        }

        Ok(items)
    }
}

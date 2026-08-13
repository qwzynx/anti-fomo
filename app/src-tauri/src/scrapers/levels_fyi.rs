use anyhow::Result;
use async_trait::async_trait;
use chrono::{Datelike, Utc};
use serde::Deserialize;

use super::Scraper;
use crate::models::{Item, ItemType};

pub struct LevelsFyi;

const ENDPOINT: &str = "https://www.levels.fyi/js/internshipData.json";
/// The dataset carries ~14k historical postings, of which ~1.5k are open for
/// the current year. Internships are the category this app exists for, so this
/// takes a deep slice rather than the token 25 it used to — but not all of
/// them, because one source contributing 1,500 rows would dominate the cache
/// even with round-robin diversification protecting the first page.
const LIMIT: usize = 300;

#[derive(Deserialize)]
struct Posting {
    title: Option<String>,
    company: Option<String>,
    link: Option<String>,
    season: Option<String>,
    yr: Option<String>,
    loc: Option<String>,
    #[serde(rename = "monthlySalary")]
    monthly_salary: Option<serde_json::Value>,
    #[serde(rename = "appNotOpen")]
    app_not_open: Option<bool>,
}

#[async_trait]
impl Scraper for LevelsFyi {
    fn source_name(&self) -> &'static str {
        "Levels.fyi"
    }

    async fn fetch(&self, client: &reqwest::Client) -> Result<Vec<Item>> {
        let postings: Vec<Posting> = client.get(ENDPOINT).send().await?.json().await?;
        let current_year = Utc::now().year().to_string();

        Ok(postings
            .into_iter()
            // The dataset is historical; keep only current-year, still-open postings.
            .filter(|p| {
                p.yr.as_deref() == Some(current_year.as_str()) && !p.app_not_open.unwrap_or(false)
            })
            .take(LIMIT)
            .map(|p| {
                let salary = match &p.monthly_salary {
                    Some(v) if !v.is_null() => format!("${}/month", render_number(v)),
                    _ => "Compensation N/A".to_string(),
                };
                let loc = p.loc.clone();
                let content = format!(
                    "{} {} · {} · {}",
                    p.season.unwrap_or_default(),
                    p.yr.unwrap_or_default(),
                    loc.clone().unwrap_or_else(|| "Location N/A".to_string()),
                    salary
                );

                Item::new(
                    format!(
                        "{} at {}",
                        p.title.unwrap_or_else(|| "Intern".into()),
                        p.company.unwrap_or_else(|| "Unknown".into())
                    ),
                    self.source_name(),
                    ItemType::Internship,
                    p.link
                        .filter(|l| !l.is_empty())
                        .unwrap_or_else(|| "https://www.levels.fyi/internships/".to_string()),
                )
                .with_content(content)
                .with_location(loc)
            })
            .collect())
    }
}

/// The feed is inconsistent about whether salary is a JSON number or a string.
fn render_number(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

//! Hacker News "Ask HN: Who is hiring?".
//!
//! A monthly thread in which companies post openings as top-level comments,
//! nearly all of them in one shape: `Company | Role | Location | Terms`, then
//! a paragraph. Startups, mostly, and the roles that never reach a job board.
//! Read through the Algolia mirror (already this app's HN host), which gives
//! the comments as JSON — about 350 of them, so one request.
//!
//! Only comments whose first line follows the convention are kept. The rest
//! are free-form and cannot be read as a posting without guessing.

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::Deserialize;

use super::sections::Sections;
use super::{collapse_ws, details, strip_html, Scraper};
use crate::models::{Item, ItemType};

pub struct HnHiring;

const THREADS: &str =
    "https://hn.algolia.com/api/v1/search_by_date?tags=story,author_whoishiring&hitsPerPage=6";

#[derive(Deserialize)]
struct Search {
    #[serde(default)]
    hits: Vec<Hit>,
}

#[derive(Deserialize)]
struct Hit {
    #[serde(rename = "objectID")]
    object_id: String,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    comment_text: Option<String>,
    #[serde(default)]
    parent_id: Option<i64>,
    #[serde(default)]
    created_at: Option<String>,
}

#[derive(Deserialize)]
struct Comments {
    #[serde(default)]
    hits: Vec<Hit>,
}

#[async_trait]
impl Scraper for HnHiring {
    fn source_name(&self) -> &'static str {
        "HN Who's Hiring"
    }

    async fn fetch(&self, client: &reqwest::Client) -> Result<Vec<Item>> {
        let threads: Search = client.get(THREADS).send().await?.json().await?;
        let thread = threads
            .hits
            .iter()
            .find(|h| {
                h.title
                    .as_deref()
                    .is_some_and(|t| t.to_ascii_lowercase().contains("who is hiring"))
            })
            .ok_or_else(|| anyhow!("no 'Who is hiring' thread found"))?;
        let story: i64 = thread.object_id.parse()?;

        let url = format!(
            "https://hn.algolia.com/api/v1/search?tags=comment,story_{story}&hitsPerPage=1000"
        );
        let comments: Comments = client.get(&url).send().await?.json().await?;
        Ok(comments
            .hits
            .into_iter()
            .filter(|h| h.parent_id == Some(story))
            .filter_map(to_item)
            .collect())
    }
}

/// "Common Prefix | Senior Software Engineer | Athens, Greece / REMOTE | Full-time"
/// → company, role, the rest. `None` when the first line is not in that shape.
fn parse_header(first_line: &str) -> Option<(String, String, String)> {
    let parts: Vec<String> = first_line
        .split('|')
        .map(collapse_ws)
        .filter(|p| !p.is_empty())
        .collect();
    if parts.len() < 3 {
        return None;
    }
    let company = parts[0].clone();
    // A header this long is a paragraph that happens to contain a pipe.
    if company.len() > 60 || parts[1].len() > 100 {
        return None;
    }
    Some((company, parts[1].clone(), parts[2..].join(" · ")))
}

fn to_item(hit: Hit) -> Option<Item> {
    let html = hit.comment_text?;
    let first_line = strip_html(html.split("<p>").next().unwrap_or(""));
    let (company, role, rest) = parse_header(&first_line)?;
    let timestamp = hit
        .created_at
        .as_deref()
        .and_then(|d| DateTime::parse_from_rfc3339(d).ok())
        .map(|d| d.with_timezone(&Utc))
        .unwrap_or_else(Utc::now);

    // The place is whichever trailing segments are not terms of employment.
    let location: String = rest
        .split(" · ")
        .filter(|s| {
            let l = s.to_ascii_lowercase();
            !matches!(
                l.as_str(),
                "full-time" | "full time" | "part-time" | "contract" | "internship" | "equity"
            ) && !l.starts_with('$')
        })
        .take(2)
        .collect::<Vec<_>>()
        .join(", ");
    let location = (!location.is_empty()).then_some(location);

    let mut item = Item::new(
        role.as_str(),
        "HN Who's Hiring",
        if role.to_ascii_lowercase().contains("intern") {
            ItemType::Internship
        } else {
            ItemType::Job
        },
        format!("https://news.ycombinator.com/item?id={}", hit.object_id),
    )
    .with_company(company.as_str())
    .with_location(location)
    .with_timestamp(timestamp)
    .with_content(format!("{company} · {rest}"));

    // The whole comment is the posting; there is nowhere else to read it.
    let body = collapse_ws(&strip_html(&html.replace("<p>", "\n<p>")));
    let mut parts = Sections::default();
    parts.overview.push(body);
    super::ats::seed_detail(&mut item, details::detail_of(parts, Vec::new()));
    Some(item)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn comment(text: &str) -> Hit {
        Hit {
            object_id: "50488627".into(),
            title: None,
            comment_text: Some(text.into()),
            parent_id: Some(1),
            created_at: Some("2026-11-13T19:37:57Z".into()),
        }
    }

    #[test]
    fn the_conventional_header_becomes_a_posting() {
        let item = to_item(comment(
            "Common Prefix | Senior Software Engineer | Athens, Greece &#x2F; REMOTE | Full-time | $80k to $160k<p>We&#x27;re hiring!",
        ))
        .unwrap();
        assert_eq!(item.title, "Senior Software Engineer");
        assert_eq!(item.company.as_deref(), Some("Common Prefix"));
        assert_eq!(item.item_type, ItemType::Job);
        assert_eq!(item.location.as_deref(), Some("Athens, Greece / REMOTE"));
        assert_eq!(item.url, "https://news.ycombinator.com/item?id=50488627");
        assert!(item
            .description
            .as_deref()
            .is_some_and(|d| d.contains("hiring")));
    }

    #[test]
    fn an_intern_role_is_an_internship() {
        let item = to_item(comment(
            "Acme | Software Engineering Intern | Toronto | Internship",
        ))
        .unwrap();
        assert_eq!(item.item_type, ItemType::Internship);
    }

    #[test]
    fn free_form_comments_are_not_guessed_at() {
        assert!(to_item(comment("We are hiring engineers, email me!")).is_none());
        assert!(to_item(comment("Acme | Engineer")).is_none());
    }
}

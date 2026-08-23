//! Reading a posting out of a career page's schema.org metadata.
//!
//! Every ATS wants its postings in Google's job search, and the price of entry
//! is a `JobPosting` object in a `<script type="application/ld+json">` tag. So
//! the pages that hand the browser nothing but a JavaScript shell still hand
//! *us* the full description, in a documented shape, on the employer's own
//! domain.
//!
//! Measured over one posting from each of the 60 hosts in a full cache, this
//! alone serves iCIMS (the whole `careers-*.icims.com` family plus the
//! `?icims=1` career sites), Ashby, Workday and Microsoft — about a fifth of
//! the corpus that no dedicated handler covers. It is the reason
//! [`super::details`] does not need one module per employer.
//!
//! It is not a generic readability fallback, and the distinction matters: this
//! reads a field a machine wrote for machines. A page without the tag yields
//! nothing rather than yielding its navigation menu.

use chrono::{DateTime, NaiveDate, Utc};
use scraper::{Html, Selector};
use serde_json::Value;
use std::sync::LazyLock;

use crate::pay::{self, Pay};

/// The fields of `JobPosting` worth having. All optional — most sites fill
/// `description` and nothing else, and a few fill the rest properly.
///
/// Not `Eq`: `baseSalary` carries floats.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct JobPostingLd {
    /// HTML, near enough always: the spec says text, and every ATS puts markup
    /// in it anyway.
    pub description: Option<String>,
    pub responsibilities: Option<String>,
    /// `qualifications`, `experienceRequirements` and `educationRequirements`
    /// joined, because they are the same section to a reader.
    pub qualifications: Option<String>,
    pub benefits: Option<String>,
    /// `skills`, which a handful of sites fill with a real list.
    pub skills: Vec<String>,
    /// `hiringOrganization.name` — the employer, stated rather than parsed out
    /// of a title.
    pub company: Option<String>,
    /// `validThrough`, the application deadline. This one field is why the
    /// deadline coverage extends past the handful of ATSs with a dedicated
    /// handler: every page the `Page` handler already reads can carry it.
    pub valid_through: Option<DateTime<Utc>>,
    /// `baseSalary`, as a structured range rather than a sentence.
    pub salary: Option<Pay>,
}

impl JobPostingLd {
    /// True when the block told us nothing. Deliberately ignores the facts
    /// below the text fields: a page carrying only a `validThrough` has still
    /// not said what the job is, so it must not end the handler chain — but
    /// `has_facts` on the resulting `JobDetail` keeps the date.
    pub fn is_empty(&self) -> bool {
        self.description.is_none()
            && self.responsibilities.is_none()
            && self.qualifications.is_none()
            && self.benefits.is_none()
            && self.skills.is_empty()
    }

    /// True when nothing at all came back, text or facts.
    pub fn is_blank(&self) -> bool {
        self.is_empty()
            && self.company.is_none()
            && self.valid_through.is_none()
            && self.salary.is_none()
    }
}

static LD_JSON: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse(r#"script[type="application/ld+json"]"#).unwrap());

/// Finds the first `JobPosting` in a page's linked-data blocks.
pub fn find(html: &str) -> Option<JobPostingLd> {
    let doc = Html::parse_document(html);
    for script in doc.select(&LD_JSON) {
        let raw = script.text().collect::<String>();
        let Ok(value) = serde_json::from_str::<Value>(&raw) else {
            continue;
        };
        if let Some(node) = find_posting(&value) {
            let posting = read(node);
            if !posting.is_blank() {
                return Some(posting);
            }
        }
    }
    None
}

/// A block may be the posting, a list of objects, or a `@graph` of them —
/// all three are in the wild, so this recurses rather than assuming one.
fn find_posting(value: &Value) -> Option<&Value> {
    match value {
        Value::Array(items) => items.iter().find_map(find_posting),
        Value::Object(map) => {
            if map
                .get("@type")
                .is_some_and(|t| type_names(t).iter().any(|n| n == "JobPosting"))
            {
                return Some(value);
            }
            map.get("@graph").and_then(find_posting)
        }
        _ => None,
    }
}

/// `@type` is a string or a list of them.
fn type_names(value: &Value) -> Vec<String> {
    match value {
        Value::String(s) => vec![s.clone()],
        Value::Array(items) => items
            .iter()
            .filter_map(|i| i.as_str().map(str::to_string))
            .collect(),
        _ => Vec::new(),
    }
}

fn read(node: &Value) -> JobPostingLd {
    JobPostingLd {
        description: text_of(node.get("description")),
        responsibilities: text_of(node.get("responsibilities")),
        qualifications: join(&[
            text_of(node.get("qualifications")),
            text_of(node.get("experienceRequirements")),
            text_of(node.get("educationRequirements")),
        ]),
        benefits: join(&[
            text_of(node.get("jobBenefits")),
            text_of(node.get("benefits")),
        ]),
        skills: list_of(node.get("skills")),
        company: node
            .get("hiringOrganization")
            .and_then(|o| text_of(o.get("name")).or_else(|| text_of(Some(o))))
            .filter(|n| !n.trim().is_empty()),
        valid_through: text_of(node.get("validThrough")).as_deref().and_then(parse_date),
        salary: node.get("baseSalary").and_then(base_salary),
    }
}

/// schema.org dates come as a full RFC 3339 timestamp or a bare `YYYY-MM-DD`,
/// and both are common. A bare date is read as end-of-day UTC: a deadline of
/// "2026-09-18" means the 18th is still open.
fn parse_date(raw: &str) -> Option<DateTime<Utc>> {
    let raw = raw.trim();
    if let Ok(dt) = DateTime::parse_from_rfc3339(raw) {
        return Some(dt.with_timezone(&Utc));
    }
    NaiveDate::parse_from_str(raw, "%Y-%m-%d")
        .ok()?
        .and_hms_opt(23, 59, 59)
        .map(|naive| naive.and_utc())
}

/// `baseSalary` is a `MonetaryAmount` whose `value` is either a
/// `QuantitativeValue` with `minValue`/`maxValue`, or a bare number.
fn base_salary(node: &Value) -> Option<Pay> {
    let currency = node
        .get("currency")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let value = node.get("value")?;

    let (min, max, unit) = match value {
        Value::Object(map) => (
            map.get("minValue").and_then(number),
            map.get("maxValue").and_then(number),
            map.get("unitText").and_then(|v| v.as_str()).map(str::to_string),
        ),
        other => (number(other), None, None),
    };
    // A single figure with no range still reads as a min; `from_parts` decides
    // whether it normalises to a believable wage.
    let min = min.or_else(|| value.get("value").and_then(number));
    pay::from_parts(min, max, currency, unit)
}

fn number(value: &Value) -> Option<f64> {
    match value {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.replace(',', "").trim().parse().ok(),
        _ => None,
    }
}

/// A schema.org text field, which may be a string, a list of strings, or a
/// nested object stating the same thing in a typed wrapper
/// (`OccupationalExperienceRequirements` is the common one).
fn text_of(value: Option<&Value>) -> Option<String> {
    let value = value?;
    let text = match value {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Array(items) => items
            .iter()
            .filter_map(|i| text_of(Some(i)))
            .collect::<Vec<_>>()
            .join("\n"),
        Value::Object(map) => ["description", "name", "value", "text"]
            .iter()
            .find_map(|key| map.get(*key).and_then(|v| text_of(Some(v))))
            .unwrap_or_default(),
        _ => String::new(),
    };
    (!text.trim().is_empty()).then_some(text)
}

fn join(parts: &[Option<String>]) -> Option<String> {
    let joined = parts
        .iter()
        .flatten()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .join("\n");
    (!joined.trim().is_empty()).then_some(joined)
}

/// `skills` is a comma-separated string as often as it is a list.
fn list_of(value: Option<&Value>) -> Vec<String> {
    let Some(text) = text_of(value) else {
        return Vec::new();
    };
    text.split(['\n', ','])
        .map(str::trim)
        .filter(|s| !s.is_empty() && s.len() < 40)
        .map(str::to_string)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page(body: &str) -> String {
        format!(r#"<html><head><script type="application/ld+json">{body}</script></head><body>shell</body></html>"#)
    }

    #[test]
    fn reads_a_plain_job_posting() {
        let found = find(&page(
            r#"{"@context":"https://schema.org","@type":"JobPosting",
                "title":"Intern","description":"<p>Build things</p>",
                "skills":"Python, SQL"}"#,
        ))
        .unwrap();
        assert_eq!(found.description.as_deref(), Some("<p>Build things</p>"));
        assert_eq!(found.skills, ["Python", "SQL"]);
    }

    #[test]
    fn finds_the_posting_inside_a_graph_or_a_list() {
        let graph = page(
            r#"{"@graph":[{"@type":"Organization","name":"Acme"},
                          {"@type":"JobPosting","description":"In the graph"}]}"#,
        );
        assert_eq!(find(&graph).unwrap().description.as_deref(), Some("In the graph"));

        let list = page(r#"[{"@type":"WebPage"},{"@type":"JobPosting","description":"In the list"}]"#);
        assert_eq!(find(&list).unwrap().description.as_deref(), Some("In the list"));
    }

    #[test]
    fn a_typed_requirements_wrapper_is_unwrapped() {
        let found = find(&page(
            r#"{"@type":"JobPosting","description":"d",
                "experienceRequirements":{"@type":"OccupationalExperienceRequirements",
                                          "description":"Two years of Rust"},
                "educationRequirements":"Bachelor's degree"}"#,
        ))
        .unwrap();
        assert_eq!(
            found.qualifications.as_deref(),
            Some("Two years of Rust\nBachelor's degree")
        );
    }

    #[test]
    fn a_page_without_the_tag_yields_nothing() {
        assert!(find("<html><body>Just a page</body></html>").is_none());
        assert!(find(&page(r#"{"@type":"Organization","name":"Acme"}"#)).is_none());
    }

    #[test]
    fn a_malformed_block_does_not_hide_a_later_good_one() {
        let html = r#"<html><head><script type="application/ld+json">{not json</script>
            <script type="application/ld+json">{"@type":"JobPosting","description":"Second"}</script></head></html>"#;
        assert_eq!(find(html).unwrap().description.as_deref(), Some("Second"));
    }

    #[test]
    fn a_type_list_still_counts_as_a_posting() {
        let found = find(&page(r#"{"@type":["JobPosting","Thing"],"description":"Listed type"}"#));
        assert_eq!(found.unwrap().description.as_deref(), Some("Listed type"));
    }
}

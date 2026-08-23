//! Reading published compensation out of whatever shape the source used.
//!
//! Only what an employer actually published ever lands here. Nothing is
//! estimated from a role and a city: a guess and a disclosure look identical
//! once they are both a range on a card, and a sort built on the mixture is
//! not something the reader can act on. A posting that does not say what it
//! pays reports no pay, and the UI says "not disclosed".
//!
//! Pure, like `location` and `skills` — no network, no database.

use regex::Regex;
use std::sync::OnceLock;

/// A published range. `min` and `max` are equal for a single figure, and
/// either can stand alone when the posting only gave one bound.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Pay {
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub currency: Option<String>,
    pub period: Option<String>,
}

impl Pay {
    pub fn is_empty(&self) -> bool {
        self.min.is_none() && self.max.is_none()
    }
}

/// Hours in a work year. The conventional 40 × 52, which is what employers
/// posting an hourly rate for a co-op term are quoting against.
const HOURS_PER_YEAR: f64 = 2080.0;
const WEEKS_PER_YEAR: f64 = 52.0;
const MONTHS_PER_YEAR: f64 = 12.0;
const DAYS_PER_YEAR: f64 = 260.0;

/// Below this, a number denominated in years is not a salary — it is a stray
/// figure that happened to sit next to a dollar sign.
const MIN_ANNUAL: f64 = 10_000.0;
/// And above this it is a contract value or an equity grant, not a wage.
const MAX_ANNUAL: f64 = 2_000_000.0;

/// Normalises a range to what it would pay over a year, so an hourly co-op
/// rate and a salaried new-grad offer can be sorted against each other. Uses
/// the midpoint of a range, which is the figure a reader compares.
pub fn annual_equivalent(pay: &Pay) -> Option<f64> {
    let value = match (pay.min, pay.max) {
        (Some(lo), Some(hi)) => (lo + hi) / 2.0,
        (Some(v), None) | (None, Some(v)) => v,
        (None, None) => return None,
    };
    let multiplier = match pay.period.as_deref() {
        Some("hour") => HOURS_PER_YEAR,
        Some("day") => DAYS_PER_YEAR,
        Some("week") => WEEKS_PER_YEAR,
        Some("month") => MONTHS_PER_YEAR,
        // Already annual. Explicit rather than falling through to the
        // inference arm below, which would read a $500 annual stipend as an
        // hourly rate and turn it into a million-dollar job.
        Some("year") => 1.0,
        // No period given: infer from magnitude. A bare "85,000" is a salary
        // and a bare "32.50" is an hourly rate, and pretending otherwise
        // produces a $32-a-year job at the bottom of every sort.
        _ => {
            if value < 1_000.0 {
                HOURS_PER_YEAR
            } else {
                1.0
            }
        }
    };
    let annual = value * multiplier;
    (MIN_ANNUAL..=MAX_ANNUAL).contains(&annual).then_some(annual)
}

/// Maps whatever the source called the period onto our five labels.
pub fn normalize_period(raw: &str) -> Option<String> {
    let s = raw.to_lowercase();
    let period = if s.contains("hour") || s.contains("hourly") || s == "hr" || s.contains("/h") {
        "hour"
    } else if s.contains("day") || s.contains("daily") || s.contains("diem") {
        "day"
    } else if s.contains("week") {
        "week"
    } else if s.contains("month") || s.contains("mo") {
        "month"
    } else if s.contains("year") || s.contains("annual") || s.contains("annum") || s.contains("yr") {
        "year"
    } else {
        return None;
    };
    Some(period.to_string())
}

fn currency_of(text: &str) -> Option<String> {
    let upper = text.to_uppercase();
    // Order matters: "CAD" and "USD" are unambiguous, a bare "$" is not, so
    // the explicit codes are checked first and the symbol only decides when
    // nothing else in the string does.
    for code in ["CAD", "USD", "EUR", "GBP", "AUD", "INR", "CHF", "SGD"] {
        if upper.contains(code) {
            return Some(code.to_string());
        }
    }
    if text.contains('£') {
        return Some("GBP".to_string());
    }
    if text.contains('€') {
        return Some("EUR".to_string());
    }
    text.contains('$').then(|| "USD".to_string())
}

fn amount_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    // A money-looking number: an optional symbol, digits with optional group
    // separators and decimals, and an optional k/K suffix ("$120k").
    RE.get_or_init(|| Regex::new(r"(?i)[$€£]?\s?(\d{1,3}(?:,\d{3})+(?:\.\d+)?|\d+(?:\.\d+)?)\s*(k\b)?").unwrap())
}

fn parse_amount(digits: &str, k_suffix: bool) -> Option<f64> {
    let n: f64 = digits.replace(',', "").parse().ok()?;
    Some(if k_suffix { n * 1_000.0 } else { n })
}

/// Reads a range out of free text — a Job Bank salary cell, a pay-transparency
/// paragraph, an ATS field that is a string rather than a number.
///
/// Returns `None` rather than a half-answer when the text has no money in it,
/// which is the common case: most postings say nothing about pay at all.
pub fn parse(text: &str) -> Option<Pay> {
    // Only look at text that claims to be about money. Without this the first
    // two numbers in "5,000 employees across 30 countries" become a salary
    // band.
    let lower = text.to_lowercase();
    let mentions_money = text.contains('$')
        || text.contains('€')
        || text.contains('£')
        || ["salary", "compensation", "pay range", "wage", "per hour", "hourly", "annually", "per year"]
            .iter()
            .any(|k| lower.contains(k));
    if !mentions_money {
        return None;
    }

    let mut amounts: Vec<f64> = Vec::new();
    for caps in amount_re().captures_iter(text) {
        let whole = caps.get(0).map(|m| m.as_str()).unwrap_or_default();
        // Require some indication this number is money: a symbol on it, a
        // thousands separator, or a k suffix. A bare "2026" in "Summer 2026"
        // is not a wage.
        let has_symbol = whole.contains('$') || whole.contains('€') || whole.contains('£');
        let digits = caps.get(1).map(|m| m.as_str()).unwrap_or_default();
        let k = caps.get(2).is_some();
        if !has_symbol && !digits.contains(',') && !k {
            continue;
        }
        if let Some(v) = parse_amount(digits, k) {
            amounts.push(v);
        }
        if amounts.len() >= 4 {
            break;
        }
    }
    amounts.retain(|v| *v > 0.0);
    if amounts.is_empty() {
        return None;
    }

    let period = normalize_period(text);
    let (min, max) = if amounts.len() >= 2 && amounts[1] >= amounts[0] {
        (Some(amounts[0]), Some(amounts[1]))
    } else {
        (Some(amounts[0]), None)
    };

    let pay = Pay {
        min,
        max,
        currency: currency_of(text),
        period,
    };
    // A range that does not normalise to a believable annual figure is a
    // misread, not a low-paying job — drop it rather than sorting on it.
    annual_equivalent(&pay).is_some().then_some(pay)
}

/// Builds a range from numeric fields an API already separated for us, which
/// is always better than re-reading them out of prose.
pub fn from_parts(
    min: Option<f64>,
    max: Option<f64>,
    currency: Option<String>,
    period: Option<String>,
) -> Option<Pay> {
    let pay = Pay {
        min: min.filter(|v| *v > 0.0),
        max: max.filter(|v| *v > 0.0),
        currency,
        period: period.as_deref().and_then(normalize_period),
    };
    if pay.is_empty() {
        return None;
    }
    annual_equivalent(&pay).is_some().then_some(pay)
}

/// How the range reads on a card. `None` when there is nothing to show, so the
/// caller renders "not disclosed" rather than an empty chip.
pub fn format(pay: &Pay) -> Option<String> {
    let unit = match pay.period.as_deref() {
        Some("hour") => "/hr",
        Some("day") => "/day",
        Some("week") => "/wk",
        Some("month") => "/mo",
        Some("year") => "/yr",
        _ => "",
    };
    let money = |v: f64| -> String {
        if v >= 10_000.0 {
            format!("{}k", (v / 1000.0).round() as i64)
        } else if v.fract() == 0.0 {
            format!("{}", v as i64)
        } else {
            format!("{v:.2}")
        }
    };
    let currency = pay.currency.as_deref().unwrap_or("USD");
    let symbol = match currency {
        "GBP" => "£",
        "EUR" => "€",
        _ => "$",
    };
    let body = match (pay.min, pay.max) {
        (Some(lo), Some(hi)) if (hi - lo).abs() > f64::EPSILON => {
            format!("{symbol}{}–{symbol}{}", money(lo), money(hi))
        }
        (Some(v), _) | (None, Some(v)) => format!("{symbol}{}", money(v)),
        (None, None) => return None,
    };
    let suffix = if currency == "CAD" { " CAD" } else { "" };
    Some(format!("{body}{unit}{suffix}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_an_annual_range() {
        let pay = parse("The base salary range is $85,000 - $110,000 per year.").unwrap();
        assert_eq!(pay.min, Some(85_000.0));
        assert_eq!(pay.max, Some(110_000.0));
        assert_eq!(pay.period.as_deref(), Some("year"));
        assert_eq!(annual_equivalent(&pay), Some(97_500.0));
    }

    #[test]
    fn reads_an_hourly_rate() {
        let pay = parse("$32.50 hourly").unwrap();
        assert_eq!(pay.min, Some(32.50));
        assert_eq!(pay.period.as_deref(), Some("hour"));
        assert_eq!(annual_equivalent(&pay), Some(32.50 * 2080.0));
    }

    #[test]
    fn reads_a_k_suffix() {
        let pay = parse("Compensation: $120k–$160k").unwrap();
        assert_eq!(pay.min, Some(120_000.0));
        assert_eq!(pay.max, Some(160_000.0));
    }

    #[test]
    fn keeps_the_currency_the_posting_named() {
        let pay = parse("CAD $75,000 to $95,000 annually").unwrap();
        assert_eq!(pay.currency.as_deref(), Some("CAD"));
    }

    #[test]
    fn ignores_text_with_no_money_in_it() {
        assert!(parse("Join a team of 5,000 engineers in 30 countries").is_none());
        assert!(parse("Summer 2026 internship, apply by 2026").is_none());
    }

    #[test]
    fn rejects_figures_that_are_not_wages() {
        // An equity grant or a contract value, not a salary.
        assert!(parse("salary competitive; the fund manages $4,000,000,000").is_none());
        // And a number too small to be anyone's annual pay.
        assert!(parse("a $500 annual learning stipend").is_none());
    }

    #[test]
    fn infers_the_period_from_magnitude_when_unstated() {
        let hourly = Pay { min: Some(30.0), max: None, currency: None, period: None };
        assert_eq!(annual_equivalent(&hourly), Some(62_400.0));
        let salaried = Pay { min: Some(90_000.0), max: None, currency: None, period: None };
        assert_eq!(annual_equivalent(&salaried), Some(90_000.0));
    }

    #[test]
    fn from_parts_rejects_an_empty_range() {
        assert!(from_parts(None, None, Some("USD".into()), Some("year".into())).is_none());
        assert!(from_parts(Some(0.0), None, None, None).is_none());
    }

    #[test]
    fn formats_for_a_card() {
        let pay = Pay {
            min: Some(85_000.0),
            max: Some(110_000.0),
            currency: Some("CAD".into()),
            period: Some("year".into()),
        };
        assert_eq!(format(&pay).as_deref(), Some("$85k–$110k/yr CAD"));
        let hourly = Pay {
            min: Some(32.5),
            max: None,
            currency: Some("USD".into()),
            period: Some("hour".into()),
        };
        assert_eq!(format(&hourly).as_deref(), Some("$32.50/hr"));
    }
}

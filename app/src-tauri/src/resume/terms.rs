//! What a posting keeps coming back to, and how much a résumé bullet echoes it.
//!
//! The catalog in [`crate::skills`] is the shared vocabulary between a posting
//! and a bullet, and it is deliberately small — about a hundred and fifty named
//! technologies. A posting says far more than that. "On-call rotation",
//! "latency budget", "design reviews", "customer-facing", "experimentation":
//! none of them are catalog skills, all of them are the thing the job is
//! actually about, and a bullet that speaks to one of them is the bullet that
//! should survive the page budget.
//!
//! So this is the second half of the match: a bag of the terms the posting
//! leans on, weighted by where and how often it said them, against the same
//! bag built from a bullet. No corpus and no model — the weighting comes from
//! the posting's own repetition and from which section a term appeared in,
//! which is information we already have and a generic IDF table would not.
//!
//! Two decisions carry most of the accuracy:
//!
//! * **Bigrams count double.** "Machine learning" and "unit testing" mean
//!   something; "machine" and "learning" separately are almost noise, and a
//!   unigram-only overlap ranks a bullet about washing machines against an ML
//!   posting.
//! * **Everything is stemmed.** A posting asks for "distributed systems" and a
//!   bullet says "distributed system"; without a stem those are two terms and
//!   the match is silently zero.

use std::collections::{HashMap, HashSet};

/// Terms that carry no information about *this* posting because they appear in
/// every posting. Ordinary English function words, plus the job-ad boilerplate
/// that would otherwise be the highest-weighted terms on the page — "team",
/// "experience", "work" and "role" are in nearly every bullet on both sides.
const STOPWORDS: &[&str] = &[
    "the",
    "and",
    "for",
    "with",
    "you",
    "your",
    "our",
    "are",
    "will",
    "that",
    "this",
    "from",
    "have",
    "has",
    "not",
    "but",
    "all",
    "any",
    "can",
    "who",
    "how",
    "what",
    "when",
    "where",
    "who",
    "why",
    "its",
    "their",
    "them",
    "they",
    "there",
    "here",
    "into",
    "over",
    "under",
    "such",
    "than",
    "then",
    "each",
    "other",
    "some",
    "more",
    "most",
    "also",
    "been",
    "being",
    "was",
    "were",
    "his",
    "her",
    "him",
    "she",
    "hers",
    "get",
    "got",
    "make",
    "made",
    "made",
    "use",
    "used",
    "using",
    "well",
    "very",
    "just",
    "own",
    "out",
    "off",
    "about",
    "across",
    "within",
    "while",
    "during",
    "per",
    "via",
    "etc",
    // Job-ad boilerplate.
    "work",
    "working",
    "team",
    "teams",
    "role",
    "roles",
    "job",
    "jobs",
    "position",
    "company",
    "companies",
    "candidate",
    "candidates",
    "applicant",
    "applicants",
    "opportunity",
    "opportunities",
    "experience",
    "experiences",
    "skill",
    "skills",
    "ability",
    "abilities",
    "able",
    "strong",
    "excellent",
    "great",
    "good",
    "best",
    "new",
    "help",
    "helping",
    "join",
    "joining",
    "look",
    "looking",
    "seek",
    "seeking",
    "want",
    "wanting",
    "need",
    "needs",
    "must",
    "should",
    "would",
    "could",
    "may",
    "might",
    "requirement",
    "requirements",
    "required",
    "qualification",
    "qualifications",
    "preferred",
    "responsibility",
    "responsibilities",
    "duty",
    "duties",
    "including",
    "include",
    "includes",
    "such",
    "plus",
    "bonus",
    "year",
    "years",
    "month",
    "months",
    "day",
    "days",
    "week",
    "weeks",
    "time",
    "full",
    "part",
    "level",
    "entry",
    "intern",
    "internship",
    "student",
    "students",
    "degree",
    "university",
    "college",
    "school",
    "program",
    "programs",
    "environment",
    "environments",
    "world",
    "class",
    "person",
    "people",
    "member",
    "members",
    "group",
    "groups",
    "organization",
    "organisation",
    "business",
    "value",
    "values",
    "culture",
    "diverse",
    "diversity",
    "inclusive",
    "inclusion",
    "equal",
    "employer",
    "benefit",
    "benefits",
    "salary",
    "compensation",
    "apply",
    "application",
    "applications",
    "please",
    "email",
    "resume",
    "cover",
    "letter",
    "contact",
    "office",
    "onsite",
    "hybrid",
    // The job-ad clichés. Every posting says them and half of every résumé
    // says them back, so a match on one is a match on the genre, not the job.
    "fast",
    "pace",
    "paced",
    "dynamic",
    "exciting",
    "innovative",
    "cutting",
    "edge",
    "passion",
    "passionate",
    "motivated",
    "driven",
    "detail",
    "oriented",
    "self",
    "starter",
    "eager",
    "enthusiastic",
    "hire",
    "hiring",
    "career",
    "success",
    "successful",
    "collaborative",
    "collaborate",
    "communication",
    "communicate",
];

/// The stem of one word, or `None` when the word carries nothing.
///
/// A deliberately small suffix stripper rather than a real Porter stemmer:
/// everything it does is reversible in the reader's head, which matters
/// because the terms it produces are shown to the user. The one rule that
/// earns its complexity is the `-ing`/`-ed` pair, which is the difference
/// between "shipped a service" and "shipping services" matching or not.
fn stem(word: &str) -> Option<String> {
    if word.len() < 3 || word.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    if STOPWORDS.contains(&word) {
        return None;
    }

    let cut = |w: &str, n: usize, floor: usize| -> Option<String> {
        (w.len() >= floor + n).then(|| collapse_double(&w[..w.len() - n]))
    };

    let stemmed = if let Some(s) = word.strip_suffix("ing").and_then(|_| cut(word, 3, 4)) {
        s
    } else if let Some(s) = word.strip_suffix("ed").and_then(|_| cut(word, 2, 4)) {
        s
    } else if word.ends_with("ly") {
        cut(word, 2, 4).unwrap_or_else(|| word.to_string())
    } else if word.ends_with('s') && !word.ends_with("ss") && !word.ends_with("us") {
        // "services" → "service"; then "processe" → "process", so the plural
        // and the singular of an -s/-x/-z/-h word land on the same stem.
        let base = &word[..word.len() - 1];
        match base.len() >= 4 && base.ends_with('e') && ends_sibilant(&base[..base.len() - 1]) {
            true => base[..base.len() - 1].to_string(),
            false => base.to_string(),
        }
    } else {
        word.to_string()
    };

    (stemmed.len() >= 3 && !STOPWORDS.contains(&stemmed.as_str())).then_some(stemmed)
}

fn ends_sibilant(s: &str) -> bool {
    matches!(s.bytes().last(), Some(b's' | b'x' | b'z' | b'h'))
}

/// "shipp" → "ship". Cutting `-ing` off a doubled consonant leaves a stem no
/// other form of the word reaches.
fn collapse_double(s: &str) -> String {
    let b = s.as_bytes();
    let doubled = b.len() >= 3
        && b[b.len() - 1] == b[b.len() - 2]
        && b[b.len() - 1].is_ascii_alphabetic()
        && !matches!(
            b[b.len() - 1],
            b'a' | b'e' | b'i' | b'o' | b'u' | b's' | b'l'
        );
    match doubled {
        true => s[..s.len() - 1].to_string(),
        false => s.to_string(),
    }
}

/// One word of a posting or a bullet.
struct Word {
    stem: String,
    /// What was actually written, which is the only form worth showing.
    surface: String,
    /// A clause ended after this word, so it forms no pair with the next one.
    /// Without this, "latency budgets. You have run an on-call rotation" makes
    /// the phrase "budgets run", which is not a phrase.
    ends_clause: bool,
}

/// Punctuation that ends a clause. Commas are in the list because a posting
/// writes its stack as a comma-separated run — "Python, Django, Postgres" —
/// and "python django" is not a thing anybody said.
fn breaks_clause(c: char) -> bool {
    matches!(
        c,
        '.' | ',' | ';' | ':' | '!' | '?' | '\n' | '(' | ')' | '|'
    )
}

/// The words of a piece of text, in order.
///
/// A word is a run of ASCII alphanumerics: the catalog side already handles
/// "C++" and "node.js", and this side is about the English around them.
fn stems(text: &str) -> Vec<Word> {
    let mut out: Vec<Word> = Vec::new();
    let lower = text.to_ascii_lowercase();
    let mut rest = lower.as_str();
    while !rest.is_empty() {
        let end = rest
            .find(|c: char| !c.is_ascii_alphanumeric())
            .unwrap_or(rest.len());
        let (word, after) = rest.split_at(end);
        let gap_end = after
            .find(|c: char| c.is_ascii_alphanumeric())
            .unwrap_or(after.len());
        let (gap, next) = after.split_at(gap_end);
        if let Some(stem) = stem(word) {
            out.push(Word {
                stem,
                surface: word.to_string(),
                ends_clause: gap.contains(breaks_clause),
            });
        } else if !word.is_empty() {
            // A dropped word — a stopword, a number — still separates: the
            // words on either side of "and" were never adjacent.
            if let Some(last) = out.last_mut() {
                last.ends_clause = true;
            }
        }
        rest = next;
    }
    out
}

/// A term and its two forms: the stem everything is keyed on, and the words
/// the posting actually used, which is what a reader can recognise.
fn bigram(a: &str, b: &str) -> String {
    format!("{a} {b}")
}

/// What a posting is about, as weighted terms.
#[derive(Debug, Default, Clone)]
pub struct Focus {
    /// Stem (or stemmed bigram) → 0.0–1.0, normalised so the posting's
    /// strongest term is 1.0. Only ever compared within one posting.
    terms: HashMap<String, f32>,
    /// Stem → the words the posting actually used for it. A stem is keyed on
    /// and never shown: "distribut system" is not what anybody wrote.
    surface: HashMap<String, String>,
    /// The strongest few, in the posting's own words. Shown to the reader, so
    /// they can see what the tailoring is aiming at rather than trust it.
    top: Vec<String>,
}

/// How much a mention counts for, by where it appeared. The same shape as
/// [`crate::skills::Field`], and for the same reason: the requirements are the
/// employer saying what they want, the blurb is the employer describing itself.
pub const REQUIREMENTS: f32 = 3.0;
pub const RESPONSIBILITIES: f32 = 2.0;
pub const DESCRIPTION: f32 = 1.0;

/// Terms kept per posting. A 9 KB Workday page yields a few thousand distinct
/// stems and bigrams, almost all of them mentioned once; the tail is noise and
/// keeping it makes every bullet's lookup slower for nothing.
const MAX_TERMS: usize = 400;

/// How many terms the UI is offered.
const TOP_TERMS: usize = 6;

impl Focus {
    /// The empty focus, for a posting whose page could not be read. `echo`
    /// answers zero for everything, which is what makes tailoring against an
    /// unreadable posting fall back to the résumé's own order.
    pub fn empty() -> Focus {
        Focus::default()
    }

    /// Reads a posting's sections. Each part is `(text, weight)`; use the
    /// constants above so both sides of the app agree what a section is worth.
    pub fn read(parts: &[(&str, f32)]) -> Focus {
        let mut raw: HashMap<String, f32> = HashMap::new();
        let mut surface: HashMap<String, String> = HashMap::new();
        let mut bigrams: HashSet<String> = HashSet::new();

        for (text, weight) in parts {
            let mut counts: HashMap<String, u32> = HashMap::new();
            let words = stems(text);
            for (index, word) in words.iter().enumerate() {
                *counts.entry(word.stem.clone()).or_default() += 1;
                surface
                    .entry(word.stem.clone())
                    .or_insert_with(|| word.surface.clone());

                let next = match word.ends_clause {
                    true => None,
                    false => words.get(index + 1),
                };
                if let Some(next) = next {
                    let key = bigram(&word.stem, &next.stem);
                    // A bigram is worth more than the words in it: it is the
                    // difference between "machine learning" and a bullet about
                    // washing machines.
                    *counts.entry(key.clone()).or_default() += 2;
                    surface
                        .entry(key.clone())
                        .or_insert_with(|| bigram(&word.surface, &next.surface));
                    bigrams.insert(key);
                }
            }
            for (term, count) in counts {
                // Diminishing returns, exactly as the skill reader does it: a
                // posting that says "latency" nine times means it more than one
                // that says it once, and not nine times more.
                *raw.entry(term).or_default() += weight * (1.0 + (count as f32).ln());
            }
        }

        let mut ranked: Vec<(String, f32)> = raw.into_iter().collect();
        ranked.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        ranked.truncate(MAX_TERMS);

        let Some(peak) = ranked.first().map(|(_, w)| *w).filter(|w| *w > 0.0) else {
            return Focus::empty();
        };

        // The headline terms: bigrams first, because they are the ones that
        // read as a subject rather than a word.
        let mut top: Vec<String> = Vec::new();
        for want_bigram in [true, false] {
            for (term, _) in &ranked {
                if top.len() >= TOP_TERMS {
                    break;
                }
                if bigrams.contains(term) != want_bigram {
                    continue;
                }
                let Some(words) = surface.get(term) else {
                    continue;
                };
                // "latency" adds nothing once "latency budget" is on the list.
                if top.iter().any(|shown| shown.contains(words.as_str())) {
                    continue;
                }
                top.push(words.clone());
            }
        }

        let terms: HashMap<String, f32> = ranked
            .into_iter()
            .map(|(term, weight)| (term, weight / peak))
            .collect();
        surface.retain(|term, _| terms.contains_key(term));
        Focus {
            terms,
            surface,
            top,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.terms.is_empty()
    }

    /// The posting's own words for what it keeps coming back to.
    pub fn top(&self) -> &[String] {
        &self.top
    }

    /// How much of this posting's vocabulary a piece of text speaks.
    ///
    /// Distinct terms only: a bullet that says "pipeline" four times is one
    /// bullet about pipelines, and without the dedupe the longest bullet wins
    /// every trim regardless of what it says.
    pub fn echo(&self, text: &str) -> f32 {
        if self.terms.is_empty() {
            return 0.0;
        }
        let mut score = 0.0;
        let mut counted: HashSet<&str> = HashSet::new();
        for term in terms_of(text) {
            let Some((key, weight)) = self.terms.get_key_value(&term) else {
                continue;
            };
            if counted.insert(key.as_str()) {
                score += weight;
            }
        }
        score
    }

    /// The posting's words that a piece of text echoes, strongest first, for
    /// the "why this bullet" row. At most `limit`, because this is a caption.
    ///
    /// Phrases lead: "on-call rotation" tells the reader what the bullet had
    /// in common with the posting, and "rotation" on its own does not.
    pub fn shared(&self, text: &str, limit: usize) -> Vec<String> {
        if self.terms.is_empty() || limit == 0 {
            return Vec::new();
        }
        let mut hits: Vec<(&str, f32)> = Vec::new();
        let mut counted: HashSet<&str> = HashSet::new();
        for term in terms_of(text) {
            let Some((key, weight)) = self.terms.get_key_value(&term) else {
                continue;
            };
            if counted.insert(key.as_str()) {
                hits.push((key.as_str(), *weight));
            }
        }
        hits.sort_by(|a, b| {
            let phrase = |t: &str| !t.contains(' ');
            phrase(a.0)
                .cmp(&phrase(b.0))
                .then_with(|| b.1.total_cmp(&a.1))
                .then_with(|| a.0.cmp(b.0))
        });
        hits.into_iter()
            .filter_map(|(term, _)| self.surface.get(term).cloned())
            .take(limit)
            .collect()
    }
}

/// Every term a piece of text contributes: each stem, and each adjacent pair.
fn terms_of(text: &str) -> Vec<String> {
    let words = stems(text);
    let mut out = Vec::with_capacity(words.len() * 2);
    for (index, word) in words.iter().enumerate() {
        if !word.ends_clause {
            if let Some(next) = words.get(index + 1) {
                out.push(bigram(&word.stem, &next.stem));
            }
        }
        out.push(word.stem.clone());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn posting() -> Focus {
        Focus::read(&[
            (
                "Experience with distributed systems and latency budgets.\n\
                 You have run an on-call rotation.",
                REQUIREMENTS,
            ),
            (
                "You will own a service end to end, from design review to on-call.",
                RESPONSIBILITIES,
            ),
            (
                "We build payments infrastructure for small businesses.",
                DESCRIPTION,
            ),
        ])
    }

    #[test]
    fn a_bullet_about_the_job_beats_one_that_is_not() {
        let focus = posting();
        let on_topic = focus.echo("Ran the on-call rotation for a distributed system");
        let off_topic = focus.echo("Organised the annual charity bake sale");
        assert!(
            on_topic > off_topic,
            "on-topic {on_topic} did not beat off-topic {off_topic}"
        );
        assert_eq!(off_topic, 0.0, "an unrelated bullet echoed something");
    }

    /// The reason everything is stemmed. Without it, a posting asking for
    /// "distributed systems" and a bullet saying "distributed system" share
    /// exactly one word.
    #[test]
    fn plural_and_singular_are_the_same_term() {
        let focus = Focus::read(&[("distributed systems", REQUIREMENTS)]);
        assert!(focus.echo("a distributed system") > 0.0);
    }

    #[test]
    fn tense_does_not_hide_a_match() {
        let focus = Focus::read(&[("shipping payment integrations", REQUIREMENTS)]);
        assert!(focus.echo("Shipped a payment integration") > 0.0);
    }

    /// Boilerplate is in every posting and every bullet, so matching on it is
    /// matching on nothing.
    #[test]
    fn boilerplate_is_not_a_match() {
        let focus = Focus::read(&[(
            "You will work with a great team in a fast-paced environment",
            REQUIREMENTS,
        )]);
        assert_eq!(
            focus.echo("Worked with a great team in a fast-paced environment"),
            0.0
        );
    }

    #[test]
    fn the_requirements_weigh_more_than_the_blurb() {
        let focus = Focus::read(&[
            ("kubernetes rollouts", REQUIREMENTS),
            ("chocolate tasting", DESCRIPTION),
        ]);
        assert!(focus.echo("kubernetes rollout") > focus.echo("chocolate tasting"));
    }

    #[test]
    fn an_unread_posting_matches_nothing() {
        let focus = Focus::empty();
        assert!(focus.is_empty());
        assert_eq!(focus.echo("anything at all"), 0.0);
        assert!(focus.top().is_empty());
    }

    #[test]
    fn the_headline_terms_lead_with_phrases() {
        let top = posting().top().to_vec();
        assert!(!top.is_empty());
        assert!(
            top.iter().any(|t| t.contains(' ')),
            "no phrase led the list: {top:?}"
        );
    }

    /// Repeating a word inside one bullet is one bullet about that word.
    #[test]
    fn a_repeated_word_is_counted_once() {
        let focus = Focus::read(&[("latency", REQUIREMENTS)]);
        let once = focus.echo("cut latency");
        let thrice = focus.echo("cut latency, watched latency, held latency");
        assert_eq!(once, thrice);
    }

    /// A pair of words is only a phrase if they were next to each other in the
    /// same clause. "latency budgets. You have run" used to produce the phrase
    /// "budgets run", which nobody wrote and which then led the headline list.
    #[test]
    fn a_phrase_does_not_cross_a_clause() {
        let focus = Focus::read(&[(
            "latency budgets.\nYou have run an on-call rotation.",
            REQUIREMENTS,
        )]);
        assert!(
            focus.top().iter().any(|t| t == "call rotation"),
            "the real phrase is missing: {:?}",
            focus.top()
        );
        assert!(
            !focus.top().iter().any(|t| t == "budgets run"),
            "a phrase was invented across a sentence break: {:?}",
            focus.top()
        );
        // And a comma-separated stack list is a list, not a compound noun.
        let stack = Focus::read(&[("python, django, postgres", REQUIREMENTS)]);
        assert!(
            !stack.top().iter().any(|term| term.contains(' ')),
            "a comma-separated list produced a phrase: {:?}",
            stack.top()
        );
    }
}

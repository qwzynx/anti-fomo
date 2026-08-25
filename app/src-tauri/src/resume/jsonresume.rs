//! Import and export against the [JSON Resume](https://jsonresume.org) schema.
//!
//! The reason this exists is the first five minutes. A résumé builder that
//! opens on an empty form asks the user to retype everything they already have
//! somewhere else, and most of them will close it instead. JSON Resume is the
//! one interchange format with real tooling around it, so "I already have this"
//! has an answer.
//!
//! It is also the backup. Everything else in this app lives in one SQLite file
//! on one device by design — no account, no server — which is right for a feed
//! cache and uncomfortable for the only copy of somebody's work history.
//!
//! Two details worth keeping:
//!
//! - **Unknown fields are ignored, not rejected.** Real `resume.json` files
//!   carry `meta`, `references`, custom blocks and half-filled optional keys. A
//!   strict parser would refuse documents that are perfectly usable.
//! - **Our ids ride along in `meta.antifomo`.** Per-posting variants address
//!   bullets by id, so an export/import round trip that minted fresh ids would
//!   silently detach every tailoring override the user had saved. The extra key
//!   is ignored by every other JSON Resume tool.

use serde::{Deserialize, Serialize};

use super::model::{new_id, Bullet, Contact, Entry, Link, Resume, Section, SectionKind};

#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
struct Location {
    city: String,
    region: String,
    address: String,
}

impl Location {
    fn joined(&self) -> String {
        let parts: Vec<&str> = [self.city.as_str(), self.region.as_str()]
            .into_iter()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .collect();
        if parts.is_empty() {
            self.address.trim().to_string()
        } else {
            parts.join(", ")
        }
    }
}

#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
struct Profile {
    network: String,
    username: String,
    url: String,
}

#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
struct Basics {
    name: String,
    label: String,
    email: String,
    phone: String,
    url: String,
    location: Location,
    profiles: Vec<Profile>,
}

/// One `work` / `volunteer` / `education` / `projects` / `awards` record.
///
/// A single struct for all five: the schema names the same ideas differently
/// per section (`name` vs `organization` vs `institution`), and an alias list
/// is a great deal less code than five near-identical types.
#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
struct Record {
    #[serde(alias = "organization", alias = "institution", alias = "awarder")]
    name: String,
    #[serde(alias = "position", alias = "studyType", alias = "title")]
    role: String,
    #[serde(alias = "area")]
    area: String,
    location: String,
    url: String,
    #[serde(alias = "startDate")]
    start_date: String,
    #[serde(alias = "endDate", alias = "date")]
    end_date: String,
    #[serde(alias = "score")]
    score: String,
    summary: String,
    #[serde(alias = "description")]
    description: String,
    highlights: Vec<String>,
    keywords: Vec<String>,
}

#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
struct SkillGroup {
    name: String,
    keywords: Vec<String>,
}

#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
struct Meta {
    /// Our own ids, so a round trip keeps per-posting overrides attached.
    antifomo: Option<Ids>,
}

#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
struct Ids {
    sections: Vec<String>,
    /// Entry id followed by its bullet ids, per section, in document order.
    entries: Vec<Vec<Vec<String>>>,
}

#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
pub struct JsonResume {
    basics: Basics,
    work: Vec<Record>,
    volunteer: Vec<Record>,
    education: Vec<Record>,
    projects: Vec<Record>,
    awards: Vec<Record>,
    skills: Vec<SkillGroup>,
    meta: Meta,
}

/// Parses a `resume.json` into our model.
pub fn import(json: &str) -> Result<Resume, String> {
    let parsed: JsonResume =
        serde_json::from_str(json).map_err(|e| format!("not a JSON Resume document: {e}"))?;

    let mut links: Vec<Link> = Vec::new();
    if !parsed.basics.url.trim().is_empty() {
        links.push(Link {
            label: "Website".into(),
            url: parsed.basics.url.clone(),
        });
    }
    for profile in &parsed.basics.profiles {
        let url = if profile.url.trim().is_empty() {
            match (
                profile.network.to_lowercase().as_str(),
                profile.username.trim(),
            ) {
                (_, "") => continue,
                ("github", user) => format!("https://github.com/{user}"),
                ("linkedin", user) => format!("https://linkedin.com/in/{user}"),
                _ => continue,
            }
        } else {
            profile.url.clone()
        };
        links.push(Link {
            label: profile.network.clone(),
            url,
        });
    }

    let contact = Contact {
        name: parsed.basics.name.trim().to_string(),
        headline: Some(parsed.basics.label.trim().to_string()).filter(|s| !s.is_empty()),
        email: parsed.basics.email.trim().to_string(),
        phone: parsed.basics.phone.trim().to_string(),
        location: parsed.basics.location.joined(),
        links,
    };

    let mut sections: Vec<Section> = Vec::new();
    let mut push = |kind: SectionKind, records: &[Record], project_like: bool| {
        if records.is_empty() {
            return;
        }
        let entries: Vec<Entry> = records
            .iter()
            .map(|r| record_to_entry(r, project_like))
            .collect();
        sections.push(Section {
            id: new_id(),
            kind,
            title: kind.default_title().to_string(),
            entries,
        });
    };
    push(SectionKind::Education, &parsed.education, false);
    push(SectionKind::Experience, &parsed.work, false);
    push(SectionKind::Projects, &parsed.projects, true);
    push(SectionKind::Leadership, &parsed.volunteer, false);
    push(SectionKind::Awards, &parsed.awards, false);

    if !parsed.skills.is_empty() {
        sections.push(Section {
            id: new_id(),
            kind: SectionKind::Skills,
            title: SectionKind::Skills.default_title().to_string(),
            entries: parsed
                .skills
                .iter()
                .filter(|g| !g.keywords.is_empty() || !g.name.trim().is_empty())
                .map(|g| Entry {
                    id: new_id(),
                    title: g.name.trim().to_string(),
                    detail: Some(g.keywords.join(", ")),
                    ..Entry::default()
                })
                .collect(),
        });
    }

    let mut resume = Resume { contact, sections };
    if let Some(ids) = parsed.meta.antifomo.as_ref() {
        restore_ids(&mut resume, ids);
    }
    // Mints anything still missing and re-reads every bullet's skills, which an
    // imported document has never had computed.
    resume.normalize();
    Ok(resume)
}

fn record_to_entry(r: &Record, project_like: bool) -> Entry {
    // The Harvard format leads with the organisation; for a project the project
    // itself is the organisation, and its stack is the detail line.
    let (org, title) = if project_like {
        (r.name.clone(), r.role.clone())
    } else {
        (r.name.clone(), degree_or_role(r))
    };

    let mut bullets: Vec<Bullet> = Vec::new();
    // A `summary` is prose rather than a bullet, but dropping it loses content
    // the user wrote, so it leads the list.
    for text in [&r.summary, &r.description] {
        let text = text.trim();
        if !text.is_empty() {
            bullets.push(Bullet::new(text));
        }
    }
    bullets.extend(
        r.highlights
            .iter()
            .filter(|h| !h.trim().is_empty())
            .map(Bullet::new),
    );

    let detail = if project_like && !r.keywords.is_empty() {
        Some(r.keywords.join(" · "))
    } else {
        Some(r.score.trim().to_string()).filter(|s| !s.is_empty())
    };

    Entry {
        id: new_id(),
        org,
        title,
        location: r.location.trim().to_string(),
        start: r.start_date.trim().to_string(),
        end: r.end_date.trim().to_string(),
        link: Some(r.url.trim().to_string()).filter(|s| !s.is_empty()),
        detail,
        bullets,
    }
}

/// "BSc, Computer Science" out of the schema's split `studyType` / `area`.
fn degree_or_role(r: &Record) -> String {
    let (role, area) = (r.role.trim(), r.area.trim());
    match (role.is_empty(), area.is_empty()) {
        (true, true) => String::new(),
        (true, false) => area.to_string(),
        (false, true) => role.to_string(),
        (false, false) => format!("{role}, {area}"),
    }
}

/// Puts previously-exported ids back, position by position.
///
/// Positional because the schema has nowhere to hang an id per record. It holds
/// as long as the document was not reordered between export and import; if it
/// was, `normalize` mints fresh ids for whatever is left over and the worst
/// case is a variant losing its reference, not a corrupted document.
fn restore_ids(resume: &mut Resume, ids: &Ids) {
    for (s, section) in resume.sections.iter_mut().enumerate() {
        if let Some(id) = ids.sections.get(s) {
            section.id = id.clone();
        }
        let Some(entries) = ids.entries.get(s) else {
            continue;
        };
        for (e, entry) in section.entries.iter_mut().enumerate() {
            let Some(row) = entries.get(e) else { continue };
            let Some((entry_id, bullet_ids)) = row.split_first() else {
                continue;
            };
            entry.id = entry_id.clone();
            for (b, bullet) in entry.bullets.iter_mut().enumerate() {
                if let Some(id) = bullet_ids.get(b) {
                    bullet.id = id.clone();
                }
            }
        }
    }
}

/// Serializes a résumé as a `resume.json`.
pub fn export(resume: &Resume) -> Result<String, String> {
    let section_of = |kind: SectionKind| resume.sections.iter().filter(move |s| s.kind == kind);
    let records = |kind: SectionKind, project_like: bool| -> Vec<Record> {
        section_of(kind)
            .flat_map(|s| &s.entries)
            .map(|e| Record {
                name: e.org.clone(),
                role: e.title.clone(),
                location: e.location.clone(),
                url: e.link.clone().unwrap_or_default(),
                start_date: e.start.clone(),
                end_date: e.end.clone(),
                score: if project_like {
                    String::new()
                } else {
                    e.detail.clone().unwrap_or_default()
                },
                keywords: if project_like {
                    e.detail
                        .as_deref()
                        .unwrap_or("")
                        .split('·')
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .map(str::to_string)
                        .collect()
                } else {
                    Vec::new()
                },
                highlights: e.bullets.iter().map(|b| b.text.clone()).collect(),
                ..Record::default()
            })
            .collect()
    };

    let doc = JsonResume {
        basics: Basics {
            name: resume.contact.name.clone(),
            label: resume.contact.headline.clone().unwrap_or_default(),
            email: resume.contact.email.clone(),
            phone: resume.contact.phone.clone(),
            url: String::new(),
            location: Location {
                city: resume.contact.location.clone(),
                ..Location::default()
            },
            profiles: resume
                .contact
                .links
                .iter()
                .map(|l| Profile {
                    network: l.label.clone(),
                    username: String::new(),
                    url: l.url.clone(),
                })
                .collect(),
        },
        work: records(SectionKind::Experience, false),
        volunteer: records(SectionKind::Leadership, false),
        education: records(SectionKind::Education, false),
        projects: records(SectionKind::Projects, true),
        awards: records(SectionKind::Awards, false),
        skills: section_of(SectionKind::Skills)
            .flat_map(|s| &s.entries)
            .map(|e| SkillGroup {
                name: e.title.clone(),
                keywords: e
                    .detail
                    .as_deref()
                    .unwrap_or("")
                    .split(',')
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_string)
                    .collect(),
            })
            .collect(),
        meta: Meta {
            antifomo: Some(Ids {
                sections: resume.sections.iter().map(|s| s.id.clone()).collect(),
                entries: resume
                    .sections
                    .iter()
                    .map(|s| {
                        s.entries
                            .iter()
                            .map(|e| {
                                let mut row = vec![e.id.clone()];
                                row.extend(e.bullets.iter().map(|b| b.id.clone()));
                                row
                            })
                            .collect()
                    })
                    .collect(),
            }),
        },
    };

    serde_json::to_string_pretty(&doc).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{
      "basics": {
        "name": "Ada Lovelace",
        "label": "Software Engineer",
        "email": "ada@example.com",
        "phone": "+1 416 555 0100",
        "location": { "city": "Toronto", "region": "ON" },
        "profiles": [{ "network": "GitHub", "username": "ada", "url": "" }]
      },
      "work": [{
        "name": "Acme Corp",
        "position": "Software Engineer Intern",
        "startDate": "2025-05",
        "endDate": "2025-08",
        "highlights": ["Shipped a Rust service handling 2,000 rps"]
      }],
      "education": [{
        "institution": "York University",
        "studyType": "BSc",
        "area": "Computer Science",
        "score": "GPA 3.9/4.0",
        "startDate": "2023-09"
      }],
      "skills": [{ "name": "Languages", "keywords": ["Rust", "TypeScript"] }],
      "references": [{ "name": "ignored" }]
    }"#;

    #[test]
    fn imports_a_real_document() {
        let r = import(SAMPLE).expect("import");
        assert_eq!(r.contact.name, "Ada Lovelace");
        assert_eq!(r.contact.location, "Toronto, ON");
        assert_eq!(r.contact.headline.as_deref(), Some("Software Engineer"));
        assert_eq!(
            r.contact.links[0].url, "https://github.com/ada",
            "username was not expanded"
        );

        let work = r
            .sections
            .iter()
            .find(|s| s.kind == SectionKind::Experience)
            .unwrap();
        assert_eq!(work.entries[0].org, "Acme Corp");
        assert_eq!(work.entries[0].title, "Software Engineer Intern");
        assert_eq!(work.entries[0].bullets.len(), 1);

        let education = r
            .sections
            .iter()
            .find(|s| s.kind == SectionKind::Education)
            .unwrap();
        assert_eq!(education.entries[0].title, "BSc, Computer Science");
        assert_eq!(education.entries[0].detail.as_deref(), Some("GPA 3.9/4.0"));
    }

    /// An imported bullet has to arrive with its skills read, or it would never
    /// match a posting until the user happened to re-save it.
    #[test]
    fn imported_bullets_have_their_skills_read() {
        let r = import(SAMPLE).unwrap();
        let work = r
            .sections
            .iter()
            .find(|s| s.kind == SectionKind::Experience)
            .unwrap();
        assert!(work.entries[0].bullets[0]
            .skills
            .contains(&"Rust".to_string()));
    }

    #[test]
    fn unknown_fields_are_ignored() {
        assert!(
            import(SAMPLE).is_ok(),
            "the `references` block should not be fatal"
        );
    }

    #[test]
    fn rejects_documents_that_are_not_json() {
        assert!(import("not json at all").is_err());
    }

    #[test]
    fn round_trips_content() {
        let original = import(SAMPLE).unwrap();
        let json = export(&original).unwrap();
        let back = import(&json).unwrap();
        assert_eq!(back.contact.name, original.contact.name);
        assert_eq!(back.contact.email, original.contact.email);
        assert_eq!(back.sections.len(), original.sections.len());
        for (a, b) in original.sections.iter().zip(&back.sections) {
            assert_eq!(a.kind, b.kind);
            assert_eq!(a.entries.len(), b.entries.len());
        }
    }

    /// The reason ids ride along in `meta`: a variant addresses bullets by id,
    /// and a round trip that renamed them would detach every override.
    #[test]
    fn round_trips_ids_so_variants_stay_attached() {
        let original = import(SAMPLE).unwrap();
        let work = original
            .sections
            .iter()
            .find(|s| s.kind == SectionKind::Experience)
            .unwrap();
        let entry_id = work.entries[0].id.clone();
        let bullet_id = work.entries[0].bullets[0].id.clone();

        let back = import(&export(&original).unwrap()).unwrap();
        assert!(
            back.entry(&entry_id).is_some(),
            "entry id was not preserved"
        );
        assert!(
            back.bullet(&bullet_id).is_some(),
            "bullet id was not preserved"
        );
    }

    #[test]
    fn an_empty_document_imports_to_an_empty_resume() {
        let r = import("{}").expect("an empty object is a valid document");
        assert!(r.contact.name.is_empty());
        assert!(r.sections.is_empty());
    }
}

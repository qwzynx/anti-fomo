//! What a résumé *is*, independent of how it is drawn.
//!
//! Deliberately loose about dates and degrees: `start`/`end` are free text
//! because "May 2025", "Summer 2025" and "Expected Apr 2027" are all things
//! people put on résumés, and a date picker that refuses the third is a worse
//! tool than a text field. Nothing downstream parses them — they are set
//! right-aligned and that is all.
//!
//! Ids are the load-bearing part. A [`Variant`](super::tailor::Variant) refers
//! to entries and bullets by id to say "keep this one for that job", so an id
//! has to survive the user rewriting the text it is attached to. They are
//! minted once, on creation, and never derived from content.

use serde::{Deserialize, Serialize};

/// A short, opaque, stable id.
///
/// Not a UUID: this is a local database with a few dozen résumé bullets in it,
/// and pulling in a crate to name them would be the only dependency in the tree
/// that exists to generate strings. Randomness comes from the address-space
/// layout and the clock, which is plenty when the only requirement is "does not
/// collide with the other forty ids in this document".
pub fn new_id() -> String {
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hasher};
    use std::time::{SystemTime, UNIX_EPOCH};

    let mut hasher = RandomState::new().build_hasher();
    hasher.write_u128(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default(),
    );
    format!("{:012x}", hasher.finish() & 0xffff_ffff_ffff)
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(default)]
pub struct Link {
    pub label: String,
    pub url: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(default)]
pub struct Contact {
    pub name: String,
    /// An optional line under the name — "Software Engineer". The tailoring
    /// pass can override it per posting without touching the master résumé.
    pub headline: Option<String>,
    pub email: String,
    pub phone: String,
    pub location: String,
    pub links: Vec<Link>,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum SectionKind {
    Education,
    #[default]
    Experience,
    Projects,
    Leadership,
    /// Rendered as label-and-run lines rather than entries, and the only
    /// section the tailoring pass reorders by itself.
    Skills,
    Awards,
    Custom,
}

impl SectionKind {
    pub fn default_title(self) -> &'static str {
        match self {
            SectionKind::Education => "Education",
            SectionKind::Experience => "Experience",
            SectionKind::Projects => "Projects",
            SectionKind::Leadership => "Leadership & Activities",
            SectionKind::Skills => "Technical Skills",
            SectionKind::Awards => "Awards",
            SectionKind::Custom => "Additional",
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(default)]
pub struct Bullet {
    pub id: String,
    pub text: String,
    /// Catalog skills this bullet names, from `skills::from_text`. Stored
    /// rather than recomputed so tailoring is a set intersection instead of a
    /// scan over every bullet on every keystroke, and editable because the
    /// extractor is keyword matching and the user is the authority on what
    /// they actually did.
    pub skills: Vec<String>,
}

impl Bullet {
    pub fn new(text: impl Into<String>) -> Bullet {
        let text = text.into();
        let skills = crate::skills::from_text(&text, true);
        Bullet {
            id: new_id(),
            text,
            skills,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(default)]
pub struct Entry {
    pub id: String,
    /// The organisation. First, because the Harvard format leads with it.
    pub org: String,
    /// Role, degree or project name.
    pub title: String,
    pub location: String,
    pub start: String,
    pub end: String,
    pub link: Option<String>,
    /// One extra line under the title — "GPA 3.9/4.0", "Rust · Tauri · SQLite".
    pub detail: Option<String>,
    pub bullets: Vec<Bullet>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(default)]
pub struct Section {
    pub id: String,
    pub kind: SectionKind,
    pub title: String,
    pub entries: Vec<Entry>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(default)]
pub struct Resume {
    pub contact: Contact,
    pub sections: Vec<Section>,
}

impl Resume {
    /// An empty résumé with the sections a software CV actually has, in the
    /// order the format wants them. A first run that opens on a blank page is
    /// a worse prompt than one that opens on the right headings.
    pub fn starter() -> Resume {
        let section = |kind: SectionKind| Section {
            id: new_id(),
            kind,
            title: kind.default_title().to_string(),
            entries: Vec::new(),
        };
        Resume {
            contact: Contact::default(),
            sections: vec![
                section(SectionKind::Education),
                section(SectionKind::Experience),
                section(SectionKind::Projects),
                section(SectionKind::Skills),
            ],
        }
    }

    pub fn entry(&self, id: &str) -> Option<&Entry> {
        self.sections
            .iter()
            .flat_map(|s| &s.entries)
            .find(|e| e.id == id)
    }

    pub fn bullet(&self, id: &str) -> Option<&Bullet> {
        self.sections
            .iter()
            .flat_map(|s| &s.entries)
            .flat_map(|e| &e.bullets)
            .find(|b| b.id == id)
    }

    /// Fills in any id the caller left blank and re-reads each bullet's skills.
    ///
    /// Called on every save. Imported documents and hand-written JSON arrive
    /// without ids, and an entry with an empty id would be silently
    /// un-addressable by every tailoring override — it would look like the
    /// toggles simply did not work.
    pub fn normalize(&mut self) {
        for section in &mut self.sections {
            if section.id.is_empty() {
                section.id = new_id();
            }
            if section.title.trim().is_empty() {
                section.title = section.kind.default_title().to_string();
            }
            for entry in &mut section.entries {
                if entry.id.is_empty() {
                    entry.id = new_id();
                }
                for bullet in &mut entry.bullets {
                    if bullet.id.is_empty() {
                        bullet.id = new_id();
                    }
                    bullet.skills = crate::skills::from_text(&bullet.text, true);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_unique() {
        let ids: std::collections::HashSet<String> = (0..500).map(|_| new_id()).collect();
        assert_eq!(ids.len(), 500, "id collision");
    }

    #[test]
    fn starter_has_the_expected_sections() {
        let r = Resume::starter();
        let kinds: Vec<_> = r.sections.iter().map(|s| s.kind).collect();
        assert_eq!(
            kinds,
            vec![
                SectionKind::Education,
                SectionKind::Experience,
                SectionKind::Projects,
                SectionKind::Skills
            ]
        );
        assert!(r.sections.iter().all(|s| !s.id.is_empty()));
    }

    #[test]
    fn normalize_fills_missing_ids_and_reads_skills() {
        let mut r = Resume {
            contact: Contact::default(),
            sections: vec![Section {
                id: String::new(),
                kind: SectionKind::Experience,
                title: String::new(),
                entries: vec![Entry {
                    id: String::new(),
                    bullets: vec![Bullet {
                        id: String::new(),
                        text: "Built a service in Rust backed by PostgreSQL".into(),
                        skills: Vec::new(),
                    }],
                    ..Entry::default()
                }],
            }],
        };
        r.normalize();
        let section = &r.sections[0];
        assert!(!section.id.is_empty());
        assert_eq!(section.title, "Experience");
        assert!(!section.entries[0].id.is_empty());
        let bullet = &section.entries[0].bullets[0];
        assert!(!bullet.id.is_empty());
        assert!(
            bullet.skills.contains(&"Rust".to_string()),
            "got {:?}",
            bullet.skills
        );
        assert!(
            bullet.skills.contains(&"PostgreSQL".to_string()),
            "got {:?}",
            bullet.skills
        );
    }

    #[test]
    fn lookups_find_nested_items() {
        let bullet = Bullet::new("Shipped a Kubernetes operator");
        let bid = bullet.id.clone();
        let entry = Entry {
            id: new_id(),
            bullets: vec![bullet],
            ..Entry::default()
        };
        let eid = entry.id.clone();
        let r = Resume {
            contact: Contact::default(),
            sections: vec![Section {
                id: new_id(),
                entries: vec![entry],
                ..Section::default()
            }],
        };
        assert!(r.entry(&eid).is_some());
        assert!(r.bullet(&bid).is_some());
        assert!(r.bullet("nope").is_none());
    }
}

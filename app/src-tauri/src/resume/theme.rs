//! How a résumé looks. The *what* is [`Resume`](super::model::Resume); this is
//! everything about putting it on paper.
//!
//! All four themes lay out identically — the Harvard Office of Career Services
//! format: a centred name, a `•`-separated contact line, ALL-CAPS section
//! headings over a rule, organisation bold on the left with the location
//! right-aligned, title italic under it with the dates right-aligned, and
//! hanging-indent bullets. That geometry is not a style choice. It is the one
//! an applicant tracking system reads without tripping, and a résumé that reads
//! beautifully and parses badly has failed at the only job it has.
//!
//! What the themes vary is treatment: the family, the weight of the rules, and
//! *where* the accent colour is allowed to land. The user picks the colour;
//! [`AccentRoles`] decides which elements take it, so "colourful" can never
//! turn into unreadable body text.

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum FamilyId {
    /// Liberation Serif — metric-compatible with Times New Roman.
    Serif,
    /// Inter, the family the app's own interface uses.
    Sans,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
#[serde(rename_all = "lowercase")]
pub enum FontStyle {
    #[default]
    Regular,
    Bold,
    Italic,
    BoldItalic,
}

impl FontStyle {
    pub fn bold(self) -> bool {
        matches!(self, FontStyle::Bold | FontStyle::BoldItalic)
    }
    pub fn italic(self) -> bool {
        matches!(self, FontStyle::Italic | FontStyle::BoldItalic)
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    pub const BLACK: Rgb = Rgb { r: 0, g: 0, b: 0 };

    pub fn to_f32(self) -> (f32, f32, f32) {
        (
            f32::from(self.r) / 255.0,
            f32::from(self.g) / 255.0,
            f32::from(self.b) / 255.0,
        )
    }

    /// The colour mixed toward white, for the `Banner` theme's heading bands.
    /// Computed rather than picked so any accent the user chooses gets a tint
    /// that black text still reads on.
    pub fn tint(self, amount: f32) -> Rgb {
        let mix = |c: u8| (f32::from(c) + (255.0 - f32::from(c)) * amount).round() as u8;
        Rgb {
            r: mix(self.r),
            g: mix(self.g),
            b: mix(self.b),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PageSize {
    Letter,
    A4,
}

impl PageSize {
    /// Width and height in points.
    pub fn dimensions(self) -> (f32, f32) {
        match self {
            PageSize::Letter => (612.0, 792.0),
            PageSize::A4 => (595.28, 841.89),
        }
    }
}

/// How a section heading is drawn.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum HeadingStyle {
    /// Caps with a rule across the column beneath — the handout.
    Rule,
    /// Caps with no rule, relying on space to separate sections.
    Plain,
    /// Caps sitting in a tinted band.
    Band,
}

/// Which elements take the accent colour. Body text and bullets are absent on
/// purpose: they are never anything but near-black, whatever theme is picked.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct AccentRoles {
    pub name: bool,
    pub headings: bool,
    pub rules: bool,
    pub org: bool,
    pub bullet_marks: bool,
}

impl AccentRoles {
    const NONE: AccentRoles = AccentRoles {
        name: false,
        headings: false,
        rules: false,
        org: false,
        bullet_marks: false,
    };
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ThemeId {
    /// Black serif, hairline rules. The handout, unchanged.
    Classic,
    /// Serif, with the accent on the name, the headings and the rules.
    Accent,
    /// Sans throughout, accent headings — the modern reading of the same layout.
    Modern,
    /// Sans with the headings in a tinted band. The colourful one.
    Banner,
}

impl ThemeId {
    pub const ALL: [ThemeId; 4] = [
        ThemeId::Classic,
        ThemeId::Accent,
        ThemeId::Modern,
        ThemeId::Banner,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ThemeId::Classic => "Classic",
            ThemeId::Accent => "Accent",
            ThemeId::Modern => "Modern",
            ThemeId::Banner => "Banner",
        }
    }
}

/// The default accent: the app's own brand indigo, so a résumé started from the
/// picker's first swatch looks like it came from this app.
pub const DEFAULT_ACCENT: Rgb = Rgb {
    r: 0x4f,
    g: 0x46,
    b: 0xe5,
};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(default)]
pub struct Theme {
    pub id: ThemeId,
    pub page: PageSize,
    pub family: FamilyId,
    /// Body size in points. The builder clamps this to 9.0–12.0.
    pub base_size: f32,
    /// Line height as a multiple of the body size.
    pub leading: f32,
    /// Page margin in points. 36 pt is half an inch, 72 pt is one.
    pub margin: f32,
    pub accent: Rgb,
    pub heading: HeadingStyle,
    pub accent_roles: AccentRoles,
    /// How many pages the tailoring pass is allowed to fill.
    pub max_pages: u8,
}

impl Default for Theme {
    fn default() -> Self {
        Theme::preset(ThemeId::Classic, DEFAULT_ACCENT)
    }
}

impl Theme {
    /// A theme at its intended proportions, with the user's colour applied.
    pub fn preset(id: ThemeId, accent: Rgb) -> Theme {
        let base = Theme {
            id,
            page: PageSize::Letter,
            family: FamilyId::Serif,
            base_size: 10.5,
            leading: 1.22,
            margin: 54.0,
            accent,
            heading: HeadingStyle::Rule,
            accent_roles: AccentRoles::NONE,
            max_pages: 1,
        };
        match id {
            ThemeId::Classic => base,
            ThemeId::Accent => Theme {
                accent_roles: AccentRoles {
                    name: true,
                    headings: true,
                    rules: true,
                    ..AccentRoles::NONE
                },
                ..base
            },
            // Inter runs larger on the body than a Times-metric serif at the
            // same point size, so it is set a touch smaller and given more
            // leading to keep a page holding roughly the same amount.
            ThemeId::Modern => Theme {
                family: FamilyId::Sans,
                base_size: 10.0,
                leading: 1.3,
                accent_roles: AccentRoles {
                    name: true,
                    headings: true,
                    rules: true,
                    bullet_marks: true,
                    ..AccentRoles::NONE
                },
                ..base
            },
            ThemeId::Banner => Theme {
                family: FamilyId::Sans,
                base_size: 10.0,
                leading: 1.3,
                heading: HeadingStyle::Band,
                accent_roles: AccentRoles {
                    name: true,
                    org: true,
                    bullet_marks: true,
                    ..AccentRoles::NONE
                },
                ..base
            },
        }
    }

    /// Clamps anything the UI can drive to a range that still produces a
    /// readable page. A 4 pt body or a 200 pt margin is not a style, it is a
    /// mistake, and the layout should not be the thing that discovers it.
    pub fn sanitized(mut self) -> Theme {
        self.base_size = self.base_size.clamp(9.0, 12.0);
        self.leading = self.leading.clamp(1.0, 1.8);
        self.margin = self.margin.clamp(36.0, 90.0);
        self.max_pages = self.max_pages.clamp(1, 4);
        self
    }

    /// The accent where a role allows it, near-black where it does not.
    pub fn ink(&self, role: fn(&AccentRoles) -> bool) -> Rgb {
        if role(&self.accent_roles) {
            self.accent
        } else {
            Rgb::BLACK
        }
    }

    /// Usable text column width in points.
    pub fn column(&self) -> f32 {
        self.page.dimensions().0 - self.margin * 2.0
    }

    pub fn line_height(&self) -> f32 {
        self.base_size * self.leading
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_have_distinct_treatments() {
        let a = Theme::preset(ThemeId::Classic, DEFAULT_ACCENT);
        let b = Theme::preset(ThemeId::Banner, DEFAULT_ACCENT);
        assert_ne!(a.accent_roles, b.accent_roles);
        assert_ne!(a.family, b.family);
    }

    /// Whatever a theme does with colour, the body stays readable.
    #[test]
    fn no_theme_tints_the_body() {
        for id in ThemeId::ALL {
            let theme = Theme::preset(
                id,
                Rgb {
                    r: 255,
                    g: 240,
                    b: 0,
                },
            );
            assert_eq!(theme.ink(|_| false), Rgb::BLACK);
        }
    }

    #[test]
    fn sanitize_clamps_nonsense() {
        let t = Theme {
            base_size: 2.0,
            margin: 400.0,
            leading: 9.0,
            max_pages: 40,
            ..Theme::default()
        }
        .sanitized();
        assert_eq!(t.base_size, 9.0);
        assert_eq!(t.margin, 90.0);
        assert_eq!(t.leading, 1.8);
        assert_eq!(t.max_pages, 4);
        assert!(t.column() > 0.0);
    }

    #[test]
    fn letter_is_612_by_792() {
        assert_eq!(PageSize::Letter.dimensions(), (612.0, 792.0));
    }

    #[test]
    fn tint_moves_toward_white() {
        let tinted = DEFAULT_ACCENT.tint(0.85);
        assert!(tinted.r > DEFAULT_ACCENT.r && tinted.g > DEFAULT_ACCENT.g);
    }
}

use gpui::{Global, Hsla, Pixels, Rgba, px};

macro_rules! palette {
    ($($field:ident = $key:literal),* $(,)?) => {
        #[derive(Clone, Copy, Debug, PartialEq)]
        pub struct Palette {
            $(pub $field: Hsla,)*
        }

        impl Palette {
            fn of(colour: impl Fn(&str) -> Hsla) -> Self {
                Self { $($field: colour($key),)* }
            }
        }
    };
}

palette! {
    win = "win", panel = "panel", bar = "bar", ctl = "ctl", ctl_hi = "ctlHi", line = "line",
    edge = "edge", well = "well", seg = "seg", chip = "chip", hover = "hover", ink = "ink",
    ink2 = "ink2", ink3 = "ink3", brand = "brand", field_line = "fieldLine", accent = "accent",
    accent_hi = "accentHi", focus = "focus", accent_ink = "accentInk", err = "err", warn = "warn",
    ok = "ok", moved = "moved", banner = "banner", danger_bg = "dangerBg", chosen = "chosen",
    scrim = "scrim", card = "card", on_selected = "onSelected", on_selected_dim = "onSelectedDim",
}

type Table = &'static [(&'static str, &'static str)];

/// `#rrggbb`, or `#aarrggbb` with the alpha first.
const RIM: Table = &[
    ("win", "#1c1b19"),
    ("panel", "#161513"),
    ("bar", "#121110"),
    ("ctl", "#2d2b27"),
    ("ctlHi", "#3b3833"),
    ("line", "#34322d"),
    ("edge", "#0f0e0d"),
    ("well", "#131211"),
    ("seg", "#141311"),
    ("chip", "#14ffffff"),
    ("hover", "#ffffff"),
    ("ink", "#ede9e2"),
    ("ink2", "#b9b3a9"),
    ("ink3", "#9c968b"),
    ("brand", "#e2bb7c"),
    ("fieldLine", "#6f6a61"),
    ("accent", "#5466c8"),
    ("accentHi", "#5c6fd4"),
    ("focus", "#a3b0f5"),
    ("accentInk", "#ffffff"),
    ("err", "#ff7b6b"),
    ("warn", "#f2b04a"),
    ("ok", "#8fce7a"),
    ("moved", "#a3b0f5"),
    ("banner", "#22263f"),
    ("dangerBg", "#3d2320"),
    ("chosen", "#272c4c"),
    ("scrim", "#66000000"),
    ("card", "#24231f"),
];
const RIM_HIGH: Table = &[
    ("chosen", "#5466c8"),
    ("onSelected", "#ffffff"),
    ("onSelectedDim", "#ebffffff"),
    ("ink2", "#e6dfd0"),
    ("ink3", "#cfc7b8"),
    ("line", "#8a8172"),
    ("fieldLine", "#c2b9a8"),
];
const DARK: Table = &[
    ("win", "#1c1f2c"),
    ("panel", "#161822"),
    ("bar", "#111319"),
    ("ctl", "#33384e"),
    ("ctlHi", "#434964"),
    ("line", "#2f3449"),
    ("edge", "#0c0d14"),
    ("well", "#10121a"),
    ("seg", "#13151e"),
    ("chip", "#18c9d3ff"),
    ("hover", "#dfe6ff"),
    ("ink", "#eef0f7"),
    ("ink2", "#b1b6c8"),
    ("ink3", "#989db2"),
    ("brand", "#dcbc8e"),
    ("fieldLine", "#6b7294"),
    ("accent", "#4161d6"),
    ("accentHi", "#4a6adc"),
    ("focus", "#7f9bff"),
    ("accentInk", "#ffffff"),
    ("err", "#ff7075"),
    ("warn", "#f2b04e"),
    ("ok", "#62d08f"),
    ("moved", "#82b4ff"),
    ("banner", "#1d2650"),
    ("dangerBg", "#3b2029"),
    ("chosen", "#253261"),
    ("scrim", "#73060710"),
    ("card", "#232738"),
];
const DARK_HIGH: Table = &[
    ("chosen", "#4161d6"),
    ("onSelected", "#ffffff"),
    ("onSelectedDim", "#ebffffff"),
    ("ink2", "#e2e5f0"),
    ("ink3", "#c6cadb"),
    ("line", "#7a81a0"),
    ("fieldLine", "#b8bdd6"),
];
const OLED: Table = &[
    ("win", "#0b0b0a"),
    ("panel", "#000000"),
    ("bar", "#121211"),
    ("ctl", "#22211f"),
    ("ctlHi", "#32312e"),
    ("line", "#262523"),
    ("edge", "#000000"),
    ("well", "#060606"),
    ("seg", "#0e0d0c"),
    ("chip", "#1cfff6ea"),
    ("hover", "#fff6ea"),
    ("ink", "#f3efe8"),
    ("ink2", "#aba7a0"),
    ("ink3", "#928e87"),
    ("brand", "#dfb98a"),
    ("fieldLine", "#69655e"),
    ("accent", "#0f7572"),
    ("accentHi", "#12807c"),
    ("focus", "#3fc4bc"),
    ("accentInk", "#ffffff"),
    ("err", "#ff6b62"),
    ("warn", "#f5b13f"),
    ("ok", "#7fd070"),
    ("moved", "#6cb0ff"),
    ("banner", "#082624"),
    ("dangerBg", "#2c1313"),
    ("chosen", "#062f2e"),
    ("scrim", "#8c000000"),
    ("card", "#161614"),
];
const OLED_HIGH: Table = &[
    ("chosen", "#0f7572"),
    ("onSelected", "#ffffff"),
    ("onSelectedDim", "#ebffffff"),
    ("ink2", "#e5e1da"),
    ("ink3", "#cac6be"),
    ("line", "#77736b"),
    ("fieldLine", "#bbb6ad"),
];
const LIGHT: Table = &[
    ("win", "#f5f5f7"),
    ("panel", "#ececef"),
    ("bar", "#e3e3e7"),
    ("ctl", "#d5d5da"),
    ("ctlHi", "#ffffff"),
    ("line", "#cdcdd3"),
    ("edge", "#c3c3c9"),
    ("well", "#ffffff"),
    ("seg", "#dadade"),
    ("chip", "#12000000"),
    ("hover", "#000000"),
    ("ink", "#1d1d1f"),
    ("ink2", "#55555c"),
    ("ink3", "#5c5c64"),
    ("brand", "#7a4d22"),
    ("fieldLine", "#85858f"),
    ("accent", "#1f63c2"),
    ("accentHi", "#2a70d4"),
    ("focus", "#1f63c2"),
    ("accentInk", "#ffffff"),
    ("err", "#ab241e"),
    ("warn", "#824b00"),
    ("ok", "#14682a"),
    ("moved", "#1558b0"),
    ("banner", "#d7e5fb"),
    ("dangerBg", "#f7dcda"),
    ("chosen", "#c6d5ec"),
    ("scrim", "#40000000"),
    ("card", "#ffffff"),
];
const LIGHT_HIGH: Table = &[
    ("chosen", "#1f63c2"),
    ("onSelected", "#ffffff"),
    ("onSelectedDim", "#ebffffff"),
    ("ink2", "#26262a"),
    ("ink3", "#3a3a40"),
    ("line", "#6d6d77"),
    ("fieldLine", "#4a4a52"),
];

/// The category colours, in category order: dark themes, then light.
pub const SWATCHES: [(&str, &str, &str); 13] = [
    ("Violet", "#a58cf0", "#6d5fa8"),
    ("Slate", "#aab5c6", "#5f6b78"),
    ("Blue", "#6f8ff0", "#3f7da6"),
    ("Green", "#6fd08a", "#4f8a36"),
    ("Sky", "#4cc6f2", "#2f86b3"),
    ("Rose", "#ee82bb", "#a44f7c"),
    ("Teal", "#3fcab6", "#3f8a80"),
    ("Lime", "#b6d84e", "#6f8420"),
    ("Grey", "#b0b0ba", "#6f6f78"),
    ("Amber", "#d2a94e", "#9a7a1c"),
    ("Coral", "#c98277", "#a8574b"),
    ("Sand", "#c9a36a", "#8a6a2a"),
    ("Orange", "#e09a5c", "#a8621e"),
];

/// `Theme.terms`: a title or label that is one of these is underlined with dots, and its
/// meaning is its tip.
pub const TERMS: [(&str, &str); 13] = [
    (
        "Load order",
        "The order in which the game loads the active mods. A mod that changes another has to load after it.",
    ),
    (
        "Own rules",
        "Rules you added: this mod loads after, or before, that one. Sort follows them before anything else.",
    ),
    (
        "Community rules",
        "Load-order rules written by players (RimSort's Community Rules Database), for mods whose own files do not say enough.",
    ),
    (
        "Detected from files",
        "What a mod's files point at in another mod. A hint for Sort, not something the game requires.",
    ),
    (
        "Texture conflicts",
        "Two active mods provide a picture under the same name. The game uses the one from the mod that loads later.",
    ),
    (
        "Patched methods",
        "The game's methods that this mod's code changes with the Harmony library. Two mods that patch the same method may conflict.",
    ),
    (
        "Group",
        "A mod's first tag. Inside a category, mods with the same first tag are kept side by side under its name.",
    ),
    (
        "Textures set aside",
        "Files RimStack renamed inside this mod so the game skips them. Nothing is deleted.",
    ),
    (
        "Folders the game loads from",
        "A mod can ship a folder per game version or per other mod; the game reads only those that apply.",
    ),
    (
        "Requires",
        "Mods this one needs active, as its About.xml says.",
    ),
    (
        "Filed under",
        "The category Sort keeps this mod in. Categories load in a fixed order: loaders, the game, libraries, content, patches, textures.",
    ),
    (
        "DDS",
        "A compressed texture file the game loads instead of the PNG beside it. Up to 1.6 it loads faster, and RimStack can write one beside each PNG; from 1.7 the game compresses every texture itself.",
    ),
    (
        "To review",
        "Not problems: things to look at when you have time.",
    ),
];

/// The category names as `LoadOrder` gives them, in category order.
pub const CATEGORY_NAMES: [&str; 9] = [
    "Loaders",
    "Game and DLC",
    "Libraries",
    "Content",
    "Patches",
    "Textures",
    "Interface",
    "Performance & tweaks",
    "Loads last",
];

pub const CATEGORY_ICONS: [&str; 9] = [
    "plug",
    "gamepad-2",
    "library",
    "package",
    "wrench",
    "image",
    "layout-dashboard",
    "clock",
    "arrow-down-to-line",
];

pub fn hex(text: &str) -> Hsla {
    let digits = text.trim_start_matches('#');
    let value = u32::from_str_radix(digits, 16).unwrap_or(0);
    let (argb, alpha) = match digits.len() {
        8 => (value & 0x00ff_ffff, (value >> 24) as f32 / 255.0),
        _ => (value, 1.0),
    };
    let rgb = Rgba {
        r: ((argb >> 16) & 0xff) as f32 / 255.0,
        g: ((argb >> 8) & 0xff) as f32 / 255.0,
        b: (argb & 0xff) as f32 / 255.0,
        a: alpha,
    };
    rgb.into()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ThemeName {
    #[default]
    Rim,
    Dark,
    Oled,
    Light,
}

impl ThemeName {
    pub fn name(self) -> &'static str {
        match self {
            Self::Rim => "rim",
            Self::Dark => "dark",
            Self::Oled => "oled",
            Self::Light => "light",
        }
    }

    pub fn named(name: &str) -> Option<Self> {
        Some(match name {
            "rim" => Self::Rim,
            "dark" => Self::Dark,
            "oled" => Self::Oled,
            "light" => Self::Light,
            _ => return None,
        })
    }

    fn tables(self) -> (Table, Table) {
        match self {
            Self::Rim => (RIM, RIM_HIGH),
            Self::Dark => (DARK, DARK_HIGH),
            Self::Oled => (OLED, OLED_HIGH),
            Self::Light => (LIGHT, LIGHT_HIGH),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Appearance {
    pub theme: ThemeName,
    pub text_scale: f32,
    pub large_rows: bool,
    pub comfortable: bool,
    pub high_contrast: bool,
    pub reduce_motion: bool,
    /// The user's own colours for this theme, from theme.json, when they are used.
    pub own: Vec<(String, String)>,
    /// `categoryColours`: a swatch's name for a category painted otherwise, as `3=Rose,5=Teal`.
    pub category_colours: String,
}

impl Default for Appearance {
    fn default() -> Self {
        Self {
            theme: ThemeName::Rim,
            text_scale: 1.0,
            large_rows: false,
            comfortable: true,
            high_contrast: false,
            reduce_motion: false,
            own: Vec::new(),
            category_colours: String::new(),
        }
    }
}

/// `Theme.colourNames`: the colours a theme sets, as theme.json names them.
pub const COLOUR_NAMES: [&str; 28] = [
    "win",
    "panel",
    "bar",
    "ctl",
    "ctlHi",
    "line",
    "edge",
    "well",
    "seg",
    "chip",
    "hover",
    "ink",
    "ink2",
    "ink3",
    "brand",
    "fieldLine",
    "accent",
    "accentHi",
    "focus",
    "accentInk",
    "err",
    "warn",
    "ok",
    "moved",
    "banner",
    "dangerBg",
    "scrim",
    "card",
];

impl Appearance {
    pub fn swatch_of(&self, category: usize) -> Option<&str> {
        self.category_colours.split(',').find_map(|pair| {
            let (at, name) = pair.split_once('=')?;
            (at.parse() == Ok(category) && !name.is_empty()).then_some(name)
        })
    }

    /// `paintCategory`: `categoryColours` with `category` painted `name`, or its own colour for "".
    pub fn painted(&self, category: usize, name: &str) -> String {
        let own = SWATCHES.get(category).map_or("", |s| s.0);
        let mut kept: Vec<(usize, String)> = (0..CATEGORY_NAMES.len())
            .filter(|at| *at != category)
            .filter_map(|at| self.swatch_of(at).map(|n| (at, n.to_string())))
            .collect();
        if !name.is_empty() && name != own {
            kept.push((category, name.to_string()));
            kept.sort();
        }
        kept.iter()
            .map(|(at, name)| format!("{at}={name}"))
            .collect::<Vec<_>>()
            .join(",")
    }

    /// `Theme.colour(key)` as its text: high contrast first, then the user's own, then the theme.
    pub fn colour_text(&self, key: &str) -> Option<String> {
        let (own, high) = self.theme.tables();
        let find = |table: Table| {
            table
                .iter()
                .find(|(k, _)| *k == key)
                .map(|(_, v)| (*v).to_string())
        };
        let raised = self.high_contrast.then(|| find(high)).flatten();
        let mine = self
            .own
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.clone());
        raised.or(mine).or_else(|| find(own))
    }

    /// What `[Appearance]` keeps; `own` gives the theme's own colours when they
    /// are used.
    pub fn of(
        prefs: &rimstack_core::kept::prefs::Prefs,
        own: impl FnOnce(&str) -> Vec<(String, String)>,
    ) -> Self {
        let group = "Appearance";
        let flag = |key: &str, otherwise: bool| prefs.flag(group, key).unwrap_or(otherwise);
        let theme = prefs
            .get(group, "theme")
            .and_then(|name| ThemeName::named(&name))
            .unwrap_or(ThemeName::Rim);
        Self {
            theme,
            text_scale: prefs.number(group, "textScale").map_or(1.0, |n| n as f32),
            large_rows: flag("largeRows", false),
            comfortable: flag("comfortable", true),
            high_contrast: flag("highContrast", false),
            reduce_motion: flag("reduceMotion", false),
            own: if flag("ownColours", false) {
                own(theme.name())
            } else {
                Vec::new()
            },
            category_colours: prefs.get(group, "categoryColours").unwrap_or_default(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Theme {
    pub appearance: Appearance,
    pub colours: Palette,
    pub light: bool,
    pub family: gpui::SharedString,
    pub mono: gpui::SharedString,
    /// The font's ascent and descent over its size: a line is their two roundings.
    pub ascent: f32,
    pub descent: f32,
    /// What a demi-bold and a medium weight give in this family: the nearest face it has.
    pub semibold: gpui::FontWeight,
    pub medium: gpui::FontWeight,
    pub scale: f32,
    pub grow: f32,
}

impl Global for Theme {}

pub const WINDOW: (Pixels, Pixels) = (px(1440.), px(860.));
pub const HANDLE: Pixels = px(7.);
/// The size a font's ascent and descent are read at, to be kept as parts of its size.
pub const PROBE: Pixels = px(1000.);
pub const CORNER: Pixels = px(6.);
pub const CORNER_LARGE: Pixels = px(9.);
pub const GAP_LINE: Pixels = px(1.);
pub const GAP_HAIR: Pixels = px(2.);
pub const GAP_THIN: Pixels = px(3.);
pub const GAP_TINY: Pixels = px(4.);
pub const GAP_SMALL: Pixels = px(6.);
pub const GAP: Pixels = px(8.);
pub const GAP_MEDIUM: Pixels = px(10.);
pub const GAP_LARGE: Pixels = px(12.);
pub const GAP_WIDE: Pixels = px(16.);
pub const GAP_CARD: Pixels = px(14.);
pub const GAP_PAGE: Pixels = px(20.);

impl Theme {
    pub fn new(appearance: Appearance, family: impl Into<gpui::SharedString>) -> Self {
        let (own, high) = appearance.theme.tables();
        let find = |table: Table, key: &str| table.iter().find(|(k, _)| *k == key).map(|(_, v)| *v);
        let ink = find(own, "ink").unwrap_or("#ffffff");
        let ink2 = find(own, "ink2").unwrap_or(ink);
        let colour = |key: &str| {
            let raised = appearance.high_contrast.then(|| find(high, key)).flatten();
            let fallback = match key {
                "onSelected" => ink,
                "onSelectedDim" => ink2,
                _ => "#ff00ff",
            };
            let mine = appearance
                .own
                .iter()
                .find(|(k, _)| k == key)
                .map(|(_, v)| v.as_str());
            hex(raised
                .or(mine)
                .or_else(|| find(own, key))
                .unwrap_or(fallback))
        };
        let scale = appearance.text_scale.clamp(0.85, 1.6);
        Self {
            colours: Palette::of(colour),
            light: appearance.theme == ThemeName::Light,
            family: family.into(),
            mono: "monospace".into(),
            ascent: 1.069,
            descent: 0.293,
            semibold: gpui::FontWeight::SEMIBOLD,
            medium: gpui::FontWeight::MEDIUM,
            scale,
            grow: scale.max(1.0),
            appearance,
        }
    }

    pub fn with_appearance(&self, appearance: Appearance) -> Self {
        Self {
            mono: self.mono.clone(),
            ascent: self.ascent,
            descent: self.descent,
            semibold: self.semibold,
            medium: self.medium,
            ..Self::new(appearance, self.family.clone())
        }
    }

    /// Takes the weights the family has, as CSS numbers; with none known, every weight is kept.
    pub fn with_weights(mut self, has: &[f32]) -> Self {
        self.semibold = gpui::FontWeight(nearest(600., has));
        self.medium = gpui::FontWeight(nearest(500., has));
        self
    }

    fn text(&self, size: f32) -> Pixels {
        px((size * self.scale).round())
    }

    pub fn sized(&self, pixels: f32) -> Pixels {
        px((pixels * self.grow).round())
    }

    pub fn caption(&self) -> Pixels {
        self.text(11.)
    }
    pub fn small(&self) -> Pixels {
        self.text(12.)
    }
    pub fn body(&self) -> Pixels {
        self.text(13.)
    }
    pub fn title(&self) -> Pixels {
        self.text(15.)
    }
    pub fn heading(&self) -> Pixels {
        self.text(17.)
    }
    pub fn display(&self) -> Pixels {
        self.text(26.)
    }
    pub fn row(&self) -> Pixels {
        self.sized(if self.appearance.large_rows { 32. } else { 26. })
    }
    pub fn control(&self) -> Pixels {
        self.sized(24.)
    }
    pub fn control_large(&self) -> Pixels {
        self.sized(28.)
    }
    pub fn field(&self) -> Pixels {
        self.sized(28.)
    }
    pub fn outline(&self) -> Pixels {
        gpui::px(if self.appearance.high_contrast {
            1.
        } else {
            0.
        })
    }
    pub fn segment(&self) -> Pixels {
        self.sized(24.)
    }
    pub fn tag(&self) -> Pixels {
        self.sized(18.)
    }
    pub fn strip(&self) -> Pixels {
        self.sized(26.)
    }
    pub fn header(&self) -> Pixels {
        self.sized(42.)
    }
    pub fn top_bar(&self) -> Pixels {
        self.sized(52.)
    }
    pub fn icon(&self) -> Pixels {
        self.sized(14.)
    }

    /// The ascent and the descent at `size`, each rounded.
    pub fn line(&self, size: Pixels) -> Pixels {
        let size = f32::from(size);
        px((size * self.ascent).round() + (size * self.descent).round())
    }

    pub fn category_colour(&self, category: usize) -> Hsla {
        let own = SWATCHES[category.min(SWATCHES.len() - 1)];
        let chosen = self.appearance.swatch_of(category);
        let (_, dark, light) = chosen
            .and_then(|name| SWATCHES.iter().find(|s| s.0 == name))
            .copied()
            .unwrap_or(own);
        hex(if self.light { light } else { dark })
    }

    pub fn swatch(&self, name: &str) -> Option<Hsla> {
        let (_, dark, light) = SWATCHES.iter().find(|s| s.0 == name)?;
        Some(hex(if self.light { light } else { dark }))
    }
}

/// The closest weight, the lighter of two as close.
fn nearest(wanted: f32, has: &[f32]) -> f32 {
    has.iter()
        .copied()
        .min_by(|a, b| {
            let (da, db) = ((a - wanted).abs(), (b - wanted).abs());
            da.total_cmp(&db).then(a.total_cmp(b))
        })
        .unwrap_or(wanted)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(text_scale: f32) -> Theme {
        Theme::new(
            Appearance {
                text_scale,
                ..Appearance::default()
            },
            "Noto Sans",
        )
    }

    #[test]
    fn sizes_follow_the_text_size_rounded_to_the_pixel() {
        assert_eq!(
            (at(1.).body(), at(1.).row(), at(1.).top_bar()),
            (px(13.), px(26.), px(52.))
        );
        assert_eq!(
            (at(0.85).body(), at(0.85).caption(), at(0.85).row()),
            (px(11.), px(9.), px(26.))
        );
        assert_eq!(
            (at(1.6).body(), at(1.6).display(), at(1.6).row()),
            (px(21.), px(42.), px(42.))
        );
        assert_eq!(at(3.).body(), px(21.));
        let large = Theme::new(
            Appearance {
                large_rows: true,
                ..Appearance::default()
            },
            "Noto Sans",
        );
        assert_eq!(large.row(), px(32.));
    }

    #[test]
    fn colours_come_from_the_theme_then_high_contrast() {
        assert_eq!(at(1.).colours.win, hex("#1c1b19"));
        assert_eq!(at(1.).colours.on_selected, hex("#ede9e2"));
        let high = Theme::new(
            Appearance {
                theme: ThemeName::Light,
                high_contrast: true,
                ..Appearance::default()
            },
            "Noto Sans",
        );
        assert_eq!(
            (high.colours.ink2, high.colours.win),
            (hex("#26262a"), hex("#f5f5f7"))
        );
    }

    #[test]
    fn a_missing_weight_falls_to_the_nearest_lighter_one_as_qt_does() {
        let noto = at(1.).with_weights(&[300., 400., 500., 700., 900.]);
        assert_eq!((noto.semibold.0, noto.medium.0), (500., 500.));
        assert_eq!(at(1.).with_weights(&[400., 600., 700.]).semibold.0, 600.);
        assert_eq!(at(1.).with_weights(&[]).semibold.0, 600.);
    }

    #[test]
    fn a_qt_colour_with_alpha_reads_alpha_first() {
        let scrim = hex("#66000000");
        assert!((scrim.a - 0.4).abs() < 0.01 && scrim.l == 0.0);
    }
}

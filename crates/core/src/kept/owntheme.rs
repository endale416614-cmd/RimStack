//! `theme.json` holds one object per theme name. An older file holds the colours themselves
//! and belongs to the theme in use when it was first read.

use crate::base::translate::named;
use serde_json::{Map, Value};

pub fn is_flat(file: &Value) -> bool {
    file.as_object()
        .is_some_and(|entries| entries.values().any(Value::is_string))
}

/// A flat (older) file gives its colours to `flat_for` alone.
pub fn colours_for(file: &Value, theme: &str, flat_for: &str) -> Map<String, Value> {
    let Some(entries) = file.as_object() else {
        return Map::new();
    };
    let own = match is_flat(file) {
        true if theme == flat_for => Some(entries),
        true => None,
        false => entries.get(theme).and_then(Value::as_object),
    };
    let colours = own.into_iter().flatten();
    colours
        .filter(|(_, colour)| colour.is_string())
        .map(|(name, colour)| (name.clone(), colour.clone()))
        .collect()
}

/// None for a flat file, or one that is no object: those are left as they are.
pub fn with_theme(file: &Value, theme: &str, colours: &Value) -> Option<Value> {
    let entries = file.as_object()?;
    if is_flat(file) || entries.contains_key(theme) {
        return None;
    }
    let mut entries = entries.clone();
    entries.insert(theme.to_string(), colours.clone());
    Some(Value::Object(entries))
}

/// WCAG 2.1 AA: 4.5 for text, 3 for non-text (rings, edges, selected rows).
const PAIRS: [(&str, &str, &str, f64); 11] = [
    (named("Text on the window"), "ink", "win", 4.5),
    (named("Secondary text on the window"), "ink2", "win", 4.5),
    (named("Tertiary text on a panel"), "ink3", "panel", 4.5),
    (named("Tertiary text on the top bar"), "ink3", "bar", 4.5),
    (named("Text on the accent"), "accentInk", "accent", 4.5),
    (named("Warning text on the window"), "warn", "win", 4.5),
    (named("Error text on the window"), "err", "win", 4.5),
    (named("OK text on the window"), "ok", "win", 4.5),
    (
        named("Selected row against the window"),
        "accent",
        "win",
        3.0,
    ),
    (named("Focus ring against the window"), "focus", "win", 3.0),
    (
        named("Field edge against the window"),
        "fieldLine",
        "win",
        3.0,
    ),
];

/// `#aarrggbb` is read without its alpha byte.
fn luminance(colour: &str) -> Option<f64> {
    let hex = colour.strip_prefix('#')?;
    if !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let hex = match hex.len() {
        6 => hex,
        8 => hex.get(2..)?,
        _ => return None,
    };
    let channel = |at: usize| {
        let c = f64::from(u8::from_str_radix(hex.get(at..at + 2)?, 16).ok()?) / 255.0;
        Some(if c <= 0.039_28 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        })
    };
    Some(0.2126 * channel(0)? + 0.7152 * channel(2)? + 0.0722 * channel(4)?)
}

pub fn contrast(a: &str, b: &str) -> Option<f64> {
    let (a, b) = (luminance(a)?, luminance(b)?);
    Some((a.max(b) + 0.05) / (a.min(b) + 0.05))
}

/// A pair with a colour missing or
/// unreadable is left out.
pub fn short_pairs(colours: &Map<String, Value>) -> Vec<(&'static str, f64, f64)> {
    let of = |name: &str| colours.get(name).and_then(Value::as_str);
    PAIRS
        .iter()
        .filter_map(|&(what, a, b, needed)| {
            let found = contrast(of(a)?, of(b)?)?;
            // Compared as shown: to one decimal.
            ((found * 10.0).round() / 10.0 < needed).then_some((what, found, needed))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_file_by_theme_gives_each_theme_its_own_colours() {
        let file = json!({ "oled": { "win": "#111111" }, "light": { "win": "#fafafa", "ink": 3 } });
        assert_eq!(colours_for(&file, "oled", "dark")["win"], "#111111");
        assert_eq!(colours_for(&file, "light", "dark").len(), 1);
        assert!(colours_for(&file, "dark", "dark").is_empty());
        assert!(colours_for(&json!([1]), "dark", "dark").is_empty());
    }

    #[test]
    fn an_older_file_of_colours_belongs_to_one_theme() {
        let file = json!({ "win": "#1e1e20", "ink": "#f2f2f4" });
        assert!(is_flat(&file));
        assert_eq!(colours_for(&file, "oled", "oled").len(), 2);
        assert!(colours_for(&file, "light", "oled").is_empty());
    }

    #[test]
    fn a_theme_is_added_once_and_never_to_an_older_file() {
        let colours = json!({ "win": "#000000" });
        let file = json!({ "oled": { "win": "#111111" } });
        let added = with_theme(&file, "rim", &colours).unwrap();
        assert_eq!(added["rim"]["win"], "#000000");
        assert_eq!(added["oled"]["win"], "#111111");
        assert_eq!(with_theme(&file, "oled", &colours), None);
        assert_eq!(
            with_theme(&json!({ "win": "#1e1e20" }), "rim", &colours),
            None
        );
    }
    #[test]
    fn contrast_is_the_wcag_ratio_and_a_colour_with_anything_but_hex_digits_has_none() {
        assert_eq!(contrast("#000000", "#ffffff").map(f64::round), Some(21.0));
        assert_eq!(contrast("#777777", "#777777"), Some(1.0));
        assert_eq!(contrast("#ff000000", "#ffffff").map(f64::round), Some(21.0));
        assert_eq!(contrast("red", "#ffffff"), None);
        assert_eq!(contrast("#aébbccd", "#000000"), None);
        assert_eq!(contrast("#+f0000", "#000000"), None);
        assert_eq!(contrast("#12345", "#000000"), None);
    }

    #[test]
    fn only_the_pairs_under_their_mark_are_short() {
        let colours = json!({
            "win": "#1e1e20", "panel": "#242426", "bar": "#2c2c2e", "ink": "#f2f2f4", "ink2": "#aeaeb4",
            "ink3": "#8e8e95", "accent": "#1f63c2", "accentInk": "#ffffff", "focus": "#2f7fe6",
            "fieldLine": "#6c728b", "warn": "#f5a93b", "err": "#ff6961", "ok": "#5fcf80",
        });
        let short = short_pairs(colours.as_object().unwrap());
        let named: Vec<&str> = short.iter().map(|(what, _, _)| *what).collect();
        assert_eq!(
            named,
            [
                "Tertiary text on the top bar",
                "Selected row against the window"
            ]
        );
        assert!(short_pairs(json!({ "win": "#000000" }).as_object().unwrap()).is_empty());
    }
}

//! `RIMSTACK_SHAPES=write cargo test -p rimstack-app shapes` writes `shapes.txt` again;
//! the diff is what QML must follow.

use crate::layers::bridges;
use serde::Serialize;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

#[derive(Default)]
struct Shape {
    kinds: BTreeSet<&'static str>,
    objects: usize,
    fields: BTreeMap<String, Shape>,
    items: Option<Box<Shape>>,
}

impl Shape {
    fn add(&mut self, value: &Value, path: &str, maps: &BTreeSet<&str>) {
        match value {
            Value::Null => self.kinds.insert("nothing"),
            Value::Bool(_) => self.kinds.insert("bool"),
            Value::Number(_) => self.kinds.insert("number"),
            Value::String(_) => self.kinds.insert("text"),
            Value::Array(items) => {
                let path = format!("{path}.[]");
                for item in items {
                    let shape = self.items.get_or_insert_with(Default::default);
                    shape.add(item, &path, maps);
                }
                self.kinds.insert("list")
            }
            Value::Object(fields) if maps.contains(path) => {
                let path = format!("{path}.{{}}");
                for value in fields.values() {
                    let shape = self.items.get_or_insert_with(Default::default);
                    shape.add(value, &path, maps);
                }
                self.kinds.insert("map")
            }
            Value::Object(fields) => {
                for (name, shape) in &mut self.fields {
                    if !fields.contains_key(name) {
                        shape.kinds.insert("absent");
                    }
                }
                for (name, value) in fields {
                    let seen_before = self.objects > 0;
                    let shape = self.fields.entry(name.clone()).or_default();
                    if seen_before && shape.kinds.is_empty() {
                        shape.kinds.insert("absent");
                    }
                    shape.add(value, &format!("{path}.{name}"), maps);
                }
                self.objects += 1;
                self.kinds.insert("object")
            }
        };
    }

    /// Seen only empty or null; what they hold when filled is unknown.
    fn unfilled(&self, path: &str, into: &mut BTreeSet<String>) {
        let filled = self
            .kinds
            .iter()
            .any(|k| !["nothing", "absent"].contains(k));
        let holds = self.kinds.contains("list") || self.kinds.contains("map");
        if !filled || (holds && self.items.is_none()) {
            into.insert(path.to_string());
        }
        for (field, shape) in &self.fields {
            shape.unfilled(&format!("{path}.{field}"), into);
        }
        if let Some(items) = &self.items {
            items.unfilled(&format!("{path}.[]"), into);
        }
    }

    fn write(&self, name: &str, depth: usize, out: &mut String) {
        let kinds: Vec<&str> = self.kinds.iter().copied().collect();
        out.push_str(&format!(
            "{}{name}: {}\n",
            "  ".repeat(depth),
            kinds.join(" or ")
        ));
        for (field, shape) in &self.fields {
            shape.write(field, depth + 1, out);
        }
        let inside = if self.kinds.contains("map") {
            "{}"
        } else {
            "[]"
        };
        match &self.items {
            Some(items) => items.write(inside, depth + 1, out),
            None if self.kinds.contains("list") || self.kinds.contains("map") => {
                let unseen = format!("{}{inside}: never seen\n", "  ".repeat(depth + 1));
                out.push_str(&unseen);
            }
            None => {}
        }
    }
}

/// Paths whose keys are data, not fields; a list's items are `[]`.
const MAPS: [&str; 7] = [
    "settings.words",
    "settings.ownColours",
    "mods.columnWidths",
    "mods.details.conflictMarks",
    "mods.detailsOf.conflictMarks",
    "downloads.notes",
    "mods.carried.also",
];

#[derive(Default)]
pub(crate) struct Shapes(BTreeMap<String, Shape>);

impl Shapes {
    /// Values under one name are merged.
    pub(crate) fn add(&mut self, name: &str, value: &impl Serialize) {
        let shown = crate::qml::values::as_shown(value);
        let maps = MAPS.into_iter().collect();
        let shape = self.0.entry(name.to_string()).or_default();
        shape.add(&shown, name, &maps);
    }

    fn written(&self) -> String {
        let mut out = String::new();
        for (name, shape) in &self.0 {
            shape.write(name, 0, &mut out);
        }
        out
    }
}

/// `settings.report` for the `report` property of `settings_data.rs`,
/// `mods.sortMoves` for the `sortMoves` invokable of `mods_data.rs`.
fn handed_over() -> BTreeSet<String> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut names = BTreeSet::new();
    for file in bridges() {
        let text = std::fs::read_to_string(root.join(&file)).unwrap();
        let stem = file.trim_start_matches("src/").trim_end_matches(".rs");
        let object = stem.trim_end_matches("_data");
        let code: Vec<&str> = text
            .lines()
            .map(str::trim)
            .filter(|line| !line.starts_with("//"))
            .collect();
        for line in &code {
            if let Some(rest) = line.strip_prefix("#[qproperty(QVariant, ") {
                let rust = rest.split([',', ')']).next().unwrap();
                names.insert(format!("{object}.{}", cxx_name_in(rest).unwrap_or(rust)));
            }
        }
        // A declaration and its attributes, which may span lines, run from
        // the block or the declaration before it to its `;`.
        for declared in code.join(" ").split(';') {
            let declared = declared.rsplit(['{', '}']).next().unwrap_or_default();
            let Some((_, rest)) = declared.split_once("fn ") else {
                continue;
            };
            if declared.trim_end().ends_with("-> QVariant") {
                let rust = rest.split('(').next().unwrap();
                let shown = cxx_name_in(declared).unwrap_or(rust);
                names.insert(format!("{object}.{shown}"));
            }
        }
    }
    names
}

fn cxx_name_in(attributes: &str) -> Option<&str> {
    let (_, rest) = attributes.split_once("cxx_name = \"")?;
    rest.split('"').next()
}

const UNFILLED: [(&str, &str); 19] = [
    ("logs.since", "a clean launch before the last"),
    (
        "mods.details.megabytes",
        "Steam's record of the Workshop folder",
    ),
    (
        "mods.details.updated",
        "Steam's record of the Workshop folder",
    ),
    (
        "mods.details.patches",
        "a built assembly with Harmony patches",
    ),
    ("mods.details.placement", "the community's rules"),
    ("mods.details.replacedBy", "the list of replaced mods"),
    ("mods.details.replacedById", "the list of replaced mods"),
    (
        "mods.details.replacedByVersions",
        "the list of replaced mods",
    ),
    (
        "mods.details.requires.[].modId",
        "never given: the list takes no action",
    ),
    (
        "mods.detailsOf.megabytes",
        "Steam's record of the Workshop folder",
    ),
    (
        "mods.detailsOf.updated",
        "Steam's record of the Workshop folder",
    ),
    (
        "mods.detailsOf.patches",
        "a built assembly with Harmony patches",
    ),
    ("mods.detailsOf.placement", "the community's rules"),
    ("mods.detailsOf.replacedBy", "the list of replaced mods"),
    ("mods.detailsOf.replacedById", "the list of replaced mods"),
    (
        "mods.detailsOf.replacedByVersions",
        "the list of replaced mods",
    ),
    (
        "mods.detailsOf.requires.[].modId",
        "never given: the list takes no action",
    ),
    ("mods.overview.communityMods", "the community's rules"),
    ("mods.overview.communityTime", "the community's rules"),
];

#[test]
fn every_field_is_seen_filled_but_those_the_made_up_install_cannot_fill() {
    let mut unfilled = BTreeSet::new();
    for (name, shape) in &crate::shapes_of::every().0 {
        shape.unfilled(name, &mut unfilled);
    }
    let known: BTreeSet<String> = UNFILLED.iter().map(|(path, _)| path.to_string()).collect();
    let new: Vec<&String> = unfilled.difference(&known).collect();
    let filled_now: Vec<&String> = known.difference(&unfilled).collect();
    assert!(
        new.is_empty() && filled_now.is_empty(),
        "never filled: {new:?}; filled now, out of UNFILLED: {filled_now:?}"
    );
}

/// A model's cells, read by role: the roles are its fields.
const NOT_RECORDS: [&str; 1] = ["mod_rows.data"];

#[test]
fn every_value_handed_to_qml_has_its_shape_written() {
    let shapes = crate::shapes_of::every();
    let given: BTreeSet<String> = shapes.0.keys().cloned().collect();
    let mut handed = handed_over();
    for name in NOT_RECORDS {
        assert!(handed.remove(name), "{name} is no longer handed over");
    }
    let missing: Vec<&String> = handed.difference(&given).collect();
    let gone: Vec<&String> = given.difference(&handed).collect();
    assert!(
        missing.is_empty() && gone.is_empty(),
        "with no shape: {missing:?}; no longer handed over: {gone:?}"
    );
}

#[test]
fn every_value_handed_to_qml_keeps_its_shape() {
    let written = crate::shapes_of::every().written();
    let file = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/shapes.txt");
    if std::env::var_os("RIMSTACK_SHAPES").is_some_and(|w| w == "write") {
        std::fs::write(&file, &written).unwrap();
    }
    let expected = std::fs::read_to_string(&file).unwrap_or_default();
    if expected != written {
        let differ: Vec<String> = diff_lines(&expected, &written);
        panic!(
            "shapes differ from shapes.txt (RIMSTACK_SHAPES=write writes it):\n{}",
            differ.join("\n")
        );
    }
}

fn diff_lines(expected: &str, written: &str) -> Vec<String> {
    let before: BTreeSet<&str> = expected.lines().collect();
    let after: BTreeSet<&str> = written.lines().collect();
    let gone = before.difference(&after).map(|l| format!("- {l}"));
    let new = after.difference(&before).map(|l| format!("+ {l}"));
    gone.chain(new).collect()
}

#[test]
fn a_shape_merges_the_kinds_and_fields_of_every_value_seen() {
    let mut shapes = Shapes::default();
    shapes.add(
        "x",
        &serde_json::json!([{ "a": 1, "b": "t" }, { "a": null, "c": [] }]),
    );
    assert_eq!(
        shapes.written(),
        "x: list\n  []: object\n    a: nothing or number\n    b: absent or text\n    c: absent or list\n      []: never seen\n"
    );
}

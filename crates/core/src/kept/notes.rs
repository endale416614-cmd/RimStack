//! Kept by package id, so they follow a mod wherever it is installed.

use crate::base::error::Error;
use crate::rimworld::id;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Note {
    pub tags: Vec<String>,
    pub text: String,
    pub version_ok: bool,
    /// By `Category::key`; empty leaves it to RimStack.
    pub category: String,
    /// Problems not to warn about, by `Trouble::key`.
    pub ignored: Vec<String>,
}

impl Note {
    pub fn group(&self) -> &str {
        self.tags.first().map_or("", String::as_str)
    }

    /// An empty name removes the first tag, whichever it is.
    pub fn set_group(&mut self, tag: &str) {
        let tag = tag.trim();
        if tag.is_empty() {
            if !self.tags.is_empty() {
                self.tags.remove(0);
            }
            return;
        }
        self.tags.retain(|t| !t.eq_ignore_ascii_case(tag));
        self.tags.insert(0, tag.to_string());
    }

    pub fn remove_tag(&mut self, tag: &str) {
        self.tags.retain(|t| !t.eq_ignore_ascii_case(tag.trim()));
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Notes {
    by_id: BTreeMap<String, Note>,
}

pub fn tags_in(text: &str) -> Vec<String> {
    let mut tags: Vec<String> = Vec::new();
    for tag in text.split(',').map(str::trim).filter(|t| !t.is_empty()) {
        if !tags.iter().any(|seen| seen.eq_ignore_ascii_case(tag)) {
            tags.push(tag.to_string());
        }
    }
    tags
}

impl Notes {
    /// A missing or damaged file reads as no notes.
    pub fn read(path: &Path) -> Self {
        let Some(Value::Object(entries)) = crate::base::atomic_file::read_json(path) else {
            return Self::default();
        };
        let note = |v: &Value| -> Note {
            let tags = v.get("tags").and_then(Value::as_array);
            let tag = |t: &Value| t.as_str().map(String::from);
            let mut read = Note {
                tags: tags.map_or(Vec::new(), |t| t.iter().filter_map(tag).collect()),
                text: v
                    .get("note")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
                version_ok: v.get("versionOk").and_then(Value::as_bool).unwrap_or(false),
                category: v
                    .get("category")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
                ignored: v
                    .get("ignored")
                    .and_then(Value::as_array)
                    .map_or(Vec::new(), |t| t.iter().filter_map(tag).collect()),
            };
            // An older file's `sub` is read as the first tag.
            if let Some(sub) = v.get("sub").and_then(Value::as_str) {
                read.set_group(sub);
            }
            read
        };
        Self {
            by_id: entries
                .iter()
                .map(|(k, v)| (id::fold(k), note(v)))
                .collect(),
        }
    }

    pub fn write(&self, path: &Path) -> Result<(), Error> {
        let entries: serde_json::Map<String, Value> = self
            .by_id
            .iter()
            .map(|(k, n)| {
                let mut entry = json!({ "tags": n.tags, "note": n.text });
                if n.version_ok {
                    entry["versionOk"] = json!(true);
                }
                if !n.category.is_empty() {
                    entry["category"] = json!(n.category);
                }
                if !n.ignored.is_empty() {
                    entry["ignored"] = json!(n.ignored);
                }
                (k.clone(), entry)
            })
            .collect();
        let text = serde_json::to_string_pretty(&Value::Object(entries))
            .map_err(|e| Error::format(format!("{}: {e}", path.display())))?;
        crate::base::atomic_file::write(path, text)
    }

    pub fn get(&self, package_id: &str) -> Option<&Note> {
        self.by_id.get(&id::fold(package_id))
    }

    /// A note that says nothing removes the mod's entry.
    pub fn set(&mut self, package_id: &str, note: Note) {
        let key = id::fold(package_id);
        let says_nothing = note.tags.is_empty() && note.text.trim().is_empty();
        let keeps = note.version_ok || !note.category.is_empty() || !note.ignored.is_empty();
        if says_nothing && !keeps {
            self.by_id.remove(&key);
        } else {
            self.by_id.insert(key, note);
        }
    }

    /// Folded package ids, as `id::fold` gives them.
    pub fn version_ok(&self) -> impl Iterator<Item = &str> {
        let trusted = self.by_id.iter().filter(|(_, n)| n.version_ok);
        trusted.map(|(k, _)| k.as_str())
    }

    /// By folded package id, each with the problems ignored for it.
    pub fn ignored(&self) -> impl Iterator<Item = (&str, &[String])> {
        let some = self.by_id.iter().filter(|(_, n)| !n.ignored.is_empty());
        some.map(|(k, n)| (k.as_str(), n.ignored.as_slice()))
    }

    pub fn len(&self) -> usize {
        self.by_id.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tags_are_split_on_commas_trimmed_and_kept_once() {
        assert_eq!(
            tags_in(" medieval, QoL ,, qol,keep "),
            ["medieval", "QoL", "keep"]
        );
        assert!(tags_in(" , ").is_empty());
    }

    #[test]
    fn a_note_is_kept_by_id_in_any_case_and_an_empty_one_forgotten() {
        let mut notes = Notes::default();
        let note = Note {
            tags: tags_in("keep"),
            text: "breaks on 1.5".into(),
            ..Note::default()
        };
        notes.set("Author.Mod", note.clone());
        assert_eq!(notes.get("author.mod"), Some(&note));
        notes.set("AUTHOR.MOD", Note::default());
        assert!(notes.is_empty());
    }

    #[test]
    fn notes_read_back_as_written_and_damage_reads_as_none() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("data").join("notes.json");
        let mut notes = Notes::default();
        notes.set(
            "a.a",
            Note {
                tags: tags_in("x, y"),
                text: "hello\nthere".into(),
                category: "Interface".into(),
                ..Note::default()
            },
        );
        let trusted = Note {
            version_ok: true,
            ..Note::default()
        };
        notes.set("B.B", trusted);
        assert_eq!(notes.version_ok().collect::<Vec<_>>(), ["b.b"]);
        let ignoring = Note {
            ignored: vec!["missing".into()],
            ..Note::default()
        };
        notes.set("C.c", ignoring);
        assert_eq!(
            notes.ignored().count(),
            1,
            "a note with only an ignored problem is kept"
        );
        notes.write(&path).unwrap();
        assert_eq!(Notes::read(&path), notes);
        std::fs::write(&path, "nope").unwrap();
        assert!(Notes::read(&path).is_empty());
    }

    #[test]
    fn the_first_tag_is_the_group_and_grouping_moves_a_tag_first() {
        let mut note = Note {
            tags: tags_in("keep, Genes"),
            ..Note::default()
        };
        assert_eq!(note.group(), "keep");
        note.set_group("genes");
        assert_eq!(note.tags, ["genes", "keep"]);
        note.set_group("Animals");
        assert_eq!(note.tags, ["Animals", "genes", "keep"]);
        note.set_group(" ");
        assert_eq!(note.group(), "genes");
        note.remove_tag("KEEP");
        assert_eq!(note.tags, ["genes"]);
        assert_eq!(Note::default().group(), "");
    }

    #[test]
    fn a_sub_category_of_an_earlier_version_is_read_as_the_first_tag() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("notes.json");
        let old = r#"{ "a.a": { "tags": ["keep"], "note": "", "sub": "Genes" }, "b.b": { "tags": [], "note": "", "sub": "" } }"#;
        std::fs::write(&path, old).unwrap();
        let notes = Notes::read(&path);
        assert_eq!(notes.get("a.a").unwrap().tags, ["Genes", "keep"]);
        assert!(notes.get("b.b").unwrap().tags.is_empty());
        notes.write(&path).unwrap();
        assert!(!std::fs::read_to_string(&path).unwrap().contains("\"sub\""));
    }
}

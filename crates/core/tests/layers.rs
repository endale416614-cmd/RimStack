use std::path::Path;

const LAYERS: [(&str, &[&str]); 5] = [
    ("base", &["base"]),
    ("rimworld", &["base", "rimworld"]),
    ("mods", &["base", "rimworld", "mods"]),
    ("kept", &["base", "rimworld", "kept"]),
    ("sources", &["base", "rimworld", "mods", "kept", "sources"]),
];

fn files(dir: &Path, into: &mut Vec<std::path::PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        match path.is_dir() {
            true => files(&path, into),
            false => into.push(path),
        }
    }
}

/// The layers `text` names: `crate::mods::…`, `crate::mods;`, a group `crate::{…, mods::…}`,
/// or `super::` as many times as `depth` (how deep the file's module is) and then the layer.
fn named(text: &str, depth: usize) -> Vec<&'static str> {
    let mut found = Vec::new();
    let ends = |rest: &str| {
        rest.is_empty() || rest.starts_with("::") || rest.starts_with([';', ',', '}', ' '])
    };
    for (layer, _) in LAYERS {
        let direct = format!("crate::{layer}");
        let by_path = text
            .match_indices(&direct)
            .any(|(at, _)| ends(&text[at + direct.len()..]));
        let grouped = |at: usize| {
            let rest = &text[at..];
            let end = rest.find("};").unwrap_or(rest.len());
            let inside = &rest[..end];
            [format!("{{{layer}"), format!(" {layer}")]
                .iter()
                .any(|start| {
                    inside
                        .match_indices(start.as_str())
                        .any(|(at, _)| ends(&inside[at + start.len()..]))
                })
        };
        let in_group = text.match_indices("crate::{").any(|(at, _)| grouped(at));
        let from_root = format!("{}{layer}::", "super::".repeat(depth));
        let climbed = depth > 0
            && text
                .match_indices(&from_root)
                .any(|(at, _)| !text[..at].ends_with("super::"));
        if by_path || in_group || climbed {
            found.push(layer);
        }
    }
    found
}

/// How many `super::` lead from `file`'s module to the crate's root.
fn depth(src: &Path, file: &Path) -> usize {
    let inside = file.strip_prefix(src).unwrap();
    let parts = inside.components().count();
    match file.file_name().and_then(|n| n.to_str()) {
        Some("mod.rs") => parts - 1,
        _ => parts,
    }
}

#[test]
fn no_layer_uses_one_above_it() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut wrong = Vec::new();
    for (layer, allowed) in LAYERS {
        let mut all = vec![src.join(format!("{layer}.rs"))];
        files(&src.join(layer), &mut all);
        for file in all {
            let text = std::fs::read_to_string(&file).unwrap();
            for used in named(&text, depth(&src, &file))
                .into_iter()
                .filter(|used| !allowed.contains(used))
            {
                wrong.push(format!("{} uses {used}", file.display()));
            }
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

#[test]
fn a_use_of_a_layer_is_found_written_either_way() {
    assert_eq!(named("use crate::mods::library::Library;", 1), ["mods"]);
    assert_eq!(
        named(
            "use crate::{\n    base::error::Error,\n    kept::notes,\n};",
            1
        ),
        ["base", "kept"]
    );
    assert_eq!(named("use crate::mods;", 1), ["mods"]);
    assert_eq!(
        named("use crate::{kept::{a, b}, mods};", 1),
        ["mods", "kept"]
    );
    assert_eq!(named("use super::super::sources::x;", 2), ["sources"]);
    assert!(named("use super::super::sources::x;", 3).is_empty());
    assert!(named("use crate::modsy::x; use std::collections::HashMap;", 1).is_empty());
}

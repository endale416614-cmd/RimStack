use rimstack_core::base::translate::{Counting, LOCALE, misfit};
use serde_json::{Map, Value};
use std::fs;
use std::path::{Path, PathBuf};

fn read(file: &Path) -> Map<String, Value> {
    let text = std::fs::read_to_string(file).unwrap();
    serde_json::from_str(&text).unwrap()
}

#[test]
fn every_language_has_every_text_and_each_wording_fits_its_english() {
    let assets = Path::new(env!("CARGO_MANIFEST_DIR")).join("../app/assets");
    let english = read(&assets.join("words.json"));
    let mut faults = Vec::new();
    let mut languages = 0;
    for entry in std::fs::read_dir(assets.join("languages")).unwrap() {
        let file = entry.unwrap().path();
        let name = file.file_stem().unwrap().to_string_lossy().into_owned();
        let words = read(&file);
        languages += 1;
        let locale = words
            .get(LOCALE)
            .and_then(Value::as_str)
            .unwrap_or_default();
        if locale.trim().is_empty() {
            faults.push(format!("{name}: no {LOCALE}"));
        }
        let forms = Counting::of(locale).plural_forms();
        for text in english.keys().filter(|text| !words.contains_key(*text)) {
            faults.push(format!("{name}: lacks {text:?}"));
        }
        for (text, said) in words.iter().filter(|(text, _)| *text != LOCALE) {
            let said = said.as_str().unwrap_or_default();
            if !english.contains_key(text) {
                faults.push(format!("{name}: {text:?} is no longer a text"));
            } else if said.is_empty() {
                faults.push(format!("{name}: {text:?} has no wording"));
            } else if let Some(why) = misfit(text, said, forms) {
                faults.push(format!("{name}: {text:?} as {said:?}: {why:?}"));
            }
        }
    }
    assert!(languages >= 7, "only {languages} language files were found");
    assert!(faults.is_empty(), "{}", faults.join("\n"));
}

fn files_under(dir: &Path, extension: &str, into: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        if path.is_dir() {
            files_under(&path, extension, into);
        } else if path.extension().is_some_and(|x| x == extension) {
            into.push(path);
        }
    }
}

/// Counts go through `say_n!`: `say!` with a bare `{} mods` reads "1 mods".
#[test]
fn every_counted_sentence_has_its_singular() {
    const NOUNS: [&str; 17] = [
        "mods",
        "files",
        "textures",
        "lines",
        "minutes",
        "hours",
        "days",
        "months",
        "years",
        "seconds",
        "rules",
        "errors",
        "saves",
        "problems",
        "warnings",
        "conflicts",
        "commits",
    ];
    let app = Path::new(env!("CARGO_MANIFEST_DIR")).join("../app");
    let mut sources = Vec::new();
    files_under(&app.join("src"), "rs", &mut sources);
    files_under(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
        "rs",
        &mut sources,
    );
    let mut found = Vec::new();
    for file in sources {
        let text = fs::read_to_string(&file).unwrap();
        // Rust texts below the tests are not shown (tools/words.py stops there too).
        let shown = text.split("#[cfg(test)]").next().unwrap_or_default();
        for (n, line) in shown.lines().enumerate() {
            if counts_without_singular(line, &NOUNS) {
                let short = file
                    .strip_prefix(&app)
                    .unwrap_or(&file)
                    .display()
                    .to_string();
                found.push(format!("{short}:{}", n + 1));
            }
        }
    }
    assert!(
        found.is_empty(),
        "{} counted sentences without a singular:\n{}",
        found.len(),
        found.join("\n")
    );
}

fn counts_without_singular(line: &str, nouns: &[&str]) -> bool {
    ["fill(\"", "say!(\""].iter().any(|opener| {
        line.match_indices(opener).any(|(at, _)| {
            let literal = line[at + opener.len()..]
                .split('"')
                .next()
                .unwrap_or_default();
            nouns.iter().any(|noun| {
                let counted = format!("{{}} {noun}");
                literal.match_indices(&counted).any(|(i, _)| {
                    let after = &literal[i + counted.len()..];
                    !after.starts_with(|c: char| c.is_alphanumeric())
                })
            })
        })
    })
}

/// Nothing checks the parts at compile time, and `Sentence::put` drops a missing part silently.
#[test]
fn every_sentence_macro_has_as_many_parts_as_places() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut files = Vec::new();
    for krate in ["core/src", "app/src"] {
        collect_rs(&root.join(krate), &mut files);
    }
    let mut calls = 0;
    let mut wrong = Vec::new();
    for file in &files {
        let text = std::fs::read_to_string(file).unwrap();
        for (name, args, line) in macro_calls(&text) {
            let Some(first) = args.first().filter(|a| a.starts_with('"')) else {
                continue;
            };
            calls += 1;
            let places = |literal: &str| literal.matches("{}").count();
            let fits = match name {
                "say_n!" => {
                    args.len() >= 3 && {
                        let parts = args.len() - 3;
                        places(&args[1]) == parts + 1 && places(&args[2]) == parts + 1
                    }
                }
                _ => places(first) == args.len() - 1,
            };
            if !fits {
                wrong.push(format!(
                    "{}:{line}: {name}({})",
                    file.display(),
                    args.join(", ")
                ));
            }
        }
    }
    assert!(calls > 150, "only {calls} calls found: the scan is broken");
    assert!(wrong.is_empty(), "{wrong:#?}");
}

fn collect_rs(dir: &Path, into: &mut Vec<std::path::PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_rs(&path, into);
        } else if path.extension().is_some_and(|x| x == "rs") {
            into.push(path);
        }
    }
}

fn macro_calls(text: &str) -> Vec<(&'static str, Vec<String>, usize)> {
    let bytes = text.as_bytes();
    let mut found = Vec::new();
    for name in ["say!(", "said!(", "say_n!("] {
        let mut from = 0;
        while let Some(at) = text[from..].find(name) {
            let start = from + at;
            from = start + name.len();
            if start > 0 && (bytes[start - 1].is_ascii_alphanumeric() || bytes[start - 1] == b'_') {
                continue;
            }
            let (mut depth, mut in_text, mut escaped) = (1, false, false);
            let (mut args, mut current) = (Vec::new(), String::new());
            for c in text[from..].chars() {
                if in_text {
                    current.push(c);
                    match c {
                        _ if escaped => escaped = false,
                        '\\' => escaped = true,
                        '"' => in_text = false,
                        _ => {}
                    }
                    continue;
                }
                match c {
                    '"' => in_text = true,
                    '(' | '[' | '{' => depth += 1,
                    ')' | ']' | '}' => depth -= 1,
                    ',' if depth == 1 => {
                        args.push(std::mem::take(&mut current).trim().to_string());
                        continue;
                    }
                    _ => {}
                }
                if depth == 0 {
                    break;
                }
                current.push(c);
            }
            if !current.trim().is_empty() {
                args.push(current.trim().to_string());
            }
            let line = text[..start].matches('\n').count() + 1;
            found.push((&name[..name.len() - 1], args, line));
        }
    }
    found
}

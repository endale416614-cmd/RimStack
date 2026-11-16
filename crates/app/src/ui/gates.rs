//! A size in a view follows the text size or a measure, never a fixed count of pixels: a
//! two-digit `px(…)` belongs in `theme.rs`, where the Theme's tokens are. A hairline (a border,
//! a bar) stays a pixel or three.

use std::path::Path;

fn views(dir: &Path, into: &mut Vec<std::path::PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        if path.is_dir() {
            views(&path, into);
        } else if name.ends_with(".rs")
            && !name.ends_with("tests.rs")
            && !matches!(name.as_str(), "theme.rs" | "gates.rs" | "layout_checks.rs")
        {
            into.push(path);
        }
    }
}

fn pixels(line: &str) -> Vec<String> {
    let mut found = Vec::new();
    for (at, _) in line.match_indices("px(") {
        let before = line[..at].chars().next_back().unwrap_or(' ');
        if before.is_alphanumeric() || matches!(before, '_' | '.') {
            continue;
        }
        let mut depth = 1;
        let inside: String = line[at + 3..]
            .chars()
            .take_while(|c| {
                depth += match c {
                    '(' => 1,
                    ')' => -1,
                    _ => 0,
                };
                depth > 0
            })
            .collect();
        let mut after = ' ';
        let mut run = String::new();
        for c in inside.chars().chain([' ']) {
            match c.is_ascii_digit() {
                true if run.is_empty()
                    && (after.is_alphanumeric() || matches!(after, '_' | '.')) => {}
                true => run.push(c),
                false if !run.is_empty() => found.push(std::mem::take(&mut run)),
                false => {}
            }
            if !c.is_ascii_digit() || run.is_empty() {
                after = c;
            }
        }
    }
    found
}

#[test]
fn no_view_sizes_a_thing_in_fixed_pixels() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/ui");
    let mut files = Vec::new();
    views(&root, &mut files);
    let mut found = Vec::new();
    for file in files {
        let text = std::fs::read_to_string(&file).unwrap();
        let shown = text
            .split("#[cfg(test)]\nmod tests {")
            .next()
            .unwrap_or_default();
        for (n, line) in shown.lines().enumerate() {
            if pixels(line).iter().any(|digits| digits.len() >= 2) {
                found.push(format!(
                    "{}:{}: {}",
                    file.strip_prefix(&root).unwrap().display(),
                    n + 1,
                    line.trim()
                ));
            }
        }
    }
    assert!(
        found.is_empty(),
        "sizes in fixed pixels; use a Theme token or a measure:\n{}",
        found.join("\n")
    );
}

/// A file drawn with `img` stays in gpui's own store, with its place in the atlas, until the
/// program ends: a view draws a file through `ui::pictures` (`pictures.img(…)`), which lets it
/// go, or through `ui::thumbs`, which draws it decoded.
#[test]
fn no_view_draws_a_file_that_is_never_let_go() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/ui");
    let mut files = Vec::new();
    views(&root, &mut files);
    let mut found = Vec::new();
    for file in files {
        if file.ends_with("pictures.rs") {
            continue;
        }
        let text = std::fs::read_to_string(&file).unwrap();
        let shown = text
            .split("#[cfg(test)]\nmod tests {")
            .next()
            .unwrap_or_default();
        for (n, line) in shown.lines().enumerate() {
            let bare = line.match_indices("img(").any(|(at, _)| {
                let before = line[..at].chars().next_back().unwrap_or(' ');
                let argument = &line[at..];
                !(before == '.' || before == '_' || before.is_alphanumeric())
                    && (argument.contains("PathBuf") || argument.contains("path"))
            });
            if bare {
                found.push(format!(
                    "{}:{}: {}",
                    file.strip_prefix(&root).unwrap().display(),
                    n + 1,
                    line.trim()
                ));
            }
        }
    }
    assert!(
        found.is_empty(),
        "files drawn with nothing to let them go; draw them with `pictures.img`:\n{}",
        found.join("\n")
    );
}

#[test]
fn no_section_holds_another_sections_view() {
    const SECTIONS: [(&str, &str); 8] = [
        ("mods", "Mods"),
        ("downloads", "Downloads"),
        ("textures", "Textures"),
        ("workshop", "Workshop"),
        ("saves", "Saves"),
        ("logs", "Logs"),
        ("find", "Find"),
        ("settings", "Settings"),
    ];
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/ui");
    let mut files = Vec::new();
    views(&root, &mut files);
    let mut found = Vec::new();
    for file in files {
        let within = file.strip_prefix(&root).unwrap();
        let first = within.iter().next().unwrap().to_string_lossy();
        let Some((own, _)) = SECTIONS
            .iter()
            .find(|(module, _)| first.trim_end_matches(".rs") == *module)
        else {
            continue;
        };
        let text = std::fs::read_to_string(&file).unwrap();
        for (n, line) in text.lines().enumerate() {
            let held = SECTIONS
                .iter()
                .filter(|(module, _)| module != own)
                .any(|(_, view)| {
                    line.match_indices("Entity<").any(|(at, _)| {
                        let inside = line[at + 7..].split(['>', ',']).next().unwrap_or_default();
                        inside.rsplit("::").next() == Some(*view)
                    })
                });
            if held {
                found.push(format!("{}:{}: {}", within.display(), n + 1, line.trim()));
            }
        }
    }
    assert!(
        found.is_empty(),
        "a section holds another's view; read its store and ask through `navigate::go`:\n{}",
        found.join("\n")
    );
}

#[test]
fn the_check_sees_a_fixed_size() {
    assert_eq!(pixels("div().w(px(120.)).h(px(2.))"), ["120", "2"]);
    assert_eq!(pixels(".top(px(-12.))"), ["12"]);
    assert_eq!(pixels("px(width + 100.)"), ["100"]);
    assert_eq!(pixels("px(0.25).max(px(f(2) * 3.5))"), ["0", "2", "3"]);
    assert!(pixels("px(size_16)").is_empty());
    assert!(pixels(".px(theme.sized(40.)).mx_px(12)").is_empty());
}

fn sources(dir: &Path, into: &mut Vec<std::path::PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        if path.is_dir() {
            sources(&path, into);
        } else if path.extension().is_some_and(|e| e == "rs")
            && !path.to_string_lossy().ends_with("tests.rs")
        {
            into.push(path);
        }
    }
}

/// Each kind of change the `App` tells is published somewhere and named by a view that reads it:
/// one no view names is either not shown or shown stale.
#[test]
fn each_change_is_published_and_read_by_name() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let declared = std::fs::read_to_string(root.join("changes.rs")).unwrap();
    let names: Vec<&str> = declared
        .lines()
        .filter_map(|line| line.trim().strip_prefix("pub const "))
        .filter(|rest| rest.contains("Self(1 <<"))
        .filter_map(|rest| rest.split(':').next())
        .collect();
    assert!(names.len() > 20);
    let mut files = Vec::new();
    sources(&root, &mut files);
    let (mut published, mut read) = (Vec::new(), Vec::new());
    for file in files.iter().filter(|f| !f.ends_with("changes.rs")) {
        let text = std::fs::read_to_string(file).unwrap();
        let publishes = text.contains("publish(") || !file.starts_with(root.join("ui"));
        for name in &names {
            if text.contains(&format!("Changes::{name}")) {
                match publishes {
                    true => published.push(*name),
                    false => read.push(*name),
                }
            }
        }
    }
    let unpublished: Vec<_> = names.iter().filter(|n| !published.contains(n)).collect();
    let unread: Vec<_> = names.iter().filter(|n| !read.contains(n)).collect();
    assert!(unpublished.is_empty(), "never published: {unpublished:?}");
    assert!(unread.is_empty(), "read by no view: {unread:?}");
}

fn change_consts(text: &str) -> std::collections::HashMap<String, Vec<String>> {
    let mut raw = std::collections::HashMap::new();
    let mut rest = text;
    while let Some(at) = rest.find("const ") {
        let after = &rest[at + 6..];
        let Some((name, body)) = after.split_once(": Changes =") else {
            rest = after;
            continue;
        };
        let body = &body[..body.find(';').unwrap_or(body.len())];
        raw.insert(name.trim().to_string(), body.to_string());
        rest = after;
    }
    let names: Vec<String> = raw.keys().cloned().collect();
    let mut resolved = std::collections::HashMap::new();
    for name in &names {
        let mut bits = Vec::new();
        let mut asked = vec![name.clone()];
        while let Some(next) = asked.pop() {
            let body = &raw[&next];
            for word in body.split(|c: char| !(c.is_alphanumeric() || c == '_' || c == ':')) {
                match word.strip_prefix("Changes::") {
                    Some(bit) => bits.push(bit.to_string()),
                    None if raw.contains_key(word) && word != next => asked.push(word.to_string()),
                    None => {}
                }
            }
        }
        resolved.insert(name.clone(), bits);
    }
    resolved
}

/// A view told only what it `READS` never tests for a change outside it: such a test is dead.
#[test]
fn each_change_a_view_tests_for_is_among_what_it_reads() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/ui");
    let mut files = Vec::new();
    sources(&root, &mut files);
    let mut checked = 0;
    for file in files {
        let text = std::fs::read_to_string(&file).unwrap();
        let consts = change_consts(&text);
        let hears = !file.ends_with("gates.rs")
            && (text.contains("super::hear(") || text.contains("ui::hear("));
        let Some(reads) = consts.get("READS") else {
            assert!(!hears, "{} hears the App without a READS", file.display());
            continue;
        };
        if reads.iter().any(|bit| bit == "ALL") {
            continue;
        }
        checked += 1;
        let mut outside = Vec::new();
        let mut in_const = false;
        for line in text.lines() {
            if line.trim_start().starts_with("const ") && line.contains(": Changes =") {
                in_const = true;
            }
            if !in_const && !line.contains("publish") {
                for word in line.split(|c: char| !(c.is_alphanumeric() || c == '_' || c == ':')) {
                    if let Some(bit) = word.strip_prefix("Changes::")
                        && !matches!(bit, "NONE" | "ALL")
                        && !reads.iter().any(|read| read == bit)
                    {
                        outside.push(bit.to_string());
                    }
                }
            }
            if in_const && line.contains(';') {
                in_const = false;
            }
        }
        assert!(
            outside.is_empty(),
            "{} tests for {outside:?}, which it does not read",
            file.display()
        );
    }
    assert!(checked >= 11, "only {checked} views declare what they read");
}

use std::path::{Path, PathBuf};

/// The most there may be; lower them as QML loses some, never raise them.
const MOST: [(&str, usize); 7] = [
    // Raised by this fork for the RimSort layout and the actions on several mods.
    ("functions", 177),
    ("timers", 11),
    ("arrow functions", 272),
    ("ifs", 236),
    ("ternaries", 754),
    ("array callbacks", 64),
    ("comparisons with a text", 32),
];

fn qml_files(dir: &Path, into: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        match path.is_dir() {
            true => qml_files(&path, into),
            false if path.extension().is_some_and(|x| x == "qml") => into.push(path),
            false => {}
        }
    }
}

fn every_qml() -> Vec<(PathBuf, String)> {
    let mut found = Vec::new();
    qml_files(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("qml"),
        &mut found,
    );
    assert!(found.len() > 40, "the QML was not found");
    found
        .into_iter()
        .map(|path| {
            let text = std::fs::read_to_string(&path).unwrap();
            (path, text)
        })
        .collect()
}

fn code_lines(all: &[(PathBuf, String)]) -> Vec<(String, String)> {
    let mut lines = Vec::new();
    for (path, text) in all {
        for (at, line) in text.lines().enumerate() {
            if !line.trim_start().starts_with("//") {
                lines.push((format!("{}:{}", path.display(), at + 1), line.to_string()));
            }
        }
    }
    lines
}

#[test]
fn the_qml_hands_over_no_json_and_reads_no_command_line() {
    let lines = code_lines(&every_qml());
    let found: Vec<&String> = lines
        .iter()
        .filter(|(_, line)| line.contains("JSON.") || line.contains("Qt.application.arguments"))
        .map(|(at, _)| at)
        .collect();
    assert!(found.is_empty(), "{found:?}");
}

fn words(line: &str, word: &str) -> usize {
    let bytes = line.as_bytes();
    let starts =
        |at: usize| at == 0 || !(bytes[at - 1].is_ascii_alphanumeric() || bytes[at - 1] == b'_');
    line.match_indices(word)
        .filter(|(at, _)| starts(*at))
        .count()
}

fn logic_in(line: &str) -> [usize; 7] {
    let callbacks = [
        "filter",
        "map",
        "some",
        "every",
        "reduce",
        "find",
        "findIndex",
        "forEach",
    ];
    let compared = ["=== \"", "!== \""]
        .iter()
        .flat_map(|op| line.match_indices(op));
    let with_text = compared.filter(|(at, op)| !line[at + op.len()..].starts_with('"'));
    [
        words(line, "function "),
        words(line, "Timer {"),
        line.matches("=>").count(),
        words(line, "if ("),
        line.matches(" ? ").count(),
        callbacks
            .iter()
            .map(|name| line.matches(&format!(".{name}(")).count())
            .sum(),
        with_text.count(),
    ]
}

#[test]
fn the_logic_in_the_qml_only_grows_less() {
    let mut shown = every_qml();
    shown.retain(|(path, _)| !path.parent().is_some_and(|dir| dir.ends_with("checks")));
    let mut found = [0; 7];
    for (_, line) in code_lines(&shown) {
        for (total, here) in found.iter_mut().zip(logic_in(&line)) {
            *total += here;
        }
    }
    let over: Vec<String> = MOST
        .iter()
        .zip(found)
        .filter(|((_, most), found)| found > most)
        .map(|((what, most), found)| format!("{found} {what}, at most {most}"))
        .collect();
    assert!(over.is_empty(), "{}", over.join("; "));
}

#[test]
fn each_kind_of_logic_is_counted_where_it_is_written() {
    let line = r#"if (a) x = list.filter(m => m.kind === "git" && m.name !== "") ? b : c; function f() {}"#;
    assert_eq!(logic_in(line), [1, 0, 1, 1, 1, 1, 1]);
    assert_eq!(logic_in("Timer { id: elif (x) }"), [0, 1, 0, 0, 0, 0, 0]);
}

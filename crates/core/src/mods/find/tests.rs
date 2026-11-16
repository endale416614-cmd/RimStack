use super::*;
use crate::base::pattern::{Pattern, Switches};
use std::fs;
use std::path::Path;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, SystemTime};

fn write(root: &Path, file: &str, text: &str) {
    let path = root.join(file);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

fn plain(text: &str) -> Pattern {
    Pattern::new(text, Switches::default()).unwrap()
}

fn mods(tmp: &Path, names: &[&str]) -> Vec<Target<String>> {
    names
        .iter()
        .map(|name| Target::Mod {
            key: name.to_string(),
            folder: tmp.join(name),
        })
        .collect()
}

fn run(
    targets: &[Target<String>],
    pattern: &Pattern,
    settings: FolderSettings,
) -> (Vec<FileHits<String>>, Summary) {
    run_with(
        targets,
        pattern,
        settings,
        &FileLists::default(),
        &AtomicBool::new(false),
    )
}

fn run_with(
    targets: &[Target<String>],
    pattern: &Pattern,
    settings: FolderSettings,
    lists: &FileLists,
    cancel: &AtomicBool,
) -> (Vec<FileHits<String>>, Summary) {
    let got = Mutex::new(Vec::new());
    let summary = search(
        targets,
        pattern,
        &settings,
        lists,
        cancel,
        usize::MAX,
        |batch| got.lock().unwrap().extend(batch.found),
    );
    let mut got = got.into_inner().unwrap();
    got.sort_by(|a, b| a.file.cmp(&b.file));
    (got, summary)
}

fn names(found: &[FileHits<String>], tmp: &Path) -> Vec<String> {
    found
        .iter()
        .map(|f| {
            f.file
                .strip_prefix(tmp)
                .unwrap()
                .to_string_lossy()
                .into_owned()
        })
        .collect()
}

#[test]
fn xml_only_leaves_out_named_folders_at_any_depth_and_all_text_takes_the_rest() {
    let tmp = tempfile::tempdir().unwrap();
    let t = tmp.path();
    write(t, "m/Defs/a.xml", "steel");
    write(t, "m/Defs/a.txt", "steel");
    write(t, "m/Languages/English/b.xml", "steel");
    write(t, "m/1.5/Defs/Source/c.xml", "steel");
    write(t, "m/x/textures/d.xml", "steel");
    write(t, "m/.git/e.xml", "steel");
    let targets = mods(t, &["m"]);
    let (found, _) = run(&targets, &plain("steel"), FolderSettings::default());
    assert_eq!(names(&found, t), ["m/Defs/a.xml"]);
    let all = FolderSettings {
        types: FileTypes::Any,
        ..Default::default()
    };
    let (found, _) = run(&targets, &plain("steel"), all);
    assert_eq!(names(&found, t), ["m/Defs/a.txt", "m/Defs/a.xml"]);
    let none = FolderSettings {
        languages: false,
        source: false,
        textures: false,
        git: false,
        ..Default::default()
    };
    let (found, _) = run(&targets, &plain("steel"), none);
    assert_eq!(found.len(), 5);
}

#[test]
fn binary_and_large_files_are_skipped_and_counted_in_all_text_mode() {
    let tmp = tempfile::tempdir().unwrap();
    let t = tmp.path();
    write(t, "m/a.txt", "steel");
    fs::write(t.join("m/b.bin"), b"steel\0\0steel").unwrap();
    let big = fs::File::create(t.join("m/big.txt")).unwrap();
    big.set_len((16 << 20) + 1).unwrap();
    let all = FolderSettings {
        types: FileTypes::Any,
        ..Default::default()
    };
    let (found, summary) = run(&mods(t, &["m"]), &plain("steel"), all);
    assert_eq!(names(&found, t), ["m/a.txt"]);
    assert_eq!(
        (summary.binary, summary.too_large, summary.files),
        (1, 1, 1)
    );
}

#[test]
fn every_match_is_found_across_a_long_file_with_line_numbers_and_context() {
    let tmp = tempfile::tempdir().unwrap();
    let t = tmp.path();
    let filler = "x".repeat(99);
    let mut text = String::new();
    let mut lines = 0;
    while text.len() < (1 << 20) - 150 {
        text.push_str(&filler);
        text.push('\n');
        lines += 1;
    }
    let lead = "x".repeat((1 << 20) - text.len() - 3);
    text.push_str(&lead);
    text.push_str("ste");
    text.push_str("el tail\nsteel\n");
    write(t, "m/a.xml", &format!("steel\n{text}"));
    let (found, summary) = run(&mods(t, &["m"]), &plain("steel"), FolderSettings::default());
    let hits = &found[0].hits;
    assert_eq!(summary.matches, 3);
    assert_eq!(
        hits.iter().map(|h| h.line).collect::<Vec<_>>(),
        [1, lines + 2, lines + 3]
    );
    assert!(text.len() > 1 << 20);
    assert!(hits[0].before.is_empty());
    assert_eq!(hits[0].after.len(), 3);
    assert_eq!(hits[1].before.len(), 3);
    assert_eq!(hits[1].after, ["steel"]);
    assert!(hits[2].after.is_empty());
    assert_eq!(&hits[2].text[hits[2].marked.clone()], "steel");
}

#[test]
fn a_long_line_is_cut_around_the_match_at_a_character_boundary() {
    let tmp = tempfile::tempdir().unwrap();
    let t = tmp.path();
    let line = format!("{}steel{}", "é".repeat(500), "é".repeat(500));
    write(t, "m/a.xml", &line);
    let (found, _) = run(&mods(t, &["m"]), &plain("steel"), FolderSettings::default());
    let hit = &found[0].hits[0];
    assert!(hit.text.chars().count() < 400);
    assert_eq!(&hit.text[hit.marked.clone()], "steel");
}

#[test]
fn a_bad_regex_is_an_error_before_a_search() {
    let regex = Switches {
        regex: true,
        ..Default::default()
    };
    assert!(Pattern::new("[", regex).is_err());
}

#[test]
fn settings_files_are_named_by_the_caller() {
    let tmp = tempfile::tempdir().unwrap();
    let t = tmp.path();
    write(t, "Config/Mod_Foo_Settings.xml", "steel");
    write(t, "Config/Prefs.xml", "steel");
    let target = Target::Settings {
        folder: t.join("Config"),
        key_of: Box::new(|name| format!("key:{name}")),
    };
    let (found, _) = run(&[target], &plain("steel"), FolderSettings::default());
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].key, "key:Mod_Foo_Settings.xml");
}

#[test]
fn cancel_stops_before_the_next_file() {
    let tmp = tempfile::tempdir().unwrap();
    let t = tmp.path();
    for i in 0..50 {
        write(t, &format!("m{i}/a.xml"), "steel");
    }
    let names: Vec<String> = (0..50).map(|i| format!("m{i}")).collect();
    let names: Vec<&str> = names.iter().map(String::as_str).collect();
    let cancel = AtomicBool::new(true);
    let (found, summary) = run_with(
        &mods(t, &names),
        &plain("steel"),
        FolderSettings::default(),
        &FileLists::default(),
        &cancel,
    );
    assert!(found.is_empty());
    assert!(summary.cancelled);
    let stopper = AtomicBool::new(false);
    let seen = Mutex::new(0);
    let summary = search(
        &mods(t, &names),
        &plain("steel"),
        &FolderSettings::default(),
        &FileLists::default(),
        &stopper,
        usize::MAX,
        |batch| {
            *seen.lock().unwrap() += batch.found.len();
            stopper.store(true, std::sync::atomic::Ordering::Relaxed);
        },
    );
    assert!(summary.cancelled && *seen.lock().unwrap() < 50);
}

#[test]
fn progress_counts_targets_done_of_total() {
    let tmp = tempfile::tempdir().unwrap();
    let t = tmp.path();
    write(t, "a/x.xml", "steel");
    write(t, "b/x.xml", "steel");
    let last = Mutex::new(Progress::default());
    search(
        &mods(t, &["a", "b"]),
        &plain("steel"),
        &FolderSettings::default(),
        &FileLists::default(),
        &AtomicBool::new(false),
        usize::MAX,
        |batch| {
            let mut last = last.lock().unwrap();
            last.total = batch.progress.total;
        },
    );
    assert_eq!(last.into_inner().unwrap().total, 2);
}

fn touch_dir(dir: &Path, later: u64) {
    let file = fs::File::open(dir).unwrap();
    file.set_modified(SystemTime::now() + Duration::from_secs(later))
        .unwrap();
}

#[test]
fn a_file_list_is_reused_until_a_folder_inside_changes() {
    let tmp = tempfile::tempdir().unwrap();
    let t = tmp.path();
    write(t, "m/a/b/one.xml", "");
    let lists = FileLists::default();
    let settings = FolderSettings::default();
    let first = lists.all(&t.join("m"), &settings);
    assert!(std::sync::Arc::ptr_eq(
        &first,
        &lists.all(&t.join("m"), &settings)
    ));
    write(t, "m/a/b/two.xml", "");
    touch_dir(&t.join("m/a/b"), 100);
    let again = lists.all(&t.join("m"), &settings);
    assert!(!std::sync::Arc::ptr_eq(&first, &again));
    assert_eq!(again.len(), 2);
    write(t, "m/a/b/three.txt", "");
    touch_dir(&t.join("m/a/b"), 200);
    let any = FolderSettings {
        types: FileTypes::Any,
        ..settings.clone()
    };
    assert_eq!(lists.get(&t.join("m"), &any).len(), 3);
    assert_eq!(lists.get(&t.join("m"), &settings).len(), 2);
}

#[test]
fn a_preview_is_a_window_around_the_line() {
    let tmp = tempfile::tempdir().unwrap();
    let t = tmp.path();
    let text: String = (1..=1000).map(|n| format!("line {n}\r\n")).collect();
    write(t, "a.xml", &text);
    let middle = preview(&t.join("a.xml"), 500).unwrap();
    assert_eq!(middle.first_line, 350);
    assert_eq!(middle.lines.len(), 301);
    assert_eq!(middle.lines[middle.marked.unwrap()], "line 500");
    let start = preview(&t.join("a.xml"), 2).unwrap();
    assert_eq!((start.first_line, start.marked), (1, Some(1)));
    let end = preview(&t.join("a.xml"), 1000).unwrap();
    assert_eq!(end.lines.last().unwrap(), "line 1000");
    assert_eq!(preview(&t.join("a.xml"), 5000).unwrap().marked, None);
    assert!(preview(&t.join("none.xml"), 1).is_err());
}

/// `RIMSTACK_FIND_MODS=<folder of mod folders> cargo test --release -p rimstack-core measure_find -- --ignored --nocapture`
#[test]
#[ignore]
fn measure_find() {
    let Some(root) = std::env::var_os("RIMSTACK_FIND_MODS") else {
        return;
    };
    let targets: Vec<Target<String>> = fs::read_dir(root)
        .unwrap()
        .flatten()
        .filter(|e| e.path().is_dir())
        .map(|e| Target::Mod {
            key: e.file_name().to_string_lossy().into_owned(),
            folder: e.path(),
        })
        .collect();
    let lists = FileLists::default();
    let regex = Switches {
        regex: true,
        ..Default::default()
    };
    let patterns = [
        ("steel", Pattern::new("steel", Switches::default()).unwrap()),
        ("steel", Pattern::new("steel", Switches::default()).unwrap()),
        (
            r"<defName>\w*Steel\w*</defName>",
            Pattern::new(r"<defName>\w*Steel\w*</defName>", regex).unwrap(),
        ),
    ];
    for (name, pattern) in patterns {
        let started = std::time::Instant::now();
        let (found, summary) = run_with(
            &targets,
            &pattern,
            FolderSettings::default(),
            &lists,
            &AtomicBool::new(false),
        );
        println!(
            "{name}: {:?}, {} mods, {summary:?}, {} files with matches",
            started.elapsed(),
            targets.len(),
            found.len()
        );
    }
}

#[test]
fn past_the_most_hits_matches_are_counted_and_files_still_named() {
    let tmp = tempfile::tempdir().unwrap();
    let t = tmp.path();
    write(t, "a/x.xml", "steel\nsteel\nsteel");
    write(t, "b/x.xml", "steel\nsteel");
    let got = Mutex::new(Vec::new());
    let summary = search(
        &mods(t, &["a", "b"]),
        &plain("steel"),
        &FolderSettings::default(),
        &FileLists::default(),
        &AtomicBool::new(false),
        4,
        |batch| got.lock().unwrap().extend(batch.found),
    );
    let got = got.into_inner().unwrap();
    assert_eq!(summary.matches, 5);
    assert_eq!(got.len(), 2);
    assert_eq!(got.iter().map(|f| f.hits.len()).sum::<usize>(), 4);
    assert_eq!(got.iter().map(|f| f.unkept).sum::<usize>(), 1);
}

#[test]
fn typed_file_types_choose_the_files() {
    let tmp = tempfile::tempdir().unwrap();
    let t = tmp.path();
    write(t, "m/a.xml", "steel");
    write(t, "m/b.cs", "steel");
    write(t, "m/c.json", "steel");
    let types = |text: &str| FolderSettings {
        types: FileTypes::parse(text).unwrap(),
        ..Default::default()
    };
    let (found, _) = run(&mods(t, &["m"]), &plain("steel"), types("cs; JSON"));
    assert_eq!(names(&found, t), ["m/b.cs", "m/c.json"]);
}

#[test]
fn the_lines_with_a_match_inside_a_file_are_listed_once_each() {
    let tmp = tempfile::tempdir().unwrap();
    let t = tmp.path();
    write(t, "a.xml", "one\r\nSteel steel\nthree\nSTEEL\n");
    let found = inside(&t.join("a.xml"), "steel").unwrap();
    assert_eq!(found, [2, 4]);
    assert!(inside(&t.join("a.xml"), "zinc").unwrap().is_empty());
    assert!(inside(&t.join("a.xml"), "").unwrap().is_empty());
    assert!(inside(&t.join("missing.xml"), "a").is_err());
}

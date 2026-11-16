use super::*;
use crate::workspace::find::type_offers;
use crate::workspace::find::{Look, Shown};
use crate::workspace::tests::installed;
use std::sync::Mutex;

fn write(app_root: &std::path::Path, file: &str, text: &str) {
    let path = app_root.join(file);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

fn made_up() -> (tempfile::TempDir, App) {
    let (tmp, app) = installed();
    let root = tmp.path();
    write(
        root,
        "game/Data/Core/Defs/core.xml",
        "<Defs>\nSteel\n</Defs>",
    );
    write(
        root,
        "game/Mods/roads/Defs/roads.xml",
        "<Defs>\n<a>steel plate</a>\n<b>Steel</b>\n</Defs>",
    );
    write(root, "game/Mods/roads/Languages/English/words.xml", "Steel");
    write(
        root,
        "workshop/900001/About/About.xml",
        "<ModMetaData><packageId>Ann.Shop</packageId><name>Shop</name></ModMetaData>",
    );
    write(
        root,
        "workshop/900001/Patches/p.xml",
        "<Patch>Steel</Patch>",
    );
    write(
        root,
        "config/Config/Mod_roads_RoadSettings.xml",
        "<Settings>Steel</Settings>",
    );
    write(
        root,
        "config/Config/Mod_gone_Other.xml",
        "<Settings>Steel</Settings>",
    );
    write(
        root,
        "config/Config/Mod_roads_Plain.xml",
        "<Settings>none</Settings>",
    );
    app.rescan();
    (tmp, app)
}

fn folder_id(app: &App, folder: &str) -> ModId {
    let loaded = app.library();
    let (library, _) = loaded.as_ref().as_ref().unwrap();
    library.ids().find(|(_, m)| m.folder == folder).unwrap().0
}

fn ask(text: &str, place: Place) -> Request {
    Request {
        text: text.into(),
        place,
        ..Request::default()
    }
}

fn run(app: &App, request: &Request) -> (Vec<(Option<String>, String)>, Summary) {
    let started = app.find_start(request).unwrap();
    let found = Mutex::new(Vec::new());
    let summary = app.find_search(&started, |batch| {
        found.lock().unwrap().push(batch.clone());
        app.find_add(started.generation, batch).unwrap();
    });
    app.find_finish(started.generation, summary).unwrap();
    let loaded = app.library();
    let (library, _) = loaded.as_ref().as_ref().unwrap();
    let mut seen: Vec<(Option<String>, String)> = found
        .into_inner()
        .unwrap()
        .into_iter()
        .flat_map(|b| b.found)
        .map(|f| {
            let folder = f.key.map(|id| library.get(id).unwrap().folder.clone());
            (
                folder,
                f.file.file_name().unwrap().to_string_lossy().into_owned(),
            )
        })
        .collect();
    seen.sort();
    (seen, summary)
}

fn sorted<const N: usize>(mut all: [(Option<String>, String); N]) -> Vec<(Option<String>, String)> {
    all.sort();
    all.to_vec()
}

fn pair(folder: Option<&str>, file: &str) -> (Option<String>, String) {
    (folder.map(String::from), file.to_string())
}

#[test]
fn each_place_resolves_to_its_mods_and_the_settings_to_the_mod_they_belong_to() {
    let (_tmp, app) = made_up();
    let core = pair(Some("Core"), "core.xml");
    let roads = pair(Some("roads"), "roads.xml");
    let shop = pair(Some("900001"), "p.xml");
    assert_eq!(
        run(&app, &ask("steel", Place::Active)).0,
        vec![core.clone()]
    );
    assert_eq!(
        run(&app, &ask("steel", Place::Inactive)).0,
        sorted([roads.clone(), shop.clone()])
    );
    assert_eq!(
        run(&app, &ask("steel", Place::Installed)).0,
        sorted([core, roads.clone(), shop.clone()])
    );
    assert_eq!(
        run(&app, &ask("steel", Place::Local)).0,
        vec![roads.clone()]
    );
    assert_eq!(run(&app, &ask("steel", Place::Workshop)).0, [shop]);
    let one = Request {
        selected: Some(folder_id(&app, "roads")),
        ..ask("steel", Place::Selected)
    };
    assert_eq!(run(&app, &one).0, [roads]);
    assert!(run(&app, &ask("steel", Place::Selected)).0.is_empty());
    assert_eq!(
        run(&app, &ask("steel", Place::Settings)).0,
        [
            pair(None, "Mod_gone_Other.xml"),
            pair(Some("roads"), "Mod_roads_RoadSettings.xml")
        ]
    );
}

#[test]
fn the_folders_left_out_and_the_files_taken_follow_the_request() {
    let (_tmp, app) = made_up();
    let mut request = ask("steel", Place::Local);
    request.folders.languages = false;
    let (found, _) = run(&app, &request);
    assert_eq!(found.len(), 2);
    request.types = ".".into();
    assert_eq!(run(&app, &request).0.len(), 2);
    request.types = "cs".into();
    assert!(run(&app, &request).0.is_empty());
}

#[test]
fn the_mods_with_hits_are_what_in_search_keeps_and_a_newer_search_replaces_them() {
    let (_tmp, app) = made_up();
    let told = Arc::new(Mutex::new(0));
    let counted = told.clone();
    app.listen(move |changes| {
        if changes.contains(Changes::FIND) {
            *counted.lock().unwrap() += 1;
        }
    });
    assert!(app.find_mods().is_empty());
    run(&app, &ask("steel", Place::Installed));
    let (core, roads, shop) = (
        folder_id(&app, "Core"),
        folder_id(&app, "roads"),
        folder_id(&app, "900001"),
    );
    assert_eq!(*app.find_mods(), HashSet::from([core, roads, shop]));
    run(&app, &ask("steel", Place::Installed));
    assert_eq!(*told.lock().unwrap(), 1, "the same mods are no change");
    run(&app, &ask("plate", Place::Installed));
    assert_eq!(*app.find_mods(), HashSet::from([roads]));
    run(&app, &ask("nothing like this", Place::Installed));
    assert!(app.find_mods().is_empty());
}

#[test]
fn hits_past_the_cap_are_not_kept_and_the_summary_says_so() {
    let (tmp, app) = made_up();
    let lines = "Steel\n".repeat(MOST_HITS + 5);
    write(tmp.path(), "game/Mods/roads/Defs/many.xml", &lines);
    let started = app.find_start(&ask("steel", Place::Local)).unwrap();
    let summary = app.find_search(&started, |batch| {
        app.find_add(started.generation, batch);
    });
    assert_eq!(summary.matches, MOST_HITS + 5 + 2);
    let said = app.find_finish(started.generation, summary).unwrap();
    assert!(crate::words::put(&said).contains("showing the first 20,000"));
    app.find_results(|r| {
        assert!(r.capped());
        let hits = r
            .refs()
            .iter()
            .filter(|&&at| matches!(r.shown(at), Some(Shown::Line { matched: true, .. })))
            .count();
        assert_eq!(hits, MOST_HITS);
        assert_eq!(r.mods_found(), 1);
    });
}

#[test]
fn a_row_resolves_to_its_file_folder_line_and_preview_without_a_path_from_outside() {
    let (tmp, app) = made_up();
    run(&app, &ask("steel", Place::Local));
    let rows = app.find_results(|r| r.refs());
    assert!(rows[0].is_mod() && rows[1].is_file());
    assert!(app.find_file(rows[0]).is_none());
    let roads = tmp.path().join("game/Mods/roads/Defs/roads.xml");
    assert_eq!(app.find_file(rows[1]), Some(roads.clone()));
    let matched = app.find_results(|r| {
        rows.iter()
            .copied()
            .find(|&at| matches!(r.shown(at), Some(Shown::Line { matched: true, .. })))
    });
    let matched = matched.unwrap();
    assert_eq!(app.find_file(matched), Some(roads.clone()));
    assert_eq!(app.find_folder(matched), roads.parent().map(PathBuf::from));
    assert_eq!(
        app.find_line(matched).as_deref(),
        Some("<a>steel plate</a>")
    );
    let looked = |look| {
        let (file, line) = app.find_located(matched).unwrap();
        app.find_preview(&file, line, look)
    };
    let shown = looked(Look::default()).shown;
    assert_eq!(shown.line, 2);
    assert!(shown.problem.is_none());
    let marked: Vec<usize> = shown
        .lines
        .iter()
        .filter(|l| l.marked)
        .map(|l| l.number)
        .collect();
    assert_eq!(marked, [2]);
    std::fs::remove_file(&roads).unwrap();
    assert!(looked(Look::default()).shown.problem.is_some());
}

#[test]
fn a_newer_search_stops_the_older_and_its_batches_are_not_taken() {
    let (_tmp, app) = made_up();
    let first = app.find_start(&ask("steel", Place::Installed)).unwrap();
    let second = app.find_start(&ask("steel", Place::Active)).unwrap();
    assert!(first.cancel.load(Ordering::Relaxed) && !second.cancel.load(Ordering::Relaxed));
    let summary = app.find_search(&first, |batch| {
        assert!(app.find_add(first.generation, batch).is_none());
    });
    assert!(summary.cancelled && summary.matches == 0);
    assert!(app.find_finish(first.generation, summary).is_none());
    app.find_stop();
    assert!(second.cancel.load(Ordering::Relaxed));
    let stopped = app.find_search(&second, |_| {});
    assert!(stopped.cancelled);
}

#[test]
fn a_bad_regex_is_said_before_anything_runs_and_an_empty_box_is_no_problem() {
    let (_tmp, app) = made_up();
    let regex = Switches {
        regex: true,
        ..Switches::default()
    };
    assert!(app.find_check("", regex).is_ok());
    assert!(app.find_check("(", regex).is_err());
    assert!(app.find_check("(", Switches::default()).is_ok());
    let bad = Request {
        switches: regex,
        ..ask("(", Place::Active)
    };
    assert!(app.find_start(&bad).is_err());
    let bad_types = Request {
        types: "xml, a/b".into(),
        ..ask("Steel", Place::Active)
    };
    assert!(app.find_prepare(&bad_types).is_err());
    assert!(app.find_prepare(&ask("Steel", Place::Active)).is_ok());
}

#[test]
fn the_last_ten_searches_are_kept_and_recalled_with_their_switches() {
    let (_tmp, app) = made_up();
    assert!(app.find_recent().is_empty());
    for n in 0..12 {
        app.find_remember(&ask(&format!("word{n}"), Place::Active), false);
    }
    let mut last = ask("last", Place::Selected);
    last.switches.regex = true;
    last.types = ".".into();
    last.folders.git = false;
    app.find_remember(&last, false);
    last.switches.case = true;
    app.find_remember(&last, true);
    let recent = app.find_recent();
    assert_eq!(recent.len(), 10);
    assert_eq!(recent[0].text, "last");
    assert!(
        recent[0].case,
        "the options changed replace the search before"
    );
    assert_eq!(recent[1].text, "word11");
    let recalled = app.find_recall(0, None).unwrap();
    assert_eq!(recalled.place, Place::Active, "no mod is selected");
    assert!(recalled.switches.regex && recalled.types == "." && !recalled.folders.git);
    let mod_id = folder_id(&app, "roads");
    let again = app.find_recall(0, Some(mod_id)).unwrap();
    assert_eq!(
        (again.place, again.selected),
        (Place::Selected, Some(mod_id))
    );
    assert!(app.find_recall(10, None).is_none());
}

#[test]
fn the_options_say_what_is_chosen_and_whether_the_selected_mod_can_be_searched() {
    let (_tmp, app) = made_up();
    let mut request = Request::default();
    request.flip("regex");
    request.types = "xml, a/b".into();
    request.choose("inactive");
    let options = app.find_options(&request);
    let chosen =
        |list: &[Choice]| -> Vec<&str> { list.iter().filter(|c| c.chosen).map(|c| c.id).collect() };
    assert_eq!(chosen(&options.switches), ["regex"]);
    assert_eq!(chosen(&options.places), ["inactive"]);
    assert!(options.types.problem.is_some());
    assert_eq!(options.types.text, "xml, a/b");
    assert_eq!(chosen(&options.leave).len(), 4);
    assert!(
        !options
            .places
            .iter()
            .find(|c| c.id == "selected")
            .unwrap()
            .available
    );
    request.selected = Some(folder_id(&app, "roads"));
    let options = app.find_options(&request);
    assert!(
        options
            .places
            .iter()
            .find(|c| c.id == "selected")
            .unwrap()
            .available
    );
}

#[test]
fn a_settings_file_is_named_for_the_longest_folder_it_starts_with() {
    let folders = HashMap::from([
        ("a".to_string(), ModId::new(0, 1)),
        ("a_b".to_string(), ModId::new(0, 2)),
    ]);
    let of = |name| mod_of_settings(name, &folders);
    assert_eq!(of("Mod_a_b_Settings.xml"), Some(ModId::new(0, 2)));
    assert_eq!(of("Mod_a_Settings.xml"), Some(ModId::new(0, 1)));
    assert_eq!(of("Mod_c_Settings.xml"), None);
    assert_eq!(of("NotMod_a_Settings.xml"), None);
}

#[test]
fn a_search_inside_the_file_finds_lines_steps_around_and_centres_the_window_on_the_match() {
    let (tmp, app) = made_up();
    let file = tmp.path().join("game/Mods/roads/Defs/roads.xml");
    let look = |text: &str, found: Option<Vec<usize>>, at, step| {
        let look = Look {
            text: text.into(),
            found,
            at,
            step,
        };
        app.find_preview(&file, 1, look)
    };
    let first = look("STEEL", None, None, 0);
    assert_eq!((first.found.clone(), first.at), (vec![2, 3], None));
    let marked: Vec<(usize, bool)> = (first.shown.lines.iter())
        .filter(|l| l.found)
        .map(|l| (l.number, l.here))
        .collect();
    assert_eq!(marked, [(2, false), (3, false)]);
    let next = look("steel", Some(vec![2, 3]), None, 1);
    assert_eq!((next.at, next.shown.line), (Some(0), 2));
    let after = look("steel", Some(vec![2, 3]), next.at, 1);
    assert_eq!((after.at, after.shown.line), (Some(1), 3));
    assert!(after.shown.lines.iter().any(|l| l.here && l.number == 3));
    let around = look("steel", Some(vec![2, 3]), after.at, 1);
    assert_eq!(around.at, Some(0));
    let back = look("steel", Some(vec![2, 3]), Some(0), -1);
    assert_eq!(back.at, Some(1));
    let none = look("zinc", None, None, 1);
    assert_eq!((none.found.len(), none.at, none.shown.line), (0, None, 1));
}

#[test]
fn extensions_are_counted_per_place_and_leave_the_folders_left_out() {
    let (_tmp, app) = made_up();
    let counted = |request: &Request| app.find_extensions(request);
    let kept = |place| counted(&ask("", place));
    assert_eq!(kept(Place::Active), [("xml".to_string(), 2)]);
    assert_eq!(kept(Place::Workshop), [("xml".to_string(), 2)]);
    assert_eq!(kept(Place::Local), [("xml".to_string(), 2)]);
    let mut all = ask("", Place::Local);
    all.folders.languages = false;
    assert_eq!(counted(&all), [("xml".to_string(), 3)]);
    let offers = type_offers(&kept(Place::Workshop), "x");
    assert_eq!((offers[0].ext.as_str(), offers[0].count), ("xml", 2));
}

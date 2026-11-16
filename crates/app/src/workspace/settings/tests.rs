use super::*;
use crate::workspace::tests::installed;
use std::sync::{Arc, Mutex};

fn heard(app: &App) -> Arc<Mutex<Changes>> {
    let heard = Arc::new(Mutex::new(Changes::NONE));
    let into = heard.clone();
    app.listen(move |changes| *into.lock().unwrap() |= changes);
    heard
}

#[test]
fn the_kept_card_counts_saves_snapshots_and_rules() {
    let (_tmp, app) = installed();
    let summary = app.summary();
    let shown: Vec<(String, String)> = summary
        .kept
        .iter()
        .map(|k| (k.label.to_string(), k.value.to_string()))
        .collect();
    assert_eq!(
        shown,
        [
            ("Saves", "0"),
            ("Snapshots", "0"),
            ("Own rules", "0"),
            ("Community rules", "Not downloaded"),
        ]
        .map(|(label, value)| (label.to_string(), value.to_string()))
    );
    assert_eq!(summary.kept_note.to_string(), "0 saves, 0 snapshots");
}

#[test]
fn a_setting_tells_what_follows_from_it() {
    let (_tmp, app) = installed();
    let heard = heard(&app);
    app.keep_setting("game", "/elsewhere").unwrap();
    assert!(heard.lock().unwrap().contains(Changes::FOLDERS));
    app.keep_setting("fileHints", "on").unwrap();
    assert!(heard.lock().unwrap().contains(Changes::PROBLEMS));
    assert!(app.keep_setting("noSuchSetting", "x").is_err());
}

#[test]
fn updating_by_itself_is_offered_until_answered_either_way() {
    let (_tmp, app) = installed();
    let report = app.settings_report();
    assert!(!report.auto_updates && !report.auto_updates_asked);
    for (answer, on) in [("on", true), ("off", false)] {
        app.keep_setting("autoUpdates", answer).unwrap();
        let report = app.settings_report();
        assert_eq!((report.auto_updates, report.auto_updates_asked), (on, true));
    }
}

#[test]
fn the_time_between_update_checks_is_one_of_those_offered() {
    let (_tmp, app) = installed();
    let report = app.settings_report();
    assert_eq!(UPDATE_EVERY[report.update_every_at], "10");
    assert_eq!(report.update_every_said.to_string(), "Every 10 minutes");
    assert!(!report.update_every_set);
    app.keep_setting("updateEvery", "60").unwrap();
    let report = app.settings_report();
    assert_eq!(report.update_every_said.to_string(), "Every hour");
    assert_eq!(report.update_every[0].to_string(), "Every minute");
    assert!(report.update_every_set);
}

#[test]
fn a_database_switched_off_joins_those_already_off() {
    let (_tmp, app) = installed();
    app.use_database("names", false).unwrap();
    app.use_database("rules", false).unwrap();
    assert_eq!(app.settings().get("databasesOff"), Some("rules,names"));
    app.use_database("names", true).unwrap();
    assert_eq!(app.settings().get("databasesOff"), Some("rules"));
    let on: Vec<bool> = app.databases().iter().map(|d| d.on).collect();
    assert_eq!(on, [false, true, true, true]);
}

#[test]
fn the_index_is_fetched_with_the_names_off_until_it_exists() {
    let (_tmp, app) = installed();
    app.use_database("names", false).unwrap();
    assert!(app.names_wanted());
    rimstack_core::rimworld::workshopindex::Index::new(Vec::new())
        .store(&rimstack_core::rimworld::workshopindex::file_in(
            app.data_dir(),
        ))
        .unwrap();
    assert!(!app.names_wanted());
    app.use_database("names", true).unwrap();
    assert!(app.names_wanted());
}

#[test]
fn version_warnings_are_set_aside_for_the_installed_version_only() {
    let (_tmp, app) = installed();
    app.version_warnings(1).unwrap();
    let installed = app.paths().game_version.clone().unwrap();
    assert_eq!(
        app.settings().get("versionAsideFor"),
        Some(installed.as_str())
    );
    app.version_warnings(2).unwrap();
    assert!(!app.settings().switch(Switch::VersionWarnings));
    assert_eq!(app.settings().get("versionAsideFor"), None);
}

#[test]
fn a_language_file_is_started_once_and_listed_after_the_shipped_ones() {
    let (_tmp, app) = installed();
    assert!(app.start_language("a/b").is_err());
    let file = app.start_language("Klingon").unwrap();
    std::fs::write(&file, r#"{"Save": "Pol"}"#).unwrap();
    assert_eq!(app.start_language("Klingon").unwrap(), file);
    assert_eq!(
        std::fs::read_to_string(&file).unwrap(),
        r#"{"Save": "Pol"}"#
    );
    let names = app.languages();
    assert_eq!(names.first().map(String::as_str), Some("English"));
    assert_eq!(names.last().map(String::as_str), Some("Klingon"));
    assert_eq!(app.language_table("Klingon")["Save"], "Pol");
}

#[test]
fn with_no_language_chosen_the_system_s_first_one_offered_is_used() {
    let (_tmp, app) = installed();
    let app = app.speaking(vec!["ja-JP".into(), "fr-CA".into(), "en-US".into()]);
    assert_eq!(app.language_in_use(), "Français");
    assert_eq!(app.speak()["Save"], "Enregistrer");
    assert_eq!(app.settings_report().language, None);

    let file = app.start_language("Nihongo").unwrap();
    std::fs::write(&file, r#"{"@locale": "ja", "Save": "Hozon"}"#).unwrap();
    assert_eq!(
        app.language_in_use(),
        "Nihongo",
        "a language file added is offered"
    );

    app.set_setting("language", "Polski").unwrap();
    assert_eq!(app.language_in_use(), "Polski");
    app.set_setting("language", "").unwrap();
    assert_eq!(app.language_in_use(), "Nihongo");

    let (_tmp, app) = installed();
    assert_eq!(app.speaking(vec!["xx".into()]).language_in_use(), "English");
}

#[test]
fn own_colours_are_written_for_one_theme_and_read_back_for_it() {
    let (_tmp, app) = installed();
    assert!(app.own_colours("dark").is_empty());
    app.start_own_theme("dark", &json!({ "ink": "#ffffff" }))
        .unwrap();
    assert_eq!(app.own_colours("dark")["ink"], "#ffffff");
    assert!(app.own_colours("light").is_empty());
}

#[test]
fn the_fonts_offered_start_with_the_systems_and_are_found_by_any_part() {
    let fonts = font_choices(vec!["noto Sans".into(), "Arial".into(), "Arial".into()]);
    assert_eq!(fonts, ["", "Arial", "noto Sans"]);
    assert_eq!(fonts_matching(&fonts, "SANS", "System font"), [2]);
    assert_eq!(fonts_matching(&fonts, "system", "System font"), [0]);
    assert_eq!(fonts_matching(&fonts, "", "System font"), [0, 1, 2]);
}

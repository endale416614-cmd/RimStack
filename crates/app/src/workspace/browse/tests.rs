use super::*;
use crate::workspace::download_list::action;
use crate::workspace::tests::installed;
use rimstack_core::base::error::Error;
use rimstack_core::rimworld::workshopdb::{self, Item};
use rimstack_core::rimworld::workshopindex::Index;
use rimstack_core::rimworld::workshopsearch::Kind;
use rimstack_core::said;
use rimstack_core::sources::browse::HARMONY;
use std::fs;
use std::sync::Mutex;

const STORAGE: &str = "3100000001";
const EXTRAS: &str = "3100000002";

fn about(folder: &std::path::Path, package_id: &str, needs: &[&str]) {
    fs::create_dir_all(folder.join("About")).unwrap();
    let needs: String = needs
        .iter()
        .map(|id| format!("<li><packageId>{id}</packageId></li>"))
        .collect();
    let xml = format!(
        "<ModMetaData><packageId>{package_id}</packageId><name>{package_id}</name><modDependencies>{needs}</modDependencies></ModMetaData>"
    );
    fs::write(folder.join("About/About.xml"), xml).unwrap();
}

pub(super) fn asked(text: &str, kind: Kind, owned: bool) -> Asked {
    let mut picked = Picked::default();
    if let Some(tag) = kind.steam_tag() {
        picked.tag(tag, true);
    }
    picked.hide_unowned = owned;
    Asked {
        text: text.into(),
        picked,
        ..Asked::default()
    }
}

fn made_up() -> (tempfile::TempDir, App) {
    let (tmp, app) = installed();
    (tmp, app.browsing_fixture(true))
}

#[test]
fn the_list_is_kept_and_a_new_app_on_the_same_folders_reads_it() {
    let (tmp, app) = made_up();
    let heard = Arc::new(Mutex::new(0));
    let counter = heard.clone();
    app.listen(move |changes| {
        if changes.contains(crate::changes::Changes::DOWNLOAD_LIST) {
            *counter.held() += 1;
        }
    });
    app.add_to_list(HARMONY, false);
    app.add_to_list(HARMONY, false);
    assert_eq!(
        *heard.held(),
        1,
        "a change that changed nothing is not told"
    );
    assert_eq!(app.listed_ids(), [HARMONY]);

    let root = tmp.path();
    let again = App::new(
        root.join("home"),
        &root.join("settings"),
        &root.join("data"),
        Default::default(),
    );
    assert_eq!(again.listed_ids(), [HARMONY]);
    again.remove_from_list(HARMONY);
    assert!(again.listed_ids().is_empty());
}

#[test]
fn a_mod_brings_what_it_needs_marked_and_an_installed_one_is_refused() {
    let (tmp, app) = made_up();
    let added = app.add_to_list(EXTRAS, true);
    assert_eq!(added.added, [EXTRAS, STORAGE, HARMONY]);
    assert_eq!(added.unknown, ["3199999999"]);
    assert!(added.unknown_said.is_some() && added.refusal.is_none());
    let shown = app.list_shown();
    assert_eq!(shown.count, 3);
    assert_eq!(
        shown.rows[1].reason,
        rimstack_core::say!("Needed by {}", "Fixture Storage Extras")
    );
    let again = app.add_to_list(EXTRAS, true);
    assert!(again.added.is_empty() && again.refusal.is_some());

    about(
        &tmp.path().join("workshop/2009463077"),
        "fixture.harmony",
        &[],
    );
    app.rescan();
    let refused = app.add_to_list(HARMONY, false);
    assert!(refused.added.is_empty() && refused.refusal.is_some());
}

#[test]
fn browse_and_downloads_agree_on_what_is_installed() {
    let (tmp, app) = made_up();
    about(
        &tmp.path().join("workshop").join(HARMONY),
        "fixture.harmony",
        &[],
    );
    about(
        &tmp.path().join("game/Mods").join(STORAGE),
        "fixture.storage",
        &[],
    );
    app.rescan();
    assert!(app.installed_workshop().contains(HARMONY));
    assert!(
        app.installed_workshop().contains(STORAGE),
        "put there by SteamCMD"
    );
    assert!(!app.installed_workshop().contains("roads"));
    let found = app
        .browse_search(&asked("fixture", Kind::Mod, false), 0, 50)
        .unwrap();
    let taken = |id: &str| found.rows.iter().find(|r| r.id == id).unwrap().clone();
    assert!(taken(HARMONY).installed && taken(STORAGE).installed);
    assert!(!taken(HARMONY).can_add && taken(HARMONY).badge.is_some());
    assert!(taken(EXTRAS).can_add && taken(EXTRAS).badge.is_none());
    app.add_to_list(EXTRAS, false);
    let listed = app
        .browse_search(&asked("extras", Kind::Mod, false), 0, 50)
        .unwrap();
    assert!(listed.rows[0].on_list && !listed.rows[0].can_add);
}

#[test]
fn only_what_needs_the_expansions_the_game_has_is_found_when_asked() {
    let (tmp, app) = made_up();
    let royalty = tmp.path().join("game/Data/Royalty");
    about(&royalty, "Ludeon.RimWorld.Royalty", &[]);
    let item = |id: &str| Item {
        workshop_id: id.into(),
        name: id.into(),
    };
    let known = std::collections::HashMap::from([
        ("ludeon.rimworld.royalty".to_string(), item("1149640")),
        ("ludeon.rimworld.biotech".to_string(), item("1826140")),
    ]);
    workshopdb::store(&known, &workshopdb::file_in(app.data_dir())).unwrap();
    assert_eq!(app.owned_dlc(), ["1149640"]);
    let ids = |owned: bool| -> Vec<String> {
        let found = app.browse_search(&asked("royal", Kind::Any, owned), 0, 50);
        found.unwrap().rows.into_iter().map(|r| r.id).collect()
    };
    assert_eq!(ids(false), ["3100000004"]);
    assert_eq!(ids(true), ["3100000004"]);
    fs::remove_dir_all(&royalty).unwrap();
    let (_tmp, without) = made_up();
    assert!(without.owned_dlc().is_empty());
    let none = without.browse_search(&asked("royal", Kind::Any, true), 0, 50);
    assert!(none.unwrap().rows.is_empty());
}

#[test]
fn details_come_through_the_source_and_name_what_is_required() {
    let (_tmp, app) = made_up();
    let shown = app.browse_details(EXTRAS).unwrap();
    assert!(shown.loaded && shown.size > 0);
    let none = std::collections::HashMap::new();
    let pending = shown.description(&none);
    assert!(
        pending[0]
            .blocks
            .iter()
            .flat_map(|b| &b.runs)
            .any(|run| run.bold)
    );
    assert!(pending[1].note.is_some() && pending[1].image.is_empty());
    let urls = shown.picture_urls();
    assert_eq!(urls.len(), 1);
    let taken = app.browse_images(&urls);
    assert!(taken[0].1.as_ref().is_some_and(|path| path.is_file()));
    assert!(
        app.browse_images(&["https://127.0.0.1/a.png".to_string()])[0]
            .1
            .is_none()
    );
    let files = std::collections::HashMap::from([(urls[0].clone(), "/p/a.img".to_string())]);
    assert_eq!(shown.description(&files)[1].image, "/p/a.img");
    assert_eq!(shown.stats.len(), 3);
    let required: Vec<&str> = shown.requires.iter().map(|r| r.id.as_str()).collect();
    assert_eq!(required, [STORAGE, HARMONY]);
    assert_eq!(shown.unknown_requirements, ["3199999999"]);
    let royal = app.browse_details("3100000004").unwrap();
    assert_eq!(royal.dlc[0].name.as_deref(), Some("Fixture Expansion One"));
    assert!(app.browse_details("1").is_none());
    assert_eq!(app.browse_preview("").unwrap(), None);
    assert!(app.browse_preview("https://evil.example/a.png").is_err());
}

#[test]
fn the_sidebar_counts_the_results_and_every_version_is_listed() {
    let (_tmp, app) = installed();
    let app = app.browsing_fixture(true);
    let mut asked = asked("", Kind::Mod, false);
    asked.picked.tag("1.5", false);
    let found = app.browse_search(&asked, 0, 50).unwrap();
    let side = found.sidebar.unwrap();
    let versions: Vec<String> = side.versions.iter().map(|f| f.id.clone()).collect();
    assert_eq!(versions, ["1.5", "1.6"]);
    assert!(side.versions[0].exclude && side.filtered && side.counted);
    assert_eq!(found.total, 2);
    assert_eq!(side.found.to_string(), "2 results matching your filters");
    assert!(app.browse_search(&asked, 30, 50).unwrap().sidebar.is_none());
}

#[test]
fn the_source_follows_the_switch_and_the_database_file() {
    let (_tmp, app) = installed();
    assert!(!app.browse_defaults().ready);
    let found = app
        .browse_search(&asked("harmony", Kind::Any, false), 0, 10)
        .unwrap();
    assert!(found.rows.is_empty(), "no database yet");

    let entry = |id: &str, title: &str| rimstack_core::rimworld::workshopindex::Entry {
        id: id.into(),
        title: title.into(),
        author: None,
        package_id: None,
        tags: vec!["Mod".into(), "1.6".into()],
        requires: Vec::new(),
        dlc: Vec::new(),
    };
    let index = Index::new(vec![entry("500000", "Cache Harmony")]);
    index
        .store(&rimstack_core::rimworld::workshopindex::file_in(
            app.data_dir(),
        ))
        .unwrap();
    assert!(app.browse_defaults().ready);
    let found = app
        .browse_search(&asked("harmony", Kind::Mod, false), 0, 10)
        .unwrap();
    assert_eq!(found.rows[0].title, "Cache Harmony");
    assert_eq!(found.rows[0].versions, ["1.6"]);

    let made_up = app.browsing_fixture(true);
    let found = made_up
        .browse_search(&asked("harmony", Kind::Mod, false), 0, 10)
        .unwrap();
    assert_eq!(found.rows[0].title, "Fixture Harmony");
}

#[test]
fn what_a_lookup_found_joins_the_list_as_added() {
    let (_tmp, app) = made_up();
    app.add_wanted_to_list(&[
        ("900".into(), "Found".into()),
        ("901".into(), "Also".into()),
    ]);
    app.add_wanted_to_list(&[("900".into(), "Found".into())]);
    assert_eq!(app.listed_ids(), ["900", "901"]);
    assert_eq!(
        app.list_shown().rows[0].reason,
        rimstack_core::say!("Added by you")
    );
}

#[test]
fn a_download_result_takes_the_arrived_off_the_list_and_marks_the_failed() {
    let (tmp, app) = made_up();
    app.add_to_list(STORAGE, false);
    app.add_to_list(EXTRAS, false);
    app.add_to_list("3100000003", false);
    let names = workshopdb::file_in(app.data_dir());
    let item = |id: &str| Item {
        workshop_id: id.into(),
        name: format!("Named {id}"),
    };
    let known = std::collections::HashMap::from([
        ("fixture.harmony".to_string(), item(HARMONY)),
        ("ludeon.rimworld.biotech".to_string(), item("1826140")),
    ]);
    workshopdb::store(&known, &names).unwrap();

    let arrived = tmp.path().join("game/Mods").join(STORAGE);
    about(
        &arrived,
        "fixture.storage",
        &["Fixture.Harmony", "ludeon.rimworld.biotech", "unheard.of"],
    );
    let outcomes: Vec<(String, Result<PathBuf, Error>)> = vec![
        (STORAGE.into(), Ok(arrived)),
        (
            EXTRAS.into(),
            Err(Error::program(said!("SteamCMD did not download it"))),
        ),
    ];
    app.apply_downloads(&outcomes);

    let shown = app.list_shown();
    let ids: Vec<&str> = shown.rows.iter().map(|r| r.id.as_str()).collect();
    assert_eq!(
        ids,
        [EXTRAS, "3100000003", HARMONY],
        "no DLC, no unknown mod"
    );
    assert!(shown.rows[0].failed.is_some() && shown.rows[1].failed.is_none());
    assert!(shown.failures);
    assert_eq!(
        shown.rows[2].reason,
        rimstack_core::say!("Needed by {}", "Fixture Better Storage")
    );
    app.clear_list_failures();
    assert!(!app.list_shown().failures);
}

#[test]
fn the_missing_cards_subscribe_fix_adds_a_mod_the_database_lists_and_nothing_else() {
    let (_tmp, app) = made_up();
    assert_eq!(app.add_missing_to_list(HARMONY).added, [HARMONY]);
    assert!(app.add_missing_to_list("1826140").added.is_empty());
    assert_eq!(app.listed_ids(), [HARMONY]);
    assert_eq!(
        app.list_shown().rows[0].reason,
        rimstack_core::say!("Missing from your list")
    );
}

#[test]
fn the_main_action_follows_the_system_steam_and_steamcmd() {
    use rimstack_core::rimworld::paths::Os;
    for os in [Os::Linux, Os::Windows] {
        for steam in [true, false] {
            let has = action(os, steam, true, false);
            let hasnt = action(os, steam, false, true);
            assert!(has.download && has.enabled && !has.install_steamcmd);
            assert!(hasnt.install_steamcmd && hasnt.enabled && !hasnt.download);
            assert_eq!(has.subscribe, steam, "Steam is offered only where it is");
            assert_eq!(has.subscribe_enabled, steam);
            assert!(!hasnt.subscribe_enabled);
            assert!(has.stuck.is_none() && hasnt.stuck.is_none());
        }
    }
    for steamcmd in [true, false] {
        let mac = action(Os::Mac, true, steamcmd, false);
        assert!(!mac.download && !mac.install_steamcmd && mac.subscribe && mac.subscribe_enabled);
        assert!(mac.stuck.is_none());
        assert!(!action(Os::Mac, true, steamcmd, true).subscribe_enabled);
        let none = action(Os::Mac, false, steamcmd, false);
        assert!(!none.download && !none.install_steamcmd && !none.subscribe && !none.enabled);
        assert!(none.stuck.is_some(), "the list says why nothing is offered");
    }
}

#[test]
fn only_what_steam_says_is_a_rimworld_item_goes_on_to_be_downloaded() {
    use crate::workspace::download_list::vet;
    use rimstack_core::rimworld::workshop::Details;
    let item = |id: &str, app_id: u64, collection: bool| Details {
        id: id.into(),
        title: id.into(),
        updated: 1,
        size: 1,
        preview_url: String::new(),
        description: String::new(),
        subscriptions: 0,
        favorited: 0,
        banned: false,
        creator: String::new(),
        app_id,
        collection,
        ..Details::default()
    };
    let ids: Vec<String> = ["mod", "other", "set", "gone"].map(String::from).into();
    let answer = Ok(vec![
        item("mod", 294_100, false),
        item("other", 440, false),
        item("set", 294_100, true),
    ]);
    let (passed, refused) = vet(&ids, &answer, &|_| true);
    assert_eq!(passed, ["mod"]);
    let refused: Vec<&str> = refused.iter().map(|(id, _)| id.as_str()).collect();
    assert_eq!(refused, ["other", "set", "gone"]);

    let offline = Err(Error::program(said!("offline")));
    let (passed, refused) = vet(&ids, &offline, &|id| id == "mod");
    assert_eq!((passed, refused.len()), (vec!["mod".to_string()], 3));
}

#[test]
fn previews_are_asked_for_every_id_once_and_a_mod_without_one_gives_none() {
    let (_tmp, app) = made_up();
    let heard = Mutex::new(Vec::new());
    let ids = [STORAGE.to_string(), EXTRAS.to_string()];
    app.browse_previews(&ids, &|id, path, _| {
        heard.held().push((id.to_string(), path));
        true
    });
    let mut heard = heard.into_inner().unwrap();
    heard.sort();
    assert_eq!(
        heard,
        [(STORAGE.to_string(), None), (EXTRAS.to_string(), None)]
    );
    assert!(app.browse_details.held().get(STORAGE).is_some());
}

#[test]
fn what_an_add_says_is_one_note_and_bad_news_only_when_nothing_was_added() {
    let (_tmp, app) = made_up();
    let (said, failed) = app.add_to_list(EXTRAS, true).note().unwrap();
    assert!(
        !failed && said.to_string().contains("requirement"),
        "{said}"
    );
    let (said, failed) = app.add_to_list(EXTRAS, false).note().unwrap();
    assert!(failed && said.to_string().contains("already on the list"));
    assert!(app.add_missing_to_list("1").note().is_some());
}

#[test]
fn a_link_in_a_description_opens_an_item_or_the_web() {
    let item = |id: &str| Followed::Item(id.into());
    assert_eq!(
        follow("https://steamcommunity.com/sharedfiles/filedetails/?id=2009463077&searchtext=x"),
        item("2009463077")
    );
    assert_eq!(
        follow("http://steamcommunity.com/workshop/filedetails/?id=818773962"),
        item("818773962")
    );
    let outside = "https://example.org/?id=2009463077";
    assert_eq!(follow(outside), Followed::Outside(outside.into()));
    assert_eq!(follow("javascript:alert(1)"), Followed::Nothing);
    assert_eq!(
        follow("steam://openurl/https://example.org"),
        Followed::Nothing
    );
    assert_eq!(follow(""), Followed::Nothing);
}

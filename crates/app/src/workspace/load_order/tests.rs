use super::*;
use crate::workspace::tests::installed;

fn roads_made_for_an_older_game(tmp: &tempfile::TempDir) {
    let about = "<ModMetaData><packageId>Ann.Roads</packageId><name>Ann.Roads</name><supportedVersions><li>1.5</li></supportedVersions></ModMetaData>";
    std::fs::write(tmp.path().join("game/Mods/roads/About/About.xml"), about).unwrap();
}

fn roads(order: &LoadOrder, side: Side) -> ModId {
    let list = order.list(side);
    let m = list.mods().iter().find(|m| m.id == "ann.roads").unwrap();
    m.installed.unwrap()
}

#[test]
fn the_two_lists_are_the_load_order_and_the_rest() {
    let (_tmp, app) = installed();
    let order = LoadOrder::read(&app, true, None);
    assert!(order.problem.is_empty() && order.is_installed());
    let ids = |side| -> Vec<String> {
        order
            .list(side)
            .mods()
            .iter()
            .map(|m| m.id.clone())
            .collect()
    };
    assert_eq!(ids(Side::Active), ["ludeon.rimworld"]);
    assert_eq!(ids(Side::Inactive), ["ann.roads"]);
    assert!(order.status[0].to_string().contains('1'));
}

#[test]
fn a_side_term_filters_its_own_list_and_matches_none_in_the_other() {
    let (_tmp, app) = installed();
    let mut order = LoadOrder::read(&app, true, None);
    let matching = |order: &LoadOrder| {
        let count = |side| order.list(side).matching();
        (count(Side::Active), count(Side::Inactive))
    };
    order.set_filter("is:inactive");
    assert_eq!(matching(&order), (0, 1));
    order.set_filter("is:active");
    assert_eq!(matching(&order), (1, 0));
    order.set_filter("");
    assert_eq!(matching(&order), (1, 1));
}

#[test]
fn the_list_left_out_is_dimmed_or_hidden_as_the_setting_says() {
    let (_tmp, app) = installed();
    let mut order = LoadOrder::read(&app, true, None);
    let left_out = |order: &LoadOrder| order.list(Side::Active).left_out();
    let shown = |order: &LoadOrder| order.rows(&app, Side::Active).len();
    order.set_filter("is:inactive");
    assert_eq!(left_out(&order), Some(LeftOut::Dimmed));
    assert!(shown(&order) > 0);
    app.keep_setting("dimLeftOut", "off").unwrap();
    order.follow_left_out(&app);
    assert_eq!(left_out(&order), Some(LeftOut::Hidden));
    assert_eq!(shown(&order), 0);
    let mut order = LoadOrder::read(&app, true, None);
    order.set_filter("is:inactive");
    assert_eq!(left_out(&order), Some(LeftOut::Hidden));
}

#[test]
fn a_row_shows_what_its_list_is_ordered_by() {
    let (tmp, app) = installed();
    let core = tmp.path().join("game/Data/Core/About/About.xml");
    let about = "<ModMetaData><packageId>Ludeon.RimWorld</packageId><name>Core</name><modVersion>1.0</modVersion></ModMetaData>";
    std::fs::write(core, about).unwrap();
    let mut order = LoadOrder::read(&app, true, Some((SortKey::PackageId, false)));
    let shown = |order: &LoadOrder, side| -> Vec<String> {
        let rows = order.rows(&app, side).into_iter();
        let mods = rows.filter(|r| !r.is_header);
        mods.map(|r| r.ordered_by.to_string()).collect()
    };
    assert_eq!(shown(&order, Side::Inactive), ["ann.roads"]);
    assert_eq!(shown(&order, Side::Active), [""]);
    order.set_active_by(Some((SortKey::PackageId, false)));
    assert_eq!(shown(&order, Side::Active), ["ludeon.rimworld"]);
    order.set_active_by(Some((SortKey::Version, false)));
    assert_eq!(shown(&order, Side::Active), ["1.0"]);
    order.set_active_by(Some((SortKey::Name, false)));
    order.set_inactive_by(&app, None);
    assert_eq!(shown(&order, Side::Active), [""]);
    assert_eq!(shown(&order, Side::Inactive), [""]);
}

#[test]
fn workshop_mods_go_by_the_time_and_size_steam_records() {
    let (tmp, _) = installed();
    let root = tmp.path();
    let steam = root.join("steamapps/workshop");
    let content = steam.join("content/294100");
    for (folder, id) in [("111", "Old.Mod"), ("222", "New.Mod")] {
        let about = content.join(folder).join("About");
        std::fs::create_dir_all(&about).unwrap();
        let xml =
            format!("<ModMetaData><packageId>{id}</packageId><name>{id}</name></ModMetaData>");
        std::fs::write(about.join("About.xml"), xml).unwrap();
    }
    let now = crate::workspace::now_secs();
    let item = |id: &str, size: u64, ago: u64| {
        format!(
            "\"{id}\" {{ \"size\" \"{size}\" \"timeupdated\" \"{}\" }}",
            now - ago
        )
    };
    let acf = format!(
        "\"AppWorkshop\" {{ \"WorkshopItemsInstalled\" {{ {} {} }} }}",
        item("111", 2 * 1_048_576, 10 * 86_400),
        item("222", 1024, 3 * 3600)
    );
    std::fs::write(steam.join("appworkshop_294100.acf"), acf).unwrap();
    let given = rimstack_core::rimworld::paths::Overrides {
        game: Some(root.join("game")),
        config: Some(root.join("config")),
        workshop: Some(content),
    };
    let app = App::new(
        root.join("home"),
        &root.join("settings"),
        &root.join("data"),
        given,
    );
    let mut order = LoadOrder::read(&app, true, Some((SortKey::Updated, true)));
    let shown = |order: &LoadOrder| -> Vec<(String, String)> {
        let rows = order.rows(&app, Side::Inactive).into_iter();
        let mods = rows.filter(|r| !r.is_header);
        mods.map(|r| (r.name.to_string(), r.ordered_by.to_string()))
            .collect()
    };
    let pair = |name: &str, value: &str| (name.to_string(), value.to_string());
    assert_eq!(
        shown(&order),
        [
            pair("New.Mod", "3 hours ago"),
            pair("Old.Mod", "10 days ago"),
            pair("Ann.Roads", "")
        ]
    );
    order.set_inactive_by(&app, Some((SortKey::Size, false)));
    assert_eq!(
        shown(&order),
        [
            pair("New.Mod", "1.0 KB"),
            pair("Old.Mod", "2.0 MB"),
            pair("Ann.Roads", "")
        ]
    );
}

#[test]
fn a_mod_is_dated_when_its_activation_is_saved_and_forgotten_when_it_leaves() {
    let (_tmp, app) = installed();
    let mut order = LoadOrder::read(&app, true, None);
    let kept = || {
        app.settle_writes();
        rimstack_core::kept::activated::read(&app.activated_file()).unwrap()
    };
    assert_eq!(kept().keys().collect::<Vec<_>>(), ["ludeon.rimworld"]);
    order.take(&app, Side::Inactive, 0, None);
    assert!(!kept().contains_key("ann.roads"), "not before it is saved");
    order.save(&app).unwrap();
    assert!(kept().contains_key("ann.roads"));
    let at = |order: &LoadOrder, id: &str| {
        let rows = order.rows(&app, Side::Active).into_iter();
        let mut mods = rows.filter(|r| !r.is_header);
        let row = mods
            .find(|r| r.name.to_string().to_lowercase() == id)
            .unwrap();
        row.activated_ago.to_string()
    };
    assert_eq!(at(&order, "ann.roads"), "just now");
    let roads = order
        .list(Side::Active)
        .mods()
        .iter()
        .position(|m| m.id == "ann.roads");
    order.take(&app, Side::Active, roads.unwrap(), None);
    order.save(&app).unwrap();
    assert!(!kept().contains_key("ann.roads"));
}

#[test]
fn a_list_the_game_emptied_to_its_own_mods_dates_nothing() {
    let (_tmp, app) = installed();
    let since = rimstack_core::kept::activated::Since::from([("ann.roads".into(), 7)]);
    rimstack_core::kept::activated::write(&app.activated_file(), &since).unwrap();
    let before = ["ludeon.rimworld".to_string(), "ann.roads".to_string()];
    launches::record(&app.launches_file(), &before, 5).unwrap();
    LoadOrder::read(&app, true, None);
    let kept = rimstack_core::kept::activated::read(&app.activated_file());
    assert_eq!(kept, Some(since));
}

#[test]
fn mods_that_could_not_be_read_give_empty_lists_and_the_reason() {
    let (tmp, app) = installed();
    std::fs::remove_dir_all(tmp.path().join("game")).unwrap();
    let order = LoadOrder::read(&app, true, None);
    assert!(!order.problem.is_empty() && !order.is_installed());
    assert!(order.list(Side::Active).mods().is_empty());
}

#[test]
fn a_mod_taken_across_is_one_step_that_undo_takes_back() {
    let (_tmp, app) = installed();
    let mut order = LoadOrder::read(&app, true, None);
    let id = roads(&order, Side::Inactive);
    order.take_all(&app, Side::Inactive, &[id], None);
    assert_eq!(app.session().current(), ["ludeon.rimworld", "ann.roads"]);
    assert!(order.list(Side::Inactive).mods().is_empty());
    assert!(app.session().is_dirty());
    assert!(order.undo(&app));
    assert_eq!(app.session().current(), ["ludeon.rimworld"]);
    assert_eq!(order.list(Side::Inactive).mods().len(), 1);
    assert!(order.redo(&app));
    assert_eq!(order.list(Side::Active).mods().len(), 2);
}

#[test]
fn an_error_ignored_is_gone_until_warned_again() {
    let (tmp, app) = installed();
    let about = "<ModMetaData><packageId>Ann.Roads</packageId><name>Ann.Roads</name><modDependencies><li><packageId>gone.lib</packageId></li></modDependencies></ModMetaData>";
    std::fs::write(tmp.path().join("game/Mods/roads/About/About.xml"), about).unwrap();
    let mut order = LoadOrder::read(&app, true, None);
    let id = roads(&order, Side::Inactive);
    order.take(&app, Side::Inactive, 0, None);
    let errors = |order: &LoadOrder| {
        order
            .troubles()
            .iter()
            .flatten()
            .filter(|t| t.error)
            .count()
    };
    assert_eq!(errors(&order), 1);
    let key = order.troubles()[1][0].key.clone();
    app.ignore_problem("Ann.Roads", &key, true);
    order.recheck(&app);
    assert_eq!(errors(&order), 0);
    let details = serde_json::to_value(order.details(&app, id)).unwrap();
    assert_eq!(details["problemsIgnored"], 1);
    app.warn_again(&[id]);
    order.recheck(&app);
    assert_eq!(errors(&order), 1);
}

#[test]
fn a_mod_made_for_another_game_version_offers_to_trust_it_once_active() {
    let (tmp, app) = installed();
    roads_made_for_an_older_game(&tmp);
    let mut order = LoadOrder::read(&app, true, None);
    let id = roads(&order, Side::Inactive);
    let shown = |order: &mut LoadOrder| serde_json::to_value(order.details(&app, id)).unwrap();
    assert_eq!(shown(&mut order)["offerTrust"], false);
    order.take(&app, Side::Inactive, 0, None);
    let details = shown(&mut order);
    assert_eq!(details["offerTrust"], true, "{}", details["problems"]);
    assert_eq!(details["name"], "Ann.Roads");
}

#[test]
fn a_mod_made_for_another_game_version_is_marked_on_either_side_until_trusted() {
    let (tmp, app) = installed();
    roads_made_for_an_older_game(&tmp);
    let mut order = LoadOrder::read(&app, true, None);
    let id = roads(&order, Side::Inactive);
    let marked = |order: &LoadOrder| {
        let rows = order.rows(&app, Side::Inactive);
        rows.iter()
            .any(|r| r.mod_id == Some(id) && r.outdated && r.made_for == "1.5")
    };
    assert!(marked(&order));
    app.set_version_ok(&[id], true);
    order.recheck(&app);
    assert!(!marked(&order));
    app.set_version_ok(&[id], false);
    order.recheck(&app);
    order.take(&app, Side::Inactive, 0, None);
    let active = order.rows(&app, Side::Active);
    assert!(
        active
            .iter()
            .any(|r| r.mod_id == Some(id) && r.made_for == "1.5")
    );
}

#[test]
fn saving_writes_the_load_order_and_marks_it_saved() {
    let (tmp, app) = installed();
    let mut order = LoadOrder::read(&app, true, None);
    order.take(&app, Side::Inactive, 0, None);
    order.save(&app).unwrap();
    let written = std::fs::read_to_string(tmp.path().join("config/Config/ModsConfig.xml")).unwrap();
    assert!(written.contains("<li>ann.roads</li>"));
    assert!(!app.session().is_dirty());
}

#[test]
fn a_list_saved_here_is_never_taken_for_one_written_outside() {
    let (_tmp, app) = installed();
    let mut order = LoadOrder::read(&app, true, None);
    order.take(&app, Side::Inactive, 0, None);
    let saving = order.to_save(&app).unwrap();

    app.write_list(&saving.file, &saving.written, &saving.expected)
        .unwrap();
    assert_eq!(app.stamps().mods_config, 0);
    order.saved(&app, saving.list);

    assert_eq!(app.stamps().mods_config, 0);
}

fn written_outside(tmp: &tempfile::TempDir, ids: &str) -> (std::path::PathBuf, String) {
    let file = tmp.path().join("config/Config/ModsConfig.xml");
    let text = format!("<ModsConfigData><activeMods>{ids}</activeMods></ModsConfigData>");
    std::fs::write(&file, &text).unwrap();
    (file, text)
}

#[test]
fn a_file_written_outside_is_not_saved_over_until_its_list_or_ours_is_kept() {
    let (tmp, app) = installed();
    let mut order = LoadOrder::read(&app, true, None);
    order.take(&app, Side::Inactive, 0, None);
    let (file, text) = written_outside(&tmp, "<li>ludeon.rimworld</li><li>other.mod</li>");

    let refused = order.save(&app).unwrap_err();

    assert_eq!(
        refused,
        rimstack_core::rimworld::modsconfig::changed_outside()
    );
    assert_eq!(std::fs::read_to_string(&file).unwrap(), text);
    assert!(app.session().is_dirty());

    order.keep_file(&app, false);
    assert_eq!(app.session().current(), ["ludeon.rimworld", "ann.roads"]);
    order.save(&app).unwrap();
    let written = std::fs::read_to_string(&file).unwrap();
    assert!(written.contains("ann.roads") && !written.contains("other.mod"));
}

#[test]
fn taking_the_list_of_a_file_written_outside_leaves_nothing_to_save() {
    let (tmp, app) = installed();
    let mut order = LoadOrder::read(&app, true, None);
    order.take(&app, Side::Inactive, 0, None);
    let (file, text) = written_outside(&tmp, "<li>ludeon.rimworld</li><li>other.mod</li>");

    assert_ne!(app.stamps().mods_config, 0);

    order.keep_file(&app, true);

    assert_eq!(app.session().current(), ["ludeon.rimworld", "other.mod"]);
    assert!(!app.session().is_dirty());
    assert_eq!(app.stamps().mods_config, 0);
    assert_eq!(std::fs::read_to_string(&file).unwrap(), text);
}

#[test]
fn every_row_has_a_key_of_its_own_and_a_mod_row_its_id() {
    let (tmp, app) = installed();
    roads_made_for_an_older_game(&tmp);
    let mut order = LoadOrder::read(&app, true, None);
    order.take(&app, Side::Inactive, 0, None);
    let rows = order.rows(&app, Side::Active);
    let keys: std::collections::HashSet<_> = rows.iter().map(|r| r.key.clone()).collect();
    assert_eq!(keys.len(), rows.len());
    let roads = rows
        .iter()
        .find(|r| r.name.to_string() == "Ann.Roads")
        .unwrap();
    assert!(roads.mod_id.is_some() && roads.number == 2 && roads.outdated);
}

#[test]
fn a_mod_row_keeps_its_key_over_a_rescan_shown_or_not_and_each_copy_has_its_own() {
    let (tmp, app) = installed();
    std::fs::create_dir_all(tmp.path().join("workshop/123/About")).unwrap();
    std::fs::write(
        tmp.path().join("workshop/123/About/About.xml"),
        "<ModMetaData><packageId>Ann.Roads</packageId></ModMetaData>",
    )
    .unwrap();
    app.rescan();
    let keys = |order: &LoadOrder| {
        [Side::Active, Side::Inactive].map(|side| {
            let rows = order.rows(&app, side).into_iter();
            rows.map(|r| r.key).collect::<Vec<_>>()
        })
    };
    let shown = LoadOrder::read(&app, true, None);
    let before = keys(&shown);
    let folders = before[1].iter();
    let folders = folders.filter(|key| matches!(key, rows::RowKey::Installed(_, 0)));
    assert_eq!(folders.count(), 2);
    app.rescan();
    assert_eq!(keys(&shown), before);
    assert_eq!(keys(&LoadOrder::read(&app, true, None)), before);
}

#[test]
fn a_sort_of_a_list_in_order_moves_nothing_and_says_so() {
    let (_tmp, app) = installed();
    let mut order = LoadOrder::read(&app, true, None);
    order.preview_sort(&app);
    assert!(!order.previewing());
    assert!(!order.sort_note.is_empty());
    assert_eq!(order.sort_count().map(|count| count.moves(&app)), Some(0));
}

fn nothing_found() -> crate::workspace::Conflicts {
    crate::workspace::Conflicts {
        members: Vec::new(),
        overlaps: rimstack_core::mods::conflicts::overlaps(&[]),
        scanned: Default::default(),
    }
}

#[test]
fn a_scan_is_kept_unless_a_change_of_textures_overtook_it() {
    let (_tmp, app) = installed();
    app.keep_files_read(app.conflicts_generation(), nothing_found());
    assert!(app.conflicts().reading().is_some());
    let (_tmp, app) = installed();
    let generation = app.conflicts_generation();
    app.textures_changed();
    app.keep_files_read(generation, nothing_found());
    assert!(app.conflicts().reading().is_none());
}

#[cfg(target_os = "linux")]
#[test]
fn nothing_is_saved_while_the_game_runs() {
    let (tmp, app) = installed();
    let mut order = LoadOrder::read(&app, true, None);
    order.take(&app, Side::Inactive, 0, None);
    let game = tmp.path().join("game/RimWorld");
    std::fs::copy("/bin/sleep", &game).unwrap();
    let mut running = (0..50)
        .find_map(
            |_| match std::process::Command::new(&game).arg("30").spawn() {
                Err(e) if e.raw_os_error() == Some(26) => {
                    std::thread::sleep(std::time::Duration::from_millis(20));
                    None
                }
                other => Some(other.unwrap()),
            },
        )
        .unwrap();
    let file = tmp.path().join("config/Config/ModsConfig.xml");
    let before = std::fs::read_to_string(&file).unwrap();

    let refused = order.save(&app);

    running.kill().unwrap();
    running.wait().unwrap();
    assert_eq!(
        refused.unwrap_err().kind(),
        rimstack_core::base::error::Kind::Refused
    );
    assert_eq!(std::fs::read_to_string(&file).unwrap(), before);
    order.save(&app).unwrap();
}

fn with_a_mod_gone(workshop_names: &str) -> (tempfile::TempDir, App, LoadOrder) {
    let (tmp, app) = installed();
    std::fs::write(
        tmp.path().join("config/Config/ModsConfig.xml"),
        "<ModsConfigData><activeMods><li>gone.mod</li><li>ludeon.rimworld</li></activeMods></ModsConfigData>",
    )
    .unwrap();
    std::fs::create_dir_all(app.data_dir()).unwrap();
    let names = rimstack_core::rimworld::workshopdb::file_in(app.data_dir());
    std::fs::write(names, workshop_names).unwrap();
    let order = LoadOrder::read(&app, true, None);
    assert_eq!(order.list(Side::Active).mods()[0].id, "gone.mod");
    (tmp, app, order)
}

#[test]
fn a_mod_on_the_list_but_not_installed_gives_its_menu_its_id() {
    let (_tmp, app, order) = with_a_mod_gone("");
    let said = serde_json::to_value(order.not_installed(&app, Side::Active, 0)).unwrap();
    assert_eq!(said["packageId"], "gone.mod");
    assert_eq!(said["fixes"][0]["action"], "search");
    assert_eq!(said["resubscribe"], serde_json::json!([]));
    assert!(order.not_installed(&app, Side::Active, 1).is_none());
    assert!(order.not_installed(&app, Side::Inactive, 0).is_none());
}

#[test]
fn a_mod_not_installed_the_workshop_knows_offers_its_page_and_to_subscribe_again() {
    let (_tmp, app, order) = with_a_mod_gone("gone.mod\t123\tGone");
    let said = serde_json::to_value(order.not_installed(&app, Side::Active, 0)).unwrap();
    assert_eq!(said["name"], "Gone");
    assert_eq!(said["fixes"][0]["action"], "subscribe");
    assert_eq!(said["fixes"][0]["target"], "123");
    assert_eq!(said["resubscribe"], serde_json::json!(["123"]));
}

#[test]
fn a_mod_not_installed_taken_off_the_list_is_gone_from_both() {
    let (_tmp, app, mut order) = with_a_mod_gone("");
    order.take(&app, Side::Active, 0, None);
    assert_eq!(app.session().current(), ["ludeon.rimworld"]);
    let inactive: Vec<&str> = order
        .list(Side::Inactive)
        .mods()
        .iter()
        .map(|m| m.id.as_str())
        .collect();
    assert_eq!(inactive, ["ann.roads"]);
    assert!(order.undo(&app));
    assert_eq!(app.session().current(), ["gone.mod", "ludeon.rimworld"]);
}

#[test]
fn a_selection_is_carried_over_a_rescan_by_folder_and_a_mod_gone_leaves_it() {
    let (tmp, app) = installed();
    let old = LoadOrder::read(&app, true, None);
    let core = old.list(Side::Active).mods()[0].installed.unwrap();
    let was = roads(&old, Side::Inactive);
    app.rescan();
    let new = LoadOrder::read(&app, true, None);
    let same = old.same_mods_in(&new);
    let carried = new.carried(&same, &[was, core]);
    let (core_now, roads_now) = (same[&core], same[&was]);
    assert!(core_now != core && roads_now == roads(&new, Side::Inactive));
    assert_eq!(carried.ids.first(), Some(&roads_now));
    assert!(!carried.active);
    assert_eq!(carried.ids[1..], [core_now]);
    std::fs::remove_dir_all(tmp.path().join("game/Mods/roads")).unwrap();
    app.rescan();
    let newest = LoadOrder::read(&app, true, None);
    let gone = newest.carried(&new.same_mods_in(&newest), &[roads_now]);
    assert!(gone.ids.is_empty());
}

fn with_more_mods(tmp: &tempfile::TempDir) {
    for (folder, id) in [("hats", "Ann.Hats"), ("bridges", "Ann.Bridges")] {
        let about = tmp.path().join("game/Mods").join(folder).join("About");
        std::fs::create_dir_all(&about).unwrap();
        let xml =
            format!("<ModMetaData><packageId>{id}</packageId><name>{id}</name></ModMetaData>");
        std::fs::write(about.join("About.xml"), xml).unwrap();
    }
}

fn ids_of(order: &LoadOrder, side: Side, keys: &[&str]) -> Vec<ModId> {
    let list = order.list(side);
    let id_of = |key: &&str| list.mods().iter().find(|m| m.id == *key)?.installed;
    keys.iter().filter_map(id_of).collect()
}

fn keys(order: &LoadOrder, side: Side) -> Vec<String> {
    order
        .list(side)
        .mods()
        .iter()
        .map(|m| m.id.clone())
        .collect()
}

#[test]
fn mods_dropped_in_together_land_side_by_side_where_dropped_in_one_step() {
    let (tmp, app) = installed();
    with_more_mods(&tmp);
    let mut order = LoadOrder::read(&app, true, None);
    let in_order: Vec<String> = keys(&order, Side::Inactive)
        .into_iter()
        .filter(|key| key != "ann.bridges")
        .collect();
    let held = ids_of(&order, Side::Inactive, &["ann.roads", "ann.hats"]);
    order.take_all(&app, Side::Inactive, &held, Some(0));
    let landed = keys(&order, Side::Active);
    assert_eq!(
        (&landed[..2], &landed[2][..]),
        (&in_order[..], "ludeon.rimworld")
    );
    assert!(order.undo(&app));
    assert_eq!(keys(&order, Side::Active), ["ludeon.rimworld"]);
}

#[test]
fn mods_moved_together_inside_a_list_are_one_step() {
    let (tmp, app) = installed();
    with_more_mods(&tmp);
    let mut order = LoadOrder::read(&app, true, None);
    let all = order
        .list(Side::Inactive)
        .mods()
        .iter()
        .filter_map(|m| m.installed)
        .collect::<Vec<_>>();
    order.take_all(&app, Side::Inactive, &all, Some(1));
    let before = keys(&order, Side::Active);
    let last_two = ids_of(&order, Side::Active, &[&before[2], &before[3]]);
    assert!(order.move_mods(&app, Side::Active, &last_two, 1));
    let after = keys(&order, Side::Active);
    assert_eq!(after, [&*before[0], &before[2], &before[3], &before[1]]);
    assert!(order.undo(&app));
    assert_eq!(keys(&order, Side::Active), before);
}

#[test]
fn the_inverse_of_a_selection_is_the_rest_of_its_list() {
    let (tmp, app) = installed();
    with_more_mods(&tmp);
    let order = LoadOrder::read(&app, true, None);
    let inactive: Vec<ModId> = order
        .list(Side::Inactive)
        .mods()
        .iter()
        .filter_map(|m| m.installed)
        .collect();
    let roads = roads(&order, Side::Inactive);
    let rest = order.inverted(Side::Inactive, Side::Inactive, &[roads]);
    let mut chosen = rest.ids;
    chosen.sort_unstable();
    let mut others: Vec<ModId> = inactive.iter().copied().filter(|id| *id != roads).collect();
    others.sort_unstable();
    assert_eq!((chosen, rest.active), (others, false));
    let core = ids_of(&order, Side::Active, &["ludeon.rimworld"]);
    let whole = order.inverted(Side::Inactive, Side::Active, &core);
    assert_eq!(whole.ids.len(), inactive.len());
    assert!(
        order
            .inverted(Side::Active, Side::Active, &core)
            .ids
            .is_empty()
    );
}

#[test]
fn a_drag_takes_the_selection_along_only_when_it_starts_on_part_of_it() {
    let (_tmp, app) = installed();
    let order = LoadOrder::read(&app, true, None);
    let roads = roads(&order, Side::Inactive);
    let core = ids_of(&order, Side::Active, &["ludeon.rimworld"])[0];
    let both = Holding::of(Some(roads), "Ann.Roads", &[core, roads]);
    assert_eq!(both.ids, [core, roads]);
    assert_eq!(both.name.to_string(), "2 mods");
    let alone = Holding::of(Some(roads), "Ann.Roads", &[core]);
    assert_eq!(
        (alone.ids, alone.name.to_string()),
        (vec![roads], "Ann.Roads".to_string())
    );
    assert!(Holding::of(None, "Gone", &[core, roads]).ids.is_empty());
}

#[cfg(unix)]
#[test]
fn mods_activated_together_each_load_after_what_they_follow() {
    let (_tmp, app) = crate::ui::made_up::made_up();
    let mut order = LoadOrder::read(&app, true, None);
    let id_of = |order: &LoadOrder, side: Side, package: &str| {
        let mods = order.list(side).mods();
        mods.iter()
            .find(|m| m.id == package)
            .and_then(|m| m.installed)
    };
    let inactive: Vec<ModId> = order
        .list(Side::Inactive)
        .mods()
        .iter()
        .filter_map(|m| m.installed)
        .collect();
    assert!(id_of(&order, Side::Inactive, "fixture.mod042").is_some());
    order.take_all(&app, Side::Inactive, &inactive, None);
    let place = |package: &str| {
        let id = id_of(&order, Side::Active, package).unwrap();
        order.list(Side::Active).position_of(id).unwrap()
    };
    for n in (42..=98).step_by(7) {
        let (then, first) = (
            format!("fixture.mod{n:03}"),
            format!("fixture.mod{:03}", n - 1),
        );
        assert!(place(&then) > place(&first), "{then} loads after {first}");
    }
    assert!(order.list(Side::Inactive).mods().is_empty());
}

#[test]
#[ignore = "reads this machine's own install; RIMSTACK_TIME_HOME names a scratch folder"]
fn times_the_mods_tab_on_this_machines_install() {
    let Ok(scratch) = std::env::var("RIMSTACK_TIME_HOME") else {
        return;
    };
    let scratch = std::path::PathBuf::from(scratch);
    let home = std::path::PathBuf::from(std::env::var("HOME").unwrap());
    let app = App::new(
        home,
        &scratch.join("settings"),
        &scratch.join("data"),
        Default::default(),
    );
    let time = |what: &str, f: &mut dyn FnMut()| {
        let runs = 5;
        let started = std::time::Instant::now();
        for _ in 0..runs {
            f();
        }
        eprintln!("{what}: {:?}", started.elapsed() / runs);
    };
    let started = std::time::Instant::now();
    let mut order = LoadOrder::read(&app, true, None);
    eprintln!("read: {:?}", started.elapsed());
    eprintln!(
        "mods: {} active, {} inactive",
        order.rows(&app, Side::Active).len(),
        order.rows(&app, Side::Inactive).len()
    );
    if std::env::var("RIMSTACK_TIME_ALL").is_ok() {
        let ids: Vec<ModId> = order
            .rows(&app, Side::Inactive)
            .iter()
            .filter(|r| !r.is_header)
            .filter_map(|r| r.mod_id)
            .collect();
        let started = std::time::Instant::now();
        order.take_all(&app, Side::Inactive, &ids, None);
        eprintln!("take_all {}: {:?}", ids.len(), started.elapsed());
        eprintln!("now {} active", order.rows(&app, Side::Active).len());
    }
    time("recheck", &mut || order.recheck(&app));
    time("refile", &mut || order.refile(&app));
    time("settle", &mut || order.settle(&app));
    time("rows both", &mut || {
        let _ = order.rows(&app, Side::Active);
        let _ = order.rows(&app, Side::Inactive);
    });
    time("overview", &mut || {
        let _ = order.overview(&app);
    });
    time("fresh", &mut || {
        let _ = order.fresh(&app, Side::Inactive);
    });
    time("in_conflict", &mut || {
        let _ = order.in_conflict(&app);
    });
    time("orders", &mut || {
        let _ = order.orders();
    });
    time("set_filter a", &mut || order.set_filter("a"));
    time("set_filter ''", &mut || order.set_filter(""));
    time("sync", &mut || order.sync(&app));
}

#[test]
fn only_the_settings_the_lists_read_count_as_read() {
    let tmp = tempfile::tempdir().unwrap();
    let app = App::new(
        tmp.path().into(),
        &tmp.path().join("s"),
        &tmp.path().join("d"),
        Default::default(),
    );
    let before = LoadOrder::settings_read(&app);
    app.keep_setting("autoUpdates", "on").unwrap();
    assert_eq!(LoadOrder::settings_read(&app), before);
    let dimmed = app.settings().switch(Switch::DimLeftOut);
    app.keep_setting("dimLeftOut", if dimmed { "off" } else { "on" })
        .unwrap();
    assert_ne!(LoadOrder::settings_read(&app), before);
}

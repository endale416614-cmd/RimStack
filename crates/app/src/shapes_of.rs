use crate::App;
use crate::reports::{Answer, ReleaseSaid, Written};
use crate::shapes::Shapes;
use crate::workspace::changed::{NotesState, Only, since_shown};
use crate::workspace::downloads::{Checkout, Queue, Updates, updates_shown};
use crate::workspace::load_order::{LoadOrder, Side};
use crate::workspace::tests::installed;
use crate::workspace::{logs, saves, textures};
use rimstack_core::kept::launches;
use rimstack_core::mods::columns;
use rimstack_core::mods::library::ModId;
use std::collections::HashSet;
use std::path::Path;

fn write(at: &Path, text: &str) {
    std::fs::create_dir_all(at.parent().unwrap()).unwrap();
    std::fs::write(at, text).unwrap();
}

fn a_mod(game: &Path, folder: &str, id: &str, more: &str) {
    write(
        &game.join("Mods").join(folder).join("About/About.xml"),
        &format!(
            "<ModMetaData><packageId>{id}</packageId><name>{id}</name><supportedVersions><li>1.6</li></supportedVersions>{more}</ModMetaData>"
        ),
    );
}

fn picture(at: &Path, colour: [u8; 4]) {
    std::fs::create_dir_all(at.parent().unwrap()).unwrap();
    image::RgbaImage::from_pixel(4, 4, image::Rgba(colour))
        .save(at)
        .unwrap();
}

fn git(dir: &Path, args: &[&str]) {
    let ran = std::process::Command::new("git")
        .args([
            "-c",
            "user.name=a",
            "-c",
            "user.email=a@b",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(
        ran.status.success(),
        "{}",
        String::from_utf8_lossy(&ran.stderr)
    );
}

const SAVE: &str = "<?xml version=\"1.0\" encoding=\"utf-8\"?>
<savegame><meta><gameVersion>1.6.4871 rev600</gameVersion>
<modIds><li>ludeon.rimworld</li><li>ann.roads</li><li>gone.mod</li></modIds>
<modNames><li>Core</li><li>Ann.Roads</li><li>Gone</li></modNames></meta><game/></savegame>";

const STARTUP: &str = "<?xml version=\"1.0\"?><StartupImpactSession><sessionData>
<loadingTime>3000</loadingTime><totalImpact>2000</totalImpact><mods>
<li><modName>Ann.A</modName><modPackageId>Ann.A</modPackageId><totalImpact>1240</totalImpact></li>
</mods></sessionData></StartupImpactSession>";

const LOG: &str = "RimWorld 1.6.4871 rev600
Could not load texture for Ann.Roads
  at Verse.ContentFinder.Get
Could not load texture for Ann.Roads
all fine
";

fn made_up() -> (tempfile::TempDir, App) {
    let (tmp, app) = installed();
    let (root, game) = (tmp.path(), tmp.path().join("game"));
    let mods = game.join("Mods");
    write(
        &mods.join("roads/About/About.xml"),
        "<ModMetaData><packageId>Ann.Roads</packageId><name>Ann.Roads</name><supportedVersions><li>1.5</li></supportedVersions></ModMetaData>",
    );
    write(
        &mods.join("roads/Patches/rock.xml"),
        "<Patch><Operation Class=\"PatchOperationAdd\"><xpath>Defs/ThingDef[defName=\"Rock\"]/comps</xpath><value><li/></value></Operation></Patch>",
    );
    git(&mods.join("roads"), &["init", "--quiet"]);
    git(&mods.join("roads"), &["add", "."]);
    git(&mods.join("roads"), &["commit", "--quiet", "-m", "Roads"]);
    a_mod(&game, "a", "Ann.A", "");
    write(
        &mods.join("a/Defs/Things.xml"),
        "<Defs><ThingDef><defName>Rock</defName></ThingDef></Defs>",
    );
    a_mod(
        &game,
        "b",
        "Ann.B",
        "<modDependencies><li><packageId>Ann.Lib</packageId><displayName>Lib</displayName></li><li><packageId>gone.mod</packageId><displayName>Gone</displayName></li></modDependencies>",
    );
    a_mod(&game, "lib", "Ann.Lib", "");
    write(&mods.join("broken/About/About.xml"), "<ModMetaData><name>");
    let workshop = root.join("workshop/900002");
    write(
        &workshop.join("About/About.xml"),
        "<ModMetaData><packageId>Ann.Shop</packageId><name>Ann.Shop</name><supportedVersions><li>1.6</li></supportedVersions></ModMetaData>",
    );
    picture(&workshop.join("About/Preview.png"), [9, 9, 9, 255]);
    write(
        &root.join("workshop/900003/About/About.xml"),
        "<ModMetaData><packageId>Ann.Hats</packageId><name>Ann.Hats</name><supportedVersions><li>1.6</li></supportedVersions></ModMetaData>",
    );
    write(
        &mods.join("hats/About/About.xml"),
        "<ModMetaData><packageId>Ann.Hats</packageId><name>Ann.Hats</name><supportedVersions><li>1.5</li></supportedVersions></ModMetaData>",
    );
    picture(&game.join("Data/Core/About/Preview.png"), [90, 60, 30, 255]);
    picture(&mods.join("a/Textures/Things/rock.png"), [200, 0, 0, 255]);
    picture(&mods.join("b/Textures/Things/rock.png"), [0, 0, 200, 255]);
    write(&mods.join("a/Textures/Things/rock.dds"), "DDS not really");
    picture(&mods.join("b/Textures/Things/pebble.png"), [0, 200, 0, 255]);
    picture(&mods.join("lib/Textures/Things/moss.png"), [0, 90, 0, 255]);
    let config = root.join("config");
    write(
        &config.join("Config/ModsConfig.xml"),
        "<ModsConfigData><activeMods><li>ludeon.rimworld</li><li>ann.roads</li><li>ann.a</li><li>ann.b</li><li>pulled.mod</li><li>lost.mod</li></activeMods></ModsConfigData>",
    );
    write(&config.join("Player.log"), LOG);
    write(&config.join("StartupImpactData.xml"), STARTUP);
    write(&config.join("Saves/Colony.rws"), SAVE);
    write(
        &config.join("RimSaves/Meta/Colony.dat"),
        "faction:New Hope,nbColonist:3,seed:abc",
    );
    write(&config.join("RimSaves/Previews/Colony.jpg"), "jpeg");
    app.rescan();
    launches::record(&app.launches_file(), &["ludeon.rimworld".to_string()], 1).unwrap();
    app.mark_all_seen();
    a_mod(&game, "fresh", "Ann.Fresh", "");
    app.rescan();
    (tmp, app)
}

fn by_folder(app: &App, folder: &str) -> ModId {
    let loaded = app.library();
    let (library, _) = loaded.as_ref().as_ref().unwrap();

    library.ids().find(|(_, m)| m.folder == folder).unwrap().0
}

pub(crate) fn every() -> Shapes {
    let (_tmp, app) = made_up();
    let mut shapes = Shapes::default();
    logs(&app, &mut shapes);
    settings(&app, &mut shapes);
    mods(&app, &mut shapes);
    lists(&app, &mut shapes);
    game(&app, &mut shapes);
    saves(&app, &mut shapes);
    textures(&app, &mut shapes);
    search(&app, &mut shapes);
    downloads(&app, &mut shapes);
    settings_once_set(&app, &mut shapes);
    shapes
}

fn settings_once_set(app: &App, shapes: &mut Shapes) {
    let paths = app.paths();
    for (name, folder) in [
        ("game", &paths.game),
        ("config", &paths.config),
        ("workshop", &paths.workshop),
    ] {
        let folder = folder.as_ref().unwrap().to_string_lossy().into_owned();
        app.keep_setting(name, &folder).unwrap();
    }
    shapes.add("settings.report", &app.settings_report());
}

fn settings(app: &App, shapes: &mut Shapes) {
    for (name, value) in [
        ("replacementsIgnored", "1"),
        ("font", "Noto Sans"),
        ("language", "Polski"),
        ("launchArguments", "-quicktest"),
    ] {
        app.keep_setting(name, value).unwrap();
    }
    app.version_warnings(1).unwrap();
    shapes.add("settings.report", &app.settings_report());
    shapes.add("settings.databases", &app.databases());
    let checked = std::collections::BTreeMap::from([(
        "game",
        crate::workspace::judge_folder("game", "/nowhere"),
    )]);
    shapes.add("settings.folderChecks", &checked);
    shapes.add("settings.languages", &app.languages());
    shapes.add("settings.words", &app.speak());
    let colours = serde_json::json!({ "accent": "#336699" });
    let written = app.start_own_theme("light", &colours);
    shapes.add("settings.ownColours", &app.own_colours("light"));
    let written = written.map(|file| Written {
        file: file.to_string_lossy().into_owned(),
    });
    shapes.add("settings.startOwnTheme", &Answer::from(written));
    let refused = app.start_language("../outside").map(|file| Written {
        file: file.to_string_lossy().into_owned(),
    });
    shapes.add("settings.startLanguage", &Answer::from(refused));
    let started = app.start_language("Polski").map(|file| Written {
        file: file.to_string_lossy().into_owned(),
    });
    shapes.add("settings.startLanguage", &Answer::from(started));
    let pair = serde_json::json!({ "ink": "#777777", "win": "#888888" });
    let contrast = crate::workspace::short_contrast(pair.as_object().unwrap());
    shapes.add("settings.shortContrast", &contrast);
    shapes.add("settings.release", &ReleaseSaid::Latest {});
    shapes.add(
        "settings.release",
        &ReleaseSaid::Newer {
            version: "2.0.0".into(),
            page: "https://example.org".into(),
        },
    );
    shapes.add(
        "settings.release",
        &ReleaseSaid::Failed {
            problem: rimstack_core::said!("refused"),
        },
    );
}

fn mods(app: &App, shapes: &mut Shapes) {
    let (roads, a, b) = (
        by_folder(app, "roads"),
        by_folder(app, "a"),
        by_folder(app, "b"),
    );
    let (shop, broken) = (by_folder(app, "900002"), by_folder(app, "broken"));
    let (hats, subscribed_hats) = (by_folder(app, "hats"), by_folder(app, "900003"));
    app.set_note(a, "Stone, Grey", "Read first");
    app.change_rule(a, b, true);
    app.set_version_ok(&[roads], true);
    write(
        &rimstack_core::rimworld::workshopdb::file_in(app.data_dir()),
        "pulled.mod\t900009\tPulled",
    );
    let mut order = LoadOrder::read(app, true, false);
    order.settle(app);
    if let Some(members) = order.files_to_read(app) {
        app.read_active_files(members);
        order.recheck(app);
    }
    shapes.add("mods.summary", &app.summary());
    shapes.add("mods.overview", &order.overview(app));
    shapes.add("mods.fresh", &order.fresh(app, Side::Inactive));
    for at in 0..order.list(Side::Active).mods().len() {
        if let Some(gone) = order.not_installed(app, Side::Active, at) {
            shapes.add("mods.notInstalled", &gone);
        }
    }
    for id in [roads, a, b, shop, broken, hats, subscribed_hats] {
        shapes.add("mods.details", &order.details(app, id));
        shapes.add("mods.detailsOf", &order.details(app, id));
        shapes.add("mods.filesOf", &LoadOrder::files(app, id));
        shapes.add("mods.textures", &LoadOrder::textures(app, id));
    }
    let ids = |found: Vec<ModId>| found.into_iter().map(ModId::to_qml).collect::<Vec<_>>();
    shapes.add("mods.versionWarned", &ids(vec![roads]));
    shapes.add("mods.packageMods", &ids(vec![a, b]));
    shapes.add("mods.upkeepOf", &order.upkeep(app, &[roads, shop, broken]));
    shapes.add("mods.findMods", &order.found(app, "Ann", 5));
    order.preview_sort(app);
    shapes.add("mods.sortMoves", &order.sort_moves());
    let list = order.list(Side::Active);
    shapes.add("mods.othersThrough", &list.others_through(a, b).map(ids));
    let ordered = list.in_load_order(a, b);
    shapes.add(
        "mods.inLoadOrder",
        &ordered.map(|(earlier, later)| [earlier.to_qml(), later.to_qml()]),
    );
    shapes.add("mods.inLoadOrder", &None::<[i32; 2]>);
    let category = list
        .mods()
        .iter()
        .find(|m| m.installed == Some(a))
        .unwrap()
        .category;
    shapes.add("mods.subsBeside", &list.subs(category));
    shapes.add("mods.columnWidths", &columns::widths("name=200,author=90"));
    shapes.add(
        "mods.columnPressed",
        &columns::pressed("name", false, "name"),
    );
    for row in order.rows(app, Side::Active) {
        shapes.add("mod_rows.at", &crate::mod_rows::fields_of(&row));
    }
    order.save(app).unwrap();
}

fn lists(app: &App, shapes: &mut Shapes) {
    let ids = vec!["ludeon.rimworld".to_string(), "ann.a".to_string()];
    app.keep_list("mine", Some(ids.clone())).unwrap();
    app.snapshot("@saving", &ids).unwrap();
    shapes.add("lists.saved", &app.saved_lists());
    shapes.add("lists.snapshots", &app.recent_snapshots());
    shapes.add("lists.named", &app.list_named("mine"));
    shapes.add("lists.named", &app.list_named("none"));
    let snapshot = app.recent_snapshots().first().map(|s| s.name.clone());
    let snapshot = snapshot.unwrap_or_default();
    shapes.add("lists.snapshotNamed", &app.snapshot_named(&snapshot));
    shapes.add(
        "lists.idsInText",
        &rimstack_core::mods::share::ids_in("ludeon.rimworld\nann.a"),
    );
    shapes.add("lists.original", &app.original_list());
}

fn game(app: &App, shapes: &mut Shapes) {
    shapes.add("game.lastStart", &app.last_start());
    shapes.add("game.resetByGame", &app.mods_config_list());
}

fn saves(app: &App, shapes: &mut Shapes) {
    let report = app.saves_report();
    shapes.add("saves.saves", &saves::shown(&report, "", "").saves);
    shapes.add("saves.folders", &report.folders);
}

fn logs(app: &App, shapes: &mut Shapes) {
    app.keep_logs();
    let (report, _) = app.logs_report(false, None).unwrap();
    shapes.add("logs.head", &logs::head(&report));
    shapes.add("logs.culprits", &report.culprits);
    shapes.add("logs.launches", &logs::launches(&report));
    shapes.add("logs.since", &logs::since(&report));
    let open = HashSet::new();
    shapes.add("logs.errors", &logs::listed(&report.errors, "", &open));
    shapes.add("logs.lines", &logs::lines(&report, ""));
    shapes.add("logs.errorsOf", &logs::errors_of(&report, "Ann.Roads"));
}

fn textures(app: &App, shapes: &mut Shapes) {
    let (a, b) = (by_folder(app, "a"), by_folder(app, "b"));
    let stop = std::sync::atomic::AtomicBool::new(false);
    app.convert_textures(&[b], false, &stop, &|_| {});
    let loaded = app.library();
    let (library, _) = loaded.as_ref().as_ref().unwrap();
    let pebble = library
        .get(b)
        .unwrap()
        .path
        .join("Textures/Things/pebble.png");
    drop(loaded);
    picture(&pebble, [0, 0, 0, 255]);
    let rows = app.texture_rows();
    shapes.add("textures.report", &textures::shown(&rows, false));
    shapes.add("textures.refused", &app.refused_dds());
    let compared = app.compare_textures(a, b).unwrap();
    shapes.add("textures.comparison", &compared);
    shapes.add("textures.shared", &compared.named(""));
    let difference = app.texture_difference(&compared.textures[0]);
    shapes.add("textures.difference", &Answer::from(difference));
    let done = app.keep_all_textures(&compared, true);
    let done = done.map(|said| serde_json::json!({ "said": said }));
    shapes.add("textures.keepAll", &Answer::from(done));
    let again = app.compare_textures(a, b).unwrap();
    shapes.add("textures.comparison", &again);
    shapes.add("textures.shared", &again.named(""));
    shapes.add("lists.history", &app.history());
}

fn search(app: &App, shapes: &mut Shapes) {
    shapes.add("search.list", &app.start_hunt(false).list);
    shapes.add("search.end", &app.end_hunt(false));
}

fn downloads(app: &App, shapes: &mut Shapes) {
    use rimstack_core::sources::downloads::{Asked, Behind, Changed, Replaceable, Repository};
    let (roads, b) = (by_folder(app, "roads"), by_folder(app, "b"));
    shapes.add("downloads.replacements", &app.replaceable());
    let replaced = Replaceable {
        id: "1".into(),
        mod_id: b,
        name: "Ann.B".into(),
        active: true,
        new_name: "Ann.B2".into(),
        new_id: "2".into(),
        new_versions: "1.6".into(),
        have_new: false,
    };
    shapes.add("downloads.replacements", &[replaced]);
    let behind = |active| Behind {
        id: "1".into(),
        mod_id: b,
        name: "Ann.B".into(),
        active,
        have: 1,
        now: 2,
    };
    let checkout = Checkout {
        mod_id: roads,
        name: "Ann.Roads".into(),
        workshop: false,
        checked: true,
        behind: Some(2),
        problem: Some(rimstack_core::said!("refused")),
    };
    let updates = Updates {
        checked: Some(1),
        asked: Some(1),
        answered: 1,
        items: vec![behind(true), behind(false)],
        checkouts: vec![checkout],
        problem: Some(rimstack_core::said!("refused")),
    };
    let asked = HashSet::from(["1".to_string()]);
    shapes.add("downloads.updates", &updates_shown(&updates, &asked));
    shapes.add(
        "downloads.updates",
        &updates_shown(&Updates::default(), &asked),
    );
    let asked = |title: Option<&str>| Asked {
        id: "1".into(),
        title: title.map(String::from),
        updated: title.map(|_| 1),
        megabytes: title.map(|_| 1.5),
        installed: false,
    };
    let queue = Queue {
        items: vec![asked(Some("Ann.B")), asked(None)],
        repositories: vec![Repository {
            link: "https://github.com/ann/roads".into(),
            name: "ann/roads".into(),
            into: Some("/mods/roads".into()),
            installed: true,
        }],
        note: None,
        wanted: vec!["1".into()],
    };
    shapes.add("downloads.queue", &queue);
    shapes.add(
        "downloads.queue",
        &app.look_up("nothing to download here").unwrap(),
    );
    let refused: Result<(), _> = Err(rimstack_core::base::error::Error::refused(
        rimstack_core::said!("refused"),
    ));
    shapes.add("downloads.queue", &Answer::from(refused));
    let mut since = app.since_launch();
    since.items.push(Changed {
        git: false,
        id: "1".into(),
        mod_id: b,
        name: "Ann.B".into(),
        active: true,
        was: 1,
        now: Some(2),
        notes: app.commits_of(roads),
    });
    let mut notes = crate::workspace::changed::ChangeNotes::new();
    notes.insert("1".into(), app.commits_of(roads));
    shapes.add("downloads.since", &since_shown(&since, Only::All, &notes));
    shapes.add("downloads.notes", &notes);
    let state = NotesState {
        done: 1,
        asked: 2,
        resume: 0,
        refused: false,
    };
    shapes.add("downloads.notesState", &state);
    shapes.add("downloads.commitsOf", &app.commits_of(roads));
}

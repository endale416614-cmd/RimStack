use super::*;
use std::fs;

const NATIVE_CONFIG: &str = ".config/unity3d/Ludeon Studios/RimWorld by Ludeon Studios";

fn library_with_game(library: &Path) {
    let game = library.join("steamapps/common/RimWorld");
    fs::create_dir_all(game.join("Data")).unwrap();
    fs::create_dir_all(game.join("Mods")).unwrap();
    fs::write(game.join("Version.txt"), "1.6.4871 rev598\n").unwrap();
    fs::write(game.join("RimWorldLinux"), "").unwrap();
    fs::create_dir_all(library.join("steamapps/workshop/content/294100")).unwrap();
}

const FLATPAK: &str = ".var/app/com.valvesoftware.Steam";

fn steam_root(home: &Path, libraries: &[(&Path, bool)]) -> PathBuf {
    let root = home.join(".local/share/Steam");
    fs::create_dir_all(root.join("steamapps")).unwrap();
    let mut vdf = String::from("\"libraryfolders\"\n{\n");
    for (i, (path, has_game)) in libraries.iter().enumerate() {
        let apps = if *has_game { "\"294100\" \"1\"" } else { "" };
        vdf += &format!(
            "\"{i}\" {{ \"path\" \"{}\" \"apps\" {{ {apps} }} }}\n",
            path.display()
        );
    }
    fs::write(root.join("steamapps/libraryfolders.vdf"), vdf + "}\n").unwrap();
    root
}

#[test]
fn native_install_is_found() {
    let home = tempfile::tempdir().unwrap();
    let root = steam_root(home.path(), &[]);
    library_with_game(&root);
    fs::create_dir_all(home.path().join(NATIVE_CONFIG)).unwrap();

    let paths = detect(home.path(), &Overrides::default());
    let root = root.canonicalize().unwrap();
    assert_eq!(paths.game, Some(root.join("steamapps/common/RimWorld")));
    assert_eq!(
        paths.workshop,
        Some(root.join("steamapps/workshop/content/294100"))
    );
    assert_eq!(paths.config, Some(home.path().join(NATIVE_CONFIG)));
    assert_eq!(paths.game_version.as_deref(), Some("1.6"));
    assert_eq!(paths.full_game_version.as_deref(), Some("1.6.4871 rev598"));
    let sources: Vec<Source> = paths.roots().iter().map(|r| r.source).collect();
    assert_eq!(sources, [Source::Official, Source::Local, Source::Workshop]);
}

#[cfg(unix)]
#[test]
fn game_in_a_second_library_is_found_and_symlinked_roots_count_once() {
    let home = tempfile::tempdir().unwrap();
    let second = home.path().join("games");
    library_with_game(&second);
    let root = steam_root(home.path(), &[(&second, true)]);
    fs::create_dir_all(home.path().join(".steam")).unwrap();
    std::os::unix::fs::symlink(&root, home.path().join(".steam/steam")).unwrap();

    assert_eq!(steam_roots(Os::Linux, home.path(), &|_| None).len(), 1);
    let paths = detect(home.path(), &Overrides::default());
    let second = second.canonicalize().unwrap();
    assert_eq!(paths.game, Some(second.join("steamapps/common/RimWorld")));
}

#[test]
fn a_workshop_folder_set_to_the_games_mods_is_read_once_as_local() {
    let home = tempfile::tempdir().unwrap();
    library_with_game(home.path());
    let game = home.path().join("steamapps/common/RimWorld");
    let paths = Paths {
        workshop: Some(game.join("Mods/.")),
        game: Some(game),
        ..Paths::default()
    };
    let sources: Vec<Source> = paths.roots().iter().map(|r| r.source).collect();
    assert_eq!(sources, [Source::Official, Source::Local]);
}

#[test]
fn library_listing_the_app_beats_a_stale_copy() {
    let home = tempfile::tempdir().unwrap();
    let (stale, real) = (home.path().join("old"), home.path().join("new"));
    library_with_game(&stale);
    library_with_game(&real);
    steam_root(home.path(), &[(&stale, false), (&real, true)]);

    let paths = detect(home.path(), &Overrides::default());
    let real = real.canonicalize().unwrap();
    assert_eq!(paths.game, Some(real.join("steamapps/common/RimWorld")));
}

#[test]
fn proton_settings_are_found() {
    let home = tempfile::tempdir().unwrap();
    let root = steam_root(home.path(), &[]);
    library_with_game(&root);
    let game = root.join("steamapps/common/RimWorld");
    fs::remove_file(game.join("RimWorldLinux")).unwrap();
    fs::write(game.join("RimWorldWin64.exe"), "").unwrap();
    let prefix = root
        .join("steamapps/compatdata/294100/pfx/drive_c/users/steamuser/AppData/LocalLow")
        .join(SETTINGS);
    fs::create_dir_all(&prefix).unwrap();
    fs::create_dir_all(home.path().join(NATIVE_CONFIG)).unwrap();

    let paths = detect(home.path(), &Overrides::default());
    assert_eq!(paths.config, Some(prefix.canonicalize().unwrap()));
}

#[test]
fn the_folders_of_windows_and_macos_are_found() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("home");
    let programs = tmp.path().join("Program Files (x86)");
    let settings = home.join("AppData/LocalLow").join(SETTINGS);
    fs::create_dir_all(programs.join("Steam/steamapps/common/RimWorld")).unwrap();
    fs::create_dir_all(programs.join("Steam/steamapps/workshop/content/294100")).unwrap();
    fs::create_dir_all(&settings).unwrap();
    let env = |name: &str| (name == "ProgramFiles(x86)").then(|| programs.clone());
    let found = detect_on(Os::Windows, &home, &env, &Overrides::default());
    assert!(
        found
            .game
            .unwrap()
            .ends_with("Steam/steamapps/common/RimWorld")
    );
    assert!(found.workshop.unwrap().ends_with("workshop/content/294100"));
    assert_eq!(found.config.unwrap(), settings);

    let support = home.join("Library/Application Support");
    let installed = support.join("Steam/steamapps/common/RimWorld");
    fs::create_dir_all(installed.join("RimWorldMac.app")).unwrap();
    fs::create_dir_all(installed.join("Data")).unwrap();
    fs::create_dir_all(support.join("Rimworld")).unwrap();
    let found = detect_on(Os::Mac, &home, &|_| None, &Overrides::default());
    assert!(found.game.unwrap().ends_with("common/RimWorld"));
    assert_eq!(found.config.unwrap(), support.join("Rimworld"));
    fs::create_dir_all(installed.join("RimWorldMac.app/Data")).unwrap();
    let found = detect_on(Os::Mac, &home, &|_| None, &Overrides::default());
    assert!(found.game.unwrap().ends_with("RimWorld/RimWorldMac.app"));
}

#[test]
fn macos_settings_follow_the_game_s_version_when_both_folders_are_left() {
    let home = tempfile::tempdir().unwrap();
    let support = home.path().join("Library/Application Support");
    let installed = support.join("Steam/steamapps/common/RimWorld");
    fs::create_dir_all(installed.join("Data")).unwrap();
    let (unity, waylith) = (
        support.join("Ludeon Studios/RimWorld"),
        support.join("Rimworld"),
    );
    fs::create_dir_all(&unity).unwrap();
    fs::create_dir_all(&waylith).unwrap();
    let config_for = |version: &str| {
        fs::write(installed.join("Version.txt"), version).unwrap();
        detect_on(Os::Mac, home.path(), &|_| None, &Overrides::default()).config
    };
    assert_eq!(config_for("1.7.4976 rev1043"), Some(waylith.clone()));
    assert_eq!(config_for("1.6.4871 rev598"), Some(unity));
    fs::remove_file(installed.join("Version.txt")).unwrap();
    let found = detect_on(Os::Mac, home.path(), &|_| None, &Overrides::default());
    assert_eq!(found.config, Some(waylith));
}

#[test]
fn steam_s_folder_is_read_from_what_the_registry_prints() {
    let printed = "\r\nHKEY_LOCAL_MACHINE\\SOFTWARE\\Valve\\Steam\r\n    InstallPath    REG_SZ    D:\\Games\\Steam\r\n";
    assert_eq!(
        install_path(printed),
        Some(PathBuf::from("D:\\Games\\Steam"))
    );
    assert_eq!(
        install_path("ERROR: The system was unable to find the key."),
        None
    );
    let tmp = tempfile::tempdir().unwrap();
    let steam = tmp.path().join("Steam");
    fs::create_dir_all(steam.join("steamapps/common/RimWorld")).unwrap();
    let env = |name: &str| (name == STEAM_INSTALL).then(|| steam.clone());
    let found = detect_on(Os::Windows, tmp.path(), &env, &Overrides::default());
    assert!(
        found
            .game
            .unwrap()
            .ends_with("Steam/steamapps/common/RimWorld")
    );
}

#[test]
fn a_program_s_own_folders_follow_the_system() {
    let home = Path::new("/h");
    let none = |_: &str| None;
    assert_eq!(
        app_dirs(Os::Linux, home, &none),
        (home.join(".config"), home.join(".local/share"))
    );
    let set = |name: &str| (name == "XDG_DATA_HOME").then(|| PathBuf::from("/data"));
    assert_eq!(app_dirs(Os::Linux, home, &set).1, PathBuf::from("/data"));
    assert_eq!(
        app_dirs(Os::Windows, home, &none),
        (home.join("AppData/Roaming"), home.join("AppData/Local"))
    );
    let support = home.join("Library/Application Support");
    assert_eq!(app_dirs(Os::Mac, home, &none), (support.clone(), support));
    let windows = |name: &str| (name == "USERPROFILE").then(|| PathBuf::from("/u"));
    assert_eq!(super::home(&windows), PathBuf::from("/u"));
}

#[test]
fn a_copy_from_another_shop_is_found_by_its_core_and_has_no_workshop() {
    let home = tempfile::tempdir().unwrap();
    let game = home.path().join("GOG Games/RimWorld 1.6");
    fs::create_dir_all(game.join("Data/Core")).unwrap();
    fs::create_dir_all(home.path().join("GOG Games/Another Game/Data")).unwrap();
    fs::write(game.join("Version.txt"), "1.6.4871 rev600").unwrap();
    let paths = detect_on(Os::Linux, home.path(), &|_| None, &Overrides::default());
    assert_eq!(paths.game, Some(game.canonicalize().unwrap()));
    assert_eq!(
        (paths.workshop, paths.game_version.as_deref()),
        (None, Some("1.6"))
    );

    let mac = tempfile::tempdir().unwrap();
    let bundle = mac.path().join("Applications/RimWorld.app");
    fs::create_dir_all(bundle.join("Data/Core")).unwrap();
    let paths = detect_on(Os::Mac, mac.path(), &|_| None, &Overrides::default());
    assert_eq!(paths.game, Some(bundle.canonicalize().unwrap()));
}

#[test]
fn steam_s_list_of_libraries_is_read_from_either_place_it_keeps_it() {
    let home = tempfile::tempdir().unwrap();
    let second = home.path().join("games");
    library_with_game(&second);
    let root = steam_root(home.path(), &[(&second, true)]);
    fs::create_dir_all(root.join("config")).unwrap();
    let (usual, other) = ("steamapps/libraryfolders.vdf", "config/libraryfolders.vdf");
    fs::rename(root.join(usual), root.join(other)).unwrap();
    let paths = detect(home.path(), &Overrides::default());
    let second = second.canonicalize().unwrap();
    assert_eq!(paths.game, Some(second.join("steamapps/common/RimWorld")));
}

#[test]
fn nothing_installed_finds_nothing() {
    let home = tempfile::tempdir().unwrap();
    let paths = detect(home.path(), &Overrides::default());
    assert_eq!(paths, Paths::default());
    assert!(paths.roots().is_empty());
    assert_eq!(paths.mods_config(), None);
}

#[test]
fn a_game_set_by_hand_wins_and_has_no_workshop() {
    let home = tempfile::tempdir().unwrap();
    let root = steam_root(home.path(), &[]);
    library_with_game(&root);
    let copy = home.path().join("drm-free");
    fs::create_dir_all(copy.join("Mods")).unwrap();

    let overrides = Overrides {
        game: Some(copy.clone()),
        ..Overrides::default()
    };
    let paths = detect(home.path(), &overrides);
    assert_eq!(paths.game, Some(copy));
    assert_eq!(paths.workshop, None);
    assert_eq!(paths.roots().len(), 1);
}

#[test]
fn a_folder_typed_by_hand_is_judged_by_what_it_holds() {
    let dir = tempfile::tempdir().unwrap();
    let game = dir.path().join("game");
    let judged = |kind, place: &Path| judge(Os::Linux, kind, place).map_err(|e| e.to_string());
    assert!(judged(Folder::Game, &game).is_err());
    fs::create_dir_all(&game).unwrap();
    assert!(judged(Folder::Game, &game).unwrap_err().contains("Data"));
    fs::create_dir_all(game.join("Data")).unwrap();
    fs::write(game.join("Version.txt"), "1.6.4871 rev598\n").unwrap();
    assert_eq!(
        judged(Folder::Game, &game).unwrap().to_string(),
        "RimWorld 1.6 found"
    );
    assert!(judged(Folder::Config, &game).is_err());
    fs::create_dir_all(game.join("Saves")).unwrap();
    assert!(judged(Folder::Config, &game).is_ok());
    assert!(judged(Folder::Workshop, &game).is_ok());
}

#[test]
fn a_settings_folder_typed_as_its_saves_is_the_folder_above() {
    let home = tempfile::tempdir().unwrap();
    let settings = home.path().join("settings");
    fs::create_dir_all(settings.join("Saves")).unwrap();
    let typed = settings.join("Saves");
    assert!(judge(Os::Linux, Folder::Config, &typed).is_ok());
    let overrides = Overrides {
        config: Some(typed),
        ..Overrides::default()
    };
    assert_eq!(detect(home.path(), &overrides).config, Some(settings));
}

fn on_linux(home: &Path, overrides: &Overrides) -> Paths {
    detect_on(Os::Linux, home, &|_| None, overrides)
}

#[test]
fn a_sandboxed_steam_s_game_keeps_its_settings_in_the_sandbox_over_the_host_s() {
    for sandbox in SANDBOXES {
        let home = tempfile::tempdir().unwrap();
        let sandbox = home.path().join(sandbox);
        let root = steam_root(&sandbox, &[]);
        library_with_game(&root);
        fs::create_dir_all(home.path().join(NATIVE_CONFIG)).unwrap();
        fs::create_dir_all(sandbox.join(NATIVE_CONFIG)).unwrap();

        let paths = on_linux(home.path(), &Overrides::default());
        assert_eq!(paths.config, Some(sandbox.join(NATIVE_CONFIG)));

        let typed = Overrides {
            game: Some(root.join("steamapps/common/RimWorld")),
            ..Overrides::default()
        };
        let paths = on_linux(home.path(), &typed);
        assert_eq!(paths.config, Some(sandbox.join(NATIVE_CONFIG)));
    }
}

#[test]
fn a_library_flatpak_steam_lists_elsewhere_keeps_its_settings_in_the_sandbox() {
    let home = tempfile::tempdir().unwrap();
    let drive = tempfile::tempdir().unwrap();
    library_with_game(drive.path());
    let sandbox = home.path().join(FLATPAK);
    steam_root(&sandbox, &[(drive.path(), true)]);
    fs::create_dir_all(home.path().join(NATIVE_CONFIG)).unwrap();
    fs::create_dir_all(sandbox.join(NATIVE_CONFIG)).unwrap();

    let paths = on_linux(home.path(), &Overrides::default());
    assert_eq!(paths.config, Some(sandbox.join(NATIVE_CONFIG)));
}

#[test]
fn host_steam_s_game_keeps_its_settings_on_the_host_over_a_stale_sandbox() {
    let home = tempfile::tempdir().unwrap();
    let root = steam_root(home.path(), &[]);
    library_with_game(&root);
    fs::create_dir_all(home.path().join(NATIVE_CONFIG)).unwrap();
    fs::create_dir_all(home.path().join(FLATPAK).join(NATIVE_CONFIG)).unwrap();

    let paths = on_linux(home.path(), &Overrides::default());
    assert_eq!(paths.config, Some(home.path().join(NATIVE_CONFIG)));
}

#[test]
fn flatpak_steam_s_game_falls_back_to_the_host_s_settings() {
    let home = tempfile::tempdir().unwrap();
    library_with_game(&steam_root(&home.path().join(FLATPAK), &[]));
    fs::create_dir_all(home.path().join(NATIVE_CONFIG)).unwrap();

    let paths = on_linux(home.path(), &Overrides::default());
    assert_eq!(paths.config, Some(home.path().join(NATIVE_CONFIG)));
}

#[cfg(unix)]
#[test]
fn flatpak_steam_reached_through_links_is_still_the_sandbox_s() {
    let real = tempfile::tempdir().unwrap();
    let sandbox = real.path().join(FLATPAK);
    let root = steam_root(&sandbox, &[]);
    library_with_game(&root);
    fs::create_dir_all(real.path().join(NATIVE_CONFIG)).unwrap();
    fs::create_dir_all(sandbox.join(NATIVE_CONFIG)).unwrap();
    fs::create_dir_all(real.path().join(".steam")).unwrap();
    std::os::unix::fs::symlink(&root, real.path().join(".steam/steam")).unwrap();
    let outside = tempfile::tempdir().unwrap();
    let home = outside.path().join("home");
    std::os::unix::fs::symlink(real.path(), &home).unwrap();

    let paths = on_linux(&home, &Overrides::default());
    let sandbox = sandbox.canonicalize().unwrap();
    assert_eq!(paths.config, Some(sandbox.join(NATIVE_CONFIG)));
}

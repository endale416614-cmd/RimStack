//! Read-only.

use super::*;
use crate::rimworld::paths::{Overrides, detect};

#[test]
#[ignore = "needs RimWorld installed"]
fn real_install_categories() {
    let home = std::path::PathBuf::from(std::env::var_os("HOME").unwrap());
    let paths = detect(&home, &Overrides::from_env());
    let started = std::time::Instant::now();
    let library = load(&paths).unwrap();
    println!("loaded in {:?}", started.elapsed());
    let all = || library.active.iter().chain(&library.inactive);
    for category in [Category::Loaders, Category::Interface, Category::Tweaks] {
        let names: Vec<&str> = all()
            .filter(|m| m.category == category)
            .map(|m| m.name.as_str())
            .collect();
        println!("{category:?}: {} {names:?}", names.len());
    }
    assert!(all().any(|m| m.category == Category::Loaders));
}

#[test]
#[ignore = "needs RimWorld installed"]
fn real_mods_config_copy_is_unchanged_by_writing_the_same_list() {
    use std::fs;
    let home = std::path::PathBuf::from(std::env::var_os("HOME").unwrap());
    let paths = detect(&home, &Overrides::from_env());
    let before = fs::read_to_string(paths.mods_config().unwrap()).unwrap();
    // Only ever a copy: the real file is never written by a test.
    let dir = tempfile::tempdir().unwrap();
    let copy = dir.path().join("ModsConfig.xml");
    fs::write(&copy, &before).unwrap();
    let ids = crate::rimworld::modsconfig::parse(&before).unwrap().active;
    crate::rimworld::modsconfig::write(&copy, &ids, &ids).unwrap();
    assert_eq!(fs::read_to_string(&copy).unwrap(), before);
    println!("{} ids written back, file identical", ids.len());
}

/// The order the game ran with before RimStack sorted it is kept as
/// `ModsConfig.xml.rimstack-original`.
#[test]
#[ignore = "needs RimWorld installed"]
fn real_install_inferred_rules_mostly_agree_with_a_working_order() {
    use rayon::prelude::*;
    let home = std::path::PathBuf::from(std::env::var_os("HOME").unwrap());
    let paths = detect(&home, &Overrides::from_env());
    let config = paths.mods_config().unwrap();
    let original = config.with_file_name("ModsConfig.xml.rimstack-original");
    let baseline = if original.exists() { original } else { config };
    let ids = crate::rimworld::modsconfig::read(&baseline).unwrap().active;
    let mut library = build(
        scan(&paths.roots()),
        &ids,
        paths.game_version.as_deref().unwrap(),
    );
    library.game_assemblies = paths
        .game
        .as_deref()
        .and_then(Game::find)
        .map(|game| game.ships().clone())
        .unwrap_or_default();
    let active: Vec<&Installed> = library
        .active
        .iter()
        .filter_map(|m| library.get(m.installed?))
        .collect();
    let keys: std::collections::HashSet<String> = active.iter().filter_map(|m| m.key()).collect();

    let started = std::time::Instant::now();
    let scanned: Vec<crate::mods::infer::Scanned> = active
        .par_iter()
        .map(|m| {
            crate::mods::infer::scan(
                &crate::rimworld::loadfolders::folders(&m.path, &library.game_version, &|k| {
                    keys.contains(k)
                }),
                &library.game_assemblies,
            )
        })
        .collect();
    let names: Vec<String> = active
        .iter()
        .map(|m| m.display_name().to_string())
        .collect();
    let skip: Vec<bool> = active
        .iter()
        .map(|m| m.source == Source::Official)
        .collect();
    let roots: Vec<std::path::PathBuf> = active.iter().map(|m| m.path.clone()).collect();
    let borrowed: Vec<&crate::mods::infer::Scanned> = scanned.iter().collect();
    let found = crate::mods::infer::infer(&borrowed, &names, &skip, &roots);
    println!(
        "{} mods scanned, {} rules inferred in {:?}",
        active.len(),
        found.len(),
        started.elapsed()
    );
    let defs: usize = scanned.iter().map(|s| s.defines.len()).sum();
    println!(
        "{defs} defs, {} patched names",
        scanned.iter().map(|s| s.patched.len()).sum::<usize>()
    );
    let broken: Vec<&crate::mods::infer::Inferred> =
        found.iter().filter(|r| r.first > r.then).collect();
    println!("{} broken by the baseline order", broken.len());
    assert!(
        broken.len() * 4 <= found.len(),
        "{} of {} broken",
        broken.len(),
        found.len()
    );
    for r in broken.iter().take(15) {
        println!(
            "  {} should load after {}: {}",
            names[r.then], names[r.first], r.why
        );
    }
    for r in found.iter().take(8) {
        println!(
            "  ok: {} after {}: {}",
            names[r.then], names[r.first], r.why
        );
    }
}

#[test]
#[ignore = "needs RimWorld installed"]
fn real_install_loads_and_matches_the_game_s_list() {
    let home = std::path::PathBuf::from(std::env::var_os("HOME").unwrap());
    let paths = detect(&home, &Overrides::from_env());
    println!("{paths:#?}");
    let started = std::time::Instant::now();
    let mods = scan(&paths.roots());
    println!("scanned {} mods in {:?}", mods.len(), started.elapsed());

    let mut reasons: HashMap<String, usize> = HashMap::new();
    for m in &mods {
        if let Err(e) = &m.about {
            let said = e.to_string();
            *reasons
                .entry(said.split(':').next().unwrap().to_string())
                .or_default() += 1;
            println!("  {}: {e}", m.path.display());
        }
    }
    println!("unreadable by reason: {reasons:?}");
    let mut copies: HashMap<String, Vec<Source>> = HashMap::new();
    for m in &mods {
        if let Some(k) = m.key() {
            copies.entry(k).or_default().push(m.source);
        }
    }
    copies.retain(|_, v| v.len() > 1);
    println!("ids installed more than once: {copies:?}");

    let active = crate::rimworld::modsconfig::read(&paths.mods_config().unwrap())
        .unwrap()
        .active;
    let resolved = resolve_active(&active, &mods);
    assert_eq!(resolved.len(), active.len());
    for (written, entry) in active.iter().zip(&resolved) {
        match entry {
            Resolved::Found(m) => assert_eq!(m.key().unwrap(), id::split_steam(written).0),
            Resolved::Missing(id) => println!("active but not installed: {id}"),
        }
    }
    let total = mods.len();
    let library = build(
        mods.clone(),
        &active,
        paths.game_version.as_deref().unwrap(),
    );
    let mut per_category: std::collections::BTreeMap<Category, usize> = Default::default();
    for m in &library.inactive {
        *per_category.entry(m.category).or_default() += 1;
    }
    println!(
        "active {}, inactive {} {per_category:?}, missing {}, unreadable {}",
        library.active.len(),
        library.inactive.len(),
        library.missing,
        library.unreadable
    );
    assert!(
        reasons.values().sum::<usize>() * 100 <= total,
        "more than 1% unreadable"
    );
}

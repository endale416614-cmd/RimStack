use rimstack_core::mods::list::{Category, GroupedList, Mod};
use rimstack_core::mods::rules::{Kind, Rule};
use rimstack_core::{
    base::markup, base::vdf, kept::notes, kept::owntheme, mods::community, mods::ddsaudit,
    mods::share, mods::sort, rimworld::about, rimworld::dotnet, rimworld::loadfolders,
    rimworld::logs, rimworld::modsconfig, rimworld::rimsaves, rimworld::steamworks,
    rimworld::workshop, sources::changes,
};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::time::{Duration, Instant};

struct Random(u64);

impl Random {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

const PIECES: &[&str] = &[
    "<",
    ">",
    "</",
    "/>",
    "[",
    "]",
    "[/",
    "=",
    "\"",
    "'",
    "&",
    "&amp;",
    "#",
    ":",
    "/",
    "\\",
    ".",
    ",",
    " ",
    "\n",
    "\r\n",
    "\t",
    "\u{feff}",
    "é",
    "ß",
    "İ",
    "日本",
    "𝔘",
    "\u{202e}",
    "0",
    "7",
    "1.6",
    "42",
    "activeMods",
    "<activeMods>",
    "</activeMods>",
    "<activeMods/>",
    "li",
    "<li>",
    "</li>",
    "url",
    "[url=",
    "[/url]",
    "img",
    "[img]",
    "[/img]",
    "b",
    "i",
    "h1",
    "list",
    "*",
    "https://",
    "http://",
    "steam://",
    "javascript:",
    "github.com/a/b",
    "?id=",
    "<p id=\"",
    "\">",
    "</p>",
    "<br>",
    "RimWorld ",
    "ERROR",
    "Exception",
    "  at ",
    "(wrapper",
    "[Ref ",
    "2026-10-12 19:37:35.060 ",
    "[Verse] ",
    "\"path\"",
    "\"apps\"",
    "{",
    "}",
    "loadFolders",
    "<v1.6>",
    "IfModActive",
    "tag:",
    "is:",
    "author:",
    "patches:",
    "subscribe:",
    "_steam",
    "packageId",
    "<ModMetaData>",
    "<name>",
    "modDependencies",
    "\0",
];

fn text(random: &mut Random) -> String {
    let pieces = random.below(40);
    (0..pieces)
        .map(|_| PIECES[random.below(PIECES.len())])
        .collect()
}

fn panics_on(rounds: usize, seed: u64, probe: impl Fn(&str)) -> Vec<String> {
    let mut random = Random(seed);
    let quiet = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let failed: Vec<String> = (0..rounds)
        .map(|_| text(&mut random))
        .filter(|made| catch_unwind(AssertUnwindSafe(|| probe(made))).is_err())
        .collect();
    std::panic::set_hook(quiet);
    failed
}

fn a_mod(name: &str, category: Category) -> Mod {
    Mod {
        id: name.to_lowercase(),
        name: name.to_string(),
        author: "someone".to_string(),
        version: "1.0".to_string(),
        category,
        installed: None,
        tags: Vec::new(),
        local: false,
        workshop: false,
        git: false,
        code: false,
        patched: std::sync::Arc::from(Vec::<String>::new()),
        sub: String::new(),
        load_micros: None,
        filed: category,
        chosen: false,
        held_by: String::new(),
    }
}

#[test]
fn no_text_parser_panics_on_thirty_thousand_made_up_texts() {
    let failed = panics_on(30_000, 0x5eed, |made| {
        let _ = modsconfig::parse(made);
        let _ = modsconfig::with_active(made, &["a.b".to_string()]);
        let _ = modsconfig::short_version(made);
        let _ = markup::rich(made);
        let _ = vdf::libraries(made);
        let _ = about::parse(made.as_bytes());
        let _ = logs::errors(made);
        let _ = logs::game_version(made);
        let _ = logs::mentioned(
            made,
            &[("İstanbul mod".to_string(), 1), ("日本 mod".into(), 2)],
        );
        let _ = changes::steam_notes(made);
        let _ = workshop::ids_in(made);
        let _ = workshop::parse_details(made);
        let _ = workshop::parse_collections(made);
        let _ = workshop::installed_times(made);
        let _ = workshop::installed_sizes(made);
        let _ = share::ids_in(made);
        let _ = loadfolders::declared(made, "1.6");
        let _ = rimsaves::parse_settings(made);
        let _ = rimsaves::parse_meta(made);
        let _ = rimsaves::split(made);
        let _ = community::parse(made);
        let _ = community::works_on(made);
        let _ = notes::tags_in(made);
        let _ = steamworks::parse(made);
        let _ = owntheme::contrast(made, "#000000");
        let _ = owntheme::contrast(&format!("#{made}"), "#000000");
        let _ = ddsaudit::problem(made.as_bytes(), made.len() as u64);
    });
    assert!(failed.is_empty(), "panicked on {:?}", failed.first());
}

#[test]
fn the_filter_box_takes_any_text() {
    let mods: Vec<Mod> = ["Harmony", "İstanbul", "日本 mod", "Core"]
        .iter()
        .map(|name| a_mod(name, Category::Content))
        .collect();
    let failed = panics_on(20_000, 0xf117e4, |made| {
        let mut list = GroupedList::new(mods.clone());
        list.set_filter(made);
        let _ = list.matching();
        let _ = list.rows().len();
    });
    assert!(failed.is_empty(), "panicked on {:?}", failed.first());
}

#[test]
#[ignore = "needs RIMSTACK_DLL: a mod's assembly"]
fn a_damaged_real_assembly_is_refused_or_read_and_never_panics() {
    let path = std::env::var_os("RIMSTACK_DLL").expect("RIMSTACK_DLL names a mod's assembly");
    let sound = std::fs::read(path).unwrap();
    assert!(dotnet::Assembly::read(&sound).is_some());
    let mut random = Random(0xd11);
    let quiet = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let (mut panicked, mut read, mut slowest) = (0, 0, Duration::ZERO);
    for round in 0..4_000 {
        let mut dll = sound.clone();
        match round % 4 {
            0 => {
                let cut = random.below(dll.len());
                dll.truncate(cut);
            }
            1 => {
                // Counts and offsets are four bytes.
                for _ in 0..1 + random.below(4) {
                    let at = random.below(dll.len() - 4);
                    dll[at..at + 4].copy_from_slice(&[0xff, 0xff, 0xff, 0x7f]);
                }
            }
            _ => {
                for _ in 0..1 + random.below(24) {
                    let span = if random.below(3) == 0 {
                        dll.len()
                    } else {
                        dll.len() / 16
                    };
                    let at = random.below(span.max(1));
                    dll[at] = random.next() as u8;
                }
            }
        }
        let started = Instant::now();
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            let Some(assembly) = dotnet::Assembly::read(&dll) else {
                return false;
            };
            let _ = assembly.references();
            let _ = assembly.types();
            let _ = assembly.patches();
            let _ = assembly.hand_applied();
            let _ = assembly.looked_up();
            true
        }));
        slowest = slowest.max(started.elapsed());
        match outcome {
            Ok(true) => read += 1,
            Ok(false) => {}
            Err(_) => panicked += 1,
        }
    }
    std::panic::set_hook(quiet);
    println!(
        "assembly fuzz: 4000 damaged files, {read} still read, {panicked} panics, slowest {slowest:?}"
    );
    assert_eq!(panicked, 0);
    assert!(
        slowest < Duration::from_secs(2),
        "one reading took {slowest:?}"
    );
}

#[test]
#[ignore = "needs RIMSTACK_DLLS: a file listing installed assemblies"]
fn installed_assemblies_are_read() {
    let listing =
        std::env::var_os("RIMSTACK_DLLS").expect("RIMSTACK_DLLS names a list of assemblies");
    let listing = std::fs::read_to_string(listing).unwrap();
    let (mut read, mut refused, mut panicked) = (0, Vec::new(), Vec::new());
    let started = Instant::now();
    for path in listing.lines().filter(|line| !line.is_empty()) {
        let Ok(bytes) = std::fs::read(path) else {
            continue;
        };
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            let assembly = dotnet::Assembly::read(&bytes)?;
            Some((assembly.types().len(), assembly.patches().len()))
        }));
        match outcome {
            Ok(Some(_)) => read += 1,
            // A native library has no .NET header.
            Ok(None) => refused.push(path),
            Err(_) => panicked.push(path),
        }
    }
    println!(
        "assemblies: {read} read, {} refused, {} panicked, in {:?}",
        refused.len(),
        panicked.len(),
        started.elapsed()
    );
    for path in refused.iter().take(8) {
        println!("  refused: {path}");
    }
    assert!(panicked.is_empty(), "panicked on {:?}", panicked.first());
}

#[test]
fn a_contradiction_among_many_rules_still_sorts_in_reasonable_time() {
    let count = 2_300;
    let mut rules: Vec<Rule> = (0..count - 1)
        .map(|m| Rule {
            first: m,
            then: m + 1,
            kind: Kind::LoadAfter,
        })
        .collect();
    for bottom in 0..5 {
        rules.extend((5..count).map(|above| Rule {
            first: above,
            then: bottom,
            kind: Kind::CommunityBottom,
        }));
    }
    let started = Instant::now();
    let sorted = sort::sort(count, &rules);
    let took = started.elapsed();
    println!(
        "sort with a loop: {count} mods, {} rules, {} dropped, {took:?}",
        rules.len(),
        sorted.dropped.len()
    );
    assert_eq!(sorted.order.len(), count);
    assert!(took < Duration::from_secs(5), "took {took:?}");
}

use crate::base::sentence::Sentence;
use crate::base::translate::named;
use crate::mods::checks::{self, Problem};
use crate::mods::details;
use crate::mods::library::Library;
use crate::mods::list::Mod;
use crate::mods::rules::{Kind, Rule};
use crate::rimworld::code::{Aim, Facts};
use crate::rimworld::game::When;
use crate::rimworld::workshopdb::Names;
use crate::say;
use crate::say_n;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trouble {
    pub error: bool,
    pub words: Sentence,
    pub short: Sentence,
    pub fix: Fix,
    /// What `Note::ignored` holds to ignore it; empty for a game version, which has `Fix::Trust`.
    pub key: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Fix {
    None,
    Activate(String),
    Subscribe(String),
    Search(String),
    Deactivate(usize),
    Trust,
    Sort,
}

pub fn warned_for_version(own: &[Trouble]) -> bool {
    own.iter().any(|t| t.fix == Fix::Trust)
}

impl Fix {
    pub fn brings_a_mod(&self) -> bool {
        matches!(self, Fix::Activate(_) | Fix::Subscribe(_) | Fix::Search(_))
    }

    pub fn action(&self) -> &'static str {
        match self {
            Fix::None => "",
            Fix::Activate(_) => "activate",
            Fix::Subscribe(_) => "subscribe",
            Fix::Search(_) => "search",
            Fix::Deactivate(_) => "deactivate",
            Fix::Trust => "trust",
            Fix::Sort => "sort",
        }
    }

    pub fn target(&self) -> String {
        match self {
            Fix::None | Fix::Trust | Fix::Sort => String::new(),
            Fix::Activate(id) | Fix::Subscribe(id) | Fix::Search(id) => id.clone(),
            Fix::Deactivate(at) => at.to_string(),
        }
    }

    pub fn page(&self) -> Option<String> {
        let Fix::Search(id) = self else {
            return None;
        };
        let text: String = id
            .bytes()
            .map(|b| match b {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                    char::from(b).to_string()
                }
                _ => format!("%{b:02X}"),
            })
            .collect();
        Some(format!(
            "https://steamcommunity.com/workshop/browse/?appid={}&searchtext={text}",
            crate::rimworld::paths::APP_ID
        ))
    }

    pub fn label(&self) -> &'static str {
        match self {
            Fix::None => "",
            Fix::Activate(_) => named("Activate it"),
            Fix::Subscribe(_) => named("Workshop page"),
            Fix::Search(_) => named("Search the Workshop"),
            Fix::Deactivate(_) => named("Deactivate the other"),
            Fix::Trust => named("Ignore"),
            Fix::Sort => named("Sort"),
        }
    }

    pub fn group(&self) -> &'static str {
        match self {
            Fix::None => named("Other"),
            Fix::Activate(_) => named("Needed and switched off"),
            Fix::Subscribe(_) | Fix::Search(_) => named("Needed and not installed"),
            Fix::Deactivate(_) => named("Cannot be used together"),
            Fix::Trust => named("Made for another game version"),
            Fix::Sort => named("In the wrong order"),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Tally {
    pub needed: Vec<String>,
    pub version_only: usize,
    pub other_warnings: usize,
}

pub fn tally(troubles: &[Vec<Trouble>]) -> Tally {
    let mut tally = Tally::default();
    for own in troubles {
        for trouble in own {
            match &trouble.fix {
                Fix::Activate(id) if !tally.needed.contains(id) => tally.needed.push(id.clone()),
                Fix::Trust => tally.version_only += 1,
                _ => {}
            }
        }
        let other = |t: &Trouble| !t.error && t.fix != Fix::Trust;
        tally.other_warnings += usize::from(own.iter().any(other));
    }
    tally
}

#[derive(Debug, Clone, Copy)]
pub enum Versions<'a> {
    CheckedBut(&'a HashSet<String>),
    Ignored,
}

impl Versions<'_> {
    fn trusts(self, package_id: &str) -> bool {
        match self {
            Versions::CheckedBut(trusted) => {
                trusted.contains(&crate::rimworld::id::fold(package_id))
            }
            Versions::Ignored => true,
        }
    }
}

// The game's own parts are of the game's version whatever they list: a test build
// ships expansions that still name the last one.
fn official(library: &Library, m: &Mod) -> bool {
    let installed = m.installed.and_then(|i| library.get(i));
    installed.is_some_and(|i| i.source == crate::rimworld::paths::Source::Official)
}

/// The newest version listed by each mod `troubles` warns of as made for another game version.
pub fn outdated(library: &Library, mods: &[Mod], versions: Versions) -> Vec<Option<String>> {
    let abouts = library.abouts(mods);
    let found = checks::check(&abouts, &library.game_version, |key| library.name_of(key));
    let each = found.into_iter().zip(&abouts).zip(mods);
    each.map(|((problems, about), m)| {
        let supports = problems.into_iter().find_map(|p| match p {
            Problem::GameVersion { supports } => Some(supports),
            _ => None,
        })?;
        let excused = official(library, m) || about.is_some_and(|a| versions.trusts(&a.package_id));
        let newest = details::newest(&supports).map(String::from);
        newest.filter(|_| !excused)
    })
    .collect()
}

pub fn version_ignored(library: &Library, mods: &[Mod], versions: Versions) -> Vec<bool> {
    let abouts = library.abouts(mods);
    let found = checks::check(&abouts, &library.game_version, |key| library.name_of(key));
    let trusted = |package_id: &str| versions.trusts(package_id);
    let mismatch = |problems: &Vec<Problem>| {
        let version = |p: &Problem| matches!(p, Problem::GameVersion { .. });
        problems.iter().any(version)
    };
    let each = found.iter().zip(&abouts);
    each.map(|(problems, about)| {
        mismatch(problems) && about.is_some_and(|a| trusted(&a.package_id))
    })
    .collect()
}

pub fn missing(workshop: &Names, id: &str) -> (String, Fix) {
    match workshop.get(&crate::rimworld::id::fold(id)) {
        Some(item) => (item.name.clone(), Fix::Subscribe(item.workshop_id.clone())),
        None => (id.to_string(), Fix::Search(id.to_string())),
    }
}

pub fn troubles(
    library: &Library,
    mods: &[Mod],
    versions: Versions,
    workshop: &Names,
) -> Vec<Vec<Trouble>> {
    let abouts = library.abouts(mods);
    let mut found = checks::check(&abouts, &library.game_version, |key| library.name_of(key));
    for ((problems, about), m) in found.iter_mut().zip(&abouts).zip(mods) {
        if official(library, m) || about.is_some_and(|a| versions.trusts(&a.package_id)) {
            problems.retain(|p| !matches!(p, Problem::GameVersion { .. }));
        }
    }
    let missing = |id: &str| missing(workshop, id);
    let in_words = |problem: Problem| -> Trouble {
        let key = match &problem {
            Problem::Requires { id, .. } => {
                format!(
                    "needs:{}",
                    crate::rimworld::id::fold(&crate::rimworld::id::split_steam(id).0)
                )
            }
            Problem::Incompatible { with } => {
                format!("not-with:{}", crate::rimworld::id::fold(&mods[*with].id))
            }
            Problem::GameVersion { .. } => String::new(),
        };
        let (words, short, fix) = match &problem {
            Problem::Requires {
                id,
                name: Some(name),
            } => (
                say!("Needs {}, which is installed but switched off.", name),
                say!("Needs {}", name),
                Fix::Activate(crate::rimworld::id::fold(
                    &crate::rimworld::id::split_steam(id).0,
                )),
            ),
            Problem::Requires { id, name: None } => {
                let (name, fix) = missing(id);
                (
                    say!("Needs {}, which is not installed.", name),
                    say!("Missing {}", name),
                    fix,
                )
            }
            Problem::Incompatible { with } => (
                say!("Cannot be used together with {}.", mods[*with].name),
                say!("Not with {}", mods[*with].name),
                Fix::Deactivate(*with),
            ),
            Problem::GameVersion { supports } => (
                say!(
                    "Made for RimWorld {}, not {}. Many such mods still work.",
                    supports.join(", "),
                    library.game_version
                ),
                say!("Made for {}", details::newest(supports).unwrap_or_default()),
                Fix::Trust,
            ),
        };
        Trouble {
            error: problem.is_error(),
            words,
            short,
            fix,
            key,
        }
    };
    let mut all: Vec<Vec<Trouble>> = found
        .into_iter()
        .map(|problems| problems.into_iter().map(in_words).collect())
        .collect();
    for broken in crate::mods::rules::among(&abouts, &library.game_version)
        .into_iter()
        .filter(Rule::broken)
    {
        let Some(by) = broken.declared_by() else {
            continue;
        };
        let after = by == broken.then;
        let other = if after { broken.first } else { broken.then };
        let key = format!("order:{}", crate::rimworld::id::fold(&mods[other].id));
        if all[by].iter().any(|t| t.key == key) {
            continue;
        }
        let name = &mods[other].name;
        let (words, short) = match after {
            true => (
                say!("Has to load after {}, which is below it.", name),
                say!("Load after {}", name),
            ),
            false => (
                say!("Has to load before {}, which is above it.", name),
                say!("Load before {}", name),
            ),
        };
        all[by].push(Trouble {
            error: false,
            words,
            short,
            fix: Fix::Sort,
            key,
        });
    }
    for (m, own) in mods.iter().zip(&mut all) {
        if m.installed.is_none() {
            own.push(Trouble {
                error: true,
                words: say!("On the list, but not installed."),
                short: say!("Not installed"),
                fix: missing(&m.id).1,
                key: "missing".to_string(),
            });
        }
    }
    all
}

/// Takes out the problems ignored, by folded package id; an incompatibility ignored on
/// either side is gone from both.
pub fn without_ignored(
    all: &mut [Vec<Trouble>],
    mods: &[Mod],
    ignored: &HashMap<String, HashSet<String>>,
) {
    let folded: Vec<String> = mods
        .iter()
        .map(|m| crate::rimworld::id::fold(&m.id))
        .collect();
    let ignores = |at: usize, key: &str| {
        ignored
            .get(&folded[at])
            .is_some_and(|keys| keys.contains(key))
    };
    for (me, own) in all.iter_mut().enumerate() {
        own.retain(|t| {
            let other_side = match t.fix {
                Fix::Deactivate(with) => ignores(with, &format!("not-with:{}", folded[me])),
                _ => false,
            };
            t.key.is_empty() || !(ignores(me, &t.key) || other_side)
        });
    }
}

pub fn code(facts: &Facts) -> Vec<Sentence> {
    if facts.types == 0 {
        return Vec::new();
    }
    let some = |count: usize, said: Sentence| (count > 0).then_some(said);
    let with = |counted: Sentence, each: [Option<Sentence>; 3]| {
        let each: Vec<Sentence> = each.into_iter().flatten().collect();
        match each.is_empty() {
            true => counted,
            false => say!("{}: {}", counted.inside(), Sentence::listed(each).inside()),
        }
    };
    let types = [
        some(facts.drawing, say!("{} drawing", facts.drawing)),
        some(facts.adding, say!("{} adding to the game", facts.adding)),
        None,
    ];
    let mut parts = vec![with(say_n!(facts.types, "{} type", "{} types"), types)];
    if facts.patches > 0 {
        let aimed = [
            some(facts.on_playing, say!("{} while playing", facts.on_playing)),
            some(
                facts.on_interface,
                say!("{} on the interface", facts.on_interface),
            ),
            some(facts.on_loading, say!("{} on loading", facts.on_loading)),
        ];
        let patched = say_n!(facts.patches, "{} patched method", "{} patched methods");
        parts.push(with(patched, aimed));
    }
    parts
}

pub fn also_patching(mine: &[Aim], others: &[(&str, &[Aim])]) -> Vec<Vec<String>> {
    let same =
        |a: &Aim, b: &Aim| !a.target.is_empty() && a.target == b.target && a.method == b.method;
    let patching = |aim: &Aim| {
        let has_it = |(_, theirs): &&(&str, &[Aim])| theirs.iter().any(|t| same(t, aim));
        others
            .iter()
            .filter(has_it)
            .map(|(name, _)| name.to_string())
            .collect()
    };
    mine.iter().map(patching).collect()
}

pub fn aims(aims: &[Aim], also: &[Vec<String>], most: usize) -> Vec<(Sentence, Sentence)> {
    let note = |aim: &Aim, others: &[String]| {
        let when = match aim.when {
            Some(When::Playing) => Some(say!("while playing")),
            Some(When::Interface) => Some(say!("interface")),
            Some(When::Loading) => Some(say!("loading")),
            None => None,
        };
        // Harmony's own names for what a patch does: not put in words.
        let does = [
            (aim.does.prefix, "prefix"),
            (aim.does.postfix, "postfix"),
            (aim.does.transpiler, "transpiler"),
            (aim.does.finalizer, "finalizer"),
        ];
        let does = does.iter().filter(|d| d.0).map(|d| Sentence::from(d.1));
        let said = Sentence::listed(when.into_iter().chain(does).collect());
        match others.is_empty() {
            true => said,
            false => {
                let others = Sentence::listed(others.iter().map(|o| o.as_str().into()).collect());
                say!("{}; also {}", said.inside(), others.inside())
            }
        }
    };
    let none = Vec::new();
    let others = |at: usize| also.get(at).unwrap_or(&none);
    let mut order: Vec<usize> = (0..aims.len()).collect();
    order.sort_by_key(|&at| others(at).is_empty());
    let mut lines: Vec<(Sentence, Sentence)> = order
        .iter()
        .take(most)
        .map(|&at| {
            let aim = &aims[at];
            let name = match aim.target.is_empty() {
                true => say!("Target named in code"),
                false => Sentence::Plain(format!("{}.{}", aim.target, aim.method)),
            };
            (name, note(aim, others(at)))
        })
        .collect();
    if aims.len() > most {
        lines.push((
            say!("{} more", aims.len() - most),
            Sentence::listed(Vec::new()),
        ));
    }
    lines
}

pub fn rule(rule: &Rule, mods: &[Mod]) -> Sentence {
    let Some(by) = rule.declared_by() else {
        let (first, then) = (&mods[rule.first].name, &mods[rule.then].name);
        return match rule.kind {
            Kind::Yours => say!("your rule puts {} before {}", first, then),
            _ => say!("the community rules put {} before {}", first, then),
        };
    };
    let other = if by == rule.first {
        rule.then
    } else {
        rule.first
    };
    let (by, other) = (&mods[by].name, &mods[other].name);
    match rule.kind {
        Kind::Requires => say!("{} requires {}", by, other),
        Kind::LoadAfter | Kind::ForceLoadAfter => say!("{} asks to load after {}", by, other),
        _ => say!("{} asks to load before {}", by, other),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_search_fix_has_its_workshop_page_and_its_text_is_encoded() {
        let page = Fix::Search("Ann.Big Roads&co".into()).page().unwrap();
        assert!(page.ends_with("searchtext=Ann.Big%20Roads%26co"));
        assert!(page.contains("appid=294100"));
        assert_eq!(Fix::Trust.page(), None);
        assert_eq!(Fix::Subscribe("1".into()).page(), None);
    }

    #[test]
    fn a_mod_is_warned_for_its_version_when_one_of_its_problems_is_trusted_away() {
        let trouble = |fix| Trouble {
            error: false,
            words: Sentence::default(),
            short: Sentence::default(),
            fix,
            key: String::new(),
        };
        assert!(warned_for_version(&[
            trouble(Fix::None),
            trouble(Fix::Trust)
        ]));
        assert!(!warned_for_version(&[trouble(Fix::Search("a".into()))]));
        assert!(!warned_for_version(&[]));
    }
    use crate::mods::library::{build, scan};
    use crate::rimworld::paths::{ModRoot, Source};
    use std::fs;

    fn two_mods() -> (tempfile::TempDir, Library) {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("Mods");
        let abouts = [
            (
                "old",
                "<ModMetaData><packageId>a.old</packageId><name>Old</name><supportedVersions><li>1.5</li></supportedVersions><modDependencies><li><packageId>gone.lib</packageId></li></modDependencies></ModMetaData>",
            ),
            (
                "fine",
                "<ModMetaData><packageId>a.fine</packageId><name>Fine</name><supportedVersions><li>1.6</li></supportedVersions></ModMetaData>",
            ),
        ];
        for (folder, about) in abouts {
            fs::create_dir_all(root.join(folder).join("About")).unwrap();
            fs::write(root.join(folder).join("About/About.xml"), about).unwrap();
        }
        let roots = [ModRoot {
            path: root,
            source: Source::Local,
        }];
        let active = [
            "a.old".to_string(),
            "a.fine".to_string(),
            "not.here".to_string(),
        ];
        (tmp, build(scan(&roots), &active, "1.6"))
    }

    #[test]
    fn each_mod_s_problems_are_put_in_words_errors_marked() {
        let (_tmp, library) = two_mods();
        let found = troubles(
            &library,
            &library.active,
            Versions::CheckedBut(&HashSet::new()),
            &Names::new(),
        );
        let said: Vec<(String, String)> = found[0]
            .iter()
            .map(|t| (t.words.to_string(), t.short.to_string()))
            .collect();
        let old: Vec<(bool, &str, &str, &Fix)> = found[0]
            .iter()
            .zip(&said)
            .map(|(t, (words, short))| (t.error, words.as_str(), short.as_str(), &t.fix))
            .collect();
        assert_eq!(
            old,
            [
                (
                    true,
                    "Needs gone.lib, which is not installed.",
                    "Missing gone.lib",
                    &Fix::Search("gone.lib".to_string())
                ),
                (
                    false,
                    "Made for RimWorld 1.5, not 1.6. Many such mods still work.",
                    "Made for 1.5",
                    &Fix::Trust
                ),
            ]
        );
        assert!(found[1].is_empty());
        assert_eq!(found[2][0].short.to_string(), "Not installed");
        assert_eq!(found[2][0].fix, Fix::Search("not.here".to_string()));
    }

    #[test]
    fn a_mod_loaded_against_its_own_rules_is_warned_with_sort_as_the_fix() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("Mods");
        let abouts = [
            (
                "switch",
                "<ModMetaData><packageId>a.switch</packageId><name>Switch</name><modDependencies><li><packageId>a.frame</packageId></li></modDependencies><loadAfter><li>A.Frame</li></loadAfter></ModMetaData>",
            ),
            (
                "frame",
                "<ModMetaData><packageId>a.frame</packageId><name>Frame</name></ModMetaData>",
            ),
        ];
        for (folder, about) in abouts {
            fs::create_dir_all(root.join(folder).join("About")).unwrap();
            fs::write(root.join(folder).join("About/About.xml"), about).unwrap();
        }
        let roots = [ModRoot {
            path: root,
            source: Source::Local,
        }];
        let check = |order: [&str; 2]| {
            let active = order.map(String::from);
            let library = build(scan(&roots), &active, "1.6");
            troubles(&library, &library.active, Versions::Ignored, &Names::new())
        };
        let wrong = check(["a.switch", "a.frame"]);
        let said: Vec<(bool, String, String, &Fix, &str)> = wrong[0]
            .iter()
            .map(|t| {
                (
                    t.error,
                    t.words.to_string(),
                    t.short.to_string(),
                    &t.fix,
                    t.key.as_str(),
                )
            })
            .collect();
        assert_eq!(
            said,
            [(
                false,
                "Has to load after Frame, which is below it.".to_string(),
                "Load after Frame".to_string(),
                &Fix::Sort,
                "order:a.frame"
            )]
        );
        assert!(wrong[1].is_empty());
        assert!(check(["a.frame", "a.switch"]).iter().all(Vec::is_empty));
    }

    #[test]
    fn an_ignored_problem_is_taken_out_and_an_incompatibility_from_both_sides() {
        let (_tmp, library) = two_mods();
        let mut found = troubles(
            &library,
            &library.active,
            Versions::CheckedBut(&HashSet::new()),
            &Names::new(),
        );
        assert_eq!(found[0][0].key, "needs:gone.lib");
        assert_eq!(found[2][0].key, "missing");
        let not_with = |with: usize| Trouble {
            error: true,
            words: Sentence::default(),
            short: Sentence::default(),
            fix: Fix::Deactivate(with),
            key: format!(
                "not-with:{}",
                crate::rimworld::id::fold(&library.active[with].id)
            ),
        };
        found[0].push(not_with(1));
        found[1].push(not_with(0));
        let ignored = HashMap::from([
            (
                "a.fine".to_string(),
                HashSet::from(["not-with:a.old".to_string()]),
            ),
            (
                "not.here".to_string(),
                HashSet::from(["missing".to_string()]),
            ),
        ]);
        without_ignored(&mut found, &library.active, &ignored);
        assert_eq!(found[0].len(), 2, "its need and its version stay");
        assert!(found[1].is_empty() && found[2].is_empty());
    }

    #[test]
    fn the_game_s_own_parts_are_not_warned_about_for_the_version_they_list() {
        let tmp = tempfile::tempdir().unwrap();
        let about = "<ModMetaData><packageId>ludeon.rimworld.royalty</packageId><name>Royalty</name><supportedVersions><li>1.6</li></supportedVersions></ModMetaData>";
        let mut roots = Vec::new();
        for (folder, source) in [("Data", Source::Official), ("Mods", Source::Local)] {
            let root = tmp.path().join(folder);
            fs::create_dir_all(root.join("Royalty/About")).unwrap();
            let own = about.replace("ludeon.rimworld.royalty", &format!("a.{folder}"));
            fs::write(root.join("Royalty/About/About.xml"), own).unwrap();
            roots.push(ModRoot { path: root, source });
        }
        let active = ["a.Data".to_string(), "a.Mods".to_string()];
        let library = build(scan(&roots), &active, "1.7");
        let nobody = HashSet::new();
        let found = troubles(
            &library,
            &library.active,
            Versions::CheckedBut(&nobody),
            &Names::new(),
        );
        assert!(found[0].is_empty());
        assert_eq!(found[1].len(), 1);
        assert_eq!(found[1][0].fix, Fix::Trust);
    }

    #[test]
    fn a_fix_has_an_action_a_target_a_button_and_a_kind() {
        let fix = Fix::Activate("a.fine".to_string());
        assert_eq!(
            (fix.action(), fix.target().as_str()),
            ("activate", "a.fine")
        );
        assert_eq!(fix.group(), "Needed and switched off");
        let other = Fix::Deactivate(3);
        assert_eq!(
            (other.action(), other.target().as_str()),
            ("deactivate", "3")
        );
        assert_eq!(
            (Fix::Trust.action(), Fix::Trust.target().as_str()),
            ("trust", "")
        );
        assert_eq!(Fix::None.label(), "");
    }

    #[test]
    fn problems_are_counted_by_what_solves_them() {
        let trouble = |error: bool, fix: Fix| Trouble {
            error,
            words: Sentence::default(),
            short: Sentence::default(),
            fix,
            key: String::new(),
        };
        let needs = |id: &str| trouble(true, Fix::Activate(id.to_string()));
        let counted = tally(&[
            vec![needs("a.lib"), trouble(false, Fix::Trust)],
            vec![needs("a.lib"), needs("b.lib")],
            vec![trouble(false, Fix::None), trouble(false, Fix::None)],
            vec![trouble(false, Fix::Trust)],
            vec![],
        ]);
        assert_eq!(counted.needed, ["a.lib", "b.lib"]);
        assert_eq!(counted.version_only, 2);
        assert_eq!(counted.other_warnings, 1);
    }

    #[test]
    fn a_trusted_mod_gets_no_version_warning_and_keeps_its_other_problems() {
        let (_tmp, library) = two_mods();
        let trusted: HashSet<String> = ["a.old".to_string()].into();
        let none = Names::new();
        let found = troubles(
            &library,
            &library.active,
            Versions::CheckedBut(&trusted),
            &none,
        );
        assert_eq!(found[0].len(), 1);
        assert!(found[0][0].error && found[0][0].short.to_string() == "Missing gone.lib");
        let found = troubles(&library, &library.active, Versions::Ignored, &none);
        assert_eq!(found[0].len(), 1);
        let ignored = version_ignored(&library, &library.active, Versions::Ignored);
        assert_eq!(ignored[..2], [true, false]);
        let nobody = HashSet::new();
        let checked = version_ignored(&library, &library.active, Versions::CheckedBut(&nobody));
        assert!(checked.iter().all(|ignored| !ignored));
        let item = crate::rimworld::workshopdb::Item {
            workshop_id: "42".into(),
            name: "Gone Library".into(),
        };
        let known: Names = [("gone.lib".to_string(), item)].into();
        let found = troubles(&library, &library.active, Versions::Ignored, &known);
        assert_eq!(
            found[0][0].words.to_string(),
            "Needs Gone Library, which is not installed."
        );
        assert_eq!(found[0][0].fix, Fix::Subscribe("42".to_string()));
    }

    #[test]
    fn a_mod_is_outdated_unless_trusted_or_version_warnings_are_off() {
        let (_tmp, library) = two_mods();
        let nobody = HashSet::new();
        let found = outdated(&library, &library.active, Versions::CheckedBut(&nobody));
        assert_eq!(found, [Some("1.5".to_string()), None, None]);
        let trusted: HashSet<String> = ["a.old".to_string()].into();
        let found = outdated(&library, &library.active, Versions::CheckedBut(&trusted));
        assert_eq!(found, [None, None, None]);
        let found = outdated(&library, &library.active, Versions::Ignored);
        assert_eq!(found, [None, None, None]);
    }

    #[test]
    fn code_and_rules_read_as_plain_lines() {
        let code = |facts: &Facts| -> Vec<String> {
            code(facts).iter().map(Sentence::to_string).collect()
        };
        let aims = |aims_of: &[Aim], also: &[Vec<String>], most: usize| -> Vec<(String, String)> {
            aims(aims_of, also, most)
                .iter()
                .map(|(name, note)| (name.to_string(), note.to_string()))
                .collect()
        };
        assert!(code(&Facts::default()).is_empty());
        let facts = Facts {
            types: 1,
            drawing: 1,
            patches: 2,
            on_playing: 1,
            ..Facts::default()
        };
        assert_eq!(
            code(&facts),
            ["1 type: 1 drawing", "2 patched methods: 1 while playing"]
        );
        let aim = |method: &str, when, prefix| Aim {
            target: "Verse.Pawn".into(),
            method: method.into(),
            does: crate::rimworld::dotnet::Does {
                prefix,
                postfix: true,
                ..Default::default()
            },
            when,
        };
        let patched = [
            aim("Tick", Some(When::Playing), true),
            aim("Draw", None, false),
            aim("Kill", Some(When::Playing), false),
        ];
        assert_eq!(
            aims(&patched, &[], 2),
            [
                (
                    "Verse.Pawn.Tick".to_string(),
                    "while playing, prefix, postfix".to_string()
                ),
                ("Verse.Pawn.Draw".to_string(), "postfix".to_string()),
                ("1 more".to_string(), "".to_string())
            ]
        );
        let unknown = Aim {
            target: String::new(),
            ..aim("", None, true)
        };
        let theirs = [
            aim("Kill", None, true),
            aim("Other", None, true),
            unknown.clone(),
        ];
        assert_eq!(
            also_patching(std::slice::from_ref(&unknown), &[("Second", &theirs)]),
            [Vec::<String>::new()]
        );
        assert_eq!(
            aims(&[unknown], &[], 9),
            [(
                "Target named in code".to_string(),
                "prefix, postfix".to_string()
            )]
        );
        let others: [(&str, &[Aim]); 2] = [("Second", &theirs), ("Third", &patched[1..2])];
        let also = also_patching(&patched, &others);
        assert_eq!(
            also,
            [
                vec![],
                vec!["Third".to_string()],
                vec!["Second".to_string()]
            ]
        );
        assert_eq!(
            aims(&patched, &also, 2),
            [
                (
                    "Verse.Pawn.Draw".to_string(),
                    "postfix; also Third".to_string()
                ),
                (
                    "Verse.Pawn.Kill".to_string(),
                    "while playing, postfix; also Second".to_string()
                ),
                ("1 more".to_string(), "".to_string())
            ]
        );
        let (_tmp, library) = two_mods();
        let mods = &library.active;
        let stated = Rule {
            first: 1,
            then: 0,
            kind: Kind::LoadAfter,
        };
        assert_eq!(
            rule(&stated, mods).to_string(),
            "Old asks to load after Fine"
        );
        let yours = Rule {
            first: 1,
            then: 0,
            kind: Kind::Yours,
        };
        assert_eq!(
            rule(&yours, mods).to_string(),
            "your rule puts Fine before Old"
        );
    }
}

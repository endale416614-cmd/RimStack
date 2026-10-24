use super::{LoadOrder, Side, rules};
use crate::App;
use rimstack_core::base::locks::Shared;
use rimstack_core::base::sentence::Sentence;
use rimstack_core::mods::describe::{self, Trouble};
use rimstack_core::mods::details::{self, Clash, Named, OwnRule, Slow};
use rimstack_core::mods::library::ModId;
use rimstack_core::mods::list::{Category, Mod};
use rimstack_core::mods::tree::folder_tree;
use rimstack_core::rimworld::loadfolders;
use rimstack_core::rimworld::paths::Source;
use rimstack_core::say;
use serde::Serialize;

const MOST_PATCHES: usize = 200;

#[derive(Serialize)]
pub struct FixSaid {
    action: &'static str,
    target: String,
    label: &'static str,
    page: Option<String>,
}

impl FixSaid {
    fn of(fix: &rimstack_core::mods::describe::Fix) -> Self {
        Self {
            action: fix.action(),
            target: fix.target(),
            label: fix.label(),
            page: fix.page(),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProblemSaid {
    /// -1 for a mod that is not installed.
    mod_id: i64,
    #[serde(skip)]
    trust: bool,
    name: String,
    error: bool,
    group: &'static str,
    words: Sentence,
    short: Sentence,
    fix: FixSaid,
    /// With `ignore`, what `App::ignore_problem` is given; empty when it cannot be ignored.
    package_id: String,
    ignore: String,
}

impl ProblemSaid {
    fn of(m: &Mod, trouble: &Trouble) -> Self {
        Self {
            mod_id: m.installed.map_or(-1, |i| i64::from(i.to_qml())),
            trust: trouble.fix == rimstack_core::mods::describe::Fix::Trust,
            name: m.name.clone(),
            error: trouble.error,
            group: trouble.fix.group(),
            words: trouble.words.clone(),
            short: trouble.short.clone(),
            fix: FixSaid::of(&trouble.fix),
            package_id: m.id.clone(),
            ignore: trouble.key.clone(),
        }
    }
}

#[derive(Serialize)]
pub struct Detected {
    name: String,
    why: Sentence,
    code: bool,
}

#[derive(Serialize)]
pub struct Branch {
    folder: String,
    files: usize,
}

#[derive(Serialize, Default)]
pub struct Files {
    tree: Vec<Branch>,
    assemblies: String,
}

#[derive(Serialize)]
pub struct Patched {
    method: Sentence,
    note: Sentence,
}

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Details {
    name: String,
    source: &'static str,
    workshop: bool,
    official: bool,
    contains: Sentence,
    path: String,
    repository: String,
    folder: String,
    preview: Option<String>,
    author: String,
    package_id: String,
    version: String,
    game_versions: String,
    description: String,
    description_rich: String,
    url: String,
    load_time: String,
    unreadable: Option<Sentence>,
    tags: String,
    note: String,
    category_chosen: String,
    version_ok: bool,
    problems_ignored: usize,
    made_for: Option<String>,
    replaced_by: Option<String>,
    replaced_by_id: Option<String>,
    replaced_by_versions: Option<String>,
    copies: Option<Sentence>,
    bin_local_copy: Option<LocalCopy>,
    updated: Option<u64>,
    megabytes: Option<f64>,
    requires: Vec<Named>,
    load_folders: Vec<Sentence>,
    code: Sentence,
    sub: String,
    patches: Vec<Patched>,
    category: Option<&'static str>,
    problems: Vec<ProblemSaid>,
    your_rules: Vec<OwnRule>,
    detected: Vec<Detected>,
    loses_to: Vec<Clash>,
    wins_over: Vec<Clash>,
    conflict_marks: std::collections::BTreeMap<ModId, rimstack_core::mods::details::Mark>,
    loads_after: Vec<Named>,
    loads_before: Vec<Named>,
    required_above: usize,
    placement: Option<Sentence>,
    offer_trust: bool,
    can_bin: bool,
    group_choices: Vec<String>,
    workshop_page: Option<String>,
    steam_page: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalCopy {
    mod_id: ModId,
    path: String,
}

/// An active row whose mod is not installed, in place of its details for its menu.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NotInstalled {
    name: String,
    take_off: Sentence,
    package_id: String,
    fixes: Vec<FixSaid>,
    resubscribe: Vec<String>,
}

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Textures {
    count: usize,
    dds_made: usize,
}

#[derive(Serialize)]
pub struct ProblemGroup {
    name: &'static str,
    error: bool,
    trusted: usize,
    shared: bool,
    lines: Vec<ProblemSaid>,
}

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Overview {
    game_version: String,
    categories: Vec<&'static str>,
    troubles: Vec<ProblemSaid>,
    to_get: Vec<ProblemSaid>,
    trouble_groups: Vec<ProblemGroup>,
    needed: Vec<String>,
    version_only: usize,
    other_warnings: usize,
    slowest: Vec<Slow>,
    load_times: bool,
    installed: usize,
    missing: usize,
    unreadable: usize,
    read_ms: u128,
    works_on: usize,
    trusted: Vec<Named>,
    community_mods: Option<usize>,
    community_time: Option<i64>,
}

/// Of several mods, those Steam can unsubscribe from and those the bin may take, each with
/// their names for the question asked first.
#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Upkeep {
    unsubscribe: Vec<i32>,
    unsubscribe_names: String,
    bin: Vec<i32>,
    bin_names: String,
}

impl LoadOrder {
    pub fn upkeep(&self, app: &App, ids: &[ModId]) -> Upkeep {
        let loaded = app.library();
        let Ok((library, _)) = &*loaded else {
            return Upkeep::default();
        };
        let listed = |ids: &[ModId]| {
            let names: Vec<&str> = (ids.iter().take(12))
                .filter_map(|&id| Some(library.get(id)?.display_name()))
                .collect();
            let more = if ids.len() > names.len() { ", …" } else { "" };
            names.join(", ") + more
        };
        let qml = |ids: Vec<ModId>| ids.into_iter().map(ModId::to_qml).collect();
        let unsubscribe: Vec<ModId> = (ids.iter().copied())
            .filter(|&id| app.workshop_id(id).is_some())
            .collect();
        let bin: Vec<ModId> = (ids.iter().copied())
            .filter(|&id| {
                library
                    .get(id)
                    .is_some_and(|m| m.source != Source::Official)
                    && self.active.position_of(id).is_none()
            })
            .collect();
        Upkeep {
            unsubscribe_names: listed(&unsubscribe),
            bin_names: listed(&bin),
            unsubscribe: qml(unsubscribe),
            bin: qml(bin),
        }
    }

    pub fn not_installed(&self, app: &App, side: Side, index: usize) -> Option<NotInstalled> {
        let m = self.list(side).mods().get(index)?;
        if m.installed.is_some() {
            return None;
        }
        let (name, fix) = describe::missing(&app.workshop_names(), &m.id);
        let resubscribe = match &fix {
            describe::Fix::Subscribe(workshop_id) => vec![workshop_id.clone()],
            _ => Vec::new(),
        };
        Some(NotInstalled {
            name,
            take_off: say!("Take off the list"),
            package_id: m.id.clone(),
            fixes: vec![FixSaid::of(&fix)],
            resubscribe,
        })
    }

    pub fn details(&mut self, app: &App, id: ModId) -> Details {
        let loaded = app.library();
        let Ok((library, _)) = &*loaded else {
            return Details::default();
        };
        let Some(m) = library.get(id) else {
            return Details::default();
        };
        let workshop = m.source == Source::Workshop;
        let note = m.key().and_then(|k| app.notes().reading().get(&k).cloned());
        let note = note.unwrap_or_default();
        let replacements = app.replacements();
        let replaced = replacements.get(&m.folder).filter(|_| workshop);
        let steam_record = app.steam_record();
        let (times, sizes) = &*steam_record;
        let mut said = Details {
            workshop,
            official: m.source == Source::Official,
            name: m.display_name().to_string(),
            source: m.source_label(),
            contains: m.contains(),
            path: m.path.to_string_lossy().into_owned(),
            repository: rimstack_core::sources::git::page(&m.path).unwrap_or_default(),
            folder: m.folder.clone(),
            preview: m.preview.as_ref().map(|p| p.to_string_lossy().into_owned()),
            tags: note.tags.join(", "),
            version_ok: note.version_ok,
            problems_ignored: note.ignored.len(),
            replaced_by: replaced.map(|new| new.name.clone()),
            replaced_by_id: replaced.map(|new| new.workshop_id.clone()),
            replaced_by_versions: replaced.map(|new| new.versions.join(", ")),
            updated: times.get(&m.folder).copied().filter(|_| workshop),
            megabytes: sizes
                .get(&m.folder)
                .filter(|_| workshop)
                .map(|bytes| *bytes as f64 / 1_048_576.0),
            requires: details::required(library.requirements(m)),
            load_folders: self.load_folders(library, m),
            code: Sentence::joined("\n", describe::code(&m.code.facts)),
            ..Details::default()
        };
        match &m.about {
            Ok(about) => {
                said.author = about.authors.join(", ");
                said.package_id = about.package_id.clone();
                said.version = about.mod_version.clone().unwrap_or_default();
                said.game_versions = about.supported_versions.join(", ");
                said.description = about.description.clone().unwrap_or_default();
                said.description_rich = rimstack_core::base::markup::rich(&said.description);
                said.url = about.url.clone().unwrap_or_default();
                said.load_time =
                    rimstack_core::rimworld::startup::in_words(&app.startup(), &about.package_id);
                said.made_for = details::made_for(&about.supported_versions, &library.game_version);
            }
            Err(reason) => said.unreadable = Some(reason.sentence().clone()),
        }
        if let Some(copies) = library.copies(id) {
            let local = m.source == Source::Local;
            said.copies = Some(match (local, copies.workshop_newer) {
                (true, true) => say!(
                    "You are also subscribed to this mod in the Workshop, and that copy is newer, but the game uses this one from its own Mods folder."
                ),
                (true, false) => say!(
                    "You are also subscribed to this mod in the Workshop, but the game uses this copy from its own Mods folder."
                ),
                (false, true) => say!(
                    "An older copy of this mod is in the game's own Mods folder, and the game uses that one instead of this one."
                ),
                (false, false) => say!(
                    "A copy of this mod is also in the game's own Mods folder, and the game uses that one instead of this one."
                ),
            });
            said.bin_local_copy = copies
                .workshop_newer
                .then(|| library.get(copies.local))
                .flatten()
                .map(|copy| LocalCopy {
                    mod_id: copies.local,
                    path: copy.path.to_string_lossy().into_owned(),
                });
        }
        let chosen = Category::from_kept(&note.category);
        said.category = chosen.or(library.category_of(id)).map(Category::label);
        said.note = note.text;
        said.category_chosen = chosen.map_or("", Category::label).to_string();
        let listed = [&self.active, &self.inactive]
            .into_iter()
            .find_map(|list| Some((list, list.mods().iter().find(|x| x.installed == Some(id))?)));
        said.sub = listed.map(|(_, x)| x.sub.clone()).unwrap_or_default();
        let beside = listed
            .map(|(list, x)| list.subs(x.category))
            .unwrap_or_default();
        let mut choices: Vec<String> = note.tags.iter().cloned().chain(beside).collect();
        choices.dedup();
        let mut seen = std::collections::HashSet::new();
        choices.retain(|tag| *tag != said.sub && seen.insert(tag.clone()));
        said.group_choices = choices;
        said.can_bin = m.source != Source::Official && self.active.position_of(id).is_none();
        if workshop {
            said.workshop_page = Some(format!(
                "https://steamcommunity.com/sharedfiles/filedetails/?id={}",
                m.folder
            ));
            said.steam_page = Some(format!("steam://url/CommunityFilePage/{}", m.folder));
        }
        let others: Vec<(&str, &[rimstack_core::rimworld::code::Aim])> = self
            .active
            .mods()
            .iter()
            .filter_map(|other| other.installed.filter(|&i| i != id))
            .filter_map(|i| library.get(i))
            .map(|other| (other.display_name(), other.code.aims.as_slice()))
            .collect();
        let also = describe::also_patching(&m.code.aims, &others);
        said.patches = describe::aims(&m.code.aims, &also, MOST_PATCHES)
            .into_iter()
            .map(|(method, note)| Patched { method, note })
            .collect();
        if let Some(at) = self.active.position_of(id) {
            let listed = &self.active.mods()[at];
            said.category = Some(listed.category.label());
            let own = self.troubles.get(at).map(Vec::as_slice).unwrap_or_default();
            said.problems = own.iter().map(|t| ProblemSaid::of(listed, t)).collect();
            said.offer_trust = said.problems.iter().any(|t| t.trust);
            let found = rules::all_rules(app, &mut self.detected, library, self.active.mods());
            let near = details::neighbours(&found, at, self.active.mods());
            let required = |n: &Named| said.requires.iter().any(|r| r.name == n.name);
            let (above, others): (Vec<Named>, Vec<Named>) =
                near.after.into_iter().partition(required);
            said.required_above = above.len();
            said.loads_after = others;
            said.loads_before = near.before;
            said.your_rules = self.own_rules(app, library, at);
            said.detected = self.detected_about(app, library, at);
            let clashes = self.texture_conflicts(app, id);
            said.loses_to = clashes.loses;
            said.wins_over = clashes.wins;
            said.conflict_marks = clashes.marks;
            said.placement = near.bottom.then(|| say!("The community rules keep this mod near the bottom of the list, below everything that does not have to follow it."));
        }
        said
    }

    pub fn textures(app: &App, id: ModId) -> Textures {
        let loaded = app.library();
        let Some(m) = loaded.as_ref().as_ref().ok().and_then(|(l, _)| l.get(id)) else {
            return Textures::default();
        };
        Textures {
            count: app.texture_count(&m.path),
            dds_made: app.texture_manifest().made_in(&m.path),
        }
    }

    pub fn files(app: &App, id: ModId) -> Files {
        let loaded = app.library();
        let Some(m) = loaded.as_ref().as_ref().ok().and_then(|(l, _)| l.get(id)) else {
            return Files::default();
        };
        Files {
            tree: folder_tree(&m.path)
                .into_iter()
                .map(|(folder, files)| Branch { folder, files })
                .collect(),
            assemblies: m.assemblies().join(", "),
        }
    }

    /// Relative to the mod; `(the mod's own folder)` for its root.
    fn load_folders(
        &self,
        library: &rimstack_core::mods::library::Library,
        m: &rimstack_core::mods::library::Installed,
    ) -> Vec<Sentence> {
        let active: std::collections::HashSet<String> = self
            .active
            .mods()
            .iter()
            .filter_map(|x| library.get(x.installed?)?.key())
            .collect();
        let found = loadfolders::folders(&m.path, &library.game_version, &|k| active.contains(k));
        found
            .iter()
            .map(|folder| match folder.strip_prefix(&m.path) {
                Ok(inside) if !inside.as_os_str().is_empty() => {
                    Sentence::Plain(inside.to_string_lossy().into_owned())
                }
                _ => say!("(the mod's own folder)"),
            })
            .collect()
    }

    fn detected_about(
        &mut self,
        app: &App,
        library: &rimstack_core::mods::library::Library,
        at: usize,
    ) -> Vec<Detected> {
        let mods = self.active.mods();
        let found = self.detected.of(app, library, mods);
        found
            .iter()
            .filter(|d| d.then == at)
            .map(|d| Detected {
                name: mods[d.first].name.clone(),
                why: d.why.clone(),
                code: d.code,
            })
            .collect()
    }

    fn own_rules(
        &self,
        app: &App,
        library: &rimstack_core::mods::library::Library,
        at: usize,
    ) -> Vec<OwnRule> {
        let mods = self.active.mods();
        let yours = app.user_rules().reading();
        details::own_rules(&yours.rules(&library.abouts(mods)), at, mods)
    }

    fn texture_conflicts(&self, app: &App, id: ModId) -> details::Clashes {
        let known = app.conflicts().reading();
        let Some(known) = known.as_ref() else {
            return Default::default();
        };
        let listed = |id: ModId| {
            let at = self.active.position_of(id)?;
            Some((at, self.active.mods()[at].name.clone()))
        };
        details::clashes(&known.members, &known.overlaps, id, &listed)
    }

    pub fn overview(&self, app: &App) -> Overview {
        let loaded = app.library();
        let Ok((library, took)) = &*loaded else {
            return Overview::default();
        };
        let mods = self.active.mods();
        let each = mods.iter().zip(&self.troubles);
        let said = |(m, own): (&Mod, &Vec<Trouble>)| -> Vec<ProblemSaid> {
            own.iter().map(|t| ProblemSaid::of(m, t)).collect()
        };
        let troubles: Vec<ProblemSaid> = each.clone().flat_map(said).collect();
        let to_get = each
            .clone()
            .flat_map(|(m, own)| {
                own.iter()
                    .filter(|t| t.fix.brings_a_mod())
                    .map(move |t| ProblemSaid::of(m, t))
            })
            .collect();
        let mut trouble_groups: Vec<ProblemGroup> =
            rimstack_core::base::grouped::by_key(each.flat_map(said), |t| t.group)
                .into_iter()
                .map(|(name, lines)| {
                    let trusted = lines.iter().filter(|t| t.trust).count();
                    ProblemGroup {
                        name,
                        error: lines.first().is_some_and(|t| t.error),
                        trusted,
                        shared: trusted > 1 && trusted == lines.len(),
                        lines,
                    }
                })
                .collect();
        trouble_groups.sort_by_key(|group| !group.error);
        let trusted = app
            .notes()
            .reading()
            .version_ok()
            .map(|key| {
                let found = library
                    .ids()
                    .find(|(_, m)| m.key().as_deref() == Some(key))
                    .map(|(id, _)| id);
                Named {
                    name: library.name_of(key).unwrap_or_else(|| key.to_string()),
                    note: match found {
                        Some(_) => Sentence::default(),
                        None => say!("not installed"),
                    },
                    trouble: false,
                    mod_id: found,
                }
            })
            .collect();
        let community = app.community();
        let tally = describe::tally(&self.troubles);
        Overview {
            game_version: library.game_version.clone(),
            categories: Category::ALL
                .into_iter()
                .filter(|c| *c != Category::Game)
                .map(Category::label)
                .collect(),
            troubles,
            to_get,
            trouble_groups,
            needed: tally.needed,
            version_only: tally.version_only,
            other_warnings: tally.other_warnings,
            slowest: details::slowest(mods, 5),
            load_times: mods.iter().any(|m| m.load_micros.is_some()),
            installed: library.installed.len(),
            missing: library.missing,
            unreadable: library.unreadable,
            read_ms: took.as_millis(),
            works_on: app.works_on().len(),
            trusted,
            community_mods: (*community).as_ref().map(|c| c.len()),
            community_time: (*community).as_ref().and_then(|c| c.timestamp),
        }
    }
}

use super::App;
use rayon::prelude::*;
use rimstack_core::base::error::Error;
use rimstack_core::base::sentence::Sentence;
use rimstack_core::base::sizes;
use rimstack_core::mods::hidden::{self, Hidden};
use rimstack_core::mods::library::{Installed, Library, ModId};
use rimstack_core::mods::{conflicts, ddsaudit, texdiff, textures};
use rimstack_core::rimworld::engine::Engine;
use rimstack_core::{said, say, say_n};
use serde::Serialize;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

mod spare;
pub use spare::SpareFound;

const PROGRESS_EVERY: usize = 20;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RowShown<'a> {
    #[serde(flatten)]
    pub row: &'a textures::Row,
    pub said: Sentence,
}

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TexturesShown<'a> {
    pub rows: Vec<RowShown<'a>>,
    pub all: usize,
    /// Empty for none.
    pub usual: &'static str,
    pub total: usize,
    pub have: usize,
    pub png_mb: f64,
    pub dds_mb: f64,
    /// Both sizes in one unit, with how much bigger the DDS is when it is.
    pub size_said: Option<Sentence>,
    pub remaining: Vec<ModId>,
    pub stale: Vec<ModId>,
    pub made: Vec<ModId>,
    pub made_files: usize,
    pub out_of_date: usize,
    pub waiting: usize,
    pub first_waiting: Option<ModId>,
}

pub fn shown(rows: &[textures::Row], active_only: bool, converts: bool) -> TexturesShown<'_> {
    let on_show: Vec<&textures::Row> = rows.iter().filter(|r| !active_only || r.active).collect();
    let ids = |keep: &dyn Fn(&textures::Row) -> bool| -> Vec<ModId> {
        on_show.iter().filter(|r| keep(r)).map(|r| r.id).collect()
    };
    let mut counts: Vec<(&'static str, usize)> = Vec::new();
    for row in &on_show {
        match counts.iter_mut().find(|(state, _)| *state == row.state) {
            Some((_, n)) => *n += 1,
            None => counts.push((row.state, 1)),
        }
    }
    let usual = counts
        .iter()
        .max_by_key(|(_, n)| *n)
        .filter(|(_, n)| *n * 2 > on_show.len())
        .map_or("", |(state, _)| state);
    let made: Vec<&&textures::Row> = on_show
        .iter()
        .filter(|r| r.converted + r.out_of_date > 0)
        .collect();
    let total: usize = on_show.iter().map(|r| r.textures).sum();
    let waiting: Vec<ModId> = ids(&|r| r.waits);
    let png_mb: f64 = on_show.iter().map(|r| r.png_mb).sum();
    let dds_mb: f64 = on_show.iter().map(|r| r.dds_mb).sum();
    TexturesShown {
        all: rows.len(),
        usual,
        total,
        have: total - on_show.iter().map(|r| r.not_converted).sum::<usize>(),
        png_mb,
        dds_mb,
        size_said: sizes_said(png_mb, dds_mb, converts),
        remaining: ids(&|r| !r.skipped && r.not_converted > 0),
        stale: ids(&|r| !r.skipped && r.out_of_date > 0),
        made_files: made.iter().map(|r| r.converted + r.out_of_date).sum(),
        made: made.iter().map(|r| r.id).collect(),
        out_of_date: on_show
            .iter()
            .filter(|r| !r.skipped)
            .map(|r| r.out_of_date)
            .sum(),
        waiting: waiting.len(),
        first_waiting: waiting.first().copied(),
        rows: on_show
            .into_iter()
            .map(|row| RowShown {
                row,
                said: Sentence::named(row.state),
            })
            .collect(),
    }
}

fn sizes_said(png_mb: f64, dds_mb: f64, converts: bool) -> Option<Sentence> {
    if png_mb <= 0.0 {
        return None;
    }
    let (png, dds) = sizes::together(png_mb, dds_mb);
    let sizes = say!("{} PNG  →  {} DDS", png.inside(), dds.inside());
    if !converts || dds_mb <= png_mb * 1.2 {
        return Some(sizes);
    }
    let bigger = say!(
        "{}× the size on disk, loads faster",
        format!("{:.1}", dds_mb / png_mb)
    );
    Some(Sentence::listed(vec![sizes, bigger]))
}

pub struct TextureRows {
    pub rows: Vec<textures::Row>,
    /// Whether the game is one a DDS loads faster in.
    pub converts: bool,
}

pub fn no_conversion() -> Sentence {
    say!(
        "From 1.7 the game compresses each texture once and keeps it, so a DDS no longer loads faster: RimStack converts nothing. The DDS files only take room."
    )
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Refused {
    #[serde(rename = "mod")]
    pub of_mod: String,
    pub file: String,
    pub name: String,
    pub why: Sentence,
}

#[derive(Serialize, Clone, PartialEq)]
pub struct Side {
    pub id: ModId,
    pub name: String,
}

#[derive(Serialize, Clone, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Shared {
    pub name: String,
    pub file: String,
    pub at: usize,
    #[serde(skip)]
    pub earlier: PathBuf,
    #[serde(skip)]
    pub later: PathBuf,
    /// On disk: a hidden one under its renamed name.
    pub earlier_shown: String,
    pub later_shown: String,
    pub earlier_bytes: u64,
    pub later_bytes: u64,
    pub keeps_earlier: bool,
    pub your_choice: bool,
    pub used: Sentence,
}

#[derive(Serialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Comparison {
    pub earlier: Side,
    pub later: Side,
    #[serde(skip)]
    pub later_folder: PathBuf,
    #[serde(skip)]
    pub textures: Vec<Shared>,
    pub total: usize,
    pub kept_earlier: usize,
    pub yours: usize,
}

impl Comparison {
    pub fn named(&self, filter: &str) -> Vec<&Shared> {
        let filter = filter.trim().to_lowercase();
        self.textures
            .iter()
            .filter(|t| t.name.to_lowercase().contains(&filter))
            .collect()
    }
}

#[derive(Serialize)]
pub struct Difference {
    pub file: String,
    pub differing: u64,
    pub total: u64,
}

impl App {
    pub fn hidden_file(&self) -> PathBuf {
        self.data_dir().join("hidden.json")
    }

    fn skipped_textures(&self) -> HashSet<String> {
        let settings = self.settings();
        settings.listed("textureSkip").map(String::from).collect()
    }

    /// An unread library counts as a game where a DDS loads faster.
    pub fn texture_rows(&self) -> TextureRows {
        let loaded = self.library();
        let Ok((library, _)) = &*loaded else {
            return TextureRows {
                rows: Vec::new(),
                converts: true,
            };
        };
        let converts = library.engine().dds_loads_faster();
        let manifest = self.texture_manifest();
        let skipped = self.skipped_textures();
        let active = self.active_now(library);
        let all: Vec<_> = library.loadable().collect();
        let rows = all
            .par_iter()
            .filter_map(|&(i, m)| {
                let counted = textures::summarize(&m.path, &manifest);
                let skipped = m.key().is_some_and(|k| skipped.contains(&k));
                let active = active.contains(&i);
                let name = m.display_name();
                textures::Row::of(i, name, &m.path, counted, skipped, active, converts)
            })
            .collect();
        TextureRows {
            rows: textures::in_order(rows),
            converts,
        }
    }

    pub fn skip_textures(&self, id: ModId, skip: bool) -> Result<(), Error> {
        let key = {
            let loaded = self.library();
            let (library, _) = loaded.as_ref().as_ref().map_err(Clone::clone)?;
            library.get(id).and_then(|m| m.key())
        };
        let Some(key) = key else {
            return Ok(());
        };
        let mut skipped = self.skipped_textures();
        match skip {
            true => skipped.insert(key),
            false => skipped.remove(&key),
        };
        let mut kept: Vec<String> = skipped.into_iter().collect();
        kept.sort();
        self.set_setting("textureSkip", &kept.join(","))?;
        self.textures_changed();
        Ok(())
    }

    /// With `remove`, deletes only the DDS files RimStack made; without, converts
    /// nothing from 1.7.
    pub fn convert_textures(
        &self,
        ids: &[ModId],
        remove: bool,
        stop: &AtomicBool,
        progress: &(dyn Fn(Sentence) + Sync),
    ) -> Sentence {
        let loaded = self.library();
        let Ok((library, _)) = &*loaded else {
            return Sentence::default();
        };
        if !remove && !library.engine().dds_loads_faster() {
            return no_conversion();
        }
        let roots: Vec<&Path> = ids
            .iter()
            .filter_map(|&i| library.get(i))
            .map(|m| m.path.as_path())
            .collect();
        let file = self.texture_manifest_file();
        let mut manifest = textures::Manifest::read(&file);
        let total: usize = match remove {
            true => 0,
            false => roots
                .iter()
                .map(|root| {
                    let counted = textures::summarize(root, &manifest);
                    counted.not_converted() + counted.out_of_date
                })
                .sum(),
        };
        let done = AtomicUsize::new(0);
        let mut failed = 0;
        for root in roots {
            if stop.load(Ordering::Relaxed) {
                break;
            }
            let mut parts = textures::Parts::default();
            if remove {
                let listed = manifest.made_in(root);
                textures::remove(root, &mut manifest);
                if manifest.made_in(root) == listed {
                    continue;
                }
            } else {
                let converted = textures::convert(root, &mut manifest, stop, &|| {
                    let n = done.fetch_add(1, Ordering::Relaxed) + 1;
                    if n.is_multiple_of(PROGRESS_EVERY) || n == total {
                        progress(say_n!(
                            total,
                            "{} texture: {} converted",
                            "{} textures: {} converted",
                            n
                        ));
                    }
                });
                let outcome = match converted {
                    Ok(outcome) => outcome,
                    Err(problem) => {
                        rimstack_core::problem!("{problem}");
                        self.textures_changed();
                        return said!("Textures were not converted: {}", problem.within());
                    }
                };
                failed += outcome.failed.len();
                for (png, reason) in outcome.failed {
                    rimstack_core::problem!("{}: {reason}", png.display());
                }
                parts = outcome.parts;
            }
            if let Err(problem) = manifest.write(&file) {
                parts.discard();
                rimstack_core::problem!("{problem}");
                self.textures_changed();
                return said!(
                    "The list of the DDS files RimStack made could not be saved: {}",
                    problem.within()
                );
            }
            for (dds, reason) in parts.place() {
                failed += 1;
                rimstack_core::problem!("{}: {reason}", dds.display());
            }
        }
        self.textures_changed();
        match failed {
            0 => Sentence::default(),
            n => say_n!(
                n,
                "{} texture could not be converted; it keeps using its PNG.",
                "{} textures could not be converted; they keep using their PNG."
            ),
        }
    }

    pub fn refused_dds(&self) -> Vec<Refused> {
        let loaded = self.library();
        let Ok((library, _)) = &*loaded else {
            return Vec::new();
        };
        let engine = library.engine();
        let active = self.active_keys(library);
        let mods: Vec<&Installed> = library.loadable().map(|(_, m)| m).collect();
        mods.into_par_iter()
            .flat_map_iter(|m| {
                let loaded = match engine {
                    Engine::Unity => Vec::new(),
                    Engine::Waylith => self.folders_read(library, &m.path, &active),
                };
                ddsaudit::audit(&m.path, engine, &loaded)
                    .into_iter()
                    .map(|bad| Refused {
                        of_mod: m.display_name().to_string(),
                        name: bad
                            .file
                            .file_name()
                            .map_or(String::new(), |n| n.to_string_lossy().into_owned()),
                        file: bad.file.to_string_lossy().into_owned(),
                        why: bad.why,
                    })
            })
            .collect()
    }

    pub fn set_dds_aside(&self, refused: &[Refused]) -> usize {
        let failed = refused
            .iter()
            .filter(|bad| ddsaudit::set_aside(Path::new(&bad.file)).is_err())
            .count();
        self.textures_changed();
        failed
    }

    fn active_keys(&self, library: &Library) -> HashSet<String> {
        let list = self.session().current().to_vec();
        library
            .arrange(&list)
            .active
            .iter()
            .filter_map(|m| library.get(m.installed?)?.key())
            .collect()
    }

    fn folders_read(
        &self,
        library: &Library,
        root: &Path,
        active: &HashSet<String>,
    ) -> Vec<PathBuf> {
        rimstack_core::rimworld::loadfolders::folders(root, &library.game_version, &|k| {
            active.contains(k)
        })
    }

    fn loaded_folders(&self, library: &Library, id: ModId) -> Vec<PathBuf> {
        let Some(root) = library.get(id).map(|m| &m.path) else {
            return Vec::new();
        };
        self.folders_read(library, root, &self.active_keys(library))
    }

    /// `earlier` loads first.
    pub fn compare_textures(&self, earlier: ModId, later: ModId) -> Option<Comparison> {
        let loaded = self.library();
        let (library, _) = loaded.as_ref().as_ref().ok()?;
        let (one, other) = (library.get(earlier)?, library.get(later)?);
        let (one_name, other_name) = (one.display_name(), other.display_name());
        let first = conflicts::files_with_hidden(&self.loaded_folders(library, earlier));
        let second = conflicts::files_with_hidden(&self.loaded_folders(library, later));
        let on_disk = |(path, hidden): &(PathBuf, bool)| match hidden {
            true => format!("{}{}", path.to_string_lossy(), hidden::SUFFIX),
            false => path.to_string_lossy().into_owned(),
        };
        let size = |file: &str| std::fs::metadata(file).map_or(0, |m| m.len());
        let textures: Vec<Shared> = first
            .iter()
            .filter_map(|(name, one)| Some((name, one, second.get(name)?)))
            .enumerate()
            .map(|(at, (name, one, other))| {
                let keeps_earlier = other.1 && !one.1;
                let your_choice = one.1 || other.1;
                let user = match keeps_earlier {
                    true => one_name,
                    false => other_name,
                };
                Shared {
                    file: name.rsplit('/').next().unwrap_or(name).to_string(),
                    name: name.clone(),
                    at,
                    earlier: one.0.clone(),
                    later: other.0.clone(),
                    earlier_bytes: size(&on_disk(one)),
                    later_bytes: size(&on_disk(other)),
                    earlier_shown: on_disk(one),
                    later_shown: on_disk(other),
                    keeps_earlier,
                    your_choice,
                    used: match your_choice {
                        true => said!("{}, chosen", user),
                        false => Sentence::Plain(user.to_string()),
                    },
                }
            })
            .collect();
        Some(Comparison {
            earlier: Side {
                id: earlier,
                name: one_name.to_string(),
            },
            later: Side {
                id: later,
                name: other_name.to_string(),
            },
            later_folder: other.path.clone(),
            total: textures.len(),
            kept_earlier: textures.iter().filter(|t| t.keeps_earlier).count(),
            yours: textures.iter().filter(|t| t.your_choice).count(),
            textures,
        })
    }

    /// Keeping the earlier mod's renames the later one's file;
    /// keeping the later one's restores every file.
    pub fn keep_textures(&self, chosen: &[&Shared], keep_earlier: bool) -> Result<(), Error> {
        let file = self.hidden_file();
        let mut hidden = Hidden::read(&file);
        let done = chosen.iter().try_for_each(|shared| {
            hidden.unhide(&shared.earlier)?;
            hidden.unhide(&shared.later)?;
            match keep_earlier {
                true => hidden.hide(&shared.later),
                false => Ok(()),
            }
        });
        let written = hidden.write(&file);
        self.textures_changed();
        done.and(written)
    }

    pub fn keep_all_textures(
        &self,
        comparison: &Comparison,
        keep_earlier: bool,
    ) -> Result<Sentence, Error> {
        let all: Vec<&Shared> = comparison.textures.iter().collect();
        self.keep_textures(&all, keep_earlier)?;
        if !keep_earlier {
            return Ok(Sentence::default());
        }
        let (n, name) = (all.len(), comparison.later.name.as_str());
        let aside = say_n!(
            n,
            "Set aside {} texture of {}",
            "Set aside {} textures of {}",
            name
        );
        let folder = comparison.later_folder.to_string_lossy();
        self.note(aside.clone(), format!("unhide {folder}"));
        Ok(aside)
    }

    pub fn hidden_in(&self, id: ModId) -> usize {
        let loaded = self.library();
        let Some(m) = loaded.as_ref().as_ref().ok().and_then(|(l, _)| l.get(id)) else {
            return 0;
        };
        let hidden = Hidden::read(&self.hidden_file());
        let pngs = hidden.in_mod(&m.path).into_iter();
        pngs.filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("png")))
            .count()
    }

    pub fn unhide_folder(&self, folder: &Path) -> Result<(), Error> {
        let installed = {
            let loaded = self.library();
            let library = loaded.as_ref().as_ref().map_err(Clone::clone)?;
            library.0.installed.iter().any(|m| m.path == folder)
        };
        if !installed {
            return Err(Error::refused(said!(
                "{} is not an installed mod's folder",
                folder.display()
            )));
        }
        let file = self.hidden_file();
        let mut hidden = Hidden::read(&file);
        let done = hidden.unhide_all(folder).and_then(|_| hidden.write(&file));
        self.textures_changed();
        done
    }

    pub fn unhide_mod(&self, id: ModId) -> Result<(), Error> {
        let folder = {
            let loaded = self.library();
            let library = loaded.as_ref().as_ref().map_err(Clone::clone)?;
            library.0.get(id).map(|m| m.path.clone())
        };
        match folder {
            Some(folder) => self.unhide_folder(&folder),
            None => Ok(()),
        }
    }

    pub fn texture_difference(&self, shared: &Shared) -> Result<Difference, Error> {
        use std::hash::{Hash, Hasher};
        let dir = self.data_dir().join("cache");
        for old in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
            if old.file_name().to_string_lossy().starts_with("difference-") {
                let _ = std::fs::remove_file(old.path());
            }
        }
        let (from, to) = (&shared.earlier_shown, &shared.later_shown);
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        (from, to).hash(&mut hasher);
        let file = dir.join(format!("difference-{:016x}.png", hasher.finish()));
        let (differing, total) = texdiff::write(Path::new(from), Path::new(to), &file)?;
        Ok(Difference {
            file: file.to_string_lossy().into_owned(),
            differing,
            total,
        })
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn both_sizes_share_a_unit_and_the_growth_is_said_when_large() {
        let said = |png, dds| super::sizes_said(png, dds, true).map(|s| s.to_string());
        assert_eq!(said(0.0, 0.0), None);
        assert_eq!(said(0.001, 0.0004).unwrap(), "1.0 KB PNG  →  0.41 KB DDS");
        assert_eq!(
            said(10.0, 20.0).unwrap(),
            "10 MB PNG  →  20 MB DDS, 2.0× the size on disk, loads faster"
        );
        let from_1_7 = super::sizes_said(10.0, 20.0, false).unwrap().to_string();
        assert_eq!(from_1_7, "10 MB PNG  →  20 MB DDS");
    }

    use super::*;
    use crate::workspace::tests::installed;

    fn texture(at: &Path) {
        std::fs::create_dir_all(at.parent().unwrap()).unwrap();
        std::fs::write(at, [0u8; 16]).unwrap();
    }

    #[test]
    fn a_mod_with_textures_is_listed_until_you_ask_to_skip_it() {
        let (tmp, app) = installed();
        texture(&tmp.path().join("game/Mods/roads/Textures/road.png"));
        let rows = app.texture_rows().rows;
        let on_show = crate::words::as_shown(&shown(&rows, false, true));
        assert_eq!(on_show["rows"][0]["name"], "Ann.Roads");
        assert_eq!(on_show["rows"][0]["state"], "Not converted");
        assert_eq!(on_show["usual"], "Not converted");
        assert_eq!(on_show["total"], 1);
        assert_eq!(on_show["have"], 0);
        assert_eq!(on_show["waiting"], 1);
        assert_eq!(on_show["remaining"].as_array().unwrap().len(), 1);
        assert!(
            shown(&rows, true, true).rows.is_empty(),
            "Roads is not active"
        );

        app.skip_textures(rows[0].id, true).unwrap();
        let rows = app.texture_rows().rows;
        assert_eq!((rows[0].state, rows[0].waits), ("Skipped", false));
        assert!(shown(&rows, false, true).remaining.is_empty());
    }

    #[test]
    fn no_dds_is_put_in_place_when_the_manifest_cannot_be_written() {
        let (tmp, app) = installed();
        let textures = tmp.path().join("game/Mods/roads/Textures");
        std::fs::create_dir_all(&textures).unwrap();
        image::RgbaImage::from_pixel(4, 4, image::Rgba([1, 2, 3, 255]))
            .save(textures.join("road.png"))
            .unwrap();
        std::fs::create_dir_all(app.texture_manifest_file()).unwrap();
        let id = app.texture_rows().rows[0].id;

        let stop = AtomicBool::new(false);
        let said = app.convert_textures(&[id], false, &stop, &|_| {});

        assert!(!said.to_string().is_empty());
        let left: Vec<_> = std::fs::read_dir(&textures)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(left, ["road.png"]);
    }

    fn pack(at: &Path, entries: &[&str]) {
        let mut listed = Vec::new();
        for name in entries {
            listed.extend(0x0201_4b50u32.to_le_bytes());
            listed.extend([0u8; 24]);
            listed.extend((name.len() as u16).to_le_bytes());
            listed.extend([0u8; 16]);
            listed.extend(name.as_bytes());
        }
        let mut bytes = listed.clone();
        bytes.extend(0x0605_4b50u32.to_le_bytes());
        bytes.extend([0u8; 8]);
        bytes.extend((listed.len() as u32).to_le_bytes());
        bytes.extend([0u8; 6]);
        std::fs::write(at, bytes).unwrap();
    }

    #[test]
    fn from_1_7_nothing_is_converted_and_a_dds_hiding_the_mod_s_pack_is_found() {
        let (tmp, app) = installed();
        let game = tmp.path().join("game");
        std::fs::write(game.join("Version.txt"), "1.7.5005 rev1082").unwrap();
        let roads = game.join("Mods/roads");
        texture(&roads.join("Textures/road.png"));
        texture(&roads.join("Textures/rock.dds"));
        pack(&roads.join("Assets.zip"), &["Textures/rock.png.dds"]);
        app.rescan();
        let counted = app.texture_rows();
        assert!(!counted.converts);
        let rows = counted.rows;
        assert_eq!((rows[0].not_converted, rows[0].waits), (1, false));
        assert_eq!(shown(&rows, false, false).waiting, 0);
        let stop = AtomicBool::new(false);
        let said = app.convert_textures(&[rows[0].id], false, &stop, &|_| {});
        assert_eq!(said, no_conversion());
        assert!(!roads.join("Textures/road.dds").exists());

        let refused = app.refused_dds();
        assert_eq!(refused.len(), 1);
        assert_eq!(refused[0].name, "rock.dds");
        assert!(refused[0].why.to_string().contains("Assets.zip"));
        assert_eq!(app.set_dds_aside(&refused), 0);
        assert!(app.refused_dds().is_empty());
    }

    fn two_mods_sharing_a_rock(tmp: &tempfile::TempDir, app: &App) -> (ModId, ModId) {
        let mods = tmp.path().join("game/Mods");
        for (folder, id) in [("a", "Ann.A"), ("b", "Ann.B")] {
            let about = mods.join(folder).join("About");
            std::fs::create_dir_all(&about).unwrap();
            std::fs::write(
                about.join("About.xml"),
                format!("<ModMetaData><packageId>{id}</packageId><name>{id}</name></ModMetaData>"),
            )
            .unwrap();
            texture(&mods.join(folder).join("Textures/Things/rock.png"));
        }
        app.rescan();
        app.session().record(vec![
            "ludeon.rimworld".into(),
            "ann.a".into(),
            "ann.b".into(),
        ]);
        let id_of = |folder: &str| {
            let loaded = app.library();
            let (library, _) = loaded.as_ref().as_ref().unwrap();

            library.ids().find(|(_, m)| m.folder == folder).unwrap().0
        };
        (id_of("a"), id_of("b"))
    }

    #[test]
    fn two_mods_sharing_a_texture_keep_either_version_and_the_history_can_undo_it() {
        let (tmp, app) = installed();
        let mods = tmp.path().join("game/Mods");
        let (a, b) = two_mods_sharing_a_rock(&tmp, &app);
        let compared = app.compare_textures(a, b).unwrap();
        assert_eq!((compared.total, compared.kept_earlier), (1, 0));
        assert_eq!(compared.textures[0].used.to_string(), "Ann.B");
        assert_eq!(compared.named(" ROCK").len(), 1);
        assert!(compared.named("tree").is_empty());

        let said = app.keep_all_textures(&compared, true).unwrap();
        assert_eq!(said.to_string(), "Set aside 1 texture of Ann.B");
        assert_eq!((app.hidden_in(b), app.hidden_in(a)), (1, 0));
        let again = app.compare_textures(a, b).unwrap();
        assert_eq!((again.kept_earlier, again.yours), (1, 1));
        assert_eq!(again.textures[0].used.to_string(), "Ann.A, chosen");
        app.settle_writes();
        let history = app.history();
        assert_eq!(history[0].said, said);
        assert_eq!(Path::new(&history[0].bring_back), mods.join("b"));

        assert!(app.unhide_folder(tmp.path()).is_err());
        app.unhide_folder(Path::new(&history[0].bring_back))
            .unwrap();
        assert_eq!(app.hidden_in(b), 0);
        assert!(mods.join("b/Textures/Things/rock.png").is_file());
    }

    #[cfg(unix)]
    #[test]
    fn a_texture_that_cannot_be_brought_back_stays_in_the_record() {
        use std::os::unix::fs::PermissionsExt;
        let (tmp, app) = installed();
        let (a, b) = two_mods_sharing_a_rock(&tmp, &app);
        let compared = app.compare_textures(a, b).unwrap();
        app.keep_all_textures(&compared, true).unwrap();
        let folder = tmp.path().join("game/Mods/b/Textures/Things");
        assert!(!folder.join("rock.png").exists());

        let sealed = |mode| std::fs::set_permissions(&folder, PermissionsExt::from_mode(mode));
        sealed(0o555).unwrap();
        let sealed_off = std::fs::write(folder.join("probe"), "").is_err();
        let again = app.compare_textures(a, b).unwrap();
        let result = app.keep_all_textures(&again, false);
        sealed(0o755).unwrap();
        if !sealed_off {
            return;
        }

        assert!(result.is_err());
        let record = Hidden::read(&app.hidden_file());
        assert_eq!(record.in_mod(&tmp.path().join("game/Mods/b")).len(), 1);
        assert!(folder.join("rock.png.rimstack-hidden").is_file());
    }
}

use crate::base::error::Error;
use crate::said;

use crate::rimworld::id;
use roxmltree::Document;
use std::collections::HashSet;
use std::io::{BufRead, Read};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Header {
    /// As the game writes it: `1.6.4871 rev600`.
    pub game_version: Option<String>,
    pub mod_ids: Vec<String>,
    /// One per id; old saves may have fewer.
    pub mod_names: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveFile {
    pub path: PathBuf,
    pub name: String,
    /// Seconds since 1970.
    pub modified: u64,
    pub bytes: u64,
    pub header: Result<Header, Error>,
}

/// Reading stops at `</meta>`, or here: a save is tens of megabytes.
const HEADER_LIMIT: usize = 4 * 1024 * 1024;

/// The file as the game wrote it, or as a mod compressed it: zstd or gzip, told
/// by their first bytes.
fn opened(path: &Path) -> std::io::Result<Box<dyn Read>> {
    const ZSTD: &[u8] = &[0x28, 0xb5, 0x2f, 0xfd];
    const GZIP: &[u8] = &[0x1f, 0x8b];
    let mut file = std::io::BufReader::new(std::fs::File::open(path)?);
    let first = file.fill_buf()?;
    Ok(if first.starts_with(ZSTD) {
        Box::new(ruzstd::decoding::StreamingDecoder::new(file).map_err(std::io::Error::other)?)
    } else if first.starts_with(GZIP) {
        Box::new(flate2::read::GzDecoder::new(file))
    } else {
        Box::new(file)
    })
}

pub fn read_header(path: &Path) -> Result<Header, Error> {
    const END: &[u8] = b"</meta>";
    let failed = |e: std::io::Error| Error::file(path, e);
    let mut file = opened(path).map_err(failed)?;
    let mut start = Vec::new();
    let mut chunk = [0u8; 64 * 1024];
    let end = loop {
        let searched_from = start.len().saturating_sub(END.len());
        let read = file.read(&mut chunk).map_err(failed)?;
        start.extend_from_slice(&chunk[..read]);
        let found = start[searched_from..]
            .windows(END.len())
            .position(|w| w == END);
        if let Some(at) = found {
            break searched_from + at + END.len();
        }
        if read == 0 || start.len() > HEADER_LIMIT {
            return Err(Error::format(said!("no mod list at the start of the file")));
        }
    };
    let text = std::str::from_utf8(&start[..end]).map_err(|_| Error::format(said!("not UTF-8")))?;
    parse_header(&format!("{text}</savegame>"))
}

fn parse_header(text: &str) -> Result<Header, Error> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let doc = Document::parse(text).map_err(|e| Error::format(said!("malformed XML: {}", e)))?;
    let meta = doc
        .root_element()
        .children()
        .find(|n| n.has_tag_name("meta"))
        .ok_or_else(|| Error::format(said!("no mod list at the start of the file")))?;
    let child = |name: &str| meta.children().find(|n| n.has_tag_name(name));
    let items = |name: &str| -> Vec<String> {
        let list = child(name).into_iter().flat_map(|n| n.children());
        list.filter(|n| n.has_tag_name("li"))
            .map(|n| n.text().unwrap_or("").trim().to_string())
            .collect()
    };
    Ok(Header {
        game_version: child("gameVersion")
            .and_then(|n| n.text())
            .map(|s| s.trim().to_string()),
        mod_ids: items("modIds"),
        mod_names: items("modNames"),
    })
}

/// Newest first; a missing folder has no saves.
pub fn list(dir: &Path) -> Vec<SaveFile> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut saves: Vec<SaveFile> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("rws")))
        .map(|path| {
            let meta = std::fs::metadata(&path).ok();
            let modified = meta.as_ref().and_then(|m| m.modified().ok());
            let since = modified.and_then(|t| t.duration_since(UNIX_EPOCH).ok());
            SaveFile {
                name: path
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned(),
                modified: since.map_or(0, |d| d.as_secs()),
                bytes: meta.map_or(0, |m| m.len()),
                header: read_header(&path),
                path,
            }
        })
        .collect();
    saves.sort_by(|a, b| b.modified.cmp(&a.modified).then(a.name.cmp(&b.name)));
    saves
}

/// Positions point into the list each came from.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Difference {
    pub missing: Vec<usize>,
    pub added: Vec<usize>,
    pub reordered: bool,
}

impl Difference {
    pub fn is_none(&self) -> bool {
        self.missing.is_empty() && self.added.is_empty() && !self.reordered
    }
}

/// Compares by package id, ignoring case and the `_steam` suffix.
pub fn compare(save_ids: &[String], list_ids: &[String]) -> Difference {
    let key = |s: &String| id::split_steam(s).0;
    let in_save: Vec<String> = save_ids.iter().map(key).collect();
    let in_list: Vec<String> = list_ids.iter().map(key).collect();
    let save_set: HashSet<&String> = in_save.iter().collect();
    let list_set: HashSet<&String> = in_list.iter().collect();
    let positions = |of: &[String], other: &HashSet<&String>, wanted: bool| -> Vec<usize> {
        let keep = |(_, k): &(usize, &String)| other.contains(k) == wanted;
        of.iter().enumerate().filter(keep).map(|(i, _)| i).collect()
    };
    let shared = |of: &[String], other: &HashSet<&String>| -> Vec<String> {
        of.iter().filter(|k| other.contains(k)).cloned().collect()
    };
    Difference {
        missing: positions(&in_save, &list_set, false),
        added: positions(&in_list, &save_set, false),
        reordered: shared(&in_save, &list_set) != shared(&in_list, &save_set),
    }
}

/// Worst first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Unreadable,
    Breaks,
    Differs,
    Same,
}

/// Serialised as one flag per verdict, e.g. `verdict.breaks`.
impl serde::Serialize for Verdict {
    fn serialize<S: serde::Serializer>(&self, to: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut flags = to.serialize_struct("Verdict", 4)?;
        flags.serialize_field("unreadable", &(*self == Verdict::Unreadable))?;
        flags.serialize_field("breaks", &(*self == Verdict::Breaks))?;
        flags.serialize_field("differs", &(*self == Verdict::Differs))?;
        flags.serialize_field("same", &(*self == Verdict::Same))?;
        flags.end()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Lacking {
    pub name: String,
    pub installed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Compared {
    pub game_version: String,
    pub other_game: bool,
    pub mods: usize,
    pub ids: Vec<String>,
    pub missing: Vec<Lacking>,
    pub added: Vec<String>,
    pub reordered: bool,
}

impl Compared {
    /// `name_of` takes a folded package id.
    pub fn of(
        header: &Header,
        list: &[String],
        game_version: &str,
        name_of: &dyn Fn(&str) -> Option<String>,
    ) -> Self {
        let difference = compare(&header.mod_ids, list);
        let lacking = |&i: &usize| {
            let key = id::split_steam(&header.mod_ids[i]).0;
            let installed = name_of(&key);
            let kept = || header.mod_names.get(i).cloned();
            Lacking {
                installed: installed.is_some(),
                name: installed.or_else(kept).unwrap_or(key),
            }
        };
        let named = |&i: &usize| {
            let key = id::split_steam(&list[i]).0;
            name_of(&key).unwrap_or(key)
        };
        let written = header.game_version.clone().unwrap_or_default();
        let short = crate::rimworld::modsconfig::short_version(&written).unwrap_or_default();
        Self {
            other_game: !short.is_empty() && short != game_version,
            game_version: written,
            mods: header.mod_ids.len(),
            ids: header.mod_ids.clone(),
            missing: difference.missing.iter().map(lacking).collect(),
            added: difference.added.iter().map(named).collect(),
            reordered: difference.reordered,
        }
    }

    pub fn verdict(&self) -> Verdict {
        if !self.missing.is_empty() || self.other_game {
            Verdict::Breaks
        } else if self.added.is_empty() && !self.reordered {
            Verdict::Same
        } else {
            Verdict::Differs
        }
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Row {
    pub file: String,
    pub folder: String,
    pub name: String,
    pub facts: Vec<(&'static str, String)>,
    pub preview: Option<String>,
    pub path: String,
    pub modified: u64,
    pub megabytes: f64,
    pub verdict: Verdict,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub problem: Option<String>,
    #[serde(flatten, skip_serializing_if = "Option::is_none")]
    pub compared: Option<Compared>,
}

impl Row {
    pub fn of(
        save: &SaveFile,
        facts: Vec<(&'static str, String)>,
        preview: Option<PathBuf>,
        compared: Option<Compared>,
    ) -> Self {
        let (folder, shown) = crate::rimworld::rimsaves::split(&save.name);
        Self {
            file: save.name.clone(),
            folder: folder.to_string(),
            name: shown.to_string(),
            facts,
            preview: preview.map(|picture| picture.to_string_lossy().into_owned()),
            path: save.path.to_string_lossy().into_owned(),
            modified: save.modified,
            megabytes: save.bytes as f64 / 1_048_576.0,
            verdict: compared
                .as_ref()
                .map_or(Verdict::Unreadable, Compared::verdict),
            problem: save.header.as_ref().err().map(Error::to_string),
            compared,
        }
    }
}

pub fn back_up(save: &Path, backups: &Path, stamp: u64, keep: usize) -> Result<PathBuf, Error> {
    let failed = |e: std::io::Error| Error::file(backups, e);
    std::fs::create_dir_all(backups).map_err(failed)?;
    let name = save.file_name().unwrap_or_default().to_string_lossy();
    let copy = backups.join(format!("{stamp:012}-{name}"));
    std::fs::copy(save, &copy).map_err(failed)?;

    // Names start with the time, so name order is age order.
    let mut copies: Vec<PathBuf> = std::fs::read_dir(backups)
        .map_err(failed)?
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .is_some_and(|n| is_own_copy(&n.to_string_lossy()))
        })
        .collect();
    copies.sort();
    let extra = copies.len().saturating_sub(keep);
    for old in &copies[..extra] {
        std::fs::remove_file(old).map_err(failed)?;
    }
    Ok(copy)
}

fn is_own_copy(file: &str) -> bool {
    let Some((stamp, name)) = file.split_at_checked(12) else {
        return false;
    };
    stamp.bytes().all(|b| b.is_ascii_digit())
        && name.strip_prefix('-').is_some_and(|name| {
            Path::new(name)
                .extension()
                .is_some_and(|x| x.eq_ignore_ascii_case("rws"))
        })
}

pub fn version_all_share(saves: &[(&str, bool)]) -> Option<String> {
    let short = |version: &str| version.split('.').take(2).collect::<Vec<_>>().join(".");
    let first = short(saves.first()?.0);
    let shared = saves.len() > 1 && saves.iter().all(|(v, other)| *other && short(v) == first);
    shared.then_some(first).filter(|v| !v.is_empty())
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_version_is_shared_only_by_several_saves_all_from_another_game() {
        let other = [("1.6.4871 rev600", true), ("1.6.4500 rev1", true)];
        assert_eq!(super::version_all_share(&other), Some("1.6".to_string()));
        assert_eq!(super::version_all_share(&other[..1]), None);
        assert_eq!(
            super::version_all_share(&[("1.6.1", true), ("1.5.2", true)]),
            None
        );
        assert_eq!(
            super::version_all_share(&[("1.6.1", true), ("1.7.2", false)]),
            None
        );
    }

    use super::*;

    fn header(ids: &[&str], version: &str) -> Header {
        Header {
            game_version: Some(version.to_string()),
            mod_ids: ids.iter().map(|id| id.to_string()).collect(),
            mod_names: ids.iter().map(|id| format!("{id} by name")).collect(),
        }
    }

    fn installed(key: &str) -> Option<String> {
        ["a.core", "b.lib", "d.new"]
            .contains(&key)
            .then(|| key.to_uppercase())
    }

    #[test]
    fn a_save_breaks_on_a_mod_the_list_lacks_or_on_another_game_version() {
        let list: Vec<String> = ["a.core", "d.new"].map(String::from).to_vec();
        let read = header(&["A.Core", "b.lib_steam", "c.gone"], "1.6.4 rev1");
        let lacks = Compared::of(&read, &list, "1.6", &installed);
        assert_eq!(lacks.verdict(), Verdict::Breaks);
        let named: Vec<(&str, bool)> = lacks
            .missing
            .iter()
            .map(|m| (m.name.as_str(), m.installed))
            .collect();
        assert_eq!(named, [("B.LIB", true), ("c.gone by name", false)]);
        assert_eq!(lacks.added, ["D.NEW"]);

        let alone: Vec<String> = vec!["a.core".to_string()];
        let older = Compared::of(
            &header(&["a.core"], "1.5.4 rev1"),
            &alone,
            "1.6",
            &installed,
        );
        assert!(older.other_game && older.verdict() == Verdict::Breaks);
        let same = Compared::of(
            &header(&["a.core"], "1.6.4 rev1"),
            &alone,
            "1.6",
            &installed,
        );
        assert_eq!(same.verdict(), Verdict::Same);
        let more = Compared::of(&header(&["a.core"], "1.6.4"), &list, "1.6", &installed);
        assert_eq!(more.verdict(), Verdict::Differs);
    }

    #[test]
    fn a_row_says_what_was_compared_or_why_the_save_could_not_be_read() {
        let save = |header| SaveFile {
            path: PathBuf::from("/saves/Colony.rws"),
            name: "Colony".to_string(),
            modified: 7,
            bytes: 3_145_728,
            header,
        };
        let unreadable = Row::of(
            &save(Err(Error::format("not UTF-8"))),
            Vec::new(),
            None,
            None,
        );
        let said = serde_json::to_value(&unreadable).unwrap();
        assert_eq!(said["verdict"]["unreadable"], true);
        assert_eq!(said["problem"], "not UTF-8");
        assert!(said.get("ids").is_none());

        let read = header(&["a.core"], "1.6.1");
        let compared = Compared::of(&read, &["a.core".to_string()], "1.6", &installed);
        let facts = vec![("Seed", "abc".to_string())];
        let row = Row::of(&save(Ok(read)), facts, None, Some(compared));
        let said = serde_json::to_value(&row).unwrap();
        assert_eq!(said["verdict"]["same"], true);
        assert_eq!(said["megabytes"], 3.0);
        assert_eq!(said["facts"][0][1], "abc");
        assert_eq!(said["gameVersion"], "1.6.1");
        assert!(said.get("problem").is_none());
    }

    const START: &str = "\u{feff}<?xml version=\"1.0\" encoding=\"utf-8\"?>
<savegame>
\t<meta>
\t\t<gameVersion>1.6.4871 rev600</gameVersion>
\t\t<modIds>
\t\t\t<li>brrainz.harmony</li>
\t\t\t<li>ludeon.rimworld</li>
\t\t</modIds>
\t\t<modSteamIds><li>2009463077</li><li>0</li></modSteamIds>
\t\t<modNames>
\t\t\t<li>Harmony</li>
\t\t\t<li>Core</li>
\t\t</modNames>
\t</meta>
\t<game><lots>";

    fn ids(names: &[&str]) -> Vec<String> {
        names.iter().map(|s| s.to_string()).collect()
    }

    fn save_in(dir: &Path, name: &str, body: &str) -> PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, body).unwrap();
        path
    }

    #[test]
    fn the_header_is_read_without_the_rest_of_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let body = format!("{START}{}", "<junk".repeat(100_000));
        let header = read_header(&save_in(dir.path(), "a.rws", &body)).unwrap();
        assert_eq!(header.game_version.as_deref(), Some("1.6.4871 rev600"));
        assert_eq!(header.mod_ids, ["brrainz.harmony", "ludeon.rimworld"]);
        assert_eq!(header.mod_names, ["Harmony", "Core"]);
    }

    #[test]
    fn a_header_that_ends_across_two_reads_is_still_found() {
        let dir = tempfile::tempdir().unwrap();
        let at = START.find("</meta>").unwrap();
        let padding = " ".repeat(64 * 1024 - at - 3);
        let body = START.replacen("<modIds>", &format!("{padding}<modIds>"), 1);
        let header = read_header(&save_in(dir.path(), "a.rws", &body)).unwrap();
        assert_eq!(header.mod_ids.len(), 2);
    }

    #[test]
    fn a_save_compressed_by_a_mod_is_read_too() {
        use std::io::Write;
        let dir = tempfile::tempdir().unwrap();
        let body = format!("\u{feff}{START}{}", "<junk".repeat(100_000));
        let zstd = ruzstd::encoding::compress_to_vec(
            body.as_bytes(),
            ruzstd::encoding::CompressionLevel::Fastest,
        );
        let mut gzip = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        gzip.write_all(body.as_bytes()).unwrap();
        for (name, bytes) in [("z.rws", zstd), ("g.rws", gzip.finish().unwrap())] {
            let path = dir.path().join(name);
            std::fs::write(&path, bytes).unwrap();
            let header = read_header(&path).unwrap();
            assert_eq!(
                header.mod_ids,
                ["brrainz.harmony", "ludeon.rimworld"],
                "{name}"
            );
        }
    }

    #[test]
    fn a_file_that_is_not_a_save_gives_a_reason() {
        let dir = tempfile::tempdir().unwrap();
        let reason = read_header(&save_in(dir.path(), "a.rws", "hello")).unwrap_err();
        assert_eq!(reason.to_string(), "no mod list at the start of the file");
    }

    #[test]
    fn saves_are_listed_newest_first_and_other_files_are_skipped() {
        let dir = tempfile::tempdir().unwrap();
        save_in(dir.path(), "notes.txt", "x");
        let at = |secs| UNIX_EPOCH + std::time::Duration::from_secs(secs);
        for (name, body, written) in [("older.rws", "x", 100), ("newer.rws", START, 200)] {
            let file = std::fs::File::options()
                .write(true)
                .open(save_in(dir.path(), name, body))
                .unwrap();
            file.set_modified(at(written)).unwrap();
        }
        let saves = list(dir.path());
        let names: Vec<&str> = saves.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, ["newer", "older"]);
        assert!(saves.iter().all(|s| s.modified > 0 && s.bytes > 0));
        assert!(saves.iter().any(|s| s.header.is_err()));
        assert!(list(&dir.path().join("nowhere")).is_empty());
    }

    #[test]
    fn missing_added_and_reordered_are_told_apart_and_the_same_mods_are_no_difference() {
        let d = compare(&ids(&["A.a", "b.b_steam"]), &ids(&["a.a", "b.b"]));
        assert!(d.is_none());
        let d = compare(&ids(&["a", "gone", "b"]), &ids(&["b", "new", "a"]));
        assert_eq!(d.missing, [1]);
        assert_eq!(d.added, [1]);
        assert!(d.reordered);
        let d = compare(&ids(&["a", "b"]), &ids(&["a", "new", "b"]));
        assert!(!d.reordered && d.missing.is_empty());
    }

    #[test]
    fn backing_up_keeps_only_the_newest_copies() {
        let dir = tempfile::tempdir().unwrap();
        let save = save_in(dir.path(), "colony.rws", START);
        let backups = dir.path().join("backups");
        for stamp in 1..=7 {
            back_up(&save, &backups, stamp, 5).unwrap();
        }
        let mut kept: Vec<String> = std::fs::read_dir(&backups)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        kept.sort();
        assert_eq!(kept.len(), 5);
        assert_eq!(kept[0], "000000000003-colony.rws");
        assert_eq!(
            std::fs::read_to_string(backups.join(&kept[4])).unwrap(),
            START
        );
    }

    #[test]
    fn backing_up_never_deletes_a_file_the_user_put_there() {
        let dir = tempfile::tempdir().unwrap();
        let save = save_in(dir.path(), "colony.rws", START);
        let backups = dir.path().join("backups");
        std::fs::create_dir_all(&backups).unwrap();
        for own in ["mine.rws", "0-first.rws", "123-colony.rws"] {
            std::fs::write(backups.join(own), "theirs").unwrap();
        }
        for stamp in 1..=8 {
            back_up(&save, &backups, stamp, 2).unwrap();
        }
        let mut names: Vec<String> = std::fs::read_dir(&backups)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        assert_eq!(
            names,
            [
                "0-first.rws",
                "000000000007-colony.rws",
                "000000000008-colony.rws",
                "123-colony.rws",
                "mine.rws"
            ]
        );
    }
}

use crate::base::error::{Error, Result};
use crate::said;

use roxmltree::{Document, Node};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ModsConfig {
    /// As the game writes it: `1.6.4871 rev598`.
    pub version: Option<String>,
    /// Not folded.
    pub active: Vec<String>,
    pub known_expansions: Vec<String>,
}

pub fn read(path: &Path) -> Result<ModsConfig> {
    let text = std::fs::read_to_string(path).map_err(|e| Error::file(path, e))?;
    parse(&text).map_err(|e| e.of(path.display()))
}

fn child<'a>(doc: &'a Document, name: &str) -> Option<Node<'a, 'a>> {
    doc.root_element()
        .children()
        .find(|n| n.tag_name().name().eq_ignore_ascii_case(name))
}

fn without_mark(text: &str) -> &str {
    text.strip_prefix('\u{feff}').unwrap_or(text)
}

fn document(text: &str) -> Result<Document<'_>> {
    Document::parse(text).map_err(|e| Error::format(said!("malformed XML: {}", e)))
}

fn no_list() -> Error {
    Error::format(said!("no activeMods element"))
}

fn config_in(doc: &Document) -> Result<ModsConfig> {
    let items = |node: Node| -> Vec<String> {
        node.children()
            .filter(|n| n.tag_name().name().eq_ignore_ascii_case("li"))
            .filter_map(|n| n.text())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    };
    let active = child(doc, "activeMods").ok_or_else(no_list)?;
    Ok(ModsConfig {
        version: child(doc, "version")
            .and_then(|n| n.text())
            .map(|s| s.trim().to_string()),
        active: items(active),
        known_expansions: child(doc, "knownExpansions").map(items).unwrap_or_default(),
    })
}

pub fn parse(text: &str) -> Result<ModsConfig> {
    config_in(&document(without_mark(text))?)
}

/// Everything but the active mods is kept byte for byte.
pub fn with_active(text: &str, ids: &[String]) -> Result<String> {
    let marked = text.len() - without_mark(text).len();
    let doc = document(&text[marked..])?;
    let before = config_in(&doc)?;
    // Found by name, whatever the tag's case.
    let element = child(&doc, "activeMods").ok_or_else(no_list)?.range();
    let (start, end) = (marked + element.start, marked + element.end);
    let line_end = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let line_start = text[..start].rfind('\n').map_or(0, |i| i + 1);
    let indent = &text[line_start..start];
    let indent = if indent.trim().is_empty() { indent } else { "" };
    let step = if indent.starts_with('\t') { "\t" } else { "  " };

    let mut block = String::from("<activeMods>");
    for id in ids {
        let escaped = id.replace('&', "&amp;").replace('<', "&lt;");
        block += &format!("{line_end}{indent}{step}<li>{escaped}</li>");
    }
    block += &format!("{line_end}{indent}</activeMods>");
    let changed = format!("{}{block}{}", &text[..start], &text[end..]);

    // The result must read back as the same list, with nothing else lost.
    let wanted = ModsConfig {
        active: ids.to_vec(),
        ..before
    };
    if parse(&changed)? != wanted {
        return Err(Error::refused(said!(
            "the new list would not read back the same"
        )));
    }
    Ok(changed)
}

/// Compared in lower case; an empty id is never written, so it is ignored.
pub fn same_order(a: &[String], b: &[String]) -> bool {
    let written = |list: &[String]| -> Vec<String> {
        let named = list.iter().filter(|id| !id.is_empty());
        named.map(|id| id.to_lowercase()).collect()
    };
    written(a) == written(b)
}

fn beside(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(suffix);
    path.with_file_name(name)
}
pub fn changed_outside() -> Error {
    Error::refused(said!(
        "ModsConfig.xml was changed outside RimStack since it was read"
    ))
}

/// Keeps `ModsConfig.xml.rimstack-backup` beside the file, and
/// `ModsConfig.xml.rimstack-original`, written once and never overwritten.
/// The new file is written under a name of its own and moved into place, so a
/// crash cannot leave half a file.
///
/// `expected` is the load order the caller last read from or wrote to the
/// file: when the file holds another, or changes while this runs, nothing is
/// replaced and the error is `changed_outside()`.
pub fn write(path: &Path, ids: &[String], expected: &[String]) -> Result<()> {
    let failed = |e: std::io::Error| Error::file(path, e);
    let before = std::fs::read_to_string(path).map_err(failed)?;
    if !same_order(
        &parse(&before).map_err(|e| e.of(path.display()))?.active,
        expected,
    ) {
        return Err(changed_outside());
    }
    let after = with_active(&before, ids).map_err(|e| e.of(path.display()))?;

    let original = beside(path, ".rimstack-original");
    if !original.exists() {
        crate::base::atomic_file::write(&original, &before)?;
    }
    crate::base::atomic_file::write(&beside(path, ".rimstack-backup"), &before)?;
    let real = crate::base::atomic_file::through_links(path);
    if std::fs::read_to_string(&real).map_err(failed)? != before {
        return Err(changed_outside());
    }
    let partial = crate::base::atomic_file::partial_beside(&real);
    crate::base::atomic_file::put(&partial, &real, after.as_bytes()).map_err(failed)
}

/// `None` unless it starts with two numbers.
pub fn short_version(version: &str) -> Option<String> {
    let mut parts = version.trim().split('.');
    let major: u32 = parts.next()?.parse().ok()?;
    let minor: u32 = parts
        .next()?
        .split(|c: char| !c.is_ascii_digit())
        .next()?
        .parse()
        .ok()?;
    Some(format!("{major}.{minor}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::base::error::Kind;

    const TYPICAL: &str = r#"<?xml version="1.0" ?>
<ModsConfigData>
  <version>1.6.4871 rev598</version>
  <activeMods>
    <li>brrainz.harmony</li>
    <li>ludeon.rimworld</li>
    <li>ludeon.rimworld.royalty</li>
  </activeMods>
  <knownExpansions>
    <li>ludeon.rimworld.royalty</li>
  </knownExpansions>
</ModsConfigData>"#;

    #[test]
    fn active_mods_keep_their_order_and_exclude_expansions() {
        let c = parse(TYPICAL).unwrap();
        assert_eq!(c.version.as_deref(), Some("1.6.4871 rev598"));
        assert_eq!(
            c.active,
            [
                "brrainz.harmony",
                "ludeon.rimworld",
                "ludeon.rimworld.royalty"
            ]
        );
        assert_eq!(c.known_expansions, ["ludeon.rimworld.royalty"]);
    }

    #[test]
    fn empty_and_missing_lists() {
        let c = parse("<ModsConfigData><activeMods/></ModsConfigData>").unwrap();
        assert!(c.active.is_empty() && c.known_expansions.is_empty());
        let no_list = parse("<ModsConfigData/>").unwrap_err();
        assert_eq!(no_list.to_string(), "no activeMods element");
        assert_eq!(no_list.kind(), Kind::Format);
        assert!(parse("<ModsConfigData>").is_err());
    }

    fn ids(names: &[&str]) -> Vec<String> {
        names.iter().map(|s| s.to_string()).collect()
    }

    fn on_disk(path: &Path) -> Vec<String> {
        read(path).unwrap().active
    }

    #[test]
    fn a_new_list_changes_only_the_active_mods() {
        let list = ids(&["ludeon.rimworld", "a.b"]);
        let changed = with_active(TYPICAL, &list).unwrap();
        assert_eq!(
            changed,
            TYPICAL.replace(
                "    <li>brrainz.harmony</li>\n    <li>ludeon.rimworld</li>\n    <li>ludeon.rimworld.royalty</li>",
                "    <li>ludeon.rimworld</li>\n    <li>a.b</li>"
            )
        );
    }

    #[test]
    fn tabs_windows_line_ends_and_a_byte_order_mark_are_kept() {
        let text = "\u{feff}<ModsConfigData>\r\n\t<activeMods>\r\n\t\t<li>a.a</li>\r\n\t</activeMods>\r\n</ModsConfigData>";
        let changed = with_active(text, &ids(&["b.b", "c.c"])).unwrap();
        assert_eq!(
            changed,
            "\u{feff}<ModsConfigData>\r\n\t<activeMods>\r\n\t\t<li>b.b</li>\r\n\t\t<li>c.c</li>\r\n\t</activeMods>\r\n</ModsConfigData>"
        );
    }

    #[test]
    fn an_empty_self_closing_list_can_be_filled_and_a_list_emptied() {
        let filled = with_active(
            "<ModsConfigData><activeMods /></ModsConfigData>",
            &ids(&["a.a"]),
        );
        assert_eq!(parse(&filled.unwrap()).unwrap().active, ["a.a"]);
        let emptied = with_active(TYPICAL, &[]).unwrap();
        assert!(parse(&emptied).unwrap().active.is_empty());
        assert_eq!(
            parse(&emptied).unwrap().known_expansions,
            ["ludeon.rimworld.royalty"]
        );
    }

    #[test]
    fn tags_written_another_way_are_found_and_nothing_beside_them_is_cut() {
        let text = "<ModsConfigData>\n  <ActiveMods >\n    <li>a.a</li>\n  </ActiveMods>\n  \
                    <knownExpansions>\n    <li>x.y</li>\n  </knownExpansions>\n  <extra/>\n</ModsConfigData>";
        let changed = with_active(text, &ids(&["b.b"])).unwrap();
        let read = parse(&changed).unwrap();
        assert_eq!(read.active, ["b.b"]);
        assert_eq!(read.known_expansions, ["x.y"]);
        assert!(changed.contains("<extra/>"));
    }

    #[test]
    fn an_id_the_file_cannot_hold_as_written_or_a_file_without_the_list_is_refused() {
        // The reader trims: this id would come back as another.
        let refused = with_active(TYPICAL, &ids(&[" a.a "])).unwrap_err();
        assert_eq!(refused.kind(), Kind::Refused);
        assert!(with_active("<ModsConfigData/>", &ids(&["a.a"])).is_err());
    }

    #[test]
    fn writing_keeps_the_first_file_and_the_one_before() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ModsConfig.xml");
        std::fs::write(&path, TYPICAL).unwrap();

        write(&path, &ids(&["a.a"]), &on_disk(&path)).unwrap();
        assert_eq!(read(&path).unwrap().active, ["a.a"]);
        let first = std::fs::read_to_string(&path).unwrap();
        write(&path, &ids(&["b.b"]), &on_disk(&path)).unwrap();

        assert_eq!(read(&path).unwrap().active, ["b.b"]);
        let kept = |name: &str| std::fs::read_to_string(dir.path().join(name)).unwrap();
        assert_eq!(kept("ModsConfig.xml.rimstack-original"), TYPICAL);
        assert_eq!(kept("ModsConfig.xml.rimstack-backup"), first);
        assert!(!dir.path().join("ModsConfig.xml.rimstack-part").exists());
    }

    #[test]
    fn a_save_does_not_touch_the_partial_file_of_another_save() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ModsConfig.xml");
        std::fs::write(&path, TYPICAL).unwrap();
        let other = dir.path().join("ModsConfig.xml.rimstack-part");
        std::fs::write(&other, "half of another save").unwrap();

        write(&path, &ids(&["a.a"]), &on_disk(&path)).unwrap();

        assert_eq!(read(&path).unwrap().active, ["a.a"]);
        assert_eq!(
            std::fs::read_to_string(&other).unwrap(),
            "half of another save"
        );
    }

    #[test]
    fn a_file_changed_since_it_was_read_is_refused_and_left_as_it_is() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ModsConfig.xml");
        std::fs::write(&path, TYPICAL).unwrap();
        let read_by_us = on_disk(&path);
        let by_the_game = TYPICAL.replace("<li>brrainz.harmony</li>", "<li>other.mod</li>");
        std::fs::write(&path, &by_the_game).unwrap();

        let refused = write(&path, &ids(&["a.a"]), &read_by_us).unwrap_err();

        assert_eq!(refused, changed_outside());
        assert_eq!(refused.kind(), Kind::Refused);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), by_the_game);
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[test]
    fn a_file_in_the_order_it_was_read_in_is_written_whatever_the_case() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ModsConfig.xml");
        std::fs::write(&path, TYPICAL).unwrap();
        let upper = ids(&[
            "Brrainz.Harmony",
            "LUDEON.rimworld",
            "",
            "ludeon.rimworld.royalty",
        ]);
        write(&path, &ids(&["a.a"]), &upper).unwrap();
        assert_eq!(on_disk(&path), ["a.a"]);
    }

    #[test]
    fn writing_to_a_missing_file_fails_and_creates_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ModsConfig.xml");
        let missing = write(&path, &ids(&["a.a"]), &[]).unwrap_err();
        assert_eq!(missing.kind(), Kind::File);
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
    }

    #[test]
    fn two_lists_are_one_order_whatever_the_case_and_without_unreadable_mods() {
        assert!(same_order(&ids(&["A.b", "", "c.d"]), &ids(&["a.b", "C.D"])));
        assert!(!same_order(&ids(&["a.b", "c.d"]), &ids(&["c.d", "a.b"])));
        assert!(same_order(&[], &ids(&[""])));
    }

    #[test]
    fn short_version_takes_two_numbers() {
        assert_eq!(short_version("1.6.4871 rev598").as_deref(), Some("1.6"));
        assert_eq!(short_version("1.5").as_deref(), Some("1.5"));
        assert_eq!(short_version("garbage"), None);
        assert_eq!(short_version("1"), None);
    }

    #[cfg(unix)]
    #[test]
    fn saving_writes_through_a_linked_file_and_keeps_its_permissions() {
        use std::os::unix::fs::PermissionsExt;
        let tmp = tempfile::tempdir().unwrap();
        let real = tmp.path().join("dotfiles/ModsConfig.xml");
        std::fs::create_dir_all(real.parent().unwrap()).unwrap();
        std::fs::write(&real, TYPICAL).unwrap();
        std::fs::set_permissions(&real, std::fs::Permissions::from_mode(0o600)).unwrap();
        let link = tmp.path().join("ModsConfig.xml");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        write(&link, &ids(&["a.b"]), &on_disk(&link)).unwrap();
        assert!(
            std::fs::symlink_metadata(&link)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        let mode = std::fs::metadata(&link).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
        assert_eq!(read(&real).unwrap().active, ["a.b"]);
    }

    #[test]
    fn a_read_only_file_is_saved_and_stays_read_only() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("ModsConfig.xml");
        std::fs::write(&path, TYPICAL).unwrap();
        let mut locked = std::fs::metadata(&path).unwrap().permissions();
        locked.set_readonly(true);
        std::fs::set_permissions(&path, locked).unwrap();
        write(&path, &ids(&["a.b"]), &on_disk(&path)).unwrap();
        assert_eq!(read(&path).unwrap().active, ["a.b"]);
        assert!(std::fs::metadata(&path).unwrap().permissions().readonly());
        assert_eq!(
            std::fs::read_to_string(beside(&path, ".rimstack-original")).unwrap(),
            TYPICAL
        );
        let left: Vec<_> = std::fs::read_dir(tmp.path())
            .unwrap()
            .flatten()
            .filter(|entry| entry.file_name().to_string_lossy().ends_with("part"))
            .collect();
        assert!(left.is_empty());
    }
}

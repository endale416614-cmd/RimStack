use crate::base::error::{Error, Result};
use crate::said;

use roxmltree::{Document, Node};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Versioned {
    pub base: Vec<String>,
    /// Keyed by the version of the XML tag without its `v`: `v1.6` is `1.6`.
    pub by_version: BTreeMap<String, Vec<String>>,
}

impl Versioned {
    /// A by-version list replaces the base list, as in the game.
    pub fn for_version(&self, version: &str) -> &[String] {
        self.by_version.get(version).unwrap_or(&self.base)
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct About {
    /// As the author wrote it; compare through `id::fold`.
    pub package_id: String,
    pub name: Option<String>,
    pub authors: Vec<String>,
    pub mod_version: Option<String>,
    pub description: Option<String>,
    pub url: Option<String>,
    pub supported_versions: Vec<String>,
    pub dependencies: Versioned,
    /// Keyed by folded id of the required mod.
    pub alternatives: BTreeMap<String, Vec<String>>,
    pub load_before: Versioned,
    pub load_after: Versioned,
    pub force_load_before: Vec<String>,
    pub force_load_after: Vec<String>,
    pub incompatible_with: Versioned,
}

impl About {
    /// The dependency itself first, then its alternatives.
    pub fn accepted_as<'a>(&'a self, dependency: &'a str) -> impl Iterator<Item = &'a str> {
        let others = self
            .alternatives
            .get(&crate::rimworld::id::fold(dependency));
        std::iter::once(dependency).chain(others.into_iter().flatten().map(String::as_str))
    }
}

pub fn locate(mod_root: &Path) -> Option<PathBuf> {
    let exact = mod_root.join("About").join("About.xml");
    if exact.is_file() {
        return Some(exact);
    }
    let dir = child_ignoring_case(mod_root, "about")?;
    child_ignoring_case(&dir, "about.xml").filter(|p| p.is_file())
}

/// `About/Preview.png`, or a JPEG of that name.
pub fn preview(mod_root: &Path) -> Option<PathBuf> {
    let dir = child_ignoring_case(mod_root, "about")?;
    ["preview.png", "preview.jpg", "preview.jpeg"]
        .iter()
        .find_map(|name| child_ignoring_case(&dir, name).filter(|p| p.is_file()))
}

fn child_ignoring_case(dir: &Path, lower_name: &str) -> Option<PathBuf> {
    std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .find(|e| e.file_name().to_string_lossy().to_lowercase() == lower_name)
        .map(|e| e.path())
}

pub fn parse(bytes: &[u8]) -> Result<About> {
    let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
    let text = std::str::from_utf8(bytes).map_err(|_| Error::format(said!("not UTF-8")))?;
    let doc = Document::parse(text).map_err(|e| Error::format(said!("malformed XML: {}", e)))?;

    let mut about = About::default();
    // Direct children only: a dependency's <packageId> is not the mod's own.
    for node in doc.root_element().children().filter(Node::is_element) {
        match node.tag_name().name().to_ascii_lowercase().as_str() {
            "packageid" => about.package_id = text_of(node),
            "name" => about.name = Some(text_of(node)).filter(|s| !s.is_empty()),
            "modversion" => about.mod_version = Some(text_of(node)).filter(|s| !s.is_empty()),
            // Authors write line breaks out as `\n`, as the game reads them.
            "description" => {
                let text = slashed_breaks(&text_of(node).replace("\\n", "\n"));
                about.description = Some(text).filter(|s| !s.is_empty())
            }
            "url" => about.url = Some(text_of(node)).filter(|s| !s.is_empty()),
            "author" => about.authors.extend(split_names(&text_of(node))),
            "authors" => about.authors.extend(items(node)),
            "supportedversions" => about.supported_versions = items(node),
            "moddependencies" => {
                about.dependencies.base = dependency_ids(node);
                read_alternatives(node, &mut about.alternatives);
            }
            "moddependenciesbyversion" => {
                about.dependencies.by_version = by_version(node, dependency_ids);
                for version in node.children().filter(Node::is_element) {
                    read_alternatives(version, &mut about.alternatives);
                }
            }
            "loadbefore" => about.load_before.base = items(node),
            "loadbeforebyversion" => about.load_before.by_version = by_version(node, items),
            "loadafter" => about.load_after.base = items(node),
            "loadafterbyversion" => about.load_after.by_version = by_version(node, items),
            "forceloadbefore" => about.force_load_before = items(node),
            "forceloadafter" => about.force_load_after = items(node),
            "incompatiblewith" => about.incompatible_with.base = items(node),
            "incompatiblewithbyversion" => {
                about.incompatible_with.by_version = by_version(node, items)
            }
            _ => {}
        }
    }
    if about.package_id.is_empty() {
        return Err(Error::format(said!("no packageId")));
    }
    Ok(about)
}

fn text_of(node: Node) -> String {
    node.text().unwrap_or("").trim().to_string()
}

fn split_names(text: &str) -> impl Iterator<Item = String> + '_ {
    text.split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn list_items<'a>(node: Node<'a, 'a>) -> impl Iterator<Item = Node<'a, 'a>> {
    node.children()
        .filter(|n| n.is_element() && n.tag_name().name().eq_ignore_ascii_case("li"))
}

fn items(node: Node) -> Vec<String> {
    list_items(node)
        .map(text_of)
        .filter(|s| !s.is_empty())
        .collect()
}

fn dependency_ids(node: Node) -> Vec<String> {
    list_items(node)
        .filter_map(|li| {
            li.children()
                .find(|n| n.tag_name().name().eq_ignore_ascii_case("packageId"))
        })
        .map(text_of)
        .filter(|s| !s.is_empty())
        .collect()
}

fn child<'a>(node: Node<'a, 'a>, name: &str) -> Option<Node<'a, 'a>> {
    node.children()
        .find(|n| n.tag_name().name().eq_ignore_ascii_case(name))
}

fn read_alternatives(node: Node, into: &mut BTreeMap<String, Vec<String>>) {
    for li in list_items(node) {
        let (Some(id), Some(others)) = (child(li, "packageId"), child(li, "alternativePackageIds"))
        else {
            continue;
        };
        let others = items(others);
        if !others.is_empty() {
            into.entry(crate::rimworld::id::fold(&text_of(id)))
                .or_default()
                .extend(others);
        }
    }
}

fn by_version(node: Node, read: fn(Node) -> Vec<String>) -> BTreeMap<String, Vec<String>> {
    node.children()
        .filter(Node::is_element)
        .map(|v| {
            let tag = v.tag_name().name();
            (tag.strip_prefix('v').unwrap_or(tag).to_string(), read(v))
        })
        .collect()
}

/// Authors type a line break as `/n`; one inside a word, a link or a path stays as written.
fn slashed_breaks(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find("/n") {
        out.push_str(&rest[..at]);
        let after = &rest[at + 2..];
        let ends = out.chars().last().is_none_or(|c| ".!?:;)\n".contains(c));
        let goes_on = after
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit());
        out.push_str(if ends && !goes_on { "\n" } else { "/n" });
        rest = after;
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_break_mistyped_with_a_slash_is_a_break() {
        assert_eq!(
            slashed_breaks("comes to fruition./n/nDisclimer: none."),
            "comes to fruition.\n\nDisclimer: none."
        );
        assert_eq!(
            slashed_breaks("this and/nor that, see https://a.b/news"),
            "this and/nor that, see https://a.b/news"
        );
        assert_eq!(
            slashed_breaks("Run ./notes.sh first"),
            "Run ./notes.sh first"
        );
    }

    #[test]
    fn a_dependency_s_alternative_ids_are_read() {
        let about = parse(
            br#"<ModMetaData><packageId>a.a</packageId>
<modDependencies><li>
  <packageId>Erdelf.HumanoidAlienRaces</packageId>
  <alternativePackageIds IgnoreIfNoMatchingField="True"><li>erdelf.HumanoidAlienRaces.dev</li></alternativePackageIds>
</li></modDependencies>
<modDependenciesByVersion><v1.6><li>
  <packageId>b.b</packageId><alternativePackageIds><li>b.fork</li></alternativePackageIds>
</li></v1.6></modDependenciesByVersion>
</ModMetaData>"#,
        )
        .unwrap();
        let accepted: Vec<&str> = about.accepted_as("erdelf.humanoidalienraces").collect();
        assert_eq!(
            accepted,
            ["erdelf.humanoidalienraces", "erdelf.HumanoidAlienRaces.dev"]
        );
        assert_eq!(
            about.accepted_as("B.B").collect::<Vec<_>>(),
            ["B.B", "b.fork"]
        );
        assert_eq!(about.accepted_as("c.c").collect::<Vec<_>>(), ["c.c"]);
    }

    const FULL: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<ModMetaData>
  <modDependencies>
    <li><packageId>Dep.First</packageId><displayName>First</displayName></li>
  </modDependencies>
  <packageId> Author.Full </packageId>
  <name>Full Mod</name>
  <author>Ann, Bob</author>
  <authors><li>Cy</li></authors>
  <modVersion>2.3.1</modVersion>
  <description>Does things.
Second line.</description>
  <url>https://example.org</url>
  <supportedVersions><li>1.5</li><li>1.6</li><li> </li></supportedVersions>
  <modDependenciesByVersion>
    <v1.6><li><packageId>Dep.New</packageId></li></v1.6>
  </modDependenciesByVersion>
  <loadBefore><li>Other.Mod</li></loadBefore>
  <loadAfter><li>Ludeon.RimWorld</li></loadAfter>
  <loadAfterByVersion><v1.5><li>Old.Thing</li></v1.5></loadAfterByVersion>
  <forceLoadBefore><li>A.B</li></forceLoadBefore>
  <forceLoadAfter><li>C.D</li></forceLoadAfter>
  <incompatibleWith><li>Bad.Mod</li></incompatibleWith>
</ModMetaData>"#;

    #[test]
    fn every_field_is_read() {
        let a = parse(FULL.as_bytes()).unwrap();
        assert_eq!(a.package_id, "Author.Full");
        assert_eq!(a.dependencies.base, ["Dep.First"]);
        assert_eq!(a.name.as_deref(), Some("Full Mod"));
        assert_eq!(a.authors, ["Ann", "Bob", "Cy"]);
        assert_eq!(a.mod_version.as_deref(), Some("2.3.1"));
        assert_eq!(a.description.as_deref(), Some("Does things.\nSecond line."));
        assert_eq!(a.url.as_deref(), Some("https://example.org"));
        assert_eq!(a.supported_versions, ["1.5", "1.6"]);
        assert_eq!(a.load_before.base, ["Other.Mod"]);
        assert_eq!(a.force_load_before, ["A.B"]);
        assert_eq!(a.force_load_after, ["C.D"]);
        assert_eq!(a.incompatible_with.base, ["Bad.Mod"]);
    }

    #[test]
    fn by_version_list_replaces_the_base_list() {
        let a = parse(FULL.as_bytes()).unwrap();
        assert_eq!(a.dependencies.for_version("1.6"), ["Dep.New"]);
        assert_eq!(a.dependencies.for_version("1.5"), ["Dep.First"]);
        assert_eq!(a.load_after.for_version("1.5"), ["Old.Thing"]);
        assert_eq!(a.load_after.for_version("1.6"), ["Ludeon.RimWorld"]);
    }

    #[test]
    fn byte_order_mark_missing_declaration_and_tag_case_are_fine() {
        let mut bytes = vec![0xEF, 0xBB, 0xBF];
        bytes.extend_from_slice(b"<ModMetaData><packageId>a.b</packageId></ModMetaData>");
        assert_eq!(parse(&bytes).unwrap().package_id, "a.b");
        let a = parse(b"<ModMetaData><PackageID>a.b</PackageID><Name>N</Name></ModMetaData>");
        assert_eq!(a.unwrap().name.as_deref(), Some("N"));
    }

    #[test]
    fn problems_are_reported_not_fatal() {
        let why = |bytes: &[u8]| parse(bytes).unwrap_err().to_string();
        assert_eq!(
            why(b"<ModMetaData><name>x</name></ModMetaData>"),
            "no packageId"
        );
        assert!(why(b"<ModMetaData><name>x</ModMetaData>").starts_with("malformed XML"));
        assert_eq!(why(b"<a>\xE9</a>"), "not UTF-8");
        assert_eq!(why(&[0xFF, 0xFE, b'<', 0]), "not UTF-8");
        let unread = parse(b"").unwrap_err();
        assert_eq!(unread.kind(), crate::base::error::Kind::Format);
    }

    #[test]
    fn about_is_located_whatever_its_case() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(locate(dir.path()), None);
        std::fs::create_dir(dir.path().join("about")).unwrap();
        std::fs::write(dir.path().join("about/ABOUT.XML"), "").unwrap();
        assert_eq!(locate(dir.path()), Some(dir.path().join("about/ABOUT.XML")));
    }

    #[test]
    fn a_line_break_written_out_as_backslash_n_is_a_line_break() {
        let xml = r"<ModMetaData><packageId>a.b</packageId><description>Version 1\n\nDraws a HUD.</description></ModMetaData>";
        let about = parse(xml.as_bytes()).unwrap();
        assert_eq!(
            about.description.as_deref(),
            Some("Version 1\n\nDraws a HUD.")
        );
    }

    #[test]
    fn the_picture_is_found_whatever_its_letter_case() {
        let tmp = tempfile::tempdir().unwrap();
        let about = tmp.path().join("about");
        std::fs::create_dir(&about).unwrap();
        assert_eq!(preview(tmp.path()), None);
        std::fs::write(about.join("preview.jpg"), b"").unwrap();
        assert_eq!(preview(tmp.path()), Some(about.join("preview.jpg")));
        std::fs::write(about.join("PREVIEW.png"), b"").unwrap();
        std::fs::write(about.join("Preview2.png"), b"").unwrap();
        assert_eq!(preview(tmp.path()), Some(about.join("PREVIEW.png")));
    }
}

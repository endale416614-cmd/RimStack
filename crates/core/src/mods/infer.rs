//! Files are searched as text, not parsed: one broken file must not stop the rest.

use crate::base::sentence::Sentence;
use crate::say;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Scanned {
    pub defines: HashSet<String>,
    pub bases: HashSet<String>,
    pub parents: Vec<(String, PathBuf)>,
    pub patched: Vec<(String, PathBuf)>,
    pub asks_for: Vec<(String, PathBuf)>,
    pub assemblies: HashSet<String>,
    pub uses: Vec<(String, PathBuf)>,
    pub types: HashSet<String>,
    pub patches: Vec<(String, PathBuf)>,
}

fn dlls_of(load_folders: &[PathBuf]) -> Vec<PathBuf> {
    let mut found = Vec::new();
    for folder in load_folders {
        let Some(dir) = subfolder(folder, "Assemblies") else {
            continue;
        };
        for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
            let path = entry.path();
            if path
                .extension()
                .is_some_and(|x| x.eq_ignore_ascii_case("dll"))
            {
                found.push(path);
            }
        }
    }
    found
}

fn xml_files(dir: &Path, into: &mut Vec<PathBuf>) {
    let is_xml = |file: &PathBuf| {
        let kind = file.extension();
        kind.is_some_and(|x| x.eq_ignore_ascii_case("xml"))
    };
    into.extend(crate::base::walk::files(dir).into_iter().filter(is_xml));
}

fn subfolder(folder: &Path, name: &str) -> Option<PathBuf> {
    let entries = std::fs::read_dir(folder).ok()?;
    let found = entries
        .flatten()
        .find(|e| e.file_name().eq_ignore_ascii_case(name));
    found.map(|e| e.path()).filter(|p| p.is_dir())
}

fn between<'a>(text: &'a str, open: &'a str, close: &'a str) -> impl Iterator<Item = &'a str> {
    let mut rest = text;
    std::iter::from_fn(move || {
        let start = rest.find(open)? + open.len();
        let length = rest[start..].find(close)?;
        let found = &rest[start..start + length];
        rest = &rest[start + length + close.len()..];
        Some(found)
    })
}

fn quoted_after<'a>(text: &'a str, key: &'a str) -> impl Iterator<Item = &'a str> {
    let mut rest = text;
    std::iter::from_fn(move || {
        loop {
            let at = rest.find(key)? + key.len();
            rest = &rest[at..];
            let after = rest.trim_start();
            let Some(after) = after.strip_prefix('=') else {
                continue;
            };
            let after = after.trim_start();
            let Some(quote) = after.chars().next().filter(|c| *c == '"' || *c == '\'') else {
                continue;
            };
            let Some(end) = after[1..].find(quote) else {
                continue;
            };
            return Some(&after[1..1 + end]);
        }
    })
}

fn is_name(s: &str) -> bool {
    !s.is_empty() && s.len() < 120 && s.chars().all(|c| c.is_alphanumeric() || "_-.".contains(c))
}

pub fn scan(load_folders: &[PathBuf]) -> Scanned {
    let mut found = Scanned::default();
    for folder in load_folders {
        let mut files = Vec::new();
        if let Some(defs) = subfolder(folder, "Defs") {
            xml_files(&defs, &mut files);
        }
        for file in files.drain(..) {
            let Ok(text) = std::fs::read_to_string(&file) else {
                continue;
            };
            found.defines.extend(
                between(&text, "<defName>", "</defName>")
                    .map(str::trim)
                    .filter(|s| is_name(s))
                    .map(String::from),
            );
            // `ParentName=` also ends in `Name=`; only a bare ` Name=` is a base.
            found.bases.extend(
                quoted_after(&text, " Name")
                    .filter(|s| is_name(s))
                    .map(String::from),
            );
            let parents = quoted_after(&text, "ParentName").filter(|s| is_name(s));
            found
                .parents
                .extend(parents.map(|p| (p.to_string(), file.clone())));
        }
        if let Some(patches) = subfolder(folder, "Patches") {
            xml_files(&patches, &mut files);
        }
        for file in files.drain(..) {
            let Ok(text) = std::fs::read_to_string(&file) else {
                continue;
            };
            // Selections are written inside <xpath>, with entities for quotes.
            for xpath in between(&text, "<xpath>", "</xpath>") {
                let xpath = xpath.replace("&quot;", "\"").replace("&apos;", "'");
                let names = quoted_after(&xpath, "defName").chain(quoted_after(&xpath, "text()"));
                let names: Vec<String> = names.filter(|s| is_name(s)).map(String::from).collect();
                found
                    .patched
                    .extend(names.into_iter().map(|n| (n, file.clone())));
            }
            for mods in between(&text, "<mods>", "</mods>") {
                let asked = between(mods, "<li>", "</li>")
                    .map(str::trim)
                    .filter(|s| !s.is_empty());
                found
                    .asks_for
                    .extend(asked.map(|m| (m.to_string(), file.clone())));
            }
        }
    }
    for dll in dlls_of(load_folders) {
        let stem = dll
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_lowercase();
        found.assemblies.insert(stem);
        let Ok(bytes) = std::fs::read(&dll) else {
            continue;
        };
        let Some(assembly) = crate::rimworld::dotnet::Assembly::read(&bytes) else {
            continue;
        };
        for name in assembly.references() {
            found.uses.push((name.to_lowercase(), dll.clone()));
        }
        found.types.extend(assembly.type_names());
        let aimed: HashSet<String> = assembly
            .patches()
            .into_iter()
            .chain(assembly.hand_applied())
            .map(|p| p.target)
            .filter(|t| !t.is_empty())
            .collect();
        found
            .patches
            .extend(aimed.into_iter().map(|t| (t, dll.clone())));
    }
    let own_assemblies = found.assemblies.clone();
    found
        .uses
        .retain(|(name, _)| !own_assemblies.contains(name));
    let own = found.bases.clone();
    found.parents.retain(|(parent, _)| !own.contains(parent));
    found
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Inferred {
    pub first: usize,
    pub then: usize,
    pub why: Sentence,
    /// From compiled code or a parent def, the game needs this order; from patches, only a tidy order.
    pub code: bool,
}

pub fn infer(
    scanned: &[&Scanned],
    names: &[String],
    skip: &[bool],
    roots: &[PathBuf],
) -> Vec<Inferred> {
    let mut defined_by: HashMap<&str, Vec<usize>> = HashMap::new();
    let mut base_of: HashMap<&str, Vec<usize>> = HashMap::new();
    for (m, s) in scanned.iter().enumerate() {
        for name in &s.defines {
            defined_by.entry(name).or_default().push(m);
        }
        for name in &s.bases {
            base_of.entry(name).or_default().push(m);
        }
    }
    let mut assembly_of: HashMap<&str, Vec<usize>> = HashMap::new();
    for (m, s) in scanned.iter().enumerate() {
        for name in &s.assemblies {
            assembly_of.entry(name).or_default().push(m);
        }
    }
    let mut type_of: HashMap<&str, Vec<usize>> = HashMap::new();
    for (m, s) in scanned.iter().enumerate() {
        for name in &s.types {
            type_of.entry(name).or_default().push(m);
        }
    }
    let by_name: HashMap<String, usize> = names
        .iter()
        .enumerate()
        .map(|(i, n)| (n.to_lowercase(), i))
        .collect();
    let only = |owners: Option<&Vec<usize>>, me: usize| -> Option<usize> {
        match owners?.as_slice() {
            [one] if *one != me && !skip[*one] => Some(*one),
            _ => None,
        }
    };
    let shown = |file: &Path, m: usize| {
        file.strip_prefix(&roots[m])
            .unwrap_or(file)
            .to_string_lossy()
            .into_owned()
    };

    let mut rules: Vec<Inferred> = Vec::new();
    let mut seen: HashSet<(usize, usize)> = HashSet::new();
    let mut add = |first: usize, then: usize, why: Sentence, code: bool| {
        if seen.insert((first, then)) {
            rules.push(Inferred {
                first,
                then,
                why,
                code,
            });
        }
    };
    for (me, s) in scanned.iter().enumerate() {
        // The game checks an assembly's types as it loads it: what it uses must already be loaded.
        for (name, dll) in &s.uses {
            if let Some(other) = only(assembly_of.get(name.as_str()), me) {
                add(other, me, say!("{} uses its code", shown(dll, me)), true);
            }
        }
        for (target, dll) in &s.patches {
            if let Some(other) = only(type_of.get(target.as_str()), me) {
                add(other, me, say!("{} patches its code", shown(dll, me)), true);
            }
        }
        // The game only takes a parent from the def's own mod or an earlier one.
        for (parent, file) in &s.parents {
            if let Some(other) = only(base_of.get(parent.as_str()), me) {
                add(
                    other,
                    me,
                    say!("{} builds on its {}", shown(file, me), parent),
                    true,
                );
            }
        }
        for (asked, file) in &s.asks_for {
            if let Some(&other) = by_name
                .get(&asked.to_lowercase())
                .filter(|o| **o != me && !skip[**o])
            {
                add(
                    other,
                    me,
                    say!("{} has patches for it", shown(file, me)),
                    false,
                );
            }
        }
        for (def, file) in &s.patched {
            if let Some(other) = only(defined_by.get(def.as_str()), me) {
                add(
                    other,
                    me,
                    say!("{} patches its {}", shown(file, me), def),
                    false,
                );
            }
        }
    }
    rules
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn a_mod(files: &[(&str, &str)]) -> (tempfile::TempDir, PathBuf) {
        let tmp = tempfile::tempdir().unwrap();
        for (file, text) in files {
            let path = tmp.path().join(file);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, text).unwrap();
        }
        let root = tmp.path().to_path_buf();
        (tmp, root)
    }

    const DEFS: &str = r#"<Defs>
        <ThingDef Name="ChairBase" Abstract="True"><thingClass>Building</thingClass></ThingDef>
        <ThingDef ParentName="ChairBase"><defName>Oak_Chair</defName></ThingDef>
        <ThingDef ParentName = 'BuildingBase'><defName> Oak_Table </defName></ThingDef>
    </Defs>"#;

    const PATCH: &str = r#"<Patch>
        <Operation Class="PatchOperationFindMod">
            <mods><li>Big Furniture</li><li> Other Mod </li></mods>
            <match Class="PatchOperationAdd">
                <xpath>Defs/ThingDef[defName="Big_Bed"]/comps</xpath>
            </match>
        </Operation>
        <Operation Class="PatchOperationReplace">
            <xpath>/Defs/ThingDef[defName = 'Steel' or defName=&quot;Plasteel&quot;]/statBases</xpath>
        </Operation>
        <Operation Class="PatchOperationAdd"><xpath>Defs/RecipeDef[defName[text()="Make_Thing"]]</xpath></Operation>
    </Patch>"#;

    #[test]
    fn defs_and_patches_are_read_as_text() {
        let (_t, root) = a_mod(&[
            ("Defs/Things/chairs.xml", DEFS),
            ("1.6/Patches/p.xml", PATCH),
            ("About/About.xml", "<x Name=\"NotADef\"/>"),
        ]);
        let found = scan(&[root.clone(), root.join("1.6")]);
        assert_eq!(
            found.defines,
            ["Oak_Chair", "Oak_Table"].map(String::from).into()
        );
        assert_eq!(found.bases, ["ChairBase".to_string()].into());
        let parents: Vec<&str> = found.parents.iter().map(|(p, _)| p.as_str()).collect();
        assert_eq!(parents, ["BuildingBase"]);
        let mut patched: Vec<&str> = found.patched.iter().map(|(p, _)| p.as_str()).collect();
        patched.sort_unstable();
        assert_eq!(patched, ["Big_Bed", "Make_Thing", "Plasteel", "Steel"]);
        let asked: Vec<&str> = found.asks_for.iter().map(|(p, _)| p.as_str()).collect();
        assert_eq!(asked, ["Big Furniture", "Other Mod"]);
    }

    fn scanned(
        defines: &[&str],
        bases: &[&str],
        parents: &[&str],
        patched: &[&str],
        asks: &[&str],
    ) -> Scanned {
        let set = |list: &[&str]| list.iter().map(|s| s.to_string()).collect();
        let filed = |list: &[&str]| {
            list.iter()
                .map(|s| (s.to_string(), PathBuf::from("/m/Patches/p.xml")))
                .collect()
        };
        Scanned {
            defines: set(defines),
            bases: set(bases),
            parents: filed(parents),
            patched: filed(patched),
            asks_for: filed(asks),
            ..Scanned::default()
        }
    }

    #[test]
    fn a_mod_loads_after_the_one_whose_defs_it_points_at() {
        let mods = [
            scanned(&["Big_Bed"], &["BedBase"], &[], &[], &[]),
            scanned(&[], &[], &["BedBase"], &["Big_Bed", "Big_Bed"], &[]),
            scanned(&[], &[], &[], &[], &["big furniture"]),
        ];
        let names = ["Big Furniture", "Patch", "Compat"].map(String::from);
        let roots = vec![PathBuf::from("/m"); 3];
        let found = infer(
            &mods.iter().collect::<Vec<_>>(),
            &names,
            &[false; 3],
            &roots,
        );
        let pairs: Vec<(usize, usize)> = found.iter().map(|r| (r.first, r.then)).collect();
        assert_eq!(pairs, [(0, 1), (0, 2)]);
        assert_eq!(
            found[0].why.to_string(),
            "Patches/p.xml builds on its BedBase"
        );
        assert_eq!(found[1].why.to_string(), "Patches/p.xml has patches for it");
        assert!(found[0].code && !found[1].code);
    }

    #[test]
    fn a_mod_loads_after_the_one_whose_code_it_uses() {
        let with = |own: &[&str], uses: &[&str]| Scanned {
            assemblies: own.iter().map(|s| s.to_string()).collect(),
            uses: uses
                .iter()
                .map(|s| (s.to_string(), PathBuf::from("/m/Assemblies/Mine.dll")))
                .collect(),
            ..Scanned::default()
        };
        let mods = [
            with(&["biglib"], &[]),
            with(&["mine"], &["biglib", "system", "shared"]),
            with(&["shared"], &[]),
            with(&["shared", "other"], &["biglib"]),
        ];
        let names = ["Big Lib", "Mine", "A", "B"].map(String::from);
        let roots = vec![PathBuf::from("/m"); 4];
        let found = infer(
            &mods.iter().collect::<Vec<_>>(),
            &names,
            &[false; 4],
            &roots,
        );
        let pairs: Vec<(usize, usize)> = found.iter().map(|r| (r.first, r.then)).collect();
        assert_eq!(pairs, [(0, 1), (0, 3)]);
        assert_eq!(
            found[0].why.to_string(),
            "Assemblies/Mine.dll uses its code"
        );
        assert!(found.iter().all(|r| r.code));
    }

    fn patching(own: &[&str], targets: &[&str]) -> Scanned {
        Scanned {
            types: own.iter().map(|s| s.to_string()).collect(),
            patches: targets
                .iter()
                .map(|s| (s.to_string(), PathBuf::from("/m/Assemblies/Mine.dll")))
                .collect(),
            ..Scanned::default()
        }
    }

    #[test]
    fn a_patch_on_the_type_of_another_mod_orders_after_it_once() {
        let mods = [
            patching(&["Big.Lib.Thing", "Big.Lib.Other"], &["Big.Lib.Other"]),
            patching(&[], &["Big.Lib.Thing", "Verse.Pawn", "Nobody.Knows"]),
            patching(&["Own.Type"], &["Own.Type"]),
        ];
        let mut mods = mods.to_vec();
        mods[0].assemblies.insert("biglib".into());
        mods[1]
            .uses
            .push(("biglib".into(), PathBuf::from("/m/Assemblies/Mine.dll")));
        let names = ["Big Lib", "Mine", "Selfish"].map(String::from);
        let roots = vec![PathBuf::from("/m"); 3];
        let borrowed: Vec<&Scanned> = mods.iter().collect();
        let found = infer(&borrowed, &names, &[false; 3], &roots);
        assert_eq!(found.len(), 1);
        assert_eq!(
            found[0].why.to_string(),
            "Assemblies/Mine.dll uses its code"
        );

        mods[1].uses.clear();
        let borrowed: Vec<&Scanned> = mods.iter().collect();
        let found = infer(&borrowed, &names, &[false; 3], &roots);
        let pairs: Vec<(usize, usize)> = found.iter().map(|r| (r.first, r.then)).collect();
        assert_eq!(pairs, [(0, 1)]);
        assert!(found[0].code);
        assert_eq!(
            found[0].why.to_string(),
            "Assemblies/Mine.dll patches its code"
        );
    }

    #[test]
    fn a_patch_on_the_games_own_type_orders_nothing() {
        let mods = [
            patching(&["Verse.Pawn"], &[]),
            patching(&[], &["Verse.Pawn"]),
        ];
        let names = ["Core", "Mine"].map(String::from);
        let roots = vec![PathBuf::from("/m"); 2];
        let borrowed: Vec<&Scanned> = mods.iter().collect();
        assert!(infer(&borrowed, &names, &[true, false], &roots).is_empty());
    }

    #[test]
    fn nothing_is_inferred_when_more_than_one_mod_could_be_meant_or_it_is_the_game() {
        let mods = [
            scanned(&["Steel", "Shared"], &[], &[], &[], &[]),
            scanned(&["Shared"], &[], &[], &[], &[]),
            scanned(
                &["Mine"],
                &[],
                &[],
                &["Steel", "Shared", "Mine", "Nobody"],
                &[],
            ),
        ];
        let names = ["Core", "Other", "Patch"].map(String::from);
        let roots = vec![PathBuf::from("/m"); 3];
        let borrowed: Vec<&Scanned> = mods.iter().collect();
        assert!(infer(&borrowed, &names, &[true, false, false], &roots).is_empty());
    }
}

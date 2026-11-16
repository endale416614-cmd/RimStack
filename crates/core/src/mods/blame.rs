use crate::mods::library::ModId;
use crate::rimworld::logs::{self, ErrorGroup};
use serde::Serialize;
use std::collections::HashMap;

/// From the surest: its code in the trace, then its package id in the error's
/// first line. Never its name: an error uses the same words of other things.
pub struct Clues<'a> {
    pub assemblies: &'a HashMap<String, ModId>,
    pub ids: &'a [(String, ModId)],
}

impl Clues<'_> {
    fn mods_of(&self, group: &ErrorGroup) -> Vec<ModId> {
        let by_code = logs::mods_in(group, self.assemblies);
        if !by_code.is_empty() {
            return by_code;
        }
        let named = logs::mentioned(&group.head, self.ids);
        named.into_iter().collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Named {
    pub name: String,
    pub mod_id: ModId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Pointed {
    pub mods: Vec<Named>,
    pub head: String,
    pub trace: Vec<String>,
    pub count: usize,
    pub line: usize,
    pub at: usize,
    pub run: usize,
    pub alike: usize,
    #[serde(rename = "mod")]
    pub first: Option<String>,
    pub mod_id: Option<ModId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Blamed {
    /// Empty for the errors that point at no mod.
    pub name: String,
    pub mod_id: Option<ModId>,
    pub kinds: usize,
    pub times: usize,
}

pub fn of(
    groups: &[ErrorGroup],
    clues: &Clues,
    name_of: &dyn Fn(ModId) -> String,
) -> (Vec<Pointed>, Vec<Blamed>) {
    let runs = logs::runs(groups);
    let mut counted: Vec<(Option<ModId>, usize)> = Vec::with_capacity(groups.len());
    let errors = groups
        .iter()
        .enumerate()
        .map(|(at, group)| {
            let mods = clues.mods_of(group);
            let first = mods.first().copied();
            counted.push((first, group.count));
            Pointed {
                mods: mods
                    .iter()
                    .map(|&mod_id| Named {
                        name: name_of(mod_id),
                        mod_id,
                    })
                    .collect(),
                head: group.head.clone(),
                trace: group.trace.clone(),
                count: group.count,
                line: group.line,
                at,
                run: runs[at],
                alike: runs.iter().filter(|&&start| start == runs[at]).count(),
                first: first.map(name_of),
                mod_id: first,
            }
        })
        .collect();
    let blamed = logs::culprits(counted)
        .iter()
        .map(|culprit| Blamed {
            name: culprit.mod_id.map(name_of).unwrap_or_default(),
            mod_id: culprit.mod_id,
            kinds: culprit.kinds,
            times: culprit.times,
        })
        .collect();
    (errors, blamed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn group(head: &str, trace: &[&str], count: usize) -> ErrorGroup {
        ErrorGroup {
            head: head.to_string(),
            trace: trace.iter().map(|line| line.to_string()).collect(),
            count,
            line: 1,
        }
    }

    fn name_of(id: ModId) -> String {
        ["Core", "Better Roads", "Hats Galore"][id.index()].to_string()
    }

    fn roads() -> ModId {
        ModId::new(0, 1)
    }

    #[test]
    fn an_error_is_a_mod_s_by_its_code_then_its_package_id_and_never_its_name() {
        let assemblies = HashMap::from([("betterroads".to_string(), roads())]);
        let ids = [("ann.betterroads".to_string(), roads())];
        let clues = Clues {
            assemblies: &assemblies,
            ids: &ids,
        };
        let groups = [
            group(
                "Exception in Hats Galore",
                &["  at BetterRoads.Patch.Prefix ()"],
                3,
            ),
            group("Could not load texture for Hats Galore", &[], 2),
            group("Mod ann.betterroads has no About.xml", &[], 1),
            group(
                "Exception filling window",
                &["  at Verse.Window.Draw ()"],
                5,
            ),
        ];
        let (errors, blamed) = of(&groups, &clues, &name_of);
        let pointed: Vec<Option<ModId>> = errors.iter().map(|error| error.mod_id).collect();
        let roads_packed = roads().to_packed();
        let roads_id = Some(roads());
        assert_eq!(pointed, [roads_id, None, roads_id, None]);
        assert_eq!(errors[0].first.as_deref(), Some("Better Roads"));
        assert_eq!(errors[3].mods, []);

        let roads = blamed.iter().find(|b| b.mod_id == roads_id).unwrap();
        assert_eq!((roads.kinds, roads.times), (2, 4));
        let nobody = blamed.iter().find(|b| b.mod_id.is_none()).unwrap();
        assert_eq!((nobody.name.as_str(), nobody.times), ("", 7));

        let said = serde_json::to_value(&errors[0]).unwrap();
        assert_eq!(said["mod"], "Better Roads");
        assert_eq!(said["mods"][0]["modId"], roads_packed);
    }
}

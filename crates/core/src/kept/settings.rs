use crate::base::error::Error;
use crate::said;
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Settings {
    values: BTreeMap<String, String>,
}

impl Settings {
    /// A missing or damaged file reads as the defaults.
    pub fn read(path: &Path) -> Self {
        let Some(Value::Object(entries)) = crate::base::atomic_file::read_json(path) else {
            return Self::default();
        };
        let values = entries
            .into_iter()
            .filter_map(|(k, v)| Some((k, v.as_str()?.to_string())))
            .collect();
        Self { values }
    }

    pub fn write(&self, path: &Path) -> Result<(), Error> {
        let entries: serde_json::Map<String, Value> = self
            .values
            .iter()
            .map(|(k, v)| (k.clone(), Value::String(v.clone())))
            .collect();
        let text = serde_json::to_string_pretty(&Value::Object(entries))
            .map_err(|e| Error::format(format!("{}: {e}", path.display())))?;
        crate::base::atomic_file::write(path, text)
    }

    /// Empty counts as never set.
    pub fn get(&self, name: &str) -> Option<&str> {
        self.values
            .get(name)
            .map(String::as_str)
            .filter(|v| !v.is_empty())
    }

    /// Several values are kept with commas between.
    pub fn listed(&self, name: &str) -> impl Iterator<Item = &str> + use<'_> {
        let kept = self.get(name).unwrap_or_default();
        kept.split(',').map(str::trim).filter(|one| !one.is_empty())
    }

    pub fn switch(&self, switch: Switch) -> bool {
        match self.get(switch.name()) {
            Some("on") => true,
            Some("off") => false,
            _ => switch.on_at_first(),
        }
    }

    /// "direct" starts the game's own program rather than asking Steam.
    pub fn launch_directly(&self) -> bool {
        self.get("launch") == Some("direct")
    }

    pub fn nightly_updates(&self) -> bool {
        self.get("updates") == Some("nightly")
    }

    pub fn update_every_minutes(&self) -> u64 {
        self.get("updateEvery")
            .and_then(|minutes| minutes.parse().ok())
            .unwrap_or(UPDATE_EVERY_AT_FIRST)
    }

    pub fn logs_kept(&self) -> Option<usize> {
        self.get("logsKept")?.parse().ok()
    }

    /// Empty forgets the setting. Refused: a setting RimStack does not know, or a value it
    /// cannot hold.
    pub fn set(&mut self, name: &str, value: &str) -> Result<(), Error> {
        let value = value.trim();
        let switch = Switch::ALL
            .iter()
            .any(|s| s.name() == name)
            .then_some(&SWITCH);
        let known = KNOWN.iter().find(|(known, _)| *known == name);
        let Some(holds) = switch.or(known.map(|(_, holds)| holds)) else {
            return Err(Error::refused(said!("{} is not a setting", name)));
        };
        let fits = match holds {
            _ if value.is_empty() => true,
            Holds::Choice(allowed) => allowed.contains(&value),
            Holds::Count => value.parse::<usize>().is_ok(),
            Holds::Text | Holds::List => true,
        };
        if !fits {
            return Err(Error::refused(said!("{} cannot be {}", name, value)));
        }
        match value.is_empty() {
            true => drop(self.values.remove(name)),
            false => drop(self.values.insert(name.to_string(), value.to_string())),
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
enum Holds {
    Text,
    List,
    Count,
    Choice(&'static [&'static str]),
}

const SWITCH: Holds = Holds::Choice(&["on", "off"]);

/// Minutes between two checks for updates made by themselves.
pub const UPDATE_EVERY: [&str; 6] = ["1", "5", "10", "15", "30", "60"];
const UPDATE_EVERY_AT_FIRST: u64 = 10;

const KNOWN: [(&str, Holds); 19] = [
    ("game", Holds::Text),
    ("config", Holds::Text),
    ("workshop", Holds::Text),
    ("launch", Holds::Choice(&["steam", "direct"])),
    ("launchArguments", Holds::Text),
    ("language", Holds::Text),
    ("font", Holds::Text),
    ("interfaceScale", Holds::Text),
    ("ownColoursFor", Holds::Text),
    ("versionAsideFor", Holds::Text),
    ("setupDone", Holds::Choice(&["yes"])),
    ("logsKept", Holds::Count),
    ("databasesOff", Holds::List),
    ("replacementsIgnored", Holds::List),
    ("textureSkip", Holds::List),
    ("updates", Holds::Choice(&["stable", "nightly"])),
    ("updateEvery", Holds::Choice(&UPDATE_EVERY)),
    ("window", Holds::Text),
    ("windowMaximized", Holds::Choice(&["yes"])),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Switch {
    CommunityRules,
    VersionWarnings,
    FileHints,
    DimLeftOut,
    GitForWorkshop,
    SelfCheck,
    ShowMature,
    SteamKeyKept,
    AutoUpdates,
}

impl Switch {
    pub const ALL: [Switch; 9] = [
        Switch::CommunityRules,
        Switch::VersionWarnings,
        Switch::FileHints,
        Switch::DimLeftOut,
        Switch::GitForWorkshop,
        Switch::SelfCheck,
        Switch::ShowMature,
        Switch::SteamKeyKept,
        Switch::AutoUpdates,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Switch::CommunityRules => "communityRules",
            Switch::VersionWarnings => "versionWarnings",
            Switch::FileHints => "fileHints",
            Switch::DimLeftOut => "dimLeftOut",
            Switch::GitForWorkshop => "gitForWorkshop",
            Switch::SelfCheck => "selfCheck",
            Switch::ShowMature => "showMature",
            Switch::SteamKeyKept => "steamKeyKept",
            Switch::AutoUpdates => "autoUpdates",
        }
    }

    fn on_at_first(self) -> bool {
        match self {
            Switch::CommunityRules
            | Switch::VersionWarnings
            | Switch::SelfCheck
            | Switch::FileHints
            | Switch::DimLeftOut => true,
            Switch::GitForWorkshop
            | Switch::ShowMature
            | Switch::SteamKeyKept
            | Switch::AutoUpdates => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_value_of_several_is_read_one_by_one() {
        let mut s = Settings::default();
        assert_eq!(s.listed("databasesOff").count(), 0);
        s.set("databasesOff", "rules, names,,").unwrap();
        assert_eq!(
            s.listed("databasesOff").collect::<Vec<_>>(),
            ["rules", "names"]
        );
    }

    #[test]
    fn values_are_kept_trimmed_and_empty_forgets() {
        let mut s = Settings::default();
        assert_eq!(s.get("game"), None);
        s.set("game", "  /games/RimWorld ").unwrap();
        assert_eq!(s.get("game"), Some("/games/RimWorld"));
        s.set("game", "  ").unwrap();
        assert_eq!(s.get("game"), None);
    }

    #[test]
    fn every_switch_is_as_it_was_set_and_otherwise_as_it_is_at_first() {
        let mut s = Settings::default();
        assert!(s.switch(Switch::CommunityRules) && s.switch(Switch::FileHints));
        assert!(!s.switch(Switch::GitForWorkshop));
        s.set("communityRules", "off").unwrap();
        s.set("fileHints", "off").unwrap();
        assert!(!s.switch(Switch::CommunityRules) && !s.switch(Switch::FileHints));
        s.set("communityRules", "").unwrap();
        assert!(s.switch(Switch::CommunityRules));
        for switch in Switch::ALL {
            for (value, on) in [("on", true), ("off", false)] {
                s.set(switch.name(), value).unwrap();
                assert_eq!(s.switch(switch), on, "{}", switch.name());
            }
        }
    }

    #[test]
    fn updates_are_checked_every_ten_minutes_until_set_otherwise() {
        let mut s = Settings::default();
        assert_eq!(s.update_every_minutes(), 10);
        s.set("updateEvery", "60").unwrap();
        assert_eq!(s.update_every_minutes(), 60);
        assert!(s.set("updateEvery", "7").is_err());
    }

    #[test]
    fn only_a_known_setting_with_a_value_it_can_hold_is_kept() {
        let mut s = Settings::default();
        assert!(s.set("gameDir", "/x").is_err());
        assert!(s.set("fileHints", "yes").is_err());
        assert!(s.set("launch", "now").is_err());
        assert!(s.set("logsKept", "ten").is_err());
        assert_eq!(s, Settings::default());
        s.set("launch", "direct").unwrap();
        s.set("logsKept", "10").unwrap();
        assert!(s.launch_directly());
        assert_eq!(s.logs_kept(), Some(10));
        let refused = s.set("selfCheck", "maybe").unwrap_err();
        assert_eq!(refused.kind(), crate::base::error::Kind::Refused);
    }

    #[test]
    fn settings_read_back_as_written_and_damage_reads_as_default() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("RimStack").join("settings.json");
        let mut s = Settings::default();
        s.set("game", "/games/RimWorld").unwrap();
        s.write(&path).unwrap();
        assert_eq!(Settings::read(&path), s);
        std::fs::write(&path, "[1,2").unwrap();
        assert_eq!(Settings::read(&path), Settings::default());
        assert_eq!(
            Settings::read(&dir.path().join("none.json")),
            Settings::default()
        );
    }
}

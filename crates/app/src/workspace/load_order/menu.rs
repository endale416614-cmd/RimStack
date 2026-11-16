use super::details::{Details, FixSaid, NotInstalled};
use super::{LoadOrder, Side};
use crate::App;
use crate::workspace::upkeep::Line;
use rimstack_core::base::sentence::Sentence;
use rimstack_core::mods::library::ModId;
use rimstack_core::mods::list::Row;
use rimstack_core::{say, say_n};
use serde::Serialize;

/// The window keeps one handler for each kind (`doers`).
#[derive(Serialize)]
#[serde(
    tag = "action",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Action {
    MoveSelected,
    MoveAcross {
        active: bool,
        index: usize,
    },
    OpenFolder {
        path: String,
    },
    OpenUrl {
        url: String,
    },
    CopyText {
        value: String,
    },
    ApplyFix {
        mod_id: Option<ModId>,
        fix: FixSaid,
    },
    Subscribe {
        workshop_id: String,
    },
    CopyToLocal {
        ids: Vec<ModId>,
        ask: Ask,
    },
    Redownload {
        mod_id: ModId,
        ask: Ask,
    },
    Unsubscribe {
        mod_id: ModId,
    },
    UpdateFromGit {
        mod_id: ModId,
    },
    PutInSub {
        mod_id: ModId,
        tag: String,
    },
    NewSub {
        mod_id: ModId,
    },
    Trust {
        ids: Vec<ModId>,
        yes: bool,
    },
    Bin {
        name: String,
        path: String,
        mod_id: ModId,
    },
    ToggleSub {
        active: bool,
        row: usize,
    },
    RemoveSub {
        active: bool,
        row: usize,
    },
}

#[derive(Serialize)]
pub struct Ask {
    pub title: Sentence,
    pub lines: Vec<Line>,
    pub label: Sentence,
}

fn unset(flag: &bool) -> bool {
    !flag
}

fn empty(icon: &&str) -> bool {
    icon.is_empty()
}

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<Sentence>,
    #[serde(skip_serializing_if = "unset")]
    pub line: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<Sentence>,
    #[serde(skip_serializing_if = "empty")]
    pub icon: &'static str,
    #[serde(skip_serializing_if = "unset")]
    pub off: bool,
    #[serde(skip_serializing_if = "unset")]
    pub danger: bool,
    #[serde(flatten)]
    pub action: Option<Action>,
}

impl Entry {
    fn item(text: Sentence, icon: &'static str, action: Action) -> Self {
        Entry {
            text: Some(text),
            icon,
            action: Some(action),
            ..Entry::default()
        }
    }

    fn title(text: Sentence) -> Self {
        Entry {
            title: Some(text),
            ..Entry::default()
        }
    }

    fn off(self, off: bool) -> Self {
        Entry { off, ..self }
    }

    fn danger(self) -> Self {
        Entry {
            danger: true,
            ..self
        }
    }
}

fn separated(groups: Vec<Vec<Entry>>) -> Vec<Entry> {
    let mut entries: Vec<Entry> = Vec::new();
    for group in groups.into_iter().filter(|group| !group.is_empty()) {
        if !entries.is_empty() {
            entries.push(Entry {
                line: true,
                ..Entry::default()
            });
        }
        entries.extend(group);
    }
    entries
}

pub struct Clicked<'a> {
    pub active: bool,
    pub index: usize,
    pub id: Option<ModId>,
    pub selected: &'a [ModId],
}

struct Subject {
    name: String,
    path: String,
    url: String,
    package_id: String,
    take_off: Option<Sentence>,
    fixes: Vec<FixSaid>,
    resubscribe: Vec<String>,
    workshop: bool,
    workshop_page: String,
    steam_page: String,
    repository: String,
    group_choices: Option<Vec<String>>,
    sub: String,
    version_ok: bool,
    offer_trust: bool,
    can_bin: bool,
}

impl Subject {
    fn gone(gone: NotInstalled) -> Self {
        Subject {
            name: gone.name,
            package_id: gone.package_id,
            take_off: Some(gone.take_off),
            fixes: gone.fixes,
            resubscribe: gone.resubscribe,
            ..Subject::unknown()
        }
    }

    fn held(about: Details) -> Self {
        let repository = match about.git {
            true => rimstack_core::sources::git::page(std::path::Path::new(&about.path))
                .unwrap_or_default(),
            false => String::new(),
        };
        Subject {
            name: about.name,
            path: about.path,
            url: about.url,
            package_id: about.package_id,
            workshop: about.workshop,
            workshop_page: about.workshop_page.unwrap_or_default(),
            steam_page: about.steam_page.unwrap_or_default(),
            repository,
            group_choices: Some(about.group_choices),
            sub: about.sub,
            version_ok: about.version_ok,
            offer_trust: about.offer_trust,
            can_bin: about.can_bin,
            ..Subject::unknown()
        }
    }

    fn unknown() -> Self {
        Subject {
            name: String::new(),
            path: String::new(),
            url: String::new(),
            package_id: String::new(),
            take_off: None,
            fixes: Vec::new(),
            resubscribe: Vec::new(),
            workshop: false,
            workshop_page: String::new(),
            steam_page: String::new(),
            repository: String::new(),
            group_choices: None,
            sub: String::new(),
            version_ok: false,
            offer_trust: false,
            can_bin: false,
        }
    }
}

impl LoadOrder {
    /// The chosen mods whose version warning the player can set aside; none for fewer than two.
    pub fn version_warned(&self, chosen: &[ModId]) -> Vec<ModId> {
        let list = self.list(Side::Active);
        let warned = |id: &&ModId| {
            let own = list
                .position_of(**id)
                .and_then(|at| self.troubles().get(at));
            own.is_some_and(|own| rimstack_core::mods::describe::warned_for_version(own))
        };
        chosen.iter().filter(warned).copied().collect()
    }

    pub fn row_menu(&mut self, app: &App, at: &Clicked) -> Vec<Entry> {
        let side = Side::of(at.active);
        let subject = match self.not_installed(app, side, at.index) {
            Some(gone) => Subject::gone(gone),
            None => at
                .id
                .map(|id| self.details(app, id))
                .map_or_else(Subject::unknown, Subject::held),
        };
        let many = at.selected.len() > 1;
        let warned = if many {
            self.version_warned(at.selected)
        } else {
            Vec::new()
        };
        let mut groups = vec![self.moving(at, &subject, many)];
        groups.push(Self::opening(&subject));
        groups.push(Self::fetching(at, &subject));
        if subject.workshop {
            groups.push(Self::workshop(app, at, &subject));
        }
        if !subject.repository.is_empty() {
            groups.push(Self::git(at, &subject));
        }
        groups.push(Self::filing(at, &subject));
        groups.push(Self::last(at, &subject, many, &warned));
        separated(groups)
    }

    fn moving(&self, at: &Clicked, subject: &Subject, many: bool) -> Vec<Entry> {
        let count = at.selected.len();
        let text = subject
            .take_off
            .clone()
            .unwrap_or_else(|| match (many, at.active) {
                (true, true) => say_n!(count, "Deactivate {} mod", "Deactivate {} mods"),
                (true, false) => say_n!(count, "Activate {} mod", "Activate {} mods"),
                (false, true) => say!("Deactivate"),
                (false, false) => say!("Activate"),
            });
        let action = if many {
            Action::MoveSelected
        } else {
            Action::MoveAcross {
                active: at.active,
                index: at.index,
            }
        };
        let icon = if at.active { "minus" } else { "plus" };
        vec![Entry::item(text, icon, action).off(self.previewing())]
    }

    fn opening(subject: &Subject) -> Vec<Entry> {
        let mut group = Vec::new();
        if !subject.path.is_empty() {
            let action = Action::OpenFolder {
                path: subject.path.clone(),
            };
            group.push(Entry::item(say!("Open folder"), "folder-open", action));
        }
        if !subject.url.is_empty() {
            let action = Action::OpenUrl {
                url: subject.url.clone(),
            };
            group.push(Entry::item(say!("Mod's own page"), "link", action));
        }
        let copy = |value: &str| Action::CopyText {
            value: value.to_string(),
        };
        group.push(Entry::item(say!("Copy name"), "copy", copy(&subject.name)));
        if !subject.package_id.is_empty() {
            let text = say!("Copy package id");
            group.push(Entry::item(text, "copy", copy(&subject.package_id)));
        }
        group
    }

    fn fetching(at: &Clicked, subject: &Subject) -> Vec<Entry> {
        let mut group = Vec::new();
        for fix in &subject.fixes {
            let action = Action::ApplyFix {
                mod_id: at.id,
                fix: fix.clone(),
            };
            group.push(Entry::item(
                Sentence::named(fix.label),
                "external-link",
                action,
            ));
        }
        for workshop_id in &subject.resubscribe {
            let action = Action::Subscribe {
                workshop_id: workshop_id.clone(),
            };
            group.push(Entry::item(say!("Subscribe again"), "download", action));
        }
        group
    }

    fn workshop(app: &App, at: &Clicked, subject: &Subject) -> Vec<Entry> {
        let page = subject.workshop_page.clone();
        let open = |url: &str| Action::OpenUrl {
            url: url.to_string(),
        };
        let copy = app.copy_offer(at.selected);
        let copying = copy.ids.is_empty();
        let ask = Ask {
            title: copy.title,
            lines: copy.lines,
            label: say!("Copy"),
        };
        let mut group = vec![
            Entry::title(say!("Steam Workshop")),
            Entry::item(
                say!("Page in Steam"),
                "external-link",
                open(&subject.steam_page),
            ),
            Entry::item(say!("Page in the browser"), "external-link", open(&page)),
            Entry::item(
                say!("Copy link"),
                "copy",
                Action::CopyText {
                    value: page.clone(),
                },
            ),
            Entry::item(
                say!("Copy to the game's Mods folder…"),
                "copy",
                Action::CopyToLocal { ids: copy.ids, ask },
            )
            .off(copying),
        ];
        if let Some(id) = at.id {
            if app.steamcmd().is_some() {
                let ask = Ask {
                    title: say!("Download {} again", subject.name),
                    lines: vec![
                        Line::Said(say!(
                            "SteamCMD downloads the mod again, and the new copy replaces this folder:"
                        )),
                        Line::Path {
                            path: subject.path.clone(),
                        },
                    ],
                    label: say!("Download"),
                };
                let action = Action::Redownload { mod_id: id, ask };
                group.push(Entry::item(
                    say!("Download again with SteamCMD…"),
                    "download",
                    action,
                ));
            }
            if !at.active {
                let action = Action::Unsubscribe { mod_id: id };
                group.push(Entry::item(say!("Unsubscribe…"), "minus", action).danger());
            }
        }
        group
    }

    fn git(at: &Clicked, subject: &Subject) -> Vec<Entry> {
        let Some(id) = at.id else {
            return Vec::new();
        };
        let repository = Action::OpenUrl {
            url: subject.repository.clone(),
        };
        vec![
            Entry::title(say!("Git")),
            Entry::item(say!("Repository"), "link", repository),
            Entry::item(
                say!("Update"),
                "download",
                Action::UpdateFromGit { mod_id: id },
            ),
        ]
    }

    fn filing(at: &Clicked, subject: &Subject) -> Vec<Entry> {
        let (Some(id), Some(choices)) = (at.id, &subject.group_choices) else {
            return Vec::new();
        };
        let mut group = vec![Entry::title(say!("Group under a tag"))];
        for tag in choices {
            let action = Action::PutInSub {
                mod_id: id,
                tag: tag.clone(),
            };
            group.push(Entry::item(say!("Group under {}", tag), "", action));
        }
        group.push(Entry::item(
            say!("A new tag…"),
            "plus",
            Action::NewSub { mod_id: id },
        ));
        if !subject.sub.is_empty() {
            let action = Action::PutInSub {
                mod_id: id,
                tag: String::new(),
            };
            group.push(Entry::item(
                say!("Remove the tag {}", subject.sub),
                "",
                action,
            ));
        }
        group
    }

    fn last(at: &Clicked, subject: &Subject, many: bool, warned: &[ModId]) -> Vec<Entry> {
        let mut group = Vec::new();
        let trust = |ids: Vec<ModId>, yes| Action::Trust { ids, yes };
        let one = at.id.into_iter().collect::<Vec<_>>();
        if many && !warned.is_empty() {
            group.push(Entry::item(
                say_n!(
                    warned.len(),
                    "Ignore the game version mismatch of {} mod",
                    "Ignore the game version mismatch of {} mods"
                ),
                "check",
                trust(warned.to_vec(), true),
            ));
        } else if many && subject.version_ok {
            group.push(Entry::item(
                say_n!(
                    at.selected.len(),
                    "Warn about the game version of {} mod again",
                    "Warn about the game version of {} mods again"
                ),
                "",
                trust(at.selected.to_vec(), false),
            ));
        } else if subject.version_ok {
            let text = say!("Warn about its game version again");
            group.push(Entry::item(text, "", trust(one, false)));
        } else if subject.offer_trust {
            let text = say!("Ignore the game version mismatch");
            group.push(Entry::item(text, "check", trust(one, true)));
        }
        if let (true, Some(id)) = (subject.can_bin, at.id) {
            let action = Action::Bin {
                name: subject.name.clone(),
                path: subject.path.clone(),
                mod_id: id,
            };
            group.push(Entry::item(say!("Move to the bin…"), "trash-2", action).danger());
        }
        group
    }

    pub fn sub_menu(&self, active: bool, row: usize) -> Vec<Entry> {
        let Some(Row::Sub { collapsed, .. }) = self.list(Side::of(active)).rows().get(row) else {
            return Vec::new();
        };
        let fold = if *collapsed {
            say!("Show its mods")
        } else {
            say!("Hide its mods")
        };
        vec![
            Entry::item(fold, "", Action::ToggleSub { active, row }),
            Entry::item(
                say!("Take this tag off these mods"),
                "x",
                Action::RemoveSub { active, row },
            ),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::tests::installed;

    fn mod_at(root: &std::path::Path, folder: &str, id: &str) {
        let about = root.join(folder).join("About");
        std::fs::create_dir_all(&about).unwrap();
        let xml =
            format!("<ModMetaData><packageId>{id}</packageId><name>{id}</name></ModMetaData>");
        std::fs::write(about.join("About.xml"), xml).unwrap();
    }

    fn id_of(order: &LoadOrder, key: &str) -> (Side, usize, ModId) {
        [Side::Active, Side::Inactive]
            .into_iter()
            .find_map(|side| {
                let at = order.list(side).mods().iter().position(|m| m.id == key)?;
                Some((side, at, order.list(side).mods()[at].installed?))
            })
            .unwrap()
    }

    fn shown(entries: &[Entry]) -> Vec<String> {
        let each = |e: &Entry| match (&e.title, &e.text, e.line) {
            (Some(title), _, _) => format!("# {title}"),
            (_, Some(text), _) => text.to_string(),
            _ => "-".to_string(),
        };
        entries.iter().map(each).collect()
    }

    fn menu(order: &mut LoadOrder, app: &App, key: &str, selected: &[&str]) -> Vec<Entry> {
        let (side, index, id) = id_of(order, key);
        let chosen: Vec<ModId> = selected.iter().map(|k| id_of(order, k).2).collect();
        let at = Clicked {
            active: side == Side::Active,
            index,
            id: Some(id),
            selected: &chosen,
        };
        order.row_menu(app, &at)
    }

    #[test]
    fn a_local_mod_offers_its_folder_a_tag_and_the_bin() {
        let (_tmp, app) = installed();
        let mut order = LoadOrder::read(&app, true, None);
        let entries = menu(&mut order, &app, "ann.roads", &["ann.roads"]);
        assert_eq!(
            shown(&entries),
            [
                "Activate",
                "-",
                "Open folder",
                "Copy name",
                "Copy package id",
                "-",
                "# Group under a tag",
                "A new tag…",
                "-",
                "Move to the bin…",
            ]
        );
        assert!(entries.last().unwrap().danger);
        assert!(matches!(
            entries[0].action,
            Some(Action::MoveAcross {
                active: false,
                index: 0
            })
        ));
    }

    #[test]
    fn a_workshop_mod_offers_steam_and_can_be_unsubscribed_only_while_inactive() {
        let (tmp, app) = installed();
        mod_at(tmp.path(), "workshop/900002", "Ann.Shop");
        app.rescan();
        let mut order = LoadOrder::read(&app, true, None);
        let entries = menu(&mut order, &app, "ann.shop", &["ann.shop"]);
        let texts = shown(&entries);
        assert!(texts.contains(&"# Steam Workshop".to_string()));
        assert!(texts.contains(&"Page in Steam".to_string()));
        assert!(texts.contains(&"Unsubscribe…".to_string()));
        assert!(!texts.contains(&"Download again with SteamCMD…".to_string()));
        let copy = entries
            .iter()
            .find(|e| matches!(e.action, Some(Action::CopyToLocal { .. })))
            .unwrap();
        assert!(!copy.off);
        let unsubscribe = entries
            .iter()
            .find(|e| matches!(e.action, Some(Action::Unsubscribe { .. })))
            .unwrap();
        assert!(unsubscribe.danger);
    }

    #[test]
    fn a_git_mod_offers_its_repository_and_an_update() {
        let (tmp, app) = installed();
        let root = tmp.path().join("game/Mods/roads");
        std::fs::create_dir_all(root.join(".git")).unwrap();
        std::fs::write(
            root.join(".git/config"),
            "[remote \"origin\"]\n\turl = https://github.com/ann/roads.git\n",
        )
        .unwrap();
        app.rescan();
        let mut order = LoadOrder::read(&app, true, None);
        let entries = menu(&mut order, &app, "ann.roads", &["ann.roads"]);
        let texts = shown(&entries);
        let at = texts.iter().position(|t| t == "# Git").unwrap();
        assert_eq!(texts[at + 1..at + 3], ["Repository", "Update"]);
        assert!(matches!(
            &entries[at + 1].action,
            Some(Action::OpenUrl { url }) if url == "https://github.com/ann/roads"
        ));
    }

    #[test]
    fn a_mod_among_several_chosen_moves_them_all() {
        let (tmp, app) = installed();
        mod_at(tmp.path(), "game/Mods/hats", "Ann.Hats");
        app.rescan();
        let mut order = LoadOrder::read(&app, true, None);
        let entries = menu(&mut order, &app, "ann.roads", &["ann.roads", "ann.hats"]);
        assert_eq!(
            entries[0].text.as_ref().unwrap().to_string(),
            "Activate 2 mods"
        );
        assert!(matches!(entries[0].action, Some(Action::MoveSelected)));
    }

    #[test]
    fn a_tag_heading_offers_to_fold_and_to_untag() {
        let (_tmp, app) = installed();
        let mut order = LoadOrder::read(&app, true, None);
        let (_, _, roads) = id_of(&order, "ann.roads");
        app.set_group(&[roads], "Stone");
        order.sync(&app);
        let row = order
            .list(Side::Inactive)
            .rows()
            .iter()
            .position(|row| matches!(row, Row::Sub { .. }))
            .unwrap();
        assert_eq!(
            shown(&order.sub_menu(false, row)),
            ["Hide its mods", "Take this tag off these mods"]
        );
        order.toggle_sub(&app, Side::Inactive, row);
        assert_eq!(shown(&order.sub_menu(false, row))[0], "Show its mods");
        assert!(order.sub_menu(false, 99).is_empty());
    }
}

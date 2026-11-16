use super::{Mod, Side, flag};
use crate::mods::facts::{Facts, Has};

/// `wanted` is lower case.
pub(super) fn tagged(tags: &[String], wanted: &str) -> bool {
    tags.iter().any(|t| t.to_lowercase().starts_with(wanted))
}

/// A term after a `!` leaves out what it would match; `!is:active` asks for the other list.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct Filter {
    words: Vec<String>,
    authors: Vec<String>,
    tags: Vec<String>,
    versions: Vec<String>,
    local: bool,
    workshop: bool,
    git: bool,
    pub(super) leftover: bool,
    code: bool,
    patches: Vec<String>,
    has: Vec<Has>,
    defines: Vec<String>,
    patches_def: Vec<String>,
    flags: u16,
    only: Option<Side>,
    both: bool,
    left_out: Vec<Filter>,
}

impl Filter {
    pub(super) fn parse(text: &str) -> Self {
        let mut filter = Self::default();
        for term in text.to_lowercase().split_whitespace() {
            match term.strip_prefix('!').filter(|rest| !rest.is_empty()) {
                Some("is:active") => filter.ask(Side::Inactive),
                Some("is:inactive") => filter.ask(Side::Active),
                Some(rest) => {
                    let mut left_out = Self::default();
                    left_out.take(rest);
                    filter.left_out.push(left_out);
                }
                None => filter.take(term),
            }
        }
        filter
    }

    fn take(&mut self, term: &str) {
        match term.split_once(':') {
            Some(("author", name)) if !name.is_empty() => self.authors.push(name.to_string()),
            Some(("tag", label)) if !label.is_empty() => self.tags.push(label.to_string()),
            Some(("version", game)) if !game.is_empty() => self.versions.push(game.to_string()),
            Some(("is", "local")) => self.local = true,
            Some(("is", "workshop")) => self.workshop = true,
            Some(("is", "git")) => self.git = true,
            Some(("is", "leftover")) => self.leftover = true,
            Some(("is", "active")) => self.ask(Side::Active),
            Some(("is", "inactive")) => self.ask(Side::Inactive),
            Some(("has", "code")) => self.code = true,
            Some(("patches", method)) if !method.is_empty() => {
                self.patches.push(method.to_string())
            }
            Some(("has", what)) if Has::named(what).is_some() => self.has.extend(Has::named(what)),
            Some(("defines", name)) if !name.is_empty() => self.defines.push(name.to_string()),
            Some(("patchesdef", name)) if !name.is_empty() => {
                self.patches_def.push(name.to_string())
            }
            Some(("is", "error")) => self.flags |= flag::ERROR,
            Some(("is", "warning")) => self.flags |= flag::WARNING,
            Some(("is", "conflict")) => self.flags |= flag::CONFLICT,
            Some(("is", "updated")) => self.flags |= flag::UPDATED,
            Some(("is", "notinsave")) => self.flags |= flag::NOT_IN_SAVE,
            Some(("is", "recent")) => self.flags |= flag::RECENT,
            Some(("is", "duplicate")) => self.flags |= flag::DUPLICATE,
            Some(("is", "new")) => self.flags |= flag::NEW,
            Some(("is", "changed")) => self.flags |= flag::CHANGED,
            Some(("in", "search")) => self.flags |= flag::IN_SEARCH,
            _ => self.words.push(term.to_string()),
        }
    }

    fn ask(&mut self, side: Side) {
        self.both |= self.only.is_some_and(|asked| asked != side);
        self.only = Some(side);
    }

    pub(super) fn shuts(&self, side: Option<Side>) -> bool {
        self.both || side.zip(self.only).is_some_and(|(side, only)| side != only)
    }

    pub(super) fn is_empty(&self) -> bool {
        *self == Self::default()
    }

    pub(super) fn reads_flags(&self) -> bool {
        self.flags != 0 || self.left_out.iter().any(|f| f.flags != 0)
    }

    fn reads_own_facts(&self) -> bool {
        !(self.has.is_empty() && self.defines.is_empty() && self.patches_def.is_empty())
    }

    pub(super) fn needs_facts(&self) -> bool {
        self.reads_own_facts() || self.left_out.iter().any(Filter::reads_own_facts)
    }

    /// A mod whose files are not read yet passes no term that reads them, left out or not.
    pub(super) fn admits(
        &self,
        m: &Mod,
        haystack: &str,
        flags: u16,
        facts: Option<&Facts>,
    ) -> bool {
        if self.needs_facts() && facts.is_none() {
            return false;
        }
        let holds = |f: &Filter| f.holds(m, haystack, flags, facts);
        holds(self) && !self.left_out.iter().any(holds)
    }

    fn holds(&self, m: &Mod, haystack: &str, flags: u16, facts: Option<&Facts>) -> bool {
        if flags & self.flags != self.flags {
            return false;
        }
        let read = |facts: &Facts| {
            self.has.iter().all(|&what| facts.has(what))
                && self.defines.iter().all(|name| facts.defines(name))
                && self.patches_def.iter().all(|name| facts.patches_def(name))
        };
        if self.reads_own_facts() && !facts.is_some_and(read) {
            return false;
        }
        let author = || m.author.to_lowercase();
        self.words.iter().all(|w| haystack.contains(w))
            && self.authors.iter().all(|a| author().contains(a))
            && self.tags.iter().all(|wanted| tagged(&m.tags, wanted))
            && self
                .versions
                .iter()
                .all(|v| m.latest_game.starts_with(v.as_str()))
            && (!self.local || m.local)
            && (!self.workshop || m.workshop)
            && (!self.git || m.git)
            && (!self.leftover || m.leftover)
            && (!self.code || m.code)
            && self
                .patches
                .iter()
                .all(|p| m.patched.iter().any(|at| at.contains(p)))
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::{id, installed_list, read_with, sample, shown};
    use super::super::{GroupedList, LeftOut, Row, Side, flag};
    use std::collections::HashMap;
    use std::sync::Arc;

    fn names(list: &GroupedList) -> Vec<String> {
        let name = |r: &Row| match r {
            Row::Mod { index } => Some(list.mods()[*index].name.clone()),
            _ => None,
        };
        list.rows().iter().filter_map(name).collect()
    }

    #[test]
    fn left_out_terms_stack_and_mix_with_the_others() {
        let mut list = GroupedList::flat(sample().mods().to_vec());
        list.set_filter("!author:ludeon");
        assert_eq!(
            names(&list),
            ["Harmony", "HugsLib", "Dubs Bad Hygiene", "Rimefeller"]
        );
        list.set_filter("!author:ludeon !DUB");
        assert_eq!(names(&list), ["Harmony", "HugsLib"]);
        list.set_filter("dub !hygiene");
        assert_eq!(names(&list), ["Rimefeller"]);
    }

    #[test]
    fn a_left_out_tag_keeps_the_mods_without_it() {
        let mut mods = sample().mods().to_vec();
        mods[0].tags = vec!["HUD".into()];
        mods[4].tags = vec!["Hud bits".into(), "Big".into()];
        mods[5].tags = vec!["Big".into()];
        let mut list = GroupedList::flat(mods);
        list.set_filter("!tag:hud");
        assert_eq!(names(&list), ["Core", "Royalty", "HugsLib", "Rimefeller"]);
        list.set_filter("tag:big !tag:hud");
        assert_eq!(names(&list), ["Rimefeller"]);
    }

    #[test]
    fn a_version_matches_the_latest_game_version_listed() {
        let mut mods = sample().mods().to_vec();
        for (m, game) in mods.iter_mut().zip(["1.6", "1.7", "1.7", "1.5", "", "1.6"]) {
            m.latest_game = game.into();
        }
        let mut list = GroupedList::flat(mods);
        list.set_filter("version:1.7");
        assert_eq!(names(&list), ["Core", "Royalty"]);
        list.set_filter("!version:1.7");
        assert_eq!(
            names(&list),
            ["Harmony", "HugsLib", "Dubs Bad Hygiene", "Rimefeller"]
        );
        list.set_filter("version:1. !version:1.7");
        assert_eq!(names(&list), ["Harmony", "HugsLib", "Rimefeller"]);
        list.set_filter("version:1.6 dub");
        assert_eq!(names(&list), ["Rimefeller"]);
    }

    #[test]
    fn a_leftover_is_listed_only_while_the_filter_asks_for_it() {
        let mut mods = sample().mods().to_vec();
        mods[2].leftover = true;
        let mut list = GroupedList::flat(mods);
        assert_eq!((shown(&list), list.counted()), (5, 5));
        list.set_filter("is:leftover");
        assert_eq!(
            (names(&list), list.counted()),
            (vec!["Royalty".to_string()], 5)
        );
        list.set_filter("!is:leftover");
        assert_eq!(shown(&list), 5);
        list.set_filter("nothing-matches");
        list.set_left_out(LeftOut::Dimmed);
        assert_eq!(shown(&list), 5);
    }

    #[test]
    fn a_lone_bang_or_an_empty_term_after_it_leaves_nothing_out() {
        let mut list = GroupedList::flat(sample().mods().to_vec());
        list.set_filter("!tag:");
        assert_eq!(shown(&list), 6);
        list.set_filter("!patches:");
        assert_eq!(shown(&list), 6);
        list.set_filter("!");
        assert!(list.rows().is_empty());
    }

    #[test]
    fn leaving_out_every_mod_leaves_the_list_out() {
        let mut list = sample();
        list.set_filter("!ludeon !pardeike !hugs !dubwise");
        assert!(list.rows().is_empty());
        assert_eq!(
            (list.matching(), list.left_out()),
            (0, Some(LeftOut::Hidden))
        );
    }

    #[test]
    fn a_left_out_flag_waits_for_the_flags() {
        let mut list = GroupedList::flat(sample().mods().to_vec());
        list.set_filter("!is:error");
        assert_eq!(shown(&list), 6);
        list.set_flags(vec![
            0,
            flag::ERROR,
            flag::WARNING,
            0,
            flag::ERROR | flag::UPDATED,
            0,
        ]);
        assert_eq!(
            names(&list),
            ["Harmony", "Royalty", "HugsLib", "Rimefeller"]
        );
        list.set_filter("!is:error !is:warning");
        assert_eq!(names(&list), ["Harmony", "HugsLib", "Rimefeller"]);
    }

    #[test]
    fn leaving_out_one_list_asks_for_the_other() {
        let mut active = sample();
        let mut inactive = sample();
        active.set_filter_on(Side::Active, "!is:active");
        inactive.set_filter_on(Side::Inactive, "!is:active");
        assert_eq!((active.matching(), inactive.matching()), (0, 6));
        active.set_filter_on(Side::Active, "!is:inactive is:inactive");
        assert_eq!(active.matching(), 0);
    }

    #[test]
    fn a_left_out_file_term_waits_for_the_files_and_counts_them() {
        let gold = read_with(&[(
            "Defs/a.xml",
            "<Defs><ThingDef><defName>Gold</defName></ThingDef></Defs>",
        )]);
        let plain = read_with(&[("About/About.xml", "<ModMetaData/>")]);
        let mut list = installed_list();
        list.set_filter("!has:defs");
        assert_eq!((list.rows().len(), list.unread()), (0, 3));
        list.set_facts(Arc::new(HashMap::from([(id(0), gold), (id(1), plain)])));
        assert_eq!(
            (list.rows(), list.unread()),
            (&[Row::Mod { index: 1 }][..], 1)
        );
        list.set_filter("!defines:gold");
        assert_eq!(list.rows(), &[Row::Mod { index: 1 }]);
    }
}

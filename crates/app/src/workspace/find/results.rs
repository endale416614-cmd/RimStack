use rimstack_core::base::sentence::Sentence;
use rimstack_core::mods::find::{FileHits, Hit, Line, lines_of};
use rimstack_core::mods::library::ModId;
use rimstack_core::say;
use serde::Serialize;
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

/// Hits kept; a hit holds up to seven lines of about 300 characters, so this bounds the
/// results to some tens of megabytes. Mods with hits past it are still counted.
pub const MOST_HITS: usize = 20_000;

const NONE: u32 = u32::MAX;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RowRef {
    group: u32,
    file: u32,
    hit: u32,
}

impl RowRef {
    fn of_mod(group: usize) -> Self {
        Self {
            group: group as u32,
            file: NONE,
            hit: NONE,
        }
    }

    fn of_file(group: usize, file: usize) -> Self {
        Self {
            file: file as u32,
            ..Self::of_mod(group)
        }
    }

    fn of_hit(group: usize, file: usize, hit: u32) -> Self {
        Self {
            hit,
            ..Self::of_file(group, file)
        }
    }

    pub fn is_mod(self) -> bool {
        self.file == NONE
    }

    pub fn is_file(self) -> bool {
        self.file != NONE && self.hit == NONE
    }

    pub fn same_mod(self, other: Self) -> bool {
        self.group == other.group
    }

    pub fn same_file(self, other: Self) -> bool {
        self.group == other.group && self.file == other.file
    }
}

/// Positions are those of the rows shown when the step is applied, in order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Op {
    Insert { at: usize, rows: Vec<RowRef> },
    Changed { at: usize },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Folded {
    Hid,
    Showed(Vec<RowRef>),
}

struct FileGroup {
    path: PathBuf,
    shown: String,
    hits: Vec<Hit>,
    visible: Vec<u32>,
    lines: Vec<Line>,
    collapsed: bool,
}

struct Group {
    name: Sentence,
    files: Vec<FileGroup>,
    collapsed: bool,
    hidden: bool,
    rows: usize,
    matches: usize,
    shown_files: usize,
}

impl FileGroup {
    fn refilter(&mut self, group_name: &str, filter: &str) {
        self.visible = visible_hits(self, group_name, filter);
        self.lines = lines_of(self.visible.iter().map(|&at| &self.hits[at as usize]));
    }

    fn rows(&self) -> usize {
        match (self.visible.is_empty(), self.collapsed) {
            (true, _) => 0,
            (false, true) => 1,
            (false, false) => 1 + self.lines.len(),
        }
    }
}

impl Group {
    fn recount(&mut self) {
        self.matches = self.files.iter().map(|f| f.visible.len()).sum();
        self.shown_files = self.files.iter().filter(|f| !f.visible.is_empty()).count();
        self.rows = match (self.shown_files, self.collapsed) {
            (0, _) => 0,
            _ if self.hidden => 0,
            (_, true) => 1,
            _ => 1 + self.files.iter().map(FileGroup::rows).sum::<usize>(),
        };
    }

    fn name_text(&self) -> String {
        crate::words::put(&self.name).to_lowercase()
    }
}

#[derive(Default)]
pub struct Results {
    groups: Vec<Group>,
    by_key: HashMap<Option<ModId>, usize>,
    filter: String,
    stored: usize,
    capped: bool,
    files_found: usize,
    mods: HashSet<ModId>,
    picked: Option<usize>,
}

#[derive(Serialize)]
pub struct FoundMod {
    pub index: usize,
    pub name: Sentence,
    #[serde(skip)]
    pub count: usize,
    pub counted: Sentence,
    pub chosen: bool,
}

/// In the order the mods were found.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FoundMods {
    pub all_label: Sentence,
    #[serde(skip)]
    pub all_count: usize,
    pub all_counted: Sentence,
    pub all_chosen: bool,
    pub rows: Vec<FoundMod>,
}

pub enum Shown<'a> {
    Mod {
        name: Cow<'a, Sentence>,
        matches: usize,
        files: usize,
        collapsed: bool,
    },
    File {
        path: Cow<'a, str>,
        matches: usize,
        collapsed: bool,
    },
    /// A gap's `number` is that of the line after it.
    Line {
        number: usize,
        matched: bool,
        gap: bool,
        parts: Vec<(Cow<'a, str>, bool)>,
    },
}

impl Shown<'_> {
    pub fn into_owned(self) -> Shown<'static> {
        match self {
            Shown::Mod {
                name,
                matches,
                files,
                collapsed,
            } => Shown::Mod {
                name: Cow::Owned(name.into_owned()),
                matches,
                files,
                collapsed,
            },
            Shown::File {
                path,
                matches,
                collapsed,
            } => Shown::File {
                path: Cow::Owned(path.into_owned()),
                matches,
                collapsed,
            },
            Shown::Line {
                number,
                matched,
                gap,
                parts,
            } => Shown::Line {
                number,
                matched,
                gap,
                parts: parts
                    .into_iter()
                    .map(|(text, mark)| (Cow::Owned(text.into_owned()), mark))
                    .collect(),
            },
        }
    }
}

pub struct Located {
    pub file: PathBuf,
    pub line: usize,
    pub text: String,
}

fn visible_hits(file: &FileGroup, group_name: &str, filter: &str) -> Vec<u32> {
    let everything = filter.is_empty()
        || group_name.contains(filter)
        || file.shown.to_lowercase().contains(filter);
    (0..file.hits.len() as u32)
        .filter(|&at| everything || file.hits[at as usize].text.to_lowercase().contains(filter))
        .collect()
}

impl Results {
    pub fn capped(&self) -> bool {
        self.capped
    }

    pub fn files_found(&self) -> usize {
        self.files_found
    }

    pub fn mods_found(&self) -> usize {
        self.mods.len() + usize::from(self.by_key.contains_key(&None))
    }

    pub fn mods(&self) -> &HashSet<ModId> {
        &self.mods
    }

    pub fn is_empty(&self) -> bool {
        self.groups.iter().all(|g| g.rows == 0)
    }

    pub fn add(
        &mut self,
        mut found: FileHits<Option<ModId>>,
        shown: String,
        name: impl FnOnce() -> Sentence,
    ) -> Vec<Op> {
        self.files_found += 1;
        if let Some(key) = found.key {
            self.mods.insert(key);
        }
        let room = MOST_HITS - self.stored;
        if found.hits.len() > room {
            found.hits.truncate(room);
            self.capped = true;
        }
        self.capped |= found.unkept > 0;
        if found.hits.is_empty() {
            return Vec::new();
        }
        self.stored += found.hits.len();
        let at = match self.by_key.get(&found.key) {
            Some(&at) => at,
            None => {
                let hidden = self
                    .picked
                    .is_some_and(|picked| picked != self.groups.len());
                self.groups.push(Group {
                    name: name(),
                    files: Vec::new(),
                    collapsed: false,
                    hidden,
                    rows: 0,
                    matches: 0,
                    shown_files: 0,
                });
                self.by_key.insert(found.key, self.groups.len() - 1);
                self.groups.len() - 1
            }
        };
        let start: usize = self.groups[..at].iter().map(|g| g.rows).sum();
        let group = &mut self.groups[at];
        let mut file = FileGroup {
            path: found.file,
            shown,
            hits: found.hits,
            visible: Vec::new(),
            lines: Vec::new(),
            collapsed: false,
        };
        file.refilter(&group.name_text(), &self.filter);
        let (had, before) = (group.rows > 0, group.rows);
        let file_at = group.files.len();
        let shown_now = !file.visible.is_empty();
        let shown_lines = file.lines.len() as u32;
        group.files.push(file);
        group.recount();
        if !shown_now || group.hidden {
            return Vec::new();
        }
        let mut rows = Vec::new();
        if !had {
            rows.push(RowRef::of_mod(at));
        }
        if !group.collapsed {
            rows.push(RowRef::of_file(at, file_at));
            rows.extend((0..shown_lines).map(|l| RowRef::of_hit(at, file_at, l)));
        }
        let mut ops = Vec::new();
        if !rows.is_empty() {
            ops.push(Op::Insert {
                at: start + before,
                rows,
            });
        }
        if had {
            ops.push(Op::Changed { at: start });
        }
        ops
    }

    pub fn refs(&self) -> Vec<RowRef> {
        let mut rows = Vec::new();
        for (g, group) in self.groups.iter().enumerate() {
            if group.rows == 0 {
                continue;
            }
            rows.push(RowRef::of_mod(g));
            if !group.collapsed {
                self.files_of(g, &mut rows);
            }
        }
        rows
    }

    fn files_of(&self, g: usize, into: &mut Vec<RowRef>) {
        for (f, file) in self.groups[g].files.iter().enumerate() {
            if file.visible.is_empty() {
                continue;
            }
            into.push(RowRef::of_file(g, f));
            if !file.collapsed {
                into.extend((0..file.lines.len() as u32).map(|l| RowRef::of_hit(g, f, l)));
            }
        }
    }

    pub fn set_filter(&mut self, text: &str) -> Vec<RowRef> {
        self.filter = text.trim().to_lowercase();
        for group in &mut self.groups {
            let name = group.name_text();
            for file in &mut group.files {
                file.refilter(&name, &self.filter);
            }
            group.recount();
        }
        self.refs()
    }

    pub fn set_pick(&mut self, group: Option<usize>) -> Vec<RowRef> {
        self.picked = group.filter(|&at| at < self.groups.len());
        for (at, one) in self.groups.iter_mut().enumerate() {
            one.hidden = self.picked.is_some_and(|picked| picked != at);
            one.recount();
        }
        self.refs()
    }

    pub fn found_mods(&self) -> FoundMods {
        let rows = (self.groups.iter().enumerate())
            .filter(|(_, g)| g.matches > 0)
            .map(|(index, g)| FoundMod {
                index,
                name: g.name.clone(),
                count: g.matches,
                counted: Sentence::Number(g.matches as i64),
                chosen: self.picked == Some(index),
            })
            .collect();
        let all: usize = self.groups.iter().map(|g| g.matches).sum();
        FoundMods {
            all_label: say!("All mods"),
            all_count: all,
            all_counted: Sentence::Number(all as i64),
            all_chosen: self.picked.is_none(),
            rows,
        }
    }

    pub fn is_folded(&self, at: RowRef) -> bool {
        let file = self
            .groups
            .get(at.group as usize)
            .and_then(|g| g.files.get(at.file as usize));
        file.is_some_and(|f| f.collapsed)
    }

    fn selectable(&self, at: RowRef) -> bool {
        if at.is_mod() {
            return false;
        }
        at.is_file() || !matches!(self.shown(at), Some(Shown::Line { gap: true, .. }))
    }

    /// The position of the file or line row `by` such rows from `from` in `rows`, none past
    /// the ends; with no `from`, the first or the last.
    pub fn step(&self, rows: &[RowRef], from: Option<usize>, by: i32) -> Option<usize> {
        let down = by > 0;
        let mut at =
            from.map(|at| at as isize)
                .unwrap_or(if down { -1 } else { rows.len() as isize });
        let mut left = by.unsigned_abs();
        let mut found = None;
        while left > 0 {
            at += if down { 1 } else { -1 };
            let Some(row) = usize::try_from(at).ok().and_then(|at| rows.get(at)) else {
                break;
            };
            if self.selectable(*row) {
                found = Some(at as usize);
                left -= 1;
            }
        }
        found
    }

    pub fn toggle(&mut self, at: RowRef) -> Option<Folded> {
        let g = at.group as usize;
        let group = self.groups.get_mut(g)?;
        if at.is_mod() {
            group.collapsed = !group.collapsed;
        } else if at.is_file() {
            let file = group.files.get_mut(at.file as usize)?;
            file.collapsed = !file.collapsed;
        } else {
            return None;
        }
        group.recount();
        let mut shown = Vec::new();
        let group = &self.groups[g];
        if at.is_mod() && !group.collapsed {
            self.files_of(g, &mut shown);
        } else if at.is_file() {
            let f = at.file as usize;
            let file = &group.files[f];
            if !file.collapsed {
                shown.extend((0..file.lines.len() as u32).map(|l| RowRef::of_hit(g, f, l)));
            }
        }
        Some(match shown.is_empty() {
            true => Folded::Hid,
            false => Folded::Showed(shown),
        })
    }

    pub fn shown(&self, at: RowRef) -> Option<Shown<'_>> {
        let group = self.groups.get(at.group as usize)?;
        if at.is_mod() {
            return Some(Shown::Mod {
                name: Cow::Borrowed(&group.name),
                matches: group.matches,
                files: group.shown_files,
                collapsed: group.collapsed,
            });
        }
        let file = group.files.get(at.file as usize)?;
        if at.is_file() {
            return Some(Shown::File {
                path: Cow::Borrowed(&file.shown),
                matches: file.visible.len(),
                collapsed: file.collapsed,
            });
        }
        let line = file.lines.get(at.hit as usize)?;
        Some(Shown::Line {
            number: line.number,
            matched: line.is_match(),
            gap: line.gap,
            parts: line
                .parts()
                .into_iter()
                .map(|(text, mark)| (Cow::Borrowed(text), mark))
                .collect(),
        })
    }

    pub fn locate(&self, at: RowRef) -> Option<Located> {
        let file = self
            .groups
            .get(at.group as usize)?
            .files
            .get(at.file as usize)?;
        let line = if at.is_file() {
            file.lines.iter().find(|l| l.is_match())?
        } else {
            file.lines.get(at.hit as usize)?
        };
        Some(Located {
            file: file.path.clone(),
            line: line.number,
            text: line.text.clone(),
        })
    }
}

impl Folded {
    /// The rows shown after the fold at `row`, and where the row chosen went: none once hidden.
    pub fn apply(
        self,
        rows: &mut Vec<RowRef>,
        row: usize,
        current: Option<usize>,
    ) -> Option<usize> {
        let at = rows[row];
        match self {
            Folded::Hid => {
                let inside = |other: &&RowRef| match at.is_mod() {
                    true => other.same_mod(at),
                    false => other.same_file(at),
                };
                let below = rows[row + 1..].iter().take_while(inside).count();
                rows.drain(row + 1..row + 1 + below);
                match current {
                    Some(current) if current > row + below => Some(current - below),
                    Some(current) if current > row => None,
                    current => current,
                }
            }
            Folded::Showed(coming) => {
                let count = coming.len();
                rows.splice(row + 1..row + 1, coming);
                current.map(|current| {
                    if current > row {
                        current + count
                    } else {
                        current
                    }
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_folded_file_takes_its_lines_and_the_row_chosen_among_them() {
        let mut rows = vec![
            RowRef::of_mod(0),
            RowRef::of_file(0, 0),
            RowRef::of_hit(0, 0, 0),
            RowRef::of_hit(0, 0, 1),
            RowRef::of_file(0, 1),
        ];
        let lines = rows[2..4].to_vec();
        assert_eq!(Folded::Hid.apply(&mut rows, 1, Some(3)), None);
        assert_eq!(rows.len(), 3);
        assert_eq!(Folded::Showed(lines).apply(&mut rows, 1, Some(2)), Some(4));
        assert_eq!(rows.len(), 5);
        assert_eq!(Folded::Hid.apply(&mut rows, 1, Some(4)), Some(2));
        assert_eq!(Folded::Hid.apply(&mut rows, 0, Some(0)), Some(0));
        assert_eq!(rows.len(), 1);
    }

    fn hit(line: usize, text: &str, word: &str) -> Hit {
        let from = text.find(word).unwrap();
        Hit {
            line,
            before: Vec::new(),
            text: text.into(),
            marked: from..from + word.len(),
            after: Vec::new(),
        }
    }

    fn file(n: usize, name: &str, hits: usize) -> FileHits<Option<ModId>> {
        FileHits {
            key: Some(ModId::new(0, n)),
            file: PathBuf::from(format!("/mods/{n}/{name}")),
            hits: (1..=hits).map(|l| hit(l, "a Steel b", "Steel")).collect(),
            unkept: 0,
        }
    }

    fn put(results: &mut Results, found: FileHits<Option<ModId>>) -> Vec<Op> {
        let n = found.key.map_or(0, ModId::to_packed);
        let shown = found
            .file
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        results.add(found, shown, || Sentence::Plain(format!("Mod {n}")))
    }

    fn apply(rows: &mut Vec<RowRef>, ops: Vec<Op>) {
        for op in ops {
            if let Op::Insert { at, rows: new } = op {
                rows.splice(at..at, new);
            }
        }
    }

    #[test]
    fn files_arriving_in_any_order_land_where_a_full_listing_puts_them() {
        let mut results = Results::default();
        let mut shown = Vec::new();
        for (n, name) in [
            (1, "a.xml"),
            (2, "b.xml"),
            (1, "c.xml"),
            (3, "d.xml"),
            (2, "e.xml"),
        ] {
            let ops = put(&mut results, file(n, name, 2));
            apply(&mut shown, ops);
            assert_eq!(shown, results.refs());
        }
        assert_eq!(shown.len(), 3 + 5 + 10);
        assert_eq!(results.mods_found(), 3);
    }

    #[test]
    fn a_folded_mod_takes_no_rows_from_a_file_that_comes_after() {
        let mut results = Results::default();
        let mut shown = Vec::new();
        let ops = put(&mut results, file(1, "a.xml", 1));
        apply(&mut shown, ops);
        let header = shown[0];
        assert_eq!(results.toggle(header), Some(Folded::Hid));
        shown.truncate(1);
        let ops = put(&mut results, file(1, "b.xml", 1));
        assert_eq!(ops, vec![Op::Changed { at: 0 }]);
        assert_eq!(results.refs(), shown);
        let Some(Folded::Showed(back)) = results.toggle(header) else {
            panic!("it opens again");
        };
        assert_eq!(back.len(), 4);
    }

    #[test]
    fn a_filter_keeps_the_hits_with_the_text_and_the_files_and_mods_named_by_it() {
        let mut results = Results::default();
        let mut found = file(1, "a.xml", 1);
        found.hits.push(hit(9, "other Steel", "Steel"));
        put(&mut results, found);
        put(&mut results, file(2, "roads.xml", 1));
        assert_eq!(results.set_filter("other").len(), 3);
        assert_eq!(results.set_filter("ROADS").len(), 3);
        assert_eq!(results.set_filter("mod 1").len(), 5);
        assert!(results.set_filter("nothing").is_empty());
        assert!(results.is_empty());
        assert_eq!(results.set_filter("").len(), 8);
    }

    #[test]
    fn picking_a_mod_shows_only_its_rows_and_the_list_keeps_every_count() {
        let mut results = Results::default();
        let mut shown = Vec::new();
        for (n, name) in [(1, "a.xml"), (2, "b.xml")] {
            let ops = put(&mut results, file(n, name, 2));
            apply(&mut shown, ops);
        }
        let list = results.found_mods();
        assert_eq!(
            list.rows.iter().map(|m| m.count).collect::<Vec<_>>(),
            [2, 2]
        );
        assert_eq!((list.all_count, list.all_chosen), (4, true));
        let only = results.set_pick(Some(1));
        assert_eq!(only.len(), 4);
        assert!(only.iter().all(|r| r.same_mod(only[0])));
        let ops = put(&mut results, file(1, "c.xml", 1));
        assert!(ops.is_empty());
        let ops = put(&mut results, file(2, "d.xml", 1));
        assert!(!ops.is_empty());
        let list = results.found_mods();
        assert_eq!(
            list.rows
                .iter()
                .map(|m| (m.count, m.chosen))
                .collect::<Vec<_>>(),
            [(3, false), (3, true)]
        );
        assert_eq!(results.set_pick(None).len(), results.refs().len());
        assert_eq!(results.refs().len(), 12);
    }

    #[test]
    fn hits_past_the_cap_are_dropped_but_their_mods_are_still_counted() {
        let mut results = Results::default();
        put(&mut results, file(1, "a.xml", MOST_HITS - 1));
        put(&mut results, file(2, "b.xml", 5));
        assert!(results.capped());
        let ops = put(&mut results, file(3, "c.xml", 1));
        assert!(ops.is_empty());
        assert_eq!(results.mods_found(), 3);
        assert_eq!(results.files_found(), 3);
        let rows = results.refs();
        assert_eq!(rows.len(), (MOST_HITS - 1 + 2) + 1 + 2);
    }

    #[test]
    fn a_line_row_has_its_own_number_marks_and_context_and_a_file_row_locates_its_first_match() {
        let mut results = Results::default();
        let mut found = file(1, "a.xml", 0);
        found.hits.push(Hit {
            line: 5,
            before: vec!["four".into()],
            text: "x Steel y Steel".into(),
            marked: 2..7,
            after: vec!["six".into()],
        });
        found.hits.push(Hit {
            line: 5,
            before: vec!["four".into()],
            text: "x Steel y Steel".into(),
            marked: 10..15,
            after: vec!["six".into()],
        });
        put(&mut results, found);
        let rows = results.refs();
        assert_eq!(rows.len(), 2 + 3);
        let line = |at| match results.shown(at) {
            Some(Shown::Line {
                number,
                matched,
                parts,
                ..
            }) => (number, matched, parts),
            _ => panic!("not a line"),
        };
        assert_eq!(
            [line(rows[2]).0, line(rows[3]).0, line(rows[4]).0],
            [4, 5, 6]
        );
        assert!(line(rows[3]).1 && !line(rows[2]).1);
        let parts = line(rows[3]).2;
        let marked: Vec<&str> = parts.iter().filter(|p| p.1).map(|p| p.0.as_ref()).collect();
        assert_eq!(marked, ["Steel", "Steel"]);
        let located = results.locate(rows[1]).unwrap();
        assert_eq!(
            (located.line, located.file),
            (5, PathBuf::from("/mods/1/a.xml"))
        );
        assert_eq!(results.locate(rows[4]).unwrap().line, 6);
        assert!(results.locate(rows[0]).is_none());
    }

    #[test]
    fn stepping_goes_from_file_to_line_row_passing_mod_rows_and_gaps_and_stops_at_the_ends() {
        let mut results = Results::default();
        let mut found = file(1, "a.xml", 1);
        found.hits.push(hit(9, "other Steel", "Steel"));
        put(&mut results, found);
        put(&mut results, file(2, "b.xml", 1));
        let rows = results.refs();
        let is_gap = |at| matches!(results.shown(at), Some(Shown::Line { gap: true, .. }));
        assert!(rows[0].is_mod() && rows[1].is_file() && is_gap(rows[3]) && rows[5].is_mod());
        assert_eq!(results.locate(rows[3]).unwrap().line, 9);
        let step = |from, by| results.step(&rows, from, by);
        assert_eq!(step(None, 1), Some(1));
        assert_eq!(step(None, -1), Some(7));
        assert_eq!(step(Some(1), 1), Some(2));
        assert_eq!(step(Some(2), 1), Some(4));
        assert_eq!(step(Some(4), 1), Some(6));
        assert_eq!(step(Some(6), -1), Some(4));
        assert_eq!(step(Some(4), -1), Some(2));
        assert_eq!(step(Some(1), -1), None);
        assert_eq!(step(Some(7), 1), None);
        assert_eq!(step(Some(2), 3), Some(7));
        assert_eq!(step(Some(2), 0), None);
    }

    #[test]
    fn a_file_is_folded_when_it_or_a_line_of_it_is_asked() {
        let mut results = Results::default();
        put(&mut results, file(1, "a.xml", 1));
        let rows = results.refs();
        assert!(!results.is_folded(rows[2]));
        results.toggle(rows[1]);
        assert!(results.is_folded(rows[1]) && results.is_folded(rows[2]));
    }
}

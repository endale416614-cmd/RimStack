use super::{LoadOrder, Preview, rules};
use crate::App;
use rimstack_core::base::sentence::Sentence;
use rimstack_core::mods::describe;
use rimstack_core::mods::library::ModId;
use rimstack_core::mods::list::{Category, Mod, Row};
use rimstack_core::mods::rules::Kind;
use rimstack_core::mods::sort_reasons::{self, Why};
use rimstack_core::{say, say_n};
use serde::Serialize;

pub struct SortCount {
    mods: Vec<Mod>,
    detected: rules::Detected,
    reading: std::sync::Arc<crate::workspace::Loaded>,
}

impl SortCount {
    pub fn moves(mut self, app: &App) -> usize {
        let loaded = self.reading.clone();
        let Ok((library, _)) = &*loaded else {
            return 0;
        };
        let (_, sorted) = rules::sorted_by_category(app, &mut self.detected, library, self.mods);
        sorted.moved().iter().filter(|moved| **moved).count()
    }
}

#[derive(Serialize)]
pub struct SortGroup {
    pub name: &'static str,
    pub moves: Vec<SortMove>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SortMove {
    pub name: String,
    pub from: usize,
    pub to: usize,
    /// -1 for a mod that is not installed.
    pub mod_id: i32,
    pub why: Sentence,
}

impl LoadOrder {
    pub fn preview_sort(&mut self, app: &App) {
        if self.preview.is_some() || !self.is_installed() {
            return;
        }
        let loaded = self.reading(app);
        let Ok((library, _)) = &*loaded else {
            return;
        };
        let started = std::time::Instant::now();
        let before = self.active.mods().to_vec();
        let (mods, sorted) =
            rules::sorted_by_category(app, &mut self.detected, library, before.clone());
        let moved = sorted.moved();
        rimstack_core::note!("sort_us={}", started.elapsed().as_micros());
        let moves = moved.iter().filter(|m| **m).count();
        self.sort_note = note(&mods, &sorted, moves);
        if moves == 0 {
            return;
        }
        let names: Vec<&str> = mods.iter().map(|m| m.name.as_str()).collect();
        let categories: Vec<Category> = mods.iter().map(|m| m.category).collect();
        let why = sorted
            .order
            .iter()
            .zip(&moved)
            .map(|(&old, moved)| {
                moved.then(|| sort_reasons::moved(old, &sorted.kept, &names, &categories))
            })
            .collect();
        let moved_from = sorted
            .order
            .iter()
            .zip(moved)
            .map(|(&old, moved)| if moved { old + 1 } else { 0 })
            .collect();
        let after: Vec<Mod> = sorted.order.iter().map(|&i| mods[i].clone()).collect();
        self.preview = Some(Preview {
            before,
            grouped_before: self.active.is_grouped(),
            moved_from,
            why,
        });
        self.active.set_grouped(self.headings);
        self.active.set_mods(after);
        self.settle(app);
    }

    pub fn keep_sort(&mut self, app: &App) {
        if self.preview.is_some() {
            self.end_preview();
            self.settle(app);
        }
    }

    pub fn drop_sort(&mut self, app: &App) {
        if let Some(preview) = self.preview.take() {
            self.active.set_grouped(preview.grouped_before);
            self.active.set_mods(preview.before);
            self.sort_note.clear();
            self.settle(app);
        }
    }

    pub(super) fn end_preview(&mut self) {
        self.preview = None;
        self.sort_note.clear();
    }

    /// Off the window's thread. `None` while a sort is on show or the list is made up.
    pub fn sort_count(&self) -> Option<SortCount> {
        let reading = self.reading.clone().filter(|_| self.preview.is_none())?;
        Some(SortCount {
            mods: self.active.mods().to_vec(),
            detected: self.detected.clone(),
            reading,
        })
    }

    fn previewed_moves(&self) -> Vec<(usize, &Mod, usize, &Why)> {
        let Some(preview) = &self.preview else {
            return Vec::new();
        };
        let moved = self
            .active
            .mods()
            .iter()
            .zip(&preview.moved_from)
            .enumerate();
        moved
            .filter_map(|(now, (m, &from))| {
                let why = preview.why.get(now)?.as_ref()?;
                (from > 0).then_some((now, m, from, why))
            })
            .collect()
    }

    pub fn sort_move_count(&self) -> usize {
        self.previewed_moves().len()
    }

    /// Largest kind first.
    pub fn sort_moves(&self) -> Vec<SortGroup> {
        let reasoned = self.previewed_moves().into_iter();
        let mut groups = rimstack_core::base::grouped::by_key(reasoned, |(_, _, _, why)| why.group);
        groups.sort_by_key(|(_, moves)| std::cmp::Reverse(moves.len()));
        groups
            .into_iter()
            .map(|(name, moves)| SortGroup {
                name,
                moves: moves
                    .into_iter()
                    .map(|(now, m, from, why)| SortMove {
                        name: m.name.clone(),
                        from,
                        to: now + 1,
                        mod_id: m.installed.map_or(-1, ModId::to_qml),
                        why: why.text.clone(),
                    })
                    .collect(),
            })
            .collect()
    }

    pub(super) fn why_moved(&self, index: usize) -> Option<&Sentence> {
        let why = self.preview.as_ref()?.why.get(index)?.as_ref();
        why.map(|w| &w.text)
    }

    pub(super) fn moved_from(&self, index: usize) -> usize {
        let preview = self.preview.as_ref();
        preview
            .and_then(|p| p.moved_from.get(index).copied())
            .unwrap_or(0)
    }

    pub fn first_moved(&self) -> Option<usize> {
        self.active
            .rows()
            .iter()
            .position(|row| matches!(row, Row::Mod { index } if self.moved_from(*index) > 0))
    }
}

fn note(mods: &[Mod], sorted: &rimstack_core::mods::sort::Sorted, moves: usize) -> Vec<Sentence> {
    let mut note = vec![match moves {
        0 => say!("Already in order."),
        n => say_n!(
            n,
            "Sorting would move {} mod.",
            "Sorting would move {} mods."
        ),
    }];
    let apart = rimstack_core::mods::list::split_groups(sorted.order.iter().map(|&i| &mods[i]));
    if !apart.is_empty() {
        note.push(say_n!(
            apart.len(),
            "A rule keeps {} group in more than one place: {}.",
            "A rule keeps {} groups in more than one place: {}.",
            apart.join(", ")
        ));
    }
    let each: Vec<Sentence> = sorted
        .dropped
        .iter()
        .filter(|r| !matches!(r.kind, Kind::CommunityBottom | Kind::Inferred | Kind::Hint))
        .map(|r| describe::rule(r, mods))
        .collect();
    if !each.is_empty() {
        note.push(say_n!(
            each.len(),
            "Ignored {} rule that contradicts another: {}.",
            "Ignored {} rules that contradict another: {}.",
            Sentence::joined("; ", each).inside()
        ));
    }
    note
}

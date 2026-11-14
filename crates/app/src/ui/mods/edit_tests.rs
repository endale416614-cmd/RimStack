use super::Mods;
use crate::ui::made_up::made_up;
use crate::ui::shell::Shell;
use crate::ui::theme::Theme;
use crate::ui::{Options, install};
use crate::workspace::load_order::Side;
use gpui::{AppContext, Entity, HeadlessAppContext, size};
use rimstack_core::mods::library::ModId;
use std::sync::Arc;
use std::time::{Duration, Instant};

fn opened() -> (tempfile::TempDir, HeadlessAppContext, Entity<Mods>) {
    opened_with(|_| {})
}

pub(super) fn opened_with(
    prepare: impl FnOnce(&std::path::Path),
) -> (tempfile::TempDir, HeadlessAppContext, Entity<Mods>) {
    opened_at(None, prepare)
}

/// As `opened_with`, the text at `text_scale`.
pub(super) fn opened_at(
    text_scale: Option<f32>,
    prepare: impl FnOnce(&std::path::Path),
) -> (tempfile::TempDir, HeadlessAppContext, Entity<Mods>) {
    let (tmp, app) = made_up();
    prepare(tmp.path());
    let text = crate::ui::made_up::text_system();
    let mut cx = HeadlessAppContext::with_asset_source(text, Arc::new(crate::ui::assets::Assets));
    let options = Options {
        text_scale,
        ..Options::default()
    };
    cx.update(|cx| install(Arc::new(app), &options, cx));
    let shape = cx.update(|cx| {
        let _ = cx.global::<Theme>();
        size(crate::ui::WINDOW.0, crate::ui::WINDOW.1)
    });
    let shell = cx
        .open_window(shape, |window, cx| {
            cx.new(|cx| Shell::new(&options, window, cx))
        })
        .unwrap();
    let mods = cx
        .update(|cx| shell.read(cx).map(|shell| shell.sections.mods.clone()))
        .unwrap();
    let started = Instant::now();
    while cx.update(|cx| {
        let mods = mods.read(cx);
        mods.list.read(cx).count(Side::Active) == 0 || !mods.list.read(cx).files_read
    }) {
        assert!(
            started.elapsed() < Duration::from_secs(20),
            "the made-up mods were not read"
        );
        crate::ui::made_up::step(&cx);
    }
    (tmp, cx, mods)
}

fn names(mods: &Mods, side: Side, cx: &gpui::App) -> Vec<String> {
    mods.list
        .read(cx)
        .order
        .list(side)
        .mods()
        .iter()
        .map(|m| m.name.clone())
        .collect()
}

#[test]
fn a_mod_moved_across_comes_back_with_undo_and_goes_again_with_redo() {
    let (_tmp, mut cx, mods) = opened();
    cx.update(|cx| {
        mods.update(cx, |mods, cx| {
            let (active, inactive) = (
                names(mods, Side::Active, cx),
                names(mods, Side::Inactive, cx),
            );
            let last = active.len() - 1;
            let name = active[last].clone();
            mods.take(Side::Active, last, None, cx);
            assert!(!names(mods, Side::Active, cx).contains(&name));
            assert!(names(mods, Side::Inactive, cx).contains(&name));
            assert!(crate::ui::app(cx).session().is_dirty());
            mods.undo(cx);
            assert_eq!(names(mods, Side::Active, cx), active);
            assert_eq!(names(mods, Side::Inactive, cx), inactive);
            mods.redo(cx);
            assert!(names(mods, Side::Inactive, cx).contains(&name));
            mods.discard_changes(cx);
            assert_eq!(names(mods, Side::Active, cx), active);
        })
    });
}

#[test]
fn a_drop_moves_one_mod_or_the_mods_chosen_together() {
    let (_tmp, mut cx, mods) = opened();
    cx.update(|cx| {
        mods.update(cx, |mods, cx| {
            let before = names(mods, Side::Active, cx);
            let count = before.len();
            mods.drop_on(
                (Side::Active, count - 1, Vec::new()),
                (Side::Active, count - 3),
                cx,
            );
            let after = names(mods, Side::Active, cx);
            assert_eq!(after[count - 3], before[count - 1]);
            mods.undo(cx);
            let ids: Vec<ModId> = mods
                .list
                .read(cx)
                .order
                .list(Side::Inactive)
                .mods()
                .iter()
                .filter_map(|m| m.installed)
                .take(2)
                .collect();
            assert_eq!(ids.len(), 2, "two inactive mods to take along");
            mods.drop_on((Side::Inactive, 0, ids), (Side::Active, count), cx);
            assert_eq!(names(mods, Side::Active, cx).len(), count + 2);
        })
    });
}

#[test]
fn shift_ctrl_and_invert_choose_several_mods_in_one_list() {
    let (_tmp, mut cx, mods) = opened();
    cx.update(|cx| {
        mods.update(cx, |mods, cx| {
            let ids: Vec<ModId> = mods
                .list
                .read(cx)
                .order
                .list(Side::Active)
                .mods()
                .iter()
                .filter_map(|m| m.installed)
                .collect();
            mods.select(Some(ids[1]), true, cx);
            mods.select_through(ids[4], true, cx);
            assert_eq!(mods.list.read(cx).selection.len(), 4);
            mods.select_also(ids[2], true, cx);
            assert_eq!(mods.list.read(cx).selection.len(), 3);
            assert_eq!(mods.selected_id(cx), Some(ids[1]));
            mods.invert_selection(true, cx);
            assert_eq!(mods.list.read(cx).selection.len(), ids.len() - 3);
        })
    });
}

#[test]
fn sort_is_shown_before_it_is_kept_and_save_writes_the_list() {
    let (tmp, mut cx, mods) = opened();
    cx.update(|cx| {
        mods.update(cx, |mods, cx| {
            let before = names(mods, Side::Active, cx);
            mods.preview_sort(cx);
            assert!(mods.list.read(cx).order.previewing());
            mods.end_sort(false, cx);
            assert!(!mods.list.read(cx).order.previewing());
            assert_eq!(names(mods, Side::Active, cx), before);
            let last = before.len() - 1;
            mods.take(Side::Active, last, None, cx);
            mods.save(None, cx);
        })
    });
    let started = Instant::now();
    while cx.update(|cx| mods.read(cx).list.read(cx).saving) {
        assert!(
            started.elapsed() < Duration::from_secs(20),
            "the save did not end"
        );
        crate::ui::made_up::step(&cx);
    }
    cx.update(|cx| {
        let mods = mods.read(cx);
        assert!(
            mods.status.save_problem.is_empty(),
            "{}",
            mods.status.save_problem
        );
        assert!(!crate::ui::app(cx).session().is_dirty());
    });
    let written =
        std::fs::read_to_string(tmp.path().join("fx/config/Config/ModsConfig.xml")).unwrap();
    let count = cx.update(|cx| mods.read(cx).list.read(cx).count(Side::Active));
    assert_eq!(
        written.matches("<li>").count() - known_expansions(&written),
        count
    );
}

fn known_expansions(written: &str) -> usize {
    written
        .split("<knownExpansions>")
        .nth(1)
        .and_then(|rest| rest.split("</knownExpansions>").next())
        .map_or(0, |known| known.matches("<li>").count())
}

#[test]
fn sort_counts_the_moves_it_would_make_once_the_files_are_read() {
    let (_tmp, mut cx, mods) = opened();
    let started = Instant::now();
    while cx.update(|cx| mods.read(cx).list.read(cx).sort_pending == 0) {
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "Sort never counted its moves"
        );
        crate::ui::made_up::step(&cx);
    }
}

#[test]
fn a_category_folded_stays_folded_when_a_mod_leaves_the_folders() {
    let (_tmp, mut cx, mods) = opened();
    let (folded, gone) = cx.update(|cx| {
        let inactive = mods
            .read(cx)
            .list
            .read(cx)
            .order
            .list(Side::Inactive)
            .mods();
        let gone = inactive.last().and_then(|m| m.installed).unwrap();
        let loaded = crate::ui::app(cx).library();
        let (library, _) = loaded.as_ref().as_ref().unwrap();
        (
            inactive[0].category,
            library.get(gone).unwrap().path.clone(),
        )
    });
    cx.update(|cx| {
        mods.update(cx, |mods, cx| {
            mods.toggle_category(Side::Inactive, folded, cx)
        })
    });
    let before = cx.update(|cx| mods.read(cx).list.read(cx).count(Side::Inactive));
    std::fs::remove_dir_all(gone).unwrap();
    cx.update(|cx| crate::ui::app(cx).rescan_soon());
    let started = Instant::now();
    while cx.update(|cx| mods.read(cx).list.read(cx).count(Side::Inactive) == before) {
        assert!(
            started.elapsed() < Duration::from_secs(20),
            "the mods were not read again"
        );
        crate::ui::made_up::step(&cx);
    }
    let heading = cx.update(|cx| {
        let rows = mods.read(cx).inactive.rows.clone();
        rows.iter()
            .find(|r| r.is_header && r.category == folded)
            .cloned()
    });
    assert!(
        heading.is_some_and(|h| h.collapsed),
        "{folded:?} was unfolded"
    );
}

#[test]
fn a_game_folder_chosen_reads_the_mods_from_it() {
    let (tmp, mut cx, mods) = opened();
    let gone = cx.update(|cx| {
        let inactive = mods
            .read(cx)
            .list
            .read(cx)
            .order
            .list(Side::Inactive)
            .mods();
        let gone = inactive.last().and_then(|m| m.installed).unwrap();
        let loaded = crate::ui::app(cx).library();
        let (library, _) = loaded.as_ref().as_ref().unwrap();
        library.get(gone).unwrap().path.clone()
    });
    let before = cx.update(|cx| mods.read(cx).list.read(cx).count(Side::Inactive));
    std::fs::remove_dir_all(gone).unwrap();
    let game = tmp.path().join("fx/game");
    cx.update(|cx| {
        let app = crate::ui::app(cx);
        app.publish(crate::changes::Changes::LIBRARY);
        app.keep_setting("game", game.to_str().unwrap())
    })
    .unwrap();
    cx.run_until_parked();
    let list = cx.update(|cx| mods.read(cx).list.clone());
    cx.update(|cx| {
        let list = list.read(cx);
        assert!(!list.rescanning);
        assert_eq!(list.count(Side::Inactive), before - 1);
    });
}

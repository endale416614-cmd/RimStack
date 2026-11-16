use super::Mods;
use super::edit_tests::opened_with;
use super::notes::Picking;
use crate::workspace::load_order::Side;
use gpui::{Entity, HeadlessAppContext};
use rimstack_core::mods::library::ModId;

fn active_id(mods: &Mods, nth: usize) -> ModId {
    mods.active
        .rows
        .iter()
        .filter(|r| !r.is_header)
        .nth(nth)
        .unwrap()
        .mod_id
        .unwrap()
}

fn chosen(cx: &mut HeadlessAppContext, mods: &Entity<Mods>, nth: usize) -> ModId {
    cx.update(|cx| {
        mods.update(cx, |mods, cx| {
            let id = active_id(mods, nth);
            mods.choose(Side::Active, id, cx);
            id
        })
    })
}

pub(super) fn shared_textures(root: &std::path::Path) {
    for (n, shade) in [("035", 60u8), ("038", 200u8)] {
        let folder = root.join(format!("fx/game/Mods/fixture-{n}/Textures/Things"));
        std::fs::create_dir_all(&folder).unwrap();
        for name in ["Shared", "Wall"] {
            image::RgbaImage::from_pixel(4, 4, image::Rgba([shade, 90, 40, 255]))
                .save(folder.join(format!("{name}.png")))
                .unwrap();
        }
    }
}

#[test]
fn picking_a_mod_while_asked_adds_a_rule_of_your_own() {
    let (_tmp, mut cx, mods) = opened_with(|_| {});
    let id = chosen(&mut cx, &mods, 5);
    cx.update(|cx| {
        mods.update(cx, |mods, cx| {
            mods.start_picking(Some(Picking::After), cx);
            let other = active_id(mods, 9);
            mods.choose(Side::Active, other, cx);
            assert!(
                mods.chosen_reads.picking.is_none(),
                "a pick ends the picking"
            );
            assert_eq!(
                mods.selected_id(cx),
                Some(id),
                "the mod chosen stays chosen"
            );
            let rules = &mods.details.as_ref().unwrap().your_rules;
            assert_eq!(rules.len(), 1);
            assert_eq!((rules[0].first, rules[0].then), (other, id));
        })
    });
}

#[test]
fn tags_and_a_note_typed_are_kept_for_the_mod_they_were_typed_for() {
    let (_tmp, mut cx, mods) = opened_with(|_| {});
    chosen(&mut cx, &mods, 4);
    cx.update(|cx| {
        mods.update(cx, |mods, cx| {
            mods.draft_note("worn, kept".into(), "First line\nsecond".into(), cx);
            mods.keep_drafts(cx);
            let details = mods.details.as_ref().unwrap();
            assert_eq!(details.tags, "worn, kept");
            assert_eq!(details.note, "First line\nsecond");
            assert!(mods.chosen_reads.notes_saved);
            mods.chosen_reads.notes_saved = false;
            mods.keep_drafts(cx);
            assert!(
                !mods.chosen_reads.notes_saved,
                "nothing waiting is not said saved"
            );
        })
    });
}

#[test]
fn keeping_all_textures_from_the_earlier_mod_sets_the_later_ones_aside() {
    let (_tmp, mut cx, mods) = opened_with(shared_textures);
    cx.update(|cx| mods.update(cx, |mods, cx| mods.compare_first(cx)));
    let view = cx
        .update(|cx| mods.read(cx).comparing.clone())
        .expect("a comparison is shown");
    let comparison = cx.update(|cx| view.read(cx).comparison()).unwrap();
    assert_eq!((comparison.total, comparison.kept_earlier), (2, 0));
    cx.update(|cx| mods.update(cx, |mods, cx| mods.keep_all_compared(true, cx)));
    cx.run_until_parked();
    let comparison = cx.update(|cx| view.read(cx).comparison()).unwrap();
    assert_eq!(comparison.kept_earlier, 2);
    let told = cx.update(|cx| mods.read(cx).compared.clone()).unwrap();
    assert_eq!(told.kept_earlier, 2, "the details tell what was kept");
    cx.update(|cx| mods.update(cx, |mods, cx| mods.close_compare(cx)));
    assert!(cx.update(|cx| mods.read(cx).comparing.is_none()));
}

#[test]
fn keys_typed_in_the_filter_of_a_comparison_keep_no_texture() {
    let (_tmp, mut cx, mods) = opened_with(shared_textures);
    cx.update(|cx| mods.update(cx, |mods, cx| mods.compare_first(cx)));
    let view = cx.update(|cx| mods.read(cx).comparing.clone()).unwrap();
    let window = cx.update(|cx| cx.windows()[0]);
    use gpui::Focusable;
    cx.update_window(window, |_, window, cx| {
        let field = view.read(cx).filter.clone();
        window.focus(&field.focus_handle(cx), cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(window, |_, window, cx| {
        window.dispatch_keystroke(gpui::Keystroke::parse("2").unwrap(), cx);
    })
    .unwrap();
    cx.run_until_parked();
    let comparison = cx.update(|cx| view.read(cx).comparison()).unwrap();
    assert_eq!(
        comparison.yours, 0,
        "a key typed in the filter chose nothing"
    );
    assert_eq!(
        cx.update(|cx| view.read(cx).filter.read(cx).text().to_string()),
        "2"
    );
}

#[test]
fn a_comparison_holds_the_watcher_and_ends_when_the_mods_are_read_again() {
    let (_tmp, mut cx, mods) = opened_with(shared_textures);
    cx.update(|cx| mods.update(cx, |mods, cx| mods.compare_first(cx)));
    assert!(cx.update(|cx| mods.read(cx).held(cx)));
    let app = cx.update(|cx| crate::ui::app(cx));
    crate::jobs::waiting_here(|| app.rescan());
    cx.run_until_parked();
    assert!(cx.update(|cx| mods.read(cx).comparing.is_none()));
    assert!(!cx.update(|cx| mods.read(cx).held(cx)));
}

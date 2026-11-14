use super::Mods;
use crate::ui::layout_checks::{TEXT_SIZES, faults};
use crate::ui::made_up::made_up;
use crate::ui::shell::Shell;
use crate::ui::theme::Theme;
use crate::ui::widgets::watch::{self, Seen};
use crate::ui::{Options, install};
use crate::workspace::load_order::Side;
use gpui::{AppContext, Context, HeadlessAppContext, Pixels, Size, size};
use rimstack_core::mods::list::Category;
use std::sync::Arc;
use std::time::{Duration, Instant};

type Act = fn(&mut Mods, &mut Context<Mods>);

type Shape<'a> = &'a dyn Fn(&Theme) -> Size<Pixels>;

fn drawn(
    text_scale: f32,
    window: &dyn Fn(&Theme) -> Size<Pixels>,
    options: &Options,
    prepare: fn(&std::path::Path),
    act: Act,
) -> Vec<Seen> {
    let (tmp, app) = made_up();
    prepare(tmp.path());
    let text = crate::ui::made_up::text_system();
    let mut cx = HeadlessAppContext::with_asset_source(text, Arc::new(crate::ui::assets::Assets));
    let options = Options {
        still: true,
        text_scale: Some(text_scale),
        ..options.clone()
    };
    cx.update(|cx| install(Arc::new(app), &options, cx));
    let shape = cx.update(|cx| window(cx.global::<Theme>()));
    let shell = cx
        .open_window(shape, |window, cx| {
            cx.new(|cx| Shell::new(&options, window, cx))
        })
        .unwrap();
    let mods = cx.update(|cx| shell.read(cx).unwrap().sections.mods.clone());
    let started = Instant::now();
    while cx.update(|cx| {
        let mods = mods.read(cx);
        mods.list.read(cx).count(Side::Active) == 0
            || !mods.list.read(cx).files_read
            || mods.list.read(cx).overview.is_none()
    }) {
        assert!(
            started.elapsed() < Duration::from_secs(20),
            "the mods were not read"
        );
        crate::ui::made_up::step(&cx);
    }
    cx.update(|cx| mods.update(cx, act));
    // The test clock stands still: what slides or fades is let finish, as the checks are of the
    // window at rest.
    cx.advance_clock(Duration::from_secs(1));
    for _ in 0..3 {
        cx.run_until_parked();
        cx.update_window(shell.into(), |_, window, cx| window.draw(cx).clear(cx))
            .unwrap();
    }
    watch::start();
    cx.update(|cx| {
        shell.update(cx, |_, _, cx| cx.notify()).unwrap();
        mods.update(cx, |mods, cx| {
            cx.notify();
            mods.details_view.update(cx, |_, cx| cx.notify());
            for side in [Side::Active, Side::Inactive] {
                mods.list(side).update(cx, |_, cx| cx.notify());
            }
            if let Some(view) = mods.comparing.clone() {
                view.update(cx, |_, cx| cx.notify());
            }
        });
        crate::ui::menu::redraw(cx);
    });
    cx.update_window(shell.into(), |_, window, cx| window.draw(cx).clear(cx))
        .unwrap();
    let seen = watch::take();
    drop(tmp);
    seen
}

/// At every text size, in the first window and the smallest, nothing past its edge or over
/// another; the texts `over` holds are drawn over the page, so are checked among themselves.
fn fits(
    name: &str,
    least: usize,
    options: Options,
    prepare: fn(&std::path::Path),
    act: Act,
    over: &[&str],
) {
    fits_beside(name, least, options, prepare, act, over, 0.);
}

/// As `fits`, with what lies left of `covered` (at 100% text) under something drawn over it,
/// which only its own texts are checked in.
fn fits_beside(
    name: &str,
    least: usize,
    options: Options,
    prepare: fn(&std::path::Path),
    act: Act,
    over: &[&str],
    covered: f32,
) {
    let first = |_: &Theme| size(crate::ui::WINDOW.0, crate::ui::WINDOW.1);
    let smallest = |theme: &Theme| size(theme.sized(1000.), theme.sized(560.));
    let windows: [(&str, Shape); 2] = [("first window", &first), ("smallest window", &smallest)];
    let mut found = Vec::new();
    for (window, shape) in windows {
        for scale in TEXT_SIZES {
            let seen = drawn(scale, shape, &options, prepare, act);
            assert!(
                seen.len() > least,
                "{name}, {window}: only {} texts drawn at {scale}",
                seen.len()
            );
            let (above, below): (Vec<Seen>, Vec<Seen>) = seen
                .into_iter()
                .partition(|one| over.iter().any(|text| one.text.as_ref() == *text));
            let edge = gpui::px(covered * scale);
            let below: Vec<Seen> = below
                .into_iter()
                .filter(|one| one.bounds.left() >= edge)
                .collect();
            for part in [above, below] {
                found.extend(
                    faults(&part)
                        .into_iter()
                        .map(|fault| format!("{name}, {window} at {scale}: {fault}")),
                );
            }
        }
    }
    assert!(
        found.is_empty(),
        "{} faults:\n{}",
        found.len(),
        found.join("\n")
    );
}

fn nothing(_: &std::path::Path) {}

fn none(_: &mut Mods, _: &mut Context<Mods>) {}

#[test]
fn each_details_tab_of_a_mod_fits() {
    for tab in 0..3 {
        let options = Options {
            pick: Some(3),
            tab,
            ..Options::default()
        };
        fits(
            &format!("details tab {tab}"),
            40,
            options,
            nothing,
            none,
            &[],
        );
    }
}

#[test]
fn sorts_preview_fits() {
    fits(
        "Sort's preview",
        40,
        Options::default(),
        nothing,
        |mods, cx| mods.preview_sort(cx),
        &["Content"],
    );
}

#[test]
fn the_rail_and_its_drawer_fit() {
    let drawer = [
        "Inactive",
        "Pin",
        "Category",
        "Filter active and inactive mods",
        "Ctrl + F",
    ];
    let open: Act = |mods, cx| {
        mods.set_hidden(true, cx);
        mods.set_drawer(true, cx);
    };
    fits_beside(
        "the drawer",
        40,
        Options::default(),
        nothing,
        open,
        &drawer,
        44. + 290.,
    );
}

#[test]
fn each_hint_fits() {
    let hints = [
        "1 / 3",
        "2 / 3",
        "3 / 3",
        "Skip",
        "Next",
        "Done",
        "The arrows beside it go from one mod with a problem to the next. Its details say what is wrong and offer the fix.",
        "Sort shows the order the mods' rules ask for and what would move. Apply moves the mods; Discard leaves the list as it was.",
        "Nothing reaches the game before Save. Until then every change can be undone or discarded.",
    ];
    let steps: [Act; 3] = [
        |mods, cx| mods.show_hint(1, cx),
        |mods, cx| mods.show_hint(2, cx),
        |mods, cx| mods.show_hint(3, cx),
    ];
    for (at, step) in steps.into_iter().enumerate() {
        fits(
            &format!("hint {}", at + 1),
            40,
            Options::default(),
            nothing,
            step,
            &hints,
        );
    }
}

#[test]
fn the_filter_fits_with_its_chips() {
    let chips: Act = |mods, cx| mods.filter_by("is:warning tag:x fixture", cx);
    fits(
        "the filter's chips",
        40,
        Options::default(),
        nothing,
        chips,
        &[],
    );
}

#[test]
fn a_texture_comparison_fits() {
    let compare: Act = |mods, cx| mods.compare_first(cx);
    fits(
        "the comparison",
        20,
        Options::default(),
        super::details_tests::shared_textures,
        compare,
        &[],
    );
}

#[test]
fn every_column_fits_in_both_lists() {
    let all: Act = |mods, cx| {
        for column in ["updated", "downloaded", "activated", "size"] {
            mods.toggle_column(Side::Active, column, cx);
        }
        for column in ["author", "version", "updated", "downloaded", "size"] {
            mods.toggle_column(Side::Inactive, column, cx);
        }
    };
    fits("every column", 40, Options::default(), nothing, all, &[]);
}

/// Three Content mods under two tags, one tag split by a mod between its two.
pub(super) fn tag_apart(mods: &mut Mods, cx: &mut Context<Mods>) {
    let content: Vec<_> = {
        let list = mods.list.read(cx).order.list(Side::Active).mods();
        let in_content = list.iter().filter(|m| m.category == Category::Content);
        in_content.filter_map(|m| m.installed).take(3).collect()
    };
    mods.group(vec![content[0], content[2]], "Combat", cx);
    mods.group(vec![content[1]], "Quality of life", cx);
}

#[test]
fn the_load_order_shown_by_group_fits() {
    let by_group: Act = |mods, cx| {
        tag_apart(mods, cx);
        mods.order_active("group", cx);
    };
    fits(
        "shown by group",
        40,
        Options::default(),
        nothing,
        by_group,
        &[],
    );
}

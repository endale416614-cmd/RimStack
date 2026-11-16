use super::Find;
use crate::ui::layout_checks::check_scene;
use crate::ui::made_up::made_up;
use crate::ui::shell::Shell;
use crate::ui::theme::Theme;
use crate::ui::widgets::watch::{self, Seen};
use crate::ui::{Options, install};
use crate::workspace::find::Shown;
use gpui::{AppContext, Entity, HeadlessAppContext, Pixels, Size, WindowHandle, size};
use std::sync::Arc;
use std::time::{Duration, Instant};

type Shape<'a> = &'a dyn Fn(&Theme) -> Size<Pixels>;

type Opened = (
    tempfile::TempDir,
    HeadlessAppContext,
    WindowHandle<Shell>,
    Entity<Find>,
);

fn opened(text_scale: f32, window: Shape<'_>, text: Option<&str>) -> Opened {
    let (tmp, app) = made_up();
    let gpu_text = crate::ui::made_up::text_system();
    let mut cx =
        HeadlessAppContext::with_asset_source(gpu_text, Arc::new(crate::ui::assets::Assets));
    let options = Options {
        section: crate::ui::section::Section::Find,
        text_scale: Some(text_scale),
        find: text.map(str::to_string),
        ..Options::default()
    };
    cx.update(|cx| install(Arc::new(app), &options, cx));
    let shape = cx.update(|cx| window(cx.global::<Theme>()));
    let shell = cx
        .open_window(shape, |window, cx| {
            cx.new(|cx| Shell::new(&options, window, cx))
        })
        .unwrap();
    let find = cx
        .update(|cx| {
            shell
                .read(cx)
                .map(|shell| shell.sections.made::<crate::ui::find::Find>().unwrap())
        })
        .unwrap();
    if text.is_some() {
        wait(&mut cx, &find, "the search to end with a preview", |find| {
            !find.running && find.preview.is_some()
        });
    }
    cx.run_until_parked();
    (tmp, cx, shell, find)
}

fn wait(
    cx: &mut HeadlessAppContext,
    find: &Entity<Find>,
    what: &str,
    done: impl Fn(&Find) -> bool,
) {
    let started = Instant::now();
    while !cx.update(|cx| done(find.read(cx))) {
        assert!(
            started.elapsed() < Duration::from_secs(20),
            "waited for {what}"
        );
        crate::ui::made_up::step(cx);
    }
}

fn first_window(_: &Theme) -> Size<Pixels> {
    size(crate::ui::WINDOW.0, crate::ui::WINDOW.1)
}

#[test]
fn a_search_lists_previews_folds_and_shows_in_mods_what_it_found() {
    let (_tmp, mut cx, shell, find) = opened(1.0, &first_window, Some("Fixture"));
    cx.update(|cx| {
        let find = find.read(cx);
        let app = crate::ui::app(cx);
        let rows = find.rows.clone();
        assert!(rows.len() > 10, "{} rows", rows.len());
        assert!(rows[0].is_mod() && rows[1].is_file() && !rows[2].is_file());
        let shown = app.find_results(|r| matches!(r.shown(rows[2]), Some(Shown::Line { .. })));
        assert!(shown);
        assert_eq!(find.current, Some(2));
        assert!(find.count > 0 && !find.found.rows.is_empty());
        let preview = find.preview.as_ref().unwrap();
        assert!(preview.lines.iter().any(|l| l.marked));
        assert!(
            !find.viewing.found.is_empty(),
            "Fixture is found inside the file"
        );
    });
    cx.update(|cx| {
        find.update(cx, |find, cx| {
            let all = find.rows.len();
            find.toggle(0, cx);
            assert_eq!(find.current, None, "the line chosen is folded away");
            assert!(find.rows.len() < all);
            find.toggle(0, cx);
            assert_eq!(find.rows.len(), all);
            find.current = Some(1);
            find.step(1, cx);
            assert_eq!(find.current, Some(2));
            find.fold(-1, cx);
            assert_eq!(find.current, Some(1), "folding a line chooses its file");
            assert!(find.rows.len() < all);
            find.fold(1, cx);
            assert_eq!(find.rows.len(), all);
            find.filter_results("no such text anywhere", cx);
            assert!(find.rows.is_empty() && find.current.is_none());
            find.filter_results("", cx);
            assert_eq!(find.rows.len(), all);
            find.pick_mod(Some(find.found.rows[0].index), cx);
            assert!(find.rows.len() < all && find.rows.iter().filter(|r| r.is_mod()).count() == 1);
        })
    });
    cx.update(|cx| find.update(cx, |find, cx| find.show_in_mods(cx)));
    cx.run_until_parked();
    let (section, filter) = cx
        .update(|cx| {
            shell.read(cx).map(|shell| {
                let mods = shell.sections.mods.read(cx);
                (shell.section(), mods.filter_text(cx))
            })
        })
        .unwrap();
    assert_eq!(
        (section, filter.as_str()),
        (crate::ui::section::Section::Mods, "in:search")
    );
}

#[test]
fn file_types_offer_what_the_mods_hold_and_complete_the_one_chosen() {
    let (_tmp, mut cx, _shell, find) = opened(1.0, &first_window, None);
    wait(&mut cx, &find, "the extensions counted", |find| {
        !find.extensions.is_empty()
    });
    cx.update(|cx| {
        find.update(cx, |find, cx| {
            find.types
                .update(cx, |field, cx| field.set_text("xml, ", cx));
            let offers = find.offers(cx);
            assert!(!offers.is_empty());
            let ext = offers[0].ext.clone();
            find.complete_with(0, cx);
            assert!(find.types.read(cx).text().contains(&ext));
        })
    });
}

#[test]
fn the_filters_are_beside_the_matches_and_the_preview_where_the_window_holds_them() {
    let wide = |_: &Theme| size(gpui::px(1600.), crate::ui::WINDOW.1);
    let (_tmp, mut cx, shell, find) = opened(1.15, &wide, Some("Fixture"));
    let layout = cx
        .update_window(shell.into(), |_, window, cx| {
            find.read(cx).layout(cx.global::<Theme>(), window)
        })
        .unwrap();
    assert!(layout.wide && layout.side_shown && layout.results_shown && layout.preview_shown);
    assert!(!layout.side_fills);
}

fn drawn_with(text: Option<&'static str>, filters: bool) -> impl Fn(f32, Shape<'_>) -> Vec<Seen> {
    move |text_scale, window| {
        let (_tmp, mut cx, shell, find) = opened(text_scale, window, text);
        watch::start();
        cx.update(|cx| {
            find.update(cx, |find, cx| {
                find.filters = filters;
                cx.notify();
            })
        });
        cx.update_window(shell.into(), |_, window, cx| window.draw(cx).clear(cx))
            .unwrap();
        watch::take()
    }
}

#[test]
fn search_fits_at_every_text_size() {
    let smallest = |theme: &Theme| size(theme.sized(1000.), theme.sized(560.));
    check_scene("Search, empty", 3, &first_window, drawn_with(None, false));
    check_scene(
        "Search, results",
        10,
        &first_window,
        drawn_with(Some("Fixture"), false),
    );
    check_scene(
        "Search, results, smallest window",
        10,
        &smallest,
        drawn_with(Some("Fixture"), false),
    );
    check_scene(
        "Search, with the filters, smallest window",
        10,
        &smallest,
        drawn_with(Some("Fixture"), true),
    );
}

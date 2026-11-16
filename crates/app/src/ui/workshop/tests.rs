use super::Workshop;
use crate::ui::layout_checks::check_scene;
use crate::ui::made_up::made_up;
use crate::ui::shell::Shell;
use crate::ui::theme::Theme;
use crate::ui::widgets::watch::{self, Seen};
use crate::ui::{Options, install};
use crate::workspace::browse::Related;
use gpui::{AppContext, Entity, HeadlessAppContext, Pixels, Size, WindowHandle, size};
use std::sync::Arc;
use std::time::{Duration, Instant};

type Shape<'a> = &'a dyn Fn(&Theme) -> Size<Pixels>;

type Opened = (
    tempfile::TempDir,
    HeadlessAppContext,
    WindowHandle<Shell>,
    Entity<Workshop>,
);

fn opened(text_scale: f32, window: Shape<'_>, browse: Option<&str>) -> Opened {
    let (tmp, app) = made_up();
    let app = app.browsing_fixture(true);
    let text = crate::ui::made_up::text_system();
    let mut cx = HeadlessAppContext::with_asset_source(text, Arc::new(crate::ui::assets::Assets));
    let options = Options {
        section: crate::ui::section::Section::Workshop,
        text_scale: Some(text_scale),
        browse: browse.map(str::to_string),
        ..Options::default()
    };
    cx.update(|cx| install(Arc::new(app), &options, cx));
    let shape = cx.update(|cx| window(cx.global::<Theme>()));
    let shell = cx
        .open_window(shape, |window, cx| {
            cx.new(|cx| Shell::new(&options, window, cx))
        })
        .unwrap();
    let workshop = cx
        .update(|cx| {
            shell.read(cx).map(|shell| {
                shell
                    .sections
                    .made::<crate::ui::workshop::Workshop>()
                    .unwrap()
            })
        })
        .unwrap();
    wait(&mut cx, &workshop, "the first results", |w| {
        !w.searching && !w.rows.is_empty()
    });
    if browse == Some("details") {
        wait(&mut cx, &workshop, "the details", |w| {
            w.opened.shown.is_some()
        });
    }
    cx.run_until_parked();
    (tmp, cx, shell, workshop)
}

fn wait(
    cx: &mut HeadlessAppContext,
    workshop: &Entity<Workshop>,
    what: &str,
    done: impl Fn(&Workshop) -> bool,
) {
    let started = Instant::now();
    while !cx.update(|cx| done(workshop.read(cx))) {
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
fn the_made_up_database_is_searched_filtered_and_an_item_opened_with_its_description() {
    let (_tmp, mut cx, _shell, workshop) = opened(1.0, &first_window, Some("details"));
    cx.update(|cx| {
        let w = workshop.read(cx);
        assert!(w.ready && w.total > 0, "total {}", w.total);
        let shown = w.opened.shown.as_ref().unwrap();
        assert_eq!(shown.row.id, "3100000002");
        let parts = shown.description(&w.opened.images);
        assert!(parts.iter().any(|p| !p.blocks.is_empty()));
        assert!(!w.sorts.is_empty() && !w.sort_label.is_empty());
    });
    let all = cx.update(|cx| workshop.read(cx).total);
    let tag = cx.update(|cx| workshop.read(cx).sidebar.types[0].id.clone());
    cx.update(|cx| workshop.update(cx, |w, cx| w.toggle_tag(tag.clone(), false, cx)));
    wait(&mut cx, &workshop, "the filtered results", |w| {
        !w.searching && w.total < all
    });
    assert!(cx.update(|cx| !workshop.read(cx).sidebar.chips.is_empty()));
    cx.update(|cx| workshop.update(cx, |w, cx| w.clear_all(cx)));
    wait(&mut cx, &workshop, "every result again", |w| {
        !w.searching && w.total == all
    });
}

#[test]
fn a_related_list_replaces_the_results_until_back() {
    let (_tmp, mut cx, _shell, workshop) = opened(1.0, &first_window, None);
    let all = cx.update(|cx| workshop.read(cx).rows.len());
    cx.update(|cx| {
        workshop.update(cx, |w, cx| {
            w.show_related("3100000002".into(), Related::Requires, cx)
        })
    });
    wait(&mut cx, &workshop, "the requirements", |w| !w.searching);
    cx.update(|cx| {
        let w = workshop.read(cx);
        let (_, id, title) = w.related.as_ref().unwrap();
        assert_eq!(id, "3100000002");
        assert!(!title.is_empty(), "the subject's title is known");
    });
    cx.update(|cx| workshop.update(cx, |w, cx| w.back(cx)));
    wait(&mut cx, &workshop, "the results again", |w| !w.searching);
    cx.update(|cx| {
        let w = workshop.read(cx);
        assert!(w.related.is_none());
        assert_eq!(w.rows.len(), all);
    });
}

fn drawn_with(browse: Option<&'static str>) -> impl Fn(f32, Shape<'_>) -> Vec<Seen> {
    move |text_scale, window| {
        let (_tmp, mut cx, shell, workshop) = opened(text_scale, window, browse);
        watch::start();
        cx.update(|cx| workshop.update(cx, |_, cx| cx.notify()));
        cx.update_window(shell.into(), |_, window, cx| {
            window.refresh();
            window.draw(cx).clear(cx)
        })
        .unwrap();
        watch::take()
    }
}

#[test]
fn the_workshop_fits_at_every_text_size() {
    let smallest = |theme: &Theme| size(theme.sized(1000.), theme.sized(560.));
    check_scene(
        "Workshop, with the list",
        10,
        &first_window,
        drawn_with(Some("list")),
    );
    check_scene(
        "Workshop, with the details",
        10,
        &first_window,
        drawn_with(Some("details")),
    );
    check_scene(
        "Workshop, with the details, smallest window",
        10,
        &smallest,
        drawn_with(Some("details")),
    );
    check_scene(
        "Workshop, with the filters, smallest window",
        5,
        &smallest,
        drawn_with(Some("filters")),
    );
}

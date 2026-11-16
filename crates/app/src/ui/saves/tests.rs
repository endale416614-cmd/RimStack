use super::Saves;
use crate::ui::layout_checks::check_scene;
use crate::ui::made_up::made_up;
use crate::ui::shell::Shell;
use crate::ui::theme::Theme;
use crate::ui::widgets::watch::{self, Seen};
use crate::ui::{Options, install};
use gpui::{AppContext, Entity, HeadlessAppContext, Pixels, Size, WindowHandle, size};
use std::sync::Arc;
use std::time::{Duration, Instant};

type Shape<'a> = &'a dyn Fn(&Theme) -> Size<Pixels>;

fn opened(
    text_scale: f32,
    window: Shape<'_>,
) -> (
    tempfile::TempDir,
    HeadlessAppContext,
    WindowHandle<Shell>,
    Entity<Saves>,
) {
    let (tmp, app) = made_up();
    let text = crate::ui::made_up::text_system();
    let mut cx = HeadlessAppContext::with_asset_source(text, Arc::new(crate::ui::assets::Assets));
    let options = Options {
        section: crate::ui::section::Section::Saves,
        text_scale: Some(text_scale),
        ..Options::default()
    };
    cx.update(|cx| install(Arc::new(app), &options, cx));
    let shape = cx.update(|cx| window(cx.global::<Theme>()));
    let shell = cx
        .open_window(shape, |window, cx| {
            cx.new(|cx| Shell::new(&options, window, cx))
        })
        .unwrap();
    let saves = cx
        .update(|cx| {
            shell
                .read(cx)
                .map(|shell| shell.sections.made::<crate::ui::saves::Saves>().unwrap())
        })
        .unwrap();
    let started = Instant::now();
    while cx.update(|cx| saves.read(cx).report.rows.is_empty()) {
        assert!(
            started.elapsed() < Duration::from_secs(20),
            "the made-up saves were not read"
        );
        crate::ui::made_up::step(&cx);
    }
    cx.run_until_parked();
    (tmp, cx, shell, saves)
}

#[test]
fn the_made_up_saves_are_read_and_a_save_list_is_kept_by_its_name() {
    let (tmp, mut cx, _shell, saves) =
        opened(1.0, &|_| size(crate::ui::WINDOW.0, crate::ui::WINDOW.1));
    let names: Vec<String> = cx.update(|cx| {
        saves
            .read(cx)
            .report
            .rows
            .iter()
            .map(|r| r.name.clone())
            .collect()
    });
    assert_eq!(names.len(), 2, "{names:?}");
    cx.update(|cx| {
        saves.update(cx, |s, cx| {
            s.selected = 1;
            s.list_name
                .update(cx, |field, cx| field.set_text("Second colony", cx));
            s.keep_list(cx);
        })
    });
    cx.run_until_parked();
    let problem = cx.update(|cx| saves.read(cx).list_problem.clone());
    assert!(problem.is_empty(), "{problem}");
    let kept = std::fs::read_dir(tmp.path().join("data/RimStack/lists"))
        .map(|d| d.count())
        .unwrap_or(0);
    assert!(kept > 0, "the list is written among the kept lists");
}

fn drawn(text_scale: f32, window: Shape<'_>) -> Vec<Seen> {
    let (_tmp, mut cx, shell, saves) = opened(text_scale, window);
    watch::start();
    cx.update(|cx| saves.update(cx, |_, cx| cx.notify()));
    cx.update_window(shell.into(), |_, window, cx| window.draw(cx).clear(cx))
        .unwrap();
    watch::take()
}

#[test]
fn saves_fits_at_every_text_size() {
    check_scene(
        "Saves, first window",
        5,
        &|_| size(crate::ui::WINDOW.0, crate::ui::WINDOW.1),
        drawn,
    );
    check_scene(
        "Saves, smallest window",
        5,
        &|theme| size(theme.sized(1000.), theme.sized(560.)),
        drawn,
    );
}

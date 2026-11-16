use super::{Logs, View};
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
    view: usize,
    text_scale: f32,
    window: Shape<'_>,
) -> (
    tempfile::TempDir,
    HeadlessAppContext,
    WindowHandle<Shell>,
    Entity<Logs>,
) {
    let (tmp, app) = made_up();
    let text = crate::ui::made_up::text_system();
    let mut cx = HeadlessAppContext::with_asset_source(text, Arc::new(crate::ui::assets::Assets));
    let options = Options {
        section: crate::ui::section::Section::Logs,
        view,
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
    let logs = cx
        .update(|cx| {
            shell
                .read(cx)
                .map(|shell| shell.sections.made::<crate::ui::logs::Logs>().unwrap())
        })
        .unwrap();
    let started = Instant::now();
    while cx.update(|cx| logs.read(cx).report.is_none()) {
        assert!(
            started.elapsed() < Duration::from_secs(20),
            "the made-up log was not read"
        );
        crate::ui::made_up::step(&cx);
    }
    cx.run_until_parked();
    (tmp, cx, shell, logs)
}

#[test]
fn the_made_up_log_shows_its_two_errors_and_a_line_shown_marks_it_in_the_log() {
    let (_tmp, mut cx, shell, logs) =
        opened(0, 1.0, &|_| size(crate::ui::WINDOW.0, crate::ui::WINDOW.1));
    let errors = cx.update(|cx| {
        let logs = logs.read(cx);
        logs.errors(logs.report.as_ref().unwrap()).len()
    });
    assert_eq!(errors, 2);
    cx.update_window(shell.into(), |_, window, cx| {
        logs.update(cx, |logs, cx| logs.show_line(3, window, cx))
    })
    .unwrap();
    let (view, marked) = cx.update(|cx| (logs.read(cx).view, logs.read(cx).marked));
    assert!(
        view == View::Log && marked == 3,
        "Show in log goes to the line"
    );
}

/// Logs drawn again once its log is read: the frames before showed less.
fn drawn(view: usize) -> impl Fn(f32, Shape<'_>) -> Vec<Seen> {
    move |text_scale, window| {
        let (_tmp, mut cx, shell, logs) = opened(view, text_scale, window);
        watch::start();
        cx.update(|cx| logs.update(cx, |_, cx| cx.notify()));
        cx.update_window(shell.into(), |_, window, cx| window.draw(cx).clear(cx))
            .unwrap();
        watch::take()
    }
}

#[test]
fn every_view_of_logs_fits_at_every_text_size() {
    for view in 0..3 {
        check_scene(
            &format!("Logs view {view}, first window"),
            5,
            &|_| size(crate::ui::WINDOW.0, crate::ui::WINDOW.1),
            drawn(view),
        );
        check_scene(
            &format!("Logs view {view}, smallest window"),
            5,
            &|theme| size(theme.sized(1000.), theme.sized(560.)),
            drawn(view),
        );
    }
}

#[test]
fn logs_hidden_read_nothing_until_they_are_shown_again() {
    use crate::changes::Changes;
    use crate::ui::section::Section;
    let (_tmp, mut cx, shell, logs) = opened(0, 1.0, &|_| size(WINDOW_W, WINDOW_H));
    let read = |cx: &mut HeadlessAppContext| cx.update(|cx| logs.read(cx).report.clone().unwrap());
    let go = |cx: &mut HeadlessAppContext, section| {
        cx.update(|cx| {
            shell
                .update(cx, |shell, _, cx| shell.go(section, cx))
                .unwrap()
        });
        cx.run_until_parked();
    };
    let first = read(&mut cx);
    go(&mut cx, Section::Mods);
    cx.update(|cx| crate::ui::app(cx).publish(Changes::LOAD_ORDER));
    cx.run_until_parked();
    assert!(
        std::rc::Rc::ptr_eq(&first, &read(&mut cx)),
        "a hidden Logs read the log"
    );
    go(&mut cx, Section::Logs);
    assert!(
        !std::rc::Rc::ptr_eq(&first, &read(&mut cx)),
        "Logs shown again read what it missed"
    );
}

const WINDOW_W: Pixels = gpui::px(1440.);
const WINDOW_H: Pixels = gpui::px(860.);

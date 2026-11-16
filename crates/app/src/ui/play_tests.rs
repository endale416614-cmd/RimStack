use super::made_up::made_up;
use super::shell::Shell;
use super::{Options, install};
use crate::workspace::load_order::Side;
use gpui::{AppContext, HeadlessAppContext, WindowHandle, size};
use std::sync::Arc;
use std::time::{Duration, Instant};

fn opened() -> (tempfile::TempDir, HeadlessAppContext, WindowHandle<Shell>) {
    let (tmp, app) = made_up();
    let text = crate::ui::made_up::text_system();
    let mut cx = HeadlessAppContext::with_asset_source(text, Arc::new(super::assets::Assets));
    let options = Options {
        still: true,
        ..Options::default()
    };
    cx.update(|cx| install(Arc::new(app), &options, cx));
    let shell = cx
        .open_window(size(super::WINDOW.0, super::WINDOW.1), |window, cx| {
            cx.new(|cx| Shell::new(&options, window, cx))
        })
        .unwrap();
    let started = Instant::now();
    while active(&mut cx, shell) == 0 {
        assert!(
            started.elapsed() < Duration::from_secs(20),
            "the made-up mods were not read"
        );
        crate::ui::made_up::step(&cx);
    }
    (tmp, cx, shell)
}

fn active(cx: &mut HeadlessAppContext, shell: WindowHandle<Shell>) -> usize {
    cx.update(|cx| {
        shell
            .read(cx)
            .map(|shell| {
                shell
                    .sections
                    .mods
                    .read(cx)
                    .list
                    .read(cx)
                    .count(Side::Active)
            })
            .unwrap()
    })
}

#[test]
fn the_search_for_the_broken_mod_halves_the_list_and_gives_it_back() {
    let (_tmp, mut cx, shell) = opened();
    let all = active(&mut cx, shell);
    shell
        .update(&mut cx, |shell, _, cx| shell.start_hunt(cx))
        .unwrap();
    cx.run_until_parked();
    let (hunting, asking) = cx.update(|cx| {
        let shell = shell.read(cx).unwrap();
        (shell.hunt.active, shell.hunt.asking)
    });
    assert!(hunting && asking);
    assert!(active(&mut cx, shell) < all, "a round deactivates suspects");
    shell
        .update(&mut cx, |shell, _, cx| shell.answer_hunt(true, cx))
        .unwrap();
    cx.run_until_parked();
    assert!(cx.update(|cx| shell.read(cx).unwrap().hunt.active));
    shell
        .update(&mut cx, |shell, _, cx| shell.end_hunt(false, cx))
        .unwrap();
    cx.run_until_parked();
    assert!(!cx.update(|cx| shell.read(cx).unwrap().hunt.active));
    assert_eq!(
        active(&mut cx, shell),
        all,
        "the list the search began with comes back"
    );
}

#[test]
fn play_shows_what_is_in_its_way_before_starting() {
    let (_tmp, mut cx, shell) = opened();
    shell
        .update(&mut cx, |shell, _, cx| shell.press_play(cx))
        .unwrap();
    let (open, ways) = cx.update(|cx| {
        let shell = shell.read(cx).unwrap();
        (shell.before_play, crate::ui::around::read(cx).in_the_way)
    });
    assert!(!ways.is_empty(), "the made-up load order has an error");
    assert!(open);
}

#[test]
fn ctrl_k_finds_a_command_by_its_words_and_runs_it() {
    let (_tmp, mut cx, shell) = opened();
    shell
        .update(&mut cx, |shell, window, cx| shell.open_commands(window, cx))
        .unwrap();
    let commands = cx.update(|cx| shell.read(cx).unwrap().commands.clone().unwrap());
    cx.update(|cx| {
        commands.update(cx, |commands, cx| commands.type_text("sort load", cx));
    });
    cx.update_window(shell.into(), |_, window, cx| {
        commands.update(cx, |commands, cx| commands.run_first(window, cx));
    })
    .unwrap();
    let (closed, previewing) = cx.update(|cx| {
        let shell = shell.read(cx).unwrap();
        (
            shell.commands.is_none(),
            shell.sections.mods.read(cx).list.read(cx).previewing(),
        )
    });
    assert!(closed && previewing, "Sort was run");
}

fn first_run(
    text_scale: f32,
    window: &dyn Fn(&super::theme::Theme) -> gpui::Size<gpui::Pixels>,
    step: usize,
) -> (tempfile::TempDir, HeadlessAppContext, WindowHandle<Shell>) {
    let (tmp, app) = made_up();
    let text = crate::ui::made_up::text_system();
    let mut cx = HeadlessAppContext::with_asset_source(text, Arc::new(super::assets::Assets));
    let options = Options {
        still: true,
        first: true,
        step,
        text_scale: Some(text_scale),
        ..Options::default()
    };
    cx.update(|cx| install(Arc::new(app), &options, cx));
    let shape = cx.update(|cx| window(cx.global::<super::theme::Theme>()));
    let shell = cx
        .open_window(shape, |window, cx| {
            cx.new(|cx| Shell::new(&options, window, cx))
        })
        .unwrap();
    cx.run_until_parked();
    (tmp, cx, shell)
}

#[test]
fn start_ends_the_first_run_and_keeps_the_setup_done() {
    let (_tmp, mut cx, shell) = first_run(1.0, &|_| size(super::WINDOW.0, super::WINDOW.1), 4);
    let first = cx.update(|cx| shell.read(cx).unwrap().first_run.clone().unwrap());
    cx.update(|cx| first.update(cx, |first, cx| first.press_start(cx)));
    cx.run_until_parked();
    assert!(cx.update(|cx| shell.read(cx).unwrap().first_run.is_none()));
    assert!(cx.update(|cx| super::app(cx).settings_report().setup_done));
}

#[test]
fn every_step_of_the_first_run_fits_at_every_text_size() {
    use super::layout_checks::check_scene;
    use super::widgets::watch;
    for step in 0..5 {
        let scene =
            move |text_scale: f32,
                  window: &dyn Fn(&super::theme::Theme) -> gpui::Size<gpui::Pixels>| {
                let (_tmp, mut cx, shell) = first_run(text_scale, window, step);
                watch::start();
                let first = cx.update(|cx| shell.read(cx).unwrap().first_run.clone().unwrap());
                cx.update(|cx| first.update(cx, |_, cx| cx.notify()));
                cx.update_window(shell.into(), |_, window, cx| window.draw(cx).clear(cx))
                    .unwrap();
                watch::take()
            };
        let smallest = |theme: &super::theme::Theme| size(theme.sized(1000.), theme.sized(560.));
        check_scene(&format!("first run step {step}"), 5, &smallest, scene);
    }
}

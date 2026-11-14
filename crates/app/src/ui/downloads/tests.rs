// Most of these tests need the made-up install, which is Linux's alone.
#![cfg_attr(not(target_os = "linux"), allow(unused))]

use super::Downloads;
use crate::App;
use crate::ui::layout_checks::check_scene;
use crate::ui::shell::Shell;
use crate::ui::theme::Theme;
use crate::ui::widgets::watch::{self, Seen};
use crate::ui::{Options, install, prompt};
use gpui::{AppContext, Entity, HeadlessAppContext, Pixels, Size, WindowHandle, point, px, size};
use rimstack_core::mods::library::ModId;
use std::sync::Arc;
use std::time::{Duration, Instant};

impl Downloads {
    fn loaded(&self) -> bool {
        self.updates.checked.is_some() && !self.changed.items.is_empty() && self.list.count > 0
    }
}

fn demo(cx: &mut gpui::TestAppContext) -> (tempfile::TempDir, Options) {
    let root = tempfile::tempdir().unwrap();
    let path = root.path();
    let app = App::new(
        path.join("home"),
        &path.join("settings"),
        &path.join("data"),
        Default::default(),
    );
    let options = Options {
        demo: true,
        section: crate::ui::section::Section::Downloads,
        ..Options::default()
    };
    cx.update(|cx| install(Arc::new(app), &options, cx));
    (root, options)
}

#[gpui::test]
fn a_question_closes_on_escape_or_a_click_beside_it_and_does_nothing(
    cx: &mut gpui::TestAppContext,
) {
    let (_root, options) = demo(cx);
    let (shell, cx) = cx.add_window_view(|window, cx| Shell::new(&options, window, cx));
    let downloads = shell.update(cx, |shell, cx| shell.downloads(cx));
    let ask = |cx: &mut gpui::VisualTestContext| {
        downloads.update_in(cx, |d, window, cx| {
            d.ask_unsubscribe(ModId::new(0, 7), "Old Roads".into(), window, cx)
        });
    };
    ask(cx);
    let (title, _, danger) = cx
        .update(|_, cx| prompt::shown(cx))
        .expect("the question is shown");
    assert_eq!(title, "Unsubscribe from Old Roads");
    assert!(
        danger,
        "unsubscribing deletes a folder: its answer is the danger one"
    );
    cx.simulate_keystrokes("escape");
    assert!(cx.update(|_, cx| prompt::shown(cx)).is_none());
    ask(cx);
    cx.simulate_click(point(px(2.), px(2.)), gpui::Modifiers::none());
    assert!(cx.update(|_, cx| prompt::shown(cx)).is_none());
}

#[gpui::test]
fn the_page_under_a_question_keeps_its_place(cx: &mut gpui::TestAppContext) {
    use gpui::{MouseMoveEvent, ScrollDelta, ScrollEvent};
    let (_root, options) = demo(cx);
    let (shell, cx) = cx.add_window_view(|window, cx| Shell::new(&options, window, cx));
    let theme = cx.update(|_, cx| cx.global::<Theme>().clone());
    cx.simulate_resize(size(theme.sized(700.), theme.sized(360.)));
    cx.run_until_parked();
    let downloads = shell.update(cx, |shell, cx| shell.downloads(cx));
    let over = point(theme.sized(300.), theme.sized(250.));
    let step = theme.sized(40.);
    let wheel = |cx: &mut gpui::VisualTestContext| {
        cx.simulate_event(MouseMoveEvent {
            position: over,
            ..MouseMoveEvent::default()
        });
        let delta = ScrollDelta::Pixels(point(px(0.), -step));
        cx.simulate_event(ScrollEvent {
            position: over,
            delta,
            ..ScrollEvent::default()
        });
        cx.run_until_parked();
    };
    let top = |cx: &mut gpui::VisualTestContext| {
        downloads.read_with(cx, |d, _| d.paging.scroll.borrow().top())
    };
    downloads.update_in(cx, |d, window, cx| {
        d.ask_unsubscribe(ModId::new(0, 7), "Old Roads".into(), window, cx)
    });
    cx.run_until_parked();
    wheel(cx);
    assert_eq!(top(cx), px(0.), "the question takes the wheel");
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    wheel(cx);
    assert!(top(cx) > px(0.), "without it the wheel scrolls the page");
}

#[gpui::test]
fn the_download_list_asks_before_steamcmd_writes_the_mods_folder(cx: &mut gpui::TestAppContext) {
    let (_root, options) = demo(cx);
    let (shell, cx) = cx.add_window_view(|window, cx| Shell::new(&options, window, cx));
    let downloads = shell.update(cx, |shell, cx| shell.downloads(cx));
    downloads.update_in(cx, |d, window, cx| d.ask_download(window, cx));
    let (title, lines, danger) = cx
        .update(|_, cx| prompt::shown(cx))
        .expect("the question is shown");
    assert!(title.starts_with("Download"), "{title}");
    assert_eq!(lines.len(), 3);
    assert!(!danger);
}

#[gpui::test]
fn showing_a_mod_from_downloads_goes_to_the_mods_tab(cx: &mut gpui::TestAppContext) {
    let (_root, options) = demo(cx);
    let (shell, cx) = cx.add_window_view(|window, cx| Shell::new(&options, window, cx));
    assert_eq!(
        shell.read_with(cx, |shell, _| shell.section()),
        crate::ui::section::Section::Downloads
    );
    let downloads = shell.update(cx, |shell, cx| shell.downloads(cx));
    downloads.update(cx, |_, cx| {
        crate::ui::navigate::go(crate::ui::navigate::Go::ShowMod(ModId::new(0, 3)), cx)
    });
    assert_eq!(
        shell.read_with(cx, |shell, _| shell.section()),
        crate::ui::section::Section::Mods
    );
}

#[cfg(target_os = "linux")]
use crate::ui::made_up::made_up;

#[cfg(target_os = "linux")]
fn opened(
    text_scale: f32,
    window: &dyn Fn(&Theme) -> Size<Pixels>,
) -> (
    tempfile::TempDir,
    HeadlessAppContext,
    WindowHandle<Shell>,
    Entity<Downloads>,
) {
    let (tmp, app) = made_up();
    let text = crate::ui::made_up::text_system();
    let mut cx = HeadlessAppContext::with_asset_source(text, Arc::new(crate::ui::assets::Assets));
    let options = Options {
        section: crate::ui::section::Section::Downloads,
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
    let downloads = cx
        .update(|cx| shell.update(cx, |shell, _, cx| shell.downloads(cx)))
        .unwrap();
    let started = Instant::now();
    while !cx.update(|cx| downloads.read(cx).loaded()) {
        assert!(
            started.elapsed() < Duration::from_secs(20),
            "Downloads did not read the made-up install"
        );
        crate::ui::made_up::step(&cx);
    }
    cx.run_until_parked();
    (tmp, cx, shell, downloads)
}

/// Downloads drawn again once it has read everything: the frames before showed less.
#[cfg(target_os = "linux")]
fn drawn(
    text_scale: f32,
    window: &dyn Fn(&Theme) -> Size<Pixels>,
    side: Option<Pixels>,
) -> Vec<Seen> {
    let (_tmp, mut cx, shell, downloads) = opened(text_scale, window);
    if let Some(side) = side {
        cx.update(|cx| downloads.update(cx, |d, cx| d.set_side(side, cx)));
        cx.run_until_parked();
    }
    watch::start();
    cx.update(|cx| downloads.update(cx, |_, cx| cx.notify()));
    cx.update_window(shell.into(), |_, window, cx| {
        // Every view drawn whole: one gpui-fast keeps from the last frame tells no texts.
        window.refresh();
        window.draw(cx).clear(cx)
    })
    .unwrap();
    watch::take()
}

#[cfg(target_os = "linux")]
#[test]
fn the_made_up_install_shows_its_updates_changes_and_list_and_the_tab_counts_them() {
    let (_tmp, mut cx, shell, downloads) =
        opened(1.0, &|_| size(crate::ui::WINDOW.0, crate::ui::WINDOW.1));
    let (active, changed, listed) = cx.update(|cx| {
        let d = downloads.read(cx);
        (
            d.updates.items.iter().filter(|row| row.active).count(),
            d.changed.items.len(),
            d.list.count,
        )
    });
    assert_eq!((active, changed, listed), (2, 3, 3));
    let (by_steamcmd, untracked, unanswered) = cx.update(|cx| {
        let updates = &downloads.read(cx).updates;
        let names = |rows: &[rimstack_core::sources::downloads::Named]| {
            rows.iter().map(|row| row.name.clone()).collect::<Vec<_>>()
        };
        let by_steamcmd: Vec<String> = updates.by_steamcmd().map(|row| row.name.clone()).collect();
        (
            by_steamcmd,
            names(&updates.untracked),
            names(&updates.unanswered),
        )
    });
    assert_eq!(by_steamcmd, ["SteamCMD Fixture Epsilon"]);
    assert_eq!(
        (untracked.as_slice(), unanswered.as_slice()),
        (
            ["SteamCMD Fixture Zeta".to_string()].as_slice(),
            ["SteamCMD Fixture Eta".to_string()].as_slice()
        )
    );
    let behind = cx
        .update(|cx| shell.read(cx).map(|shell| shell.behind(cx)))
        .unwrap();
    assert_eq!(behind, 2, "the Downloads tab counts the active mods behind");
}

#[cfg(target_os = "linux")]
#[test]
fn downloads_fits_the_first_window_at_every_text_size() {
    check_scene(
        "Downloads, first window",
        30,
        &|_| size(crate::ui::WINDOW.0, crate::ui::WINDOW.1),
        |text_scale, window| drawn(text_scale, window, None),
    );
}

#[cfg(target_os = "linux")]
#[test]
fn downloads_fits_the_smallest_window_at_every_text_size() {
    check_scene(
        "Downloads, smallest window",
        30,
        &|theme| size(theme.sized(1000.), theme.sized(560.)),
        |text_scale, window| drawn(text_scale, window, None),
    );
}

#[cfg(target_os = "linux")]
#[test]
fn downloads_fits_the_first_window_with_its_side_pane_at_its_least() {
    check_scene(
        "Downloads, first window, side pane at its least",
        30,
        &|_| size(crate::ui::WINDOW.0, crate::ui::WINDOW.1),
        |text_scale, window| drawn(text_scale, window, Some(px(0.))),
    );
}

#[cfg(target_os = "linux")]
#[test]
fn the_side_pane_moves_by_its_keys_and_keeps_its_width() {
    let (_tmp, mut cx, shell, downloads) =
        opened(1.0, &|_| size(crate::ui::WINDOW.0, crate::ui::WINDOW.1));
    let theme = cx.update(|cx| cx.global::<Theme>().clone());
    let left = gpui::KeyDownEvent {
        keystroke: gpui::Keystroke::parse("left").unwrap(),
        is_held: false,
        prefer_character_input: false,
    };
    let shown = cx
        .update_window(shell.into(), |_, window, cx| {
            downloads.update(cx, |d, cx| {
                d.nudge_side(&left, window, cx);
                d.side_shown(&theme, window)
            })
        })
        .unwrap();
    let wider = super::pane::first_side(&theme) + theme.sized(24.);
    assert_eq!(shown, Some(wider), "Left widens the pane on the right");
    cx.advance_clock(crate::ui::divider::SETTLED * 2);
    cx.run_until_parked();
    let kept = cx.update(|cx| crate::ui::app(cx).downloads_side());
    assert_eq!(kept, Some(f32::from(wider).round()));
}

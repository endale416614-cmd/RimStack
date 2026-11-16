use super::widgets::watch::{self, Seen};
use super::{Options, install, shell::Shell, theme::Theme};
use crate::App;
use gpui::{AppContext, HeadlessAppContext, Pixels, Size, size};
use std::sync::Arc;

pub(crate) const TEXT_SIZES: [f32; 4] = [0.85, 1.0, 1.25, 1.6];
/// Less than this is rounding, not a fault.
const SLACK: f32 = 1.0;

fn drawn(text_scale: f32, window: &dyn Fn(&Theme) -> Size<Pixels>) -> Vec<Seen> {
    let root = tempfile::tempdir().unwrap();
    let path = root.path();
    let app = App::new(
        path.join("home"),
        &path.join("settings"),
        &path.join("data"),
        Default::default(),
    );
    let text = crate::ui::made_up::text_system();
    let mut cx = HeadlessAppContext::with_asset_source(text, Arc::new(super::assets::Assets));
    let options = Options {
        demo: true,
        text_scale: Some(text_scale),
        ..Options::default()
    };
    cx.update(|cx| install(Arc::new(app), &options, cx));
    let shape = cx.update(|cx| window(cx.global::<Theme>()));
    // From the first frame: gpui-fast draws the later ones again from it, without the texts.
    watch::start();
    let opened = cx
        .open_window(shape, |window, cx| {
            cx.new(|cx| Shell::new(&options, window, cx))
        })
        .unwrap();
    cx.update_window(opened.into(), |_, window, cx| window.draw(cx).clear(cx))
        .unwrap();
    watch::take()
}

pub(crate) fn faults(seen: &[Seen]) -> Vec<String> {
    let slack = gpui::px(SLACK);
    // Each frame drawn tells its texts again.
    let mut once: Vec<&Seen> = Vec::new();
    for one in seen {
        if !once
            .iter()
            .any(|kept| kept.text == one.text && kept.bounds == one.bounds)
        {
            once.push(one);
        }
    }
    let seen = once;
    let mut found: Vec<String> = seen
        .iter()
        .filter(|s| !s.may_cut && s.whole > s.bounds.size.width + slack)
        .map(|s| {
            format!(
                "past its edge: {:?} is {} wide in {}",
                s.text, s.whole, s.bounds.size.width
            )
        })
        .collect();
    for (at, one) in seen.iter().enumerate() {
        for other in seen.iter().skip(at + 1) {
            let both = one.bounds.intersect(&other.bounds);
            if both.size.width > slack && both.size.height > slack {
                found.push(format!(
                    "over each other: {:?} and {:?}",
                    one.text, other.text
                ));
            }
        }
    }
    found
}

fn check(name: &str, window: impl Fn(&Theme) -> Size<Pixels>) {
    check_scene(name, 50, &window, drawn);
}

/// `scene` draws at a text size, in a window shaped by the Theme, and gives what it drew: at
/// least `least` texts, none past its edge or over another.
pub(crate) fn check_scene(
    name: &str,
    least: usize,
    window: &dyn Fn(&Theme) -> Size<Pixels>,
    scene: impl Fn(f32, &dyn Fn(&Theme) -> Size<Pixels>) -> Vec<Seen>,
) {
    let mut all = Vec::new();
    for text_scale in TEXT_SIZES {
        let seen = scene(text_scale, window);
        assert!(
            seen.len() > least,
            "only {} texts drawn at {text_scale}",
            seen.len()
        );
        all.extend(
            faults(&seen)
                .into_iter()
                .map(|fault| format!("{name} at {text_scale}: {fault}")),
        );
    }
    assert!(all.is_empty(), "{} faults:\n{}", all.len(), all.join("\n"));
}

#[test]
fn the_mods_tab_fits_the_first_window_at_every_text_size() {
    check("first window", |_| size(super::WINDOW.0, super::WINDOW.1));
}

#[test]
fn the_mods_tab_fits_the_smallest_window_at_every_text_size() {
    check("smallest window", |theme| {
        size(theme.sized(1000.), theme.sized(560.))
    });
}

#[test]
fn the_check_finds_a_text_past_its_edge_and_two_over_each_other() {
    use gpui::{Bounds, point, px};
    let seen = |text: &'static str, x: f32, width: f32, whole: f32| Seen {
        text: text.into(),
        bounds: Bounds::new(point(px(x), px(0.)), size(px(width), px(18.))),
        whole: px(whole),
        may_cut: false,
    };
    let found = faults(&[
        seen("Load order", 0., 40., 70.),
        seen("Active", 100., 50., 45.),
        seen("2 000", 130., 40., 35.),
    ]);
    assert_eq!(found.len(), 2, "{found:?}");
    assert!(found[0].contains("past its edge") && found[1].contains("over each other"));
    let cut = Seen {
        may_cut: true,
        ..seen("Load order", 0., 40., 70.)
    };
    assert!(faults(&[cut]).is_empty());
}

#[test]
fn the_status_bar_tells_the_first_line_of_the_tip_pointed_at() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path();
    let app = App::new(
        path.join("home"),
        &path.join("settings"),
        &path.join("data"),
        Default::default(),
    );
    let text = crate::ui::made_up::text_system();
    let mut cx = HeadlessAppContext::with_asset_source(text, Arc::new(super::assets::Assets));
    let options = Options {
        demo: true,
        text_scale: Some(1.6),
        ..Options::default()
    };
    cx.update(|cx| install(Arc::new(app), &options, cx));
    cx.update(|cx| {
        super::tip::point_at(
            "Click to order the list by this column; click again to reverse it. Right-click to choose the columns.\nand more".into(),
            cx,
        );
    });
    watch::start();
    let opened = cx
        .open_window(
            size(crate::ui::WINDOW.0, crate::ui::WINDOW.1),
            |window, cx| cx.new(|cx| Shell::new(&options, window, cx)),
        )
        .unwrap();
    cx.update_window(opened.into(), |_, window, cx| window.draw(cx).clear(cx))
        .unwrap();
    let seen = watch::take();
    assert!(seen.iter().any(|s| s.text.starts_with("Click to order")));
    assert!(!seen.iter().any(|s| s.text.contains("and more")));
    let found = faults(&seen);
    assert!(found.is_empty(), "{}", found.join("\n"));
}

#[test]
fn the_hint_is_let_go_on_a_press_a_key_or_the_wheel() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path();
    let app = App::new(
        path.join("home"),
        &path.join("settings"),
        &path.join("data"),
        Default::default(),
    );
    let text = crate::ui::made_up::text_system();
    let mut cx = HeadlessAppContext::with_asset_source(text, Arc::new(super::assets::Assets));
    let options = Options {
        demo: true,
        ..Options::default()
    };
    cx.update(|cx| install(Arc::new(app), &options, cx));
    let opened = cx
        .open_window(
            size(crate::ui::WINDOW.0, crate::ui::WINDOW.1),
            |window, cx| cx.new(|cx| Shell::new(&options, window, cx)),
        )
        .unwrap();
    cx.update_window(opened.into(), |_, window, cx| window.draw(cx).clear(cx))
        .unwrap();
    let at = gpui::point(gpui::px(600.), gpui::px(400.));
    let inputs = [
        gpui::PlatformInput::Scroll(gpui::ScrollEvent {
            position: at,
            ..Default::default()
        }),
        gpui::PlatformInput::MouseDown(gpui::MouseDownEvent {
            button: gpui::MouseButton::Left,
            position: at,
            modifiers: Default::default(),
            click_count: 1,
            first_mouse: false,
        }),
        gpui::PlatformInput::KeyDown(gpui::KeyDownEvent {
            keystroke: gpui::Keystroke::parse("shift").unwrap(),
            is_held: false,
            prefer_character_input: false,
        }),
    ];
    for input in inputs {
        cx.update(|cx| super::tip::point_at("Pointed at".into(), cx));
        cx.update_window(opened.into(), |_, window, cx| {
            window.dispatch_event(input, cx);
        })
        .unwrap();
        assert_eq!(cx.update(|cx| super::tip::hint(cx)), "");
    }
}

use super::Mods;
use crate::App;
use crate::ui::{Options, install};
use crate::workspace::load_order::Side;
use std::sync::Arc;

fn demo(cx: &mut gpui::TestAppContext) -> tempfile::TempDir {
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
        ..Options::default()
    };
    cx.update(|cx| install(Arc::new(app), &options, cx));
    root
}

#[gpui::test]
fn typing_in_the_filter_narrows_both_lists_and_clearing_it_shows_all(
    cx: &mut gpui::TestAppContext,
) {
    let _root = demo(cx);
    let (mods, cx) = cx.add_window_view(|_, cx| Mods::new(true, cx));
    let all = mods.read_with(cx, |mods, cx| {
        (
            mods.list.read(cx).matching(Side::Active),
            mods.list.read(cx).matching(Side::Inactive),
        )
    });
    cx.update(|window, cx| {
        let focus = mods.read(cx).focus.clone();
        window.focus(&focus, cx);
    });
    cx.simulate_keystrokes("ctrl-f");
    let focused = cx.update(|window, cx| mods.read(cx).filter.read(cx).is_focused(window));
    assert!(focused, "Ctrl+F gives the filter the keys");
    cx.simulate_input("Weapons 1");
    let (typed, narrowed) = mods.read_with(cx, |mods, cx| {
        (
            mods.filter_text(cx),
            mods.list.read(cx).matching(Side::Active),
        )
    });
    assert_eq!(typed, "Weapons 1");
    assert!(narrowed > 0 && narrowed < all.0, "{narrowed} of {}", all.0);
    cx.simulate_keystrokes("backspace backspace");
    assert_eq!(
        mods.read_with(cx, |mods, cx| mods.filter_text(cx)),
        "Weapons"
    );
    cx.simulate_keystrokes("escape");
    let back = mods.read_with(cx, |mods, cx| {
        (
            mods.list.read(cx).matching(Side::Active),
            mods.list.read(cx).matching(Side::Inactive),
        )
    });
    assert_eq!(back, all);
}

fn first_heading(cx: &mut gpui::VisualTestContext) -> gpui::Point<gpui::Pixels> {
    use crate::ui::theme::{self, Theme};
    let theme = cx.update(|_, cx| cx.global::<Theme>().clone());
    let x = theme.sized(290.) + theme::HANDLE + theme.sized(120.);
    let y = theme.header()
        + theme.field()
        + theme::GAP
        + theme.control()
        + theme::GAP_TINY
        + theme.sized(32.) / 2.;
    gpui::point(x, y)
}

#[gpui::test]
fn a_click_on_a_category_heading_folds_it(cx: &mut gpui::TestAppContext) {
    let _root = demo(cx);
    let (mods, cx) = cx.add_window_view(|_, cx| Mods::new(true, cx));
    cx.run_until_parked();
    let (before, first) = mods.read_with(cx, |mods, _| {
        (mods.active.rows.len(), mods.active.rows[0].clone())
    });
    let at = first_heading(cx);
    cx.simulate_click(at, gpui::Modifiers::none());
    let after = mods.read_with(cx, |mods, _| mods.active.rows.len());
    assert_eq!(after, before - first.count);
}

#[gpui::test]
fn a_fold_keeps_the_list_where_it_was_scrolled(cx: &mut gpui::TestAppContext) {
    let _root = demo(cx);
    let (mods, cx) = cx.add_window_view(|_, cx| Mods::new(true, cx));
    cx.run_until_parked();
    let scrolled = mods.read_with(cx, |mods, cx| {
        mods.list(Side::Active).read(cx).scroll().clone()
    });
    let most = scrolled.borrow().most();
    scrolled.borrow_mut().set_top(most / 2.);
    let at = scrolled.borrow().top();
    let last = mods
        .read_with(cx, |mods, _| {
            mods.active.rows.iter().rev().find(|r| r.is_header).cloned()
        })
        .unwrap();
    mods.update(cx, |mods, cx| {
        mods.toggle_category(Side::Active, last.category, cx)
    });
    cx.run_until_parked();
    assert!(at > gpui::px(0.));
    assert_eq!(scrolled.borrow().top(), at);
}

#[gpui::test]
fn the_touchpad_moves_the_load_order_as_it_goes(cx: &mut gpui::TestAppContext) {
    use gpui::{MouseMoveEvent, ScrollDelta, ScrollEvent, point};
    let _root = demo(cx);
    let (mods, cx) = cx.add_window_view(|_, cx| Mods::new(true, cx));
    cx.run_until_parked();
    let over = first_heading(cx);
    cx.simulate_event(MouseMoveEvent {
        position: over,
        ..MouseMoveEvent::default()
    });
    let step = cx.update(|_, cx| cx.global::<crate::ui::theme::Theme>().sized(200.));
    for _ in 0..40 {
        let wheel = ScrollEvent {
            position: over,
            delta: ScrollDelta::Pixels(point(gpui::px(0.), -step)),
            ..ScrollEvent::default()
        };
        cx.simulate_event(wheel);
        cx.run_until_parked();
    }
    let top = mods.read_with(cx, |mods, cx| mods.list(Side::Active).read(cx).top_row());
    assert!(top > 100, "the load order shows row {top} at its top");
}

#[gpui::test]
fn a_wheel_notch_glides_three_lines_down(cx: &mut gpui::TestAppContext) {
    use gpui::{MouseMoveEvent, ScrollDelta, ScrollEvent, point};
    let _root = demo(cx);
    let (mods, cx) = cx.add_window_view(|_, cx| Mods::new(true, cx));
    cx.run_until_parked();
    let over = first_heading(cx);
    cx.simulate_event(MouseMoveEvent {
        position: over,
        ..MouseMoveEvent::default()
    });
    let notch = ScrollEvent {
        position: over,
        delta: ScrollDelta::Lines(point(0., -3.)),
        ..ScrollEvent::default()
    };
    cx.simulate_event(notch);
    let list = mods.read_with(cx, |mods, _| mods.list(Side::Active).clone());
    let scrolled = list.read_with(cx, |list, _| list.scroll().clone());
    assert!(
        scrolled.borrow().top() < gpui::px(1.),
        "a notch lands over time, not at once"
    );
    std::thread::sleep(std::time::Duration::from_millis(250));
    list.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();
    let line = cx.update(|_, cx| cx.global::<crate::ui::theme::Theme>().sized(40.));
    assert_eq!(scrolled.borrow().top(), line * 3.);
}

/// What drawing the scrolling load order costs, frame by frame, with no screen to wait on:
/// `just test scroll_cost -- --ignored --nocapture`.
#[gpui::test]
#[ignore]
fn scroll_cost(cx: &mut gpui::TestAppContext) {
    let _root = demo(cx);
    let (mods, cx) = cx.add_window_view(|_, cx| Mods::new(true, cx));
    cx.run_until_parked();
    let list = mods.read_with(cx, |mods, _| mods.list(Side::Active).clone());
    let scrolled = list.read_with(cx, |list, _| list.scroll().clone());
    let most = scrolled.borrow().most();
    const FRAMES: usize = 1440;
    let started = std::time::Instant::now();
    for frame in 0..FRAMES {
        scrolled
            .borrow_mut()
            .set_top(most * (frame as f32 / FRAMES as f32));
        list.update(cx, |_, cx| cx.notify());
        cx.update(|window, cx| window.draw(cx).clear(cx));
    }
    let each = started.elapsed() / FRAMES as u32;
    let reached = list.read_with(cx, |list, _| list.top_row());
    eprintln!(
        "scroll_cost frame_us={} reached_row={reached}",
        each.as_micros()
    );
}

#[gpui::test]
fn the_thumb_drags_the_load_order(cx: &mut gpui::TestAppContext) {
    use crate::ui::theme::{self, Theme};
    use gpui::{MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, point};
    let _root = demo(cx);
    let (mods, cx) = cx.add_window_view(|_, cx| Mods::new(true, cx));
    cx.run_until_parked();
    let (theme, window) =
        cx.update(|window, cx| (cx.global::<Theme>().clone(), window.viewport_size()));
    let x = window.width - theme.sized(360.) - theme::HANDLE - super::list::bar_room(&theme) / 2.;
    let top = first_heading(cx).y;
    let press = |y: Pixels| MouseDownEvent {
        button: MouseButton::Left,
        position: point(x, y),
        click_count: 1,
        ..MouseDownEvent::default()
    };
    cx.simulate_event(press(top));
    let to = point(x, window.height / 2.);
    cx.simulate_event(MouseMoveEvent {
        position: to,
        pressed_button: Some(MouseButton::Left),
        ..MouseMoveEvent::default()
    });
    cx.simulate_event(MouseUpEvent {
        button: MouseButton::Left,
        position: to,
        click_count: 1,
        ..MouseUpEvent::default()
    });
    let list = mods.read_with(cx, |mods, _| mods.list(Side::Active).clone());
    let row = list.read_with(cx, |list, _| list.top_row());
    assert!(
        row > 200,
        "the thumb held halfway down shows row {row} at the top"
    );
}

#[gpui::test]
#[ignore]
fn scroll_cost_in_the_window(cx: &mut gpui::TestAppContext) {
    use gpui::{MouseMoveEvent, ScrollDelta, ScrollEvent, point};
    let _root = demo(cx);
    let options = Options {
        demo: true,
        ..Options::default()
    };
    let (shell, cx) =
        cx.add_window_view(|window, cx| crate::ui::shell::Shell::new(&options, window, cx));
    cx.run_until_parked();
    let mods = shell.read_with(cx, |shell, _| shell.sections.mods.clone());
    let list = mods.read_with(cx, |mods, _| mods.list(Side::Active).clone());
    let over = list.read_with(cx, |list, _| list.drawn_at(8)) + gpui::px(4.);
    let over = point(gpui::px(700.), over);
    cx.simulate_event(MouseMoveEvent {
        position: over,
        ..MouseMoveEvent::default()
    });
    cx.run_until_parked();
    const FRAMES: usize = 600;
    let step = gpui::px(7.);
    let started = std::time::Instant::now();
    for _ in 0..FRAMES {
        cx.simulate_event(ScrollEvent {
            position: over,
            delta: ScrollDelta::Pixels(point(gpui::px(0.), -step)),
            ..ScrollEvent::default()
        });
        cx.update(|window, cx| window.draw(cx).clear(cx));
    }
    let each = started.elapsed() / FRAMES as u32;
    let reached = list.read_with(cx, |list, _| list.top_row());
    eprintln!(
        "scroll_cost_in_the_window frame_us={} reached_row={reached}",
        each.as_micros()
    );
}

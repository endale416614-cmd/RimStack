use super::button::Btn;
use super::controls::{Choice, Toggle};
use super::{Options, focus, install, menu, notices};
use crate::App;
use gpui::{
    AppContext, Context, Entity, InteractiveElement, IntoElement, ParentElement, Render, Styled,
    TestAppContext, VisualTestContext, Window, div, prelude::FluentBuilder,
};
use std::sync::Arc;
use std::time::Duration;

#[derive(Default)]
struct Bench {
    first: usize,
    second: usize,
    on: bool,
    picked: usize,
    chosen: Option<&'static str>,
    menu: Option<Entity<menu::Menu>>,
    notices: Option<Entity<notices::Notices>>,
    focus: Option<gpui::FocusHandle>,
}

impl Render for Bench {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let me = cx.entity().downgrade();
        let pick = me.clone();
        div()
            .size_full()
            .when_some(self.focus.clone(), |d, focus| d.track_focus(&focus))
            .on_action(|_: &focus::Next, window, cx| window.focus_next(cx))
            .on_action(|_: &focus::Previous, window, cx| window.focus_prev(cx))
            .child(Btn::new("first", "First").on_click(cx.listener(|b, _, _, _| b.first += 1)))
            .child(Btn::new("second", "Second").on_click(cx.listener(|b, _, _, _| b.second += 1)))
            .child(
                Toggle::new("toggle", self.on, "Toggle").on_flip(move |on, _, cx| {
                    let _ = me.update(cx, |b, cx| {
                        b.on = on;
                        cx.notify();
                    });
                }),
            )
            .child(
                Choice::new(
                    "choice",
                    vec!["A".into(), "B".into(), "C".into()],
                    self.picked,
                )
                .on_pick(move |at, _, cx| {
                    let _ = pick.update(cx, |b, cx| {
                        b.picked = at;
                        cx.notify();
                    });
                }),
            )
            .children(self.notices.clone())
            .children(self.menu.clone())
    }
}

fn bench(cx: &mut TestAppContext) -> (tempfile::TempDir, Entity<Bench>, &mut VisualTestContext) {
    let root = tempfile::tempdir().unwrap();
    let path = root.path();
    let app = App::new(
        path.join("home"),
        &path.join("settings"),
        &path.join("data"),
        Default::default(),
    );
    cx.update(|cx| {
        install(
            Arc::new(app),
            &Options {
                demo: true,
                ..Options::default()
            },
            cx,
        )
    });
    let (bench, cx) = cx.add_window_view(|_, cx| Bench {
        menu: Some(menu::install(cx)),
        notices: Some(notices::install(cx)),
        focus: Some(cx.focus_handle()),
        ..Bench::default()
    });
    cx.update(|window, cx| {
        let focus = bench.read(cx).focus.clone().unwrap();
        window.focus(&focus, cx);
    });
    cx.run_until_parked();
    (root, bench, cx)
}

/// Each key down then up, as a keyboard sends them: a control is pressed on the key's release.
pub(crate) fn press(cx: &mut VisualTestContext, keys: &'static str) {
    for key in keys.split_whitespace() {
        cx.simulate_keystrokes(key);
        let keystroke = gpui::Keystroke::parse(key).unwrap();
        cx.simulate_event(gpui::KeyUpEvent { keystroke });
    }
}

#[gpui::test]
fn tab_reaches_each_control_and_enter_or_space_presses_it(cx: &mut TestAppContext) {
    let (_root, bench, cx) = bench(cx);
    press(cx, "tab enter tab space");
    assert_eq!(bench.read_with(cx, |b, _| (b.first, b.second)), (1, 1));
    press(cx, "tab space");
    assert!(bench.read_with(cx, |b, _| b.on), "Space flips the toggle");
    press(cx, "tab right right");
    assert_eq!(
        bench.read_with(cx, |b, _| b.picked),
        2,
        "the arrows move the choice"
    );
    press(cx, "shift-tab space");
    assert!(!bench.read_with(cx, |b, _| b.on), "Shift+Tab goes back");
}

fn frames_asked(cx: &mut VisualTestContext) -> usize {
    cx.run_until_parked();
    cx.update(|window, cx| {
        window.draw(cx).clear(cx);
        window.simulate_next_frame(cx)
    })
}

#[gpui::test]
fn a_value_from_elsewhere_shows_at_once_and_a_pick_slides_to_its_place(cx: &mut TestAppContext) {
    let (_root, bench, cx) = bench(cx);
    cx.simulate_mouse_move(
        gpui::point(gpui::px(900.), gpui::px(900.)),
        None,
        gpui::Modifiers::none(),
    );
    while frames_asked(cx) > 0 {
        cx.executor().advance_clock(Duration::from_millis(16));
    }
    bench.update(cx, |b, cx| {
        b.picked = 2;
        b.on = true;
        cx.notify();
    });
    assert_eq!(frames_asked(cx), 0, "nothing slides that was not pressed");
    press(cx, "tab tab tab tab left");
    assert_eq!(bench.read_with(cx, |b, _| b.picked), 1);
    assert!(frames_asked(cx) > 0, "the pick slides");
    let settled = (0..60).find(|_| {
        cx.executor().advance_clock(Duration::from_millis(16));
        frames_asked(cx) == 0
    });
    assert!(settled.is_some(), "the slide comes to rest");
}

#[gpui::test]
fn a_button_pointed_at_fades_to_its_hover_colour_and_rests(cx: &mut TestAppContext) {
    let (_root, _bench, cx) = bench(cx);
    frames_asked(cx);
    cx.simulate_mouse_move(
        gpui::point(gpui::px(6.), gpui::px(6.)),
        None,
        gpui::Modifiers::none(),
    );
    assert!(frames_asked(cx) > 0, "the hover colour fades in");
    let settled = (0..60).find(|_| {
        cx.executor().advance_clock(Duration::from_millis(16));
        frames_asked(cx) == 0
    });
    assert!(settled.is_some(), "the fade comes to rest");
}

#[gpui::test]
fn a_menu_is_chosen_from_with_the_arrows_and_closes_on_escape(cx: &mut TestAppContext) {
    let (_root, bench, cx) = bench(cx);
    let entries = |bench: &Entity<Bench>| -> Vec<menu::Entry> {
        let (one, two) = (bench.downgrade(), bench.downgrade());
        vec![
            menu::Entry::Title("Things".into()),
            menu::Entry::item("Off", |_, _| {}).off(true).into(),
            menu::Entry::item("One", move |_, cx| {
                let _ = one.update(cx, |b, _| b.chosen = Some("one"));
            })
            .into(),
            menu::Entry::Line,
            menu::Entry::item("Two", move |_, cx| {
                let _ = two.update(cx, |b, _| b.chosen = Some("two"));
            })
            .into(),
        ]
    };
    cx.update(|window, cx| {
        menu::open_at(
            gpui::point(gpui::px(10.), gpui::px(10.)),
            entries(&bench),
            window,
            cx,
        )
    });
    cx.simulate_keystrokes("down down enter");
    assert_eq!(
        bench.read_with(cx, |b, _| b.chosen),
        Some("two"),
        "the arrows skip what is off"
    );
    cx.update(|window, cx| {
        menu::open_at(
            gpui::point(gpui::px(10.), gpui::px(10.)),
            entries(&bench),
            window,
            cx,
        )
    });
    cx.simulate_keystrokes("escape enter");
    assert_eq!(
        bench.read_with(cx, |b, _| b.chosen),
        Some("two"),
        "Escape closes it without a choice"
    );
}

#[gpui::test]
fn a_toast_goes_by_itself_after_a_while(cx: &mut TestAppContext) {
    let (_root, bench, cx) = bench(cx);
    cx.update(|_, cx| notices::say("Saved", notices::Tone::Ok, cx));
    let shown = |cx: &mut VisualTestContext| {
        let notices = bench.read_with(cx, |b, _| b.notices.clone().unwrap());
        notices.read_with(cx, |n, _| n.shown().len())
    };
    assert_eq!(shown(cx), 1);
    cx.executor().advance_clock(Duration::from_secs(6));
    cx.run_until_parked();
    assert_eq!(shown(cx), 0);
}

struct Asking {
    drawn: usize,
}

impl Render for Asking {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.drawn += 1;
        let me = cx.entity_id();
        let first = self.drawn == 1;
        div().size_full().child(
            gpui::canvas(
                move |_, _, cx| {
                    if first {
                        super::redraw_after(me, cx);
                    }
                },
                |_, _, _, _| {},
            )
            .size_full(),
        )
    }
}

#[test]
fn a_view_that_asks_while_it_is_drawn_is_drawn_again_after() {
    let mut cx = gpui::HeadlessAppContext::with_asset_source(
        super::made_up::text_system(),
        Arc::new(super::assets::Assets),
    );
    let window = cx
        .open_window(gpui::size(gpui::px(200.), gpui::px(100.)), |_, cx| {
            cx.new(|_| Asking { drawn: 0 })
        })
        .unwrap();
    for _ in 0..3 {
        cx.run_until_parked();
        cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear(cx))
            .unwrap();
    }
    let drawn = cx.update(|cx| window.read(cx).unwrap().drawn);
    assert_eq!(drawn, 2);
}

struct Hearer(Vec<crate::changes::Changes>);

#[gpui::test]
fn a_view_hears_only_what_it_reads(cx: &mut TestAppContext) {
    use crate::changes::Changes;
    let (_root, _bench, cx) = bench(cx);
    let hearer = cx.update(|_, cx| {
        cx.new(|cx| {
            super::hear(
                cx,
                Changes::LOGS | Changes::SAVES,
                |hearer: &mut Hearer, changes, _| hearer.0.push(changes),
            );
            Hearer(Vec::new())
        })
    });
    let publish = |changes, cx: &mut VisualTestContext| {
        cx.update(|_, cx| super::app(cx).publish(changes));
        cx.run_until_parked();
    };
    publish(Changes::FACTS, cx);
    publish(Changes::FACTS | Changes::LOGS, cx);
    assert_eq!(hearer.read_with(cx, |h, _| h.0.clone()), [Changes::LOGS]);
}

struct Hearing;

impl Render for Hearing {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

#[gpui::test]
fn a_view_released_stops_hearing_at_the_next_change_whatever_it_is(cx: &mut TestAppContext) {
    let (_root, _bench, cx) = bench(cx);
    let app = cx.update(|_, cx| super::app(cx));
    let before = app.listening();
    let hearing = cx.update(|_, cx| {
        cx.new(|cx| {
            super::hear(cx, crate::changes::Changes::NOTES, |_, _, _| {});
            Hearing
        })
    });
    assert_eq!(app.listening(), before + 1);
    drop(hearing);
    cx.update(|_, _| {});
    cx.run_until_parked();
    app.publish(crate::changes::Changes::RULES);
    assert_eq!(app.listening(), before);
}

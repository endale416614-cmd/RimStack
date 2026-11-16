use super::made_up::made_up;
use super::shell::{RENDERS, Shell};
use super::status_bar::TURNS;
use super::{Options, install};
use gpui::{AppContext, HeadlessAppContext, WindowHandle};
use std::sync::Arc;
use std::time::Duration;

fn opened() -> (tempfile::TempDir, HeadlessAppContext, WindowHandle<Shell>) {
    let (tmp, app) = made_up();
    let text = super::made_up::text_system();
    let mut cx = HeadlessAppContext::with_asset_source(text, Arc::new(super::assets::Assets));
    let options = Options::default();
    cx.update(|cx| install(Arc::new(app), &options, cx));
    let shell = cx
        .open_window(
            gpui::size(super::WINDOW.0, super::WINDOW.1),
            |window, cx| cx.new(|cx| Shell::new(&options, window, cx)),
        )
        .unwrap();
    cx.run_until_parked();
    (tmp, cx, shell)
}

/// A frame 16 ms after the last; how many of the views drawn asked for it.
fn frame(cx: &mut HeadlessAppContext, shell: WindowHandle<Shell>) -> usize {
    cx.advance_clock(Duration::from_millis(16));
    cx.run_until_parked();
    cx.update_window(shell.into(), |_, window, cx| {
        let asked = window.simulate_next_frame(cx);
        window.draw(cx).clear(cx);
        asked
    })
    .unwrap()
}

/// The frames drawn before nothing asks for another, at most `most`.
fn settled(cx: &mut HeadlessAppContext, shell: WindowHandle<Shell>, most: usize) -> usize {
    (1..=most)
        .find(|_| frame(cx, shell) == 0)
        .unwrap_or_else(|| panic!("frames were still asked for after {most}"))
}

#[test]
fn the_arrows_turn_without_drawing_the_window_again() {
    let (_tmp, mut cx, shell) = opened();
    cx.update(|cx| {
        let mods = shell.read(cx).unwrap().sections.mods.clone();
        mods.update(cx, |mods, cx| {
            mods.edit_list(cx, |list, _| list.rescanning = true);
            cx.notify();
        });
    });
    for _ in 0..5 {
        frame(&mut cx, shell);
    }
    RENDERS.with(|renders| renders.set(0));
    TURNS.with(|turns| turns.set(0));
    for _ in 0..30 {
        frame(&mut cx, shell);
    }
    let (renders, turns) = (RENDERS.with(|r| r.get()), TURNS.with(|t| t.get()));
    assert!(
        turns >= 25,
        "the arrows were drawn {turns} times in 30 frames"
    );
    assert!(
        renders < 5,
        "the window was drawn again {renders} times in 30 frames of the arrows turning"
    );
}

#[test]
fn a_section_and_ctrl_k_come_to_rest_and_ask_for_no_more_frames() {
    let (_tmp, mut cx, shell) = opened();
    settled(&mut cx, shell, 60);
    cx.update(|cx| {
        shell
            .update(cx, |shell, _, cx| {
                shell.go(crate::ui::section::Section::Settings, cx)
            })
            .unwrap()
    });
    assert!(settled(&mut cx, shell, 60) > 5, "the section faded in");
    cx.update(|cx| {
        shell
            .update(cx, |shell, window, cx| shell.open_commands(window, cx))
            .unwrap()
    });
    assert!(settled(&mut cx, shell, 60) > 3, "Ctrl + K faded in");
    cx.update_window(shell.into(), |_, window, cx| {
        window.dispatch_keystroke(gpui::Keystroke::parse("escape").unwrap(), cx)
    })
    .unwrap();
    let closing = cx.update(|cx| {
        let shell = shell.read(cx).unwrap();
        shell.commands.is_none() && shell.closing_commands.is_some()
    });
    assert!(closing, "Ctrl + K is drawn while it fades out");
    assert!(settled(&mut cx, shell, 60) > 3, "Ctrl + K faded out");
    for _ in 0..3 {
        frame(&mut cx, shell);
    }
    let gone = cx.update(|cx| shell.read(cx).unwrap().closing_commands.is_none());
    assert!(gone, "Ctrl + K is gone once faded out");
}

#[test]
fn ctrl_k_survives_any_mix_of_keys_clicks_and_waits() {
    use gpui::{Modifiers, MouseButton, MouseDownEvent, MouseUpEvent, PlatformInput, point, px};
    let (_tmp, mut cx, shell) = opened();
    let mut seed: u64 = 7;
    let mut next = |n: u64| {
        seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (seed >> 33) % n
    };
    let click = |cx: &mut HeadlessAppContext, at: gpui::Point<gpui::Pixels>| {
        cx.update_window(shell.into(), |_, window, cx| {
            window.dispatch_event(
                PlatformInput::MouseDown(MouseDownEvent {
                    button: MouseButton::Left,
                    position: at,
                    modifiers: Modifiers::default(),
                    click_count: 1,
                    first_mouse: false,
                }),
                cx,
            );
            window.dispatch_event(
                PlatformInput::MouseUp(MouseUpEvent {
                    button: MouseButton::Left,
                    position: at,
                    modifiers: Modifiers::default(),
                    click_count: 1,
                }),
                cx,
            );
        })
        .unwrap();
    };
    for step in 0..600 {
        match next(7) {
            0 | 1 => {
                let k = ["ctrl-k", "escape", "enter", "s", "o", "down"][next(6) as usize];
                eprintln!("{step}: {k}");
                cx.update_window(shell.into(), |_, window, cx| {
                    window.dispatch_keystroke(gpui::Keystroke::parse(k).unwrap(), cx)
                })
                .unwrap();
            }
            2 => {
                eprintln!("{step}: ctrl-k");
                cx.update_window(shell.into(), |_, window, cx| {
                    window.dispatch_keystroke(gpui::Keystroke::parse("ctrl-k").unwrap(), cx)
                })
                .unwrap();
            }
            3 => {
                let at = point(px(720.), px(140. + 30. * next(8) as f32));
                eprintln!("{step}: click row {at:?}");
                click(&mut cx, at);
            }
            4 => {
                eprintln!("{step}: click outside");
                click(&mut cx, point(px(40.), px(700.)));
            }
            5 => {
                let ms = [0, 16, 60, 150, 400][next(5) as usize];
                eprintln!("{step}: wait {ms}");
                cx.advance_clock(Duration::from_millis(ms));
            }
            _ => {
                frame(&mut cx, shell);
            }
        }
        cx.run_until_parked();
        cx.update_window(shell.into(), |_, window, cx| window.draw(cx).clear(cx))
            .unwrap();
    }
}

fn key(cx: &mut HeadlessAppContext, shell: WindowHandle<Shell>, k: &'static str) {
    cx.update_window(shell.into(), |_, window, cx| {
        window.dispatch_keystroke(gpui::Keystroke::parse(k).unwrap(), cx)
    })
    .unwrap();
}

fn fades_then_goes(
    cx: &mut HeadlessAppContext,
    shell: WindowHandle<Shell>,
    what: &str,
    drawn: impl Fn(&mut gpui::App) -> bool,
) {
    assert!(
        cx.update(|cx| drawn(cx)),
        "{what} is drawn while it fades out"
    );
    assert!(settled(cx, shell, 60) > 3, "{what} faded out");
    for _ in 0..3 {
        frame(cx, shell);
    }
    assert!(!cx.update(|cx| drawn(cx)), "{what} is gone once faded out");
}

#[test]
fn menus_questions_lists_and_the_tray_fade_out_and_go() {
    let (_tmp, mut cx, shell) = opened();
    settled(&mut cx, shell, 60);
    cx.update_window(shell.into(), |_, window, cx| {
        let entries = vec![super::menu::Entry::item("One", |_, _| {}).into()];
        super::menu::open_at(
            gpui::point(gpui::px(80.), gpui::px(80.)),
            entries,
            window,
            cx,
        );
    })
    .unwrap();
    frame(&mut cx, shell);
    key(&mut cx, shell, "escape");
    fades_then_goes(&mut cx, shell, "the menu", |cx| super::menu::drawn(cx));

    cx.update_window(shell.into(), |_, window, cx| {
        let question = super::prompt::Question {
            title: "Sure?".into(),
            lines: Vec::new(),
            label: "Yes".into(),
            danger: false,
            run: Box::new(|_, _| {}),
        };
        super::prompt::ask(question, window, cx);
    })
    .unwrap();
    frame(&mut cx, shell);
    key(&mut cx, shell, "escape");
    fades_then_goes(&mut cx, shell, "the question", |cx| {
        super::prompt::drawn(cx)
    });

    let lists = cx.update(|cx| shell.read(cx).unwrap().lists.clone());
    cx.update_window(shell.into(), |_, window, cx| {
        lists.update(cx, |lists, cx| {
            lists.toggle(
                super::lists::Which::Lists,
                gpui::Bounds::default(),
                window,
                cx,
            )
        })
    })
    .unwrap();
    frame(&mut cx, shell);
    key(&mut cx, shell, "escape");
    fades_then_goes(&mut cx, shell, "Lists", move |cx| lists.read(cx).drawn());

    cx.update(|cx| {
        shell
            .update(cx, |shell, _, cx| {
                shell.tray_open = true;
                cx.notify();
            })
            .unwrap()
    });
    frame(&mut cx, shell);
    cx.update(|cx| {
        shell
            .update(cx, |shell, _, cx| shell.close_tray(cx))
            .unwrap()
    });
    fades_then_goes(&mut cx, shell, "the tray", move |cx| {
        let shell = shell.read(cx).unwrap();
        shell.tray_open || shell.tray_fading
    });
}

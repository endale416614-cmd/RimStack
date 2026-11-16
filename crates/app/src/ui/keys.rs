use super::button::Btn;
use super::shell::Shell;
use super::theme::{self, Theme};
use super::widgets::{TextSized, label, measure};
use crate::words::put;
use gpui::{
    AnyElement, App, Context, FontWeight, InteractiveElement, IntoElement, KeyBinding,
    KeyDownEvent, ParentElement, Pixels, StatefulInteractiveElement, Styled, Window, actions, div,
    px,
};
use rimstack_core::base::sentence::Sentence;
use rimstack_core::say;

actions!(
    window,
    [
        ShowKeys,
        OpenCommands,
        Section1,
        Section2,
        Section3,
        Section4,
        Section5,
        Section6,
        Section7,
        Section8
    ]
);

pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("f1", ShowKeys, None),
        KeyBinding::new("?", ShowKeys, Some("!TextField")),
        KeyBinding::new(crate::command!("k"), OpenCommands, None),
        KeyBinding::new(crate::command!("1"), Section1, None),
        KeyBinding::new(crate::command!("2"), Section2, None),
        KeyBinding::new(crate::command!("3"), Section3, None),
        KeyBinding::new(crate::command!("4"), Section4, None),
        KeyBinding::new(crate::command!("5"), Section5, None),
        KeyBinding::new(crate::command!("6"), Section6, None),
        KeyBinding::new(crate::command!("7"), Section7, None),
        KeyBinding::new(crate::command!("8"), Section8, None),
    ]);
    #[cfg(feature = "profiler")]
    cx.bind_keys([KeyBinding::new("ctrl-alt-shift-f", FrameTimes, None)]);
    #[cfg(feature = "inspector")]
    cx.bind_keys([KeyBinding::new("ctrl-alt-shift-i", Inspector, None)]);
}

#[cfg(feature = "profiler")]
actions!(window, [FrameTimes]);

#[cfg(feature = "inspector")]
actions!(window, [Inspector]);

pub fn developer_tools(shown: gpui::Div) -> gpui::Div {
    #[cfg(feature = "profiler")]
    let shown =
        shown.on_action(|_: &FrameTimes, window, _| window.cycle_debug_frame_overlay_mode());
    #[cfg(feature = "inspector")]
    let shown = shown.on_action(|_: &Inspector, window, cx| window.toggle_inspector(cx));
    shown
}

fn groups() -> Vec<(Sentence, Vec<(Sentence, Sentence)>)> {
    vec![
        (
            say!("Everywhere"),
            vec![
                (say!("Ctrl + K"), say!("Search mods, commands and settings")),
                (
                    say!("Ctrl + 1 to 8"),
                    say!("Mods, Downloads, Textures, Workshop, Saves, Logs, Search, Settings"),
                ),
                (
                    say!("Ctrl + Shift + F"),
                    say!("Search the text of mods' files"),
                ),
                (Sentence::Plain("?".into()), say!("This page")),
                (say!("Left, Right"), say!("Move a choice or the section")),
                (say!("Tab"), say!("Next control")),
                (say!("Esc"), say!("Close, cancel, clear the selection")),
            ],
        ),
        (
            say!("Load order"),
            vec![
                (say!("Ctrl + S"), say!("Save")),
                (say!("Ctrl + Z"), say!("Undo")),
                (say!("Ctrl + Shift + Z"), say!("Redo")),
                (say!("Ctrl + F"), say!("Filter the load order")),
            ],
        ),
        (
            say!("A list of mods"),
            vec![
                (say!("Up, Down"), say!("Select the mod above or below")),
                (say!("Enter"), say!("Activate or deactivate")),
                (say!("Alt + Up, Alt + Down"), say!("Move the mod one place")),
                (
                    say!("Ctrl + click"),
                    say!("Add or remove one from the selection"),
                ),
                (say!("Shift + click"), say!("Select through")),
                (say!("Ctrl + A"), say!("Select every mod shown")),
                (say!("Ctrl + I"), say!("Invert the selection")),
                (Sentence::Plain("T".into()), say!("Type its tags")),
                (Sentence::Plain("N".into()), say!("Type its note")),
                (say!("Menu"), say!("The mod's menu")),
            ],
        ),
        (
            say!("Texture comparison"),
            vec![
                (say!("Up, Down"), say!("Previous or next texture")),
                (
                    Sentence::Plain("1, 2".into()),
                    say!("Keep the first or the second mod's"),
                ),
                (say!("Esc"), say!("Back to the lists")),
            ],
        ),
    ]
}

fn cap_width(window: &Window, theme: &Theme, text: &str) -> Pixels {
    let size = theme.sized(20.);
    size.max(measure(window, theme, text, theme.caption(), theme.medium) + theme::GAP_MEDIUM)
}

fn cap(theme: &Theme, text: String, width: Pixels) -> gpui::Div {
    let c = &theme.colours;
    div()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .w(width)
        .h(theme.sized(20.))
        .rounded(px(4.))
        .bg(c.chip)
        .border_1()
        .border_color(c.line)
        .child(label(theme, theme.caption(), c.ink, text).font_weight(theme.medium))
}

fn caps(window: &Window, theme: &Theme, keys: &str) -> (gpui::Div, Pixels) {
    let c = &theme.colours;
    let comma = measure(window, theme, ",", theme.small(), FontWeight::NORMAL);
    let mut width = px(0.);
    let mut row = div().flex().flex_none().items_center().gap(theme::GAP_TINY);
    for (at, combo) in keys.split(", ").enumerate() {
        if at > 0 {
            row = row.child(label(theme, theme.small(), c.ink3, ",".to_string()));
            width += comma + theme::GAP_TINY;
        }
        for (part, key) in combo.split(" + ").enumerate() {
            if at > 0 || part > 0 {
                width += theme::GAP_TINY;
            }
            let wide = cap_width(window, theme, key);
            width += wide;
            row = row.child(cap(theme, key.to_string(), wide));
        }
    }
    (row, width)
}

impl Shell {
    pub(crate) fn open_keys(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.keys_open = true;
        window.focus(&self.keys_focus, cx);
        cx.notify();
    }

    fn close_keys(&mut self, cx: &mut Context<Self>) {
        self.keys_open = false;
        cx.notify();
    }

    pub(crate) fn keys_sheet(&self, window: &Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.keys_open {
            return None;
        }
        let theme = cx.global::<Theme>().clone();
        let c = theme.colours;
        let viewport = window.viewport_size();
        let width = theme.sized(760.).min(viewport.width - theme::GAP_PAGE * 2.);
        let inner = width - theme::GAP_WIDE * 2.;
        // `FontMetrics.averageCharacterWidth`, as the lowercase letters average it.
        let alphabet = "abcdefghijklmnopqrstuvwxyz";
        let char_width = measure(window, &theme, alphabet, theme.small(), FontWeight::NORMAL) / 26.;
        let columns = ((inner + theme::GAP_PAGE) / (char_width * 48. + theme::GAP_PAGE))
            .floor()
            .clamp(1., 2.) as usize;
        // A pixel less: two columns that fill the row exactly would wrap on a rounding.
        let column_width =
            ((inner - theme::GAP_PAGE * (columns - 1) as f32) / columns as f32 - px(1.)).floor();
        let shown: Vec<AnyElement> = groups()
            .into_iter()
            .map(|(title, keys)| {
                let made: Vec<(gpui::Div, Pixels, String)> = keys
                    .iter()
                    .map(|(keys, what)| {
                        let (row, wide) = caps(window, &theme, &put(keys));
                        (row, wide, put(what))
                    })
                    .collect();
                let keys_width = made.iter().map(|(_, w, _)| *w).fold(px(0.), Pixels::max);
                div()
                    .flex()
                    .flex_col()
                    .w(column_width)
                    .gap(theme::GAP_THIN)
                    .child(
                        label(&theme, theme.small(), c.ink2, put(&title))
                            .font_weight(theme.semibold)
                            .pb(theme::GAP_THIN),
                    )
                    .children(made.into_iter().map(|(row, _, what)| {
                        div()
                            .flex()
                            .items_start()
                            .gap(theme::GAP_WIDE)
                            .child(div().w(keys_width).flex_none().child(row))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .sized_text(&theme, theme.small())
                                    .text_color(c.ink2)
                                    .child(what),
                            )
                    }))
                    .into_any_element()
            })
            .collect();
        let sheet = div()
            .id("keys-sheet")
            .flex()
            .flex_col()
            .gap(theme::GAP_LARGE)
            .w(width)
            .max_h(viewport.height - theme::GAP_PAGE * 2.)
            .p(theme::GAP_WIDE)
            .rounded(theme::CORNER_LARGE)
            .bg(c.bar)
            .border_1()
            .border_color(c.line)
            .occlude()
            .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                label(&theme, theme.title(), c.ink, put(&say!("Keyboard")))
                    .font_weight(theme.semibold),
            )
            .child(
                div()
                    .id("keys-groups")
                    .flex()
                    .flex_wrap()
                    .gap_x(theme::GAP_PAGE)
                    .gap_y(theme::GAP_CARD)
                    .min_h_0()
                    .overflow_y_scroll()
                    .children(shown),
            )
            .child(
                div().flex().justify_end().child(
                    Btn::new("keys-close", put(&say!("Close")))
                        .primary()
                        .on_click(cx.listener(|shell, _, _, cx| shell.close_keys(cx))),
                ),
            );
        Some(
            div()
                .id("keys")
                .track_focus(&self.keys_focus)
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .bg(super::widgets::with_alpha(gpui::black(), 0.5))
                .occlude()
                .on_key_down(cx.listener(|shell, event: &KeyDownEvent, _, cx| {
                    if event.keystroke.key == "escape" {
                        shell.close_keys(cx);
                    }
                }))
                .on_mouse_down(
                    gpui::MouseButton::Left,
                    cx.listener(|shell, _, _, cx| shell.close_keys(cx)),
                )
                .child(sheet)
                .into_any_element(),
        )
    }
}

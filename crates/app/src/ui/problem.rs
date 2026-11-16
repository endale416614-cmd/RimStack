use super::theme::{self, Theme};
use super::widgets::{Btn, TextSized, btn_width, icon, measure};
use crate::words::put;
use crate::workspace::load_order::ProblemSaid;
use gpui::{
    AnyElement, FontWeight, HighlightStyle, InteractiveElement, IntoElement, ParentElement, Pixels,
    StatefulInteractiveElement, Styled, StyledText, Window, div, prelude::FluentBuilder,
};
use rimstack_core::base::sentence::Sentence;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Act {
    Fix,
    Ignore,
    Show,
}

pub type Acting = std::rc::Rc<dyn Fn(Act, &mut Window, &mut gpui::App)>;

/// `room`: the line's width and the window, to know whether the fix fits beside the words;
/// `acting`: what its buttons and its words do.
pub fn problem(
    theme: &Theme,
    at: usize,
    said: &ProblemSaid,
    room: Option<(Pixels, &Window)>,
    acting: Option<Acting>,
) -> AnyElement {
    let c = &theme.colours;
    let tone = if said.error { c.err } else { c.warn };
    let text = match said.name.is_empty() {
        true => put(&said.words),
        false => format!("{}  {}", said.name, put(&said.words)),
    };
    let bold = HighlightStyle {
        font_weight: Some(FontWeight::BOLD),
        ..HighlightStyle::default()
    };
    let fix =
        (!said.fix.action.is_empty()).then(|| put(&Sentence::said(said.fix.label, Vec::new())));
    let stacked = room.is_none_or(|(width, window)| {
        let actions = fix.as_deref().map_or(Pixels::ZERO, |fix| {
            btn_width(window, theme, fix, false, false)
        }) + if said.ignore.is_empty() {
            Pixels::ZERO
        } else {
            theme.icon() + theme.sized(14.) + theme::GAP_SMALL
        };
        let twenty = measure(window, theme, "n", theme.body(), FontWeight::NORMAL) * 20.;
        width - actions - theme.icon() - theme::GAP * 2. < twenty
    });
    let press = |act: Act| {
        let acting = acting.clone();
        move |_: &gpui::ClickEvent, window: &mut Window, cx: &mut gpui::App| {
            if let Some(acting) = &acting {
                acting(act, window, cx);
            }
        }
    };
    let shows = acting.is_some() && !said.name.is_empty() && said.mod_id.is_some();
    let words = div()
        .id(("problem-words", at))
        .when(shows, |d| d.cursor_pointer().on_click(press(Act::Show)))
        .spaced_text(theme, theme.body(), 1.25)
        .text_color(c.ink)
        .child(StyledText::new(text).with_highlights([(0..said.name.len(), bold)]));
    let actions = div()
        .flex()
        .flex_none()
        .gap(theme::GAP_SMALL)
        .when_some(fix, |d, fix| {
            d.child(Btn::new(("fix", at), fix).on_click(press(Act::Fix)))
        })
        .when(!said.ignore.is_empty(), |d| {
            d.child(
                Btn::new(("ignore", at), "")
                    .icon("eye-off")
                    .spoken(put(&rimstack_core::say!("Ignore this problem")))
                    .tip(put(&rimstack_core::say!(
                        "No warning about this problem again; the mod's details can turn it back on"
                    )))
                    .on_click(press(Act::Ignore)),
            )
        });
    let mark = icon(
        if said.error {
            "circle-alert"
        } else {
            "triangle-alert"
        },
        theme.icon(),
        tone,
    )
    .mt(theme::GAP_HAIR);
    match stacked {
        true => div()
            .flex()
            .gap(theme::GAP)
            .child(mark)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .gap(theme::GAP_TINY)
                    .child(words)
                    .child(actions),
            )
            .into_any_element(),
        false => div()
            .flex()
            .items_start()
            .gap(theme::GAP)
            .child(mark)
            .child(div().flex_1().min_w_0().child(words))
            .child(actions)
            .into_any_element(),
    }
}

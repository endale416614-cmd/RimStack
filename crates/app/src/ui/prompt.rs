use super::theme::{self, Theme};
use super::widgets::{Btn, TextSized};
use crate::words::put;
use gpui::{
    AnyElement, App, AppContext, Context, Entity, FocusHandle, Focusable, Global,
    InteractiveElement, IntoElement, KeyDownEvent, ParentElement, Render,
    StatefulInteractiveElement, Styled, Window, div, prelude::FluentBuilder,
};
use rimstack_core::say;

pub enum Line {
    Text(String),
    Path(String),
    Field {
        text: Entity<super::text_field::TextField>,
        placeholder: String,
    },
}

impl Question {
    fn field(&self) -> Option<&Entity<super::text_field::TextField>> {
        self.lines.iter().find_map(|line| match line {
            Line::Field { text, .. } => Some(text),
            _ => None,
        })
    }

    fn filled(&self, cx: &App) -> bool {
        self.field()
            .is_none_or(|field| !field.read(cx).text().trim().is_empty())
    }
}

pub type Answer = Box<dyn FnOnce(&mut Window, &mut App)>;

pub struct Question {
    pub title: String,
    pub lines: Vec<Line>,
    pub label: String,
    pub danger: bool,
    pub run: Answer,
}

pub struct Prompt {
    asked: Option<Question>,
    focus: FocusHandle,
    before: Option<FocusHandle>,
    turn: usize,
    /// Answered or cancelled, drawn only while it fades out; its answer is gone.
    fading: Option<Question>,
}

struct Asking(Entity<Prompt>);

impl Global for Asking {}

pub fn install(cx: &mut App) -> Entity<Prompt> {
    let prompt = cx.new(|cx| Prompt {
        asked: None,
        focus: cx.focus_handle(),
        before: None,
        turn: 0,
        fading: None,
    });
    cx.set_global(Asking(prompt.clone()));
    prompt
}

pub fn ask(question: Question, window: &mut Window, cx: &mut App) {
    let Some(prompt) = cx.try_global::<Asking>().map(|asking| asking.0.clone()) else {
        return;
    };
    let before = window.focused(cx);
    prompt.update(cx, |prompt, cx| {
        let field = question.field().cloned();
        prompt.asked = Some(question);
        prompt.before = before;
        prompt.turn = super::motion::opening();
        prompt.fading = None;
        match field {
            Some(field) => {
                cx.subscribe_in(
                    &field,
                    window,
                    |prompt, _, _: &super::text_field::Entered, window, cx| {
                        prompt.answer(window, cx)
                    },
                )
                .detach();
                cx.observe(&field, |_, _, cx| cx.notify()).detach();
                window.focus(&field.focus_handle(cx), cx);
            }
            None => window.focus(&prompt.focus, cx),
        }
        cx.notify();
    });
}

impl Prompt {
    fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Option<Answer> {
        let run = self.asked.take().map(|mut asked| {
            let run = std::mem::replace(&mut asked.run, Box::new(|_, _| {}));
            if super::motion::fades_out(cx) {
                self.fading = Some(asked);
                let turn = self.turn;
                super::motion::after_fade(cx, move |prompt: &mut Self, cx| {
                    if prompt.turn == turn && prompt.fading.take().is_some() {
                        cx.notify();
                    }
                });
            }
            run
        });
        if let Some(before) = self.before.take() {
            window.focus(&before, cx);
        }
        cx.notify();
        run
    }

    fn answer(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.asked.as_ref().is_some_and(|asked| !asked.filled(cx)) {
            return;
        }
        if let Some(run) = self.close(window, cx) {
            run(window, cx);
        }
    }
}

impl Focusable for Prompt {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for Prompt {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(asked) = self.asked.as_ref().or(self.fading.as_ref()) else {
            return div().id("prompt").into_any_element();
        };
        let fading = self.asked.is_none();
        let theme = cx.global::<Theme>().clone();
        let c = theme.colours;
        let viewport = window.viewport_size();
        let lines: Vec<AnyElement> = asked
            .lines
            .iter()
            .map(|line| match line {
                Line::Text(text) => div()
                    .sized_text(&theme, theme.body())
                    .text_color(c.ink2)
                    .child(text.clone())
                    .into_any_element(),
                Line::Path(path) => div()
                    .sized_text(&theme, theme.small())
                    .font_family(theme.mono.clone())
                    .text_color(c.ink)
                    .child(super::widgets::breakable(path))
                    .into_any_element(),
                Line::Field { text, placeholder } => super::field::Field {
                    id: "prompt-field",
                    text,
                    icon: "",
                    placeholder: placeholder.clone(),
                    found: None,
                    keys: None,
                }
                .render(&theme, window, cx)
                .into_any_element(),
            })
            .collect();
        let answer = Btn::new("prompt-answer", asked.label.clone())
            .enabled(asked.filled(cx))
            .primary_if(!asked.danger)
            .when(asked.danger, Btn::danger)
            .on_click(cx.listener(|prompt, _, window, cx| prompt.answer(window, cx)));
        let cancel = Btn::new("prompt-cancel", put(&say!("Cancel"))).on_click(cx.listener(
            |prompt, _, window, cx| {
                prompt.close(window, cx);
            },
        ));
        let sheet = div()
            .id("prompt-sheet")
            .flex()
            .flex_col()
            .gap(theme::GAP_LARGE)
            .w(theme.sized(400.).min(viewport.width - theme::GAP_PAGE * 2.))
            .max_h(viewport.height - theme::GAP_PAGE * 2.)
            .p(theme::GAP_WIDE)
            .rounded(theme::CORNER_LARGE)
            .bg(c.bar)
            .border_1()
            .border_color(c.line)
            .when(!fading, |d| d.occlude())
            .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                div()
                    .sized_text(&theme, theme.title())
                    .font_weight(theme.semibold)
                    .text_color(c.ink)
                    .child(asked.title.clone()),
            )
            .child(
                div()
                    .id("prompt-lines")
                    .flex()
                    .flex_col()
                    .gap(theme::GAP)
                    .min_h_0()
                    .overflow_y_scroll()
                    .children(lines),
            )
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap(theme::GAP)
                    .child(cancel)
                    .child(answer),
            );
        let dimmed = div()
            .id("prompt")
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(super::widgets::with_alpha(gpui::black(), 0.5));
        if fading {
            return super::motion::pop_out(
                dimmed.child(sheet.child(super::motion::shield())),
                self.turn,
            )
            .into_any_element();
        }
        dimmed
            .track_focus(&self.focus)
            .occlude()
            .on_key_down(cx.listener(|prompt, event: &KeyDownEvent, window, cx| {
                if event.keystroke.key == "escape" {
                    prompt.close(window, cx);
                }
            }))
            .on_mouse_down(
                gpui::MouseButton::Left,
                cx.listener(|prompt, _, window, cx| {
                    prompt.close(window, cx);
                }),
            )
            .child(super::motion::pop_in(sheet, self.turn))
            .into_any_element()
    }
}

#[cfg(all(test, target_os = "linux"))]
pub(crate) fn drawn(cx: &App) -> bool {
    cx.try_global::<Asking>().is_some_and(|asking| {
        let prompt = asking.0.read(cx);
        prompt.asked.is_some() || prompt.fading.is_some()
    })
}

#[cfg(test)]
pub(crate) fn shown(cx: &App) -> Option<(String, Vec<String>, bool)> {
    let prompt = cx.try_global::<Asking>()?.0.read(cx);
    let asked = prompt.asked.as_ref()?;
    let lines = asked.lines.iter().map(|line| match line {
        Line::Text(text) | Line::Path(text) => text.clone(),
        Line::Field { placeholder, .. } => placeholder.clone(),
    });
    Some((asked.title.clone(), lines.collect(), asked.danger))
}

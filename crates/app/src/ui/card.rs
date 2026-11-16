use super::theme::{self, Theme};
use super::widgets::{TextSized, icon, label, measure};
use crate::words::put;
use gpui::AnimationExt;
use gpui::{
    AnyElement, App as Cx, ClickEvent, ElementId, Hsla, InteractiveElement, IntoElement,
    ParentElement, Pixels, StatefulInteractiveElement, Styled, Window, div, prelude::FluentBuilder,
    px,
};
use rimstack_core::base::sentence::Sentence;

pub type Fold = Box<dyn Fn(&ClickEvent, &mut Window, &mut Cx)>;

const UNFOLDING: std::time::Duration = super::motion::QUICK;

thread_local! {
    /// The cards unfolded by a press, and when: only those fade their body in, not a card shown
    /// with its page.
    static UNFOLDED: std::cell::RefCell<std::collections::HashMap<ElementId, std::time::Instant>> =
        Default::default();
}

fn turn_of(id: &ElementId, when: std::time::Instant) -> usize {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    (id, when).hash(&mut hasher);
    hasher.finish() as usize
}

fn unfolded_just_now(id: &ElementId) -> Option<std::time::Instant> {
    UNFOLDED.with_borrow_mut(|unfolded| {
        unfolded.retain(|_, when| when.elapsed() < UNFOLDING * 3);
        unfolded.get(id).copied()
    })
}

pub struct Card {
    id: ElementId,
    title: String,
    tone: Option<Hsla>,
    note: String,
    folded: Option<bool>,
    on_fold: Option<Fold>,
    actions: Vec<AnyElement>,
    actions_width: Pixels,
    tall: bool,
    spacing: Pixels,
    body: Vec<AnyElement>,
}

pub fn defined(title: &str) -> bool {
    meaning(title).is_some()
}

pub fn meaning(title: &str) -> Option<String> {
    let (_, meaning) = theme::TERMS
        .iter()
        .find(|(term, _)| *term == title || put(&Sentence::said(term, Vec::new())) == title)?;
    Some(put(&Sentence::said(meaning, Vec::new())))
}

impl Card {
    pub fn new(id: impl Into<ElementId>, title: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            tone: None,
            note: String::new(),
            folded: None,
            on_fold: None,
            actions: Vec::new(),
            actions_width: Pixels::ZERO,
            tall: false,
            spacing: theme::GAP_SMALL,
            body: Vec::new(),
        }
    }

    pub fn tone(mut self, tone: Hsla) -> Self {
        self.tone = Some(tone);
        self
    }

    pub fn note(mut self, note: impl Into<String>) -> Self {
        self.note = note.into();
        self
    }

    pub fn folds(
        mut self,
        folded: bool,
        fold: impl Fn(&ClickEvent, &mut Window, &mut Cx) + 'static,
    ) -> Self {
        self.folded = Some(folded);
        let id = self.id.clone();
        self.on_fold = Some(Box::new(move |event, window, cx| {
            if folded {
                UNFOLDED.with_borrow_mut(|unfolded| {
                    unfolded.insert(id.clone(), std::time::Instant::now())
                });
            }
            fold(event, window, cx)
        }));
        self
    }

    pub fn actions(mut self, actions: Vec<AnyElement>, width: Pixels) -> Self {
        self.actions = actions;
        self.actions_width = width;
        self
    }

    /// As tall a head as one with actions: an action not shown yet counts.
    pub fn tall(mut self) -> Self {
        self.tall = true;
        self
    }

    pub fn spacing(mut self, spacing: Pixels) -> Self {
        self.spacing = spacing;
        self
    }

    pub fn body(mut self, body: Vec<AnyElement>) -> Self {
        self.body = body;
        self
    }

    pub fn when(self, shown: bool, then: impl FnOnce(Self) -> Self) -> Self {
        if shown { then(self) } else { self }
    }

    pub fn when_some<T>(self, given: Option<T>, then: impl FnOnce(Self, T) -> Self) -> Self {
        match given {
            Some(given) => then(self, given),
            None => self,
        }
    }

    pub fn child(mut self, child: impl IntoElement) -> Self {
        self.body.push(child.into_any_element());
        self
    }

    pub fn render(
        self,
        theme: &Theme,
        fit: Option<(Pixels, &Window)>,
    ) -> gpui::Stateful<gpui::Div> {
        let c = &theme.colours;
        let tone = self.tone.unwrap_or(c.ink2);
        let folded = self.folded == Some(true);
        let has_actions = !self.actions.is_empty();
        let fold_width = if self.folded.is_some() {
            theme.icon()
        } else {
            Pixels::ZERO
        };
        let wrapped = has_actions
            && fit.is_some_and(|(width, window)| {
                let name = measure(window, theme, &self.title, theme.small(), theme.semibold);
                let note = measure(
                    window,
                    theme,
                    &self.note,
                    theme.small(),
                    gpui::FontWeight::NORMAL,
                );
                name + note.min(theme.sized(120.))
                    + self.actions_width
                    + fold_width
                    + theme::GAP
                    + theme::GAP_SMALL * 3.
                    > width
            });
        let line = theme.sized(if (has_actions || self.tall) && !wrapped {
            34.
        } else {
            28.
        });
        let actions = div()
            .flex()
            .flex_none()
            .items_center()
            .gap(theme::GAP_SMALL)
            .children(self.actions);
        let (beside, under) = match (has_actions, wrapped) {
            (true, false) => (Some(actions), None),
            (true, true) => (
                None,
                Some(
                    div()
                        .flex()
                        .items_center()
                        .h(theme.sized(34.))
                        .child(actions),
                ),
            ),
            (false, _) => (None, None),
        };
        let meaning = meaning(&self.title);
        let spoken_meaning = meaning.clone();
        let title = label(theme, theme.small(), tone, self.title.clone())
            .font_weight(theme.semibold)
            .when(meaning.is_some(), |d| {
                d.border_b_1().border_dashed().border_color(tone)
            });
        let title = super::tip::tipped(title.id("card-title"), meaning.unwrap_or_default().into());
        let note = div()
            .text_decoration_none()
            .flex_1()
            .min_w_0()
            .mr(theme::GAP_SMALL)
            .overflow_hidden()
            .whitespace_nowrap()
            .text_ellipsis()
            .sized_text(theme, theme.small())
            .text_color(c.ink3)
            .child(self.note);
        let card_id = self.id.clone();
        let head = div()
            .id("card-head")
            .flex()
            .flex_col()
            .w_full()
            .child(
                div()
                    .flex()
                    .items_center()
                    .h(line)
                    .gap(theme::GAP)
                    .child(title)
                    .child(note)
                    .when(beside.is_some() || self.folded.is_some(), |d| {
                        let fold = match self.folded {
                            Some(folded) => icon(
                                if folded {
                                    "chevron-right"
                                } else {
                                    "chevron-down"
                                },
                                theme.icon(),
                                c.ink3,
                            )
                            .into_any_element(),
                            None => div().into_any_element(),
                        };
                        d.child(
                            div()
                                .text_decoration_none()
                                .flex()
                                .flex_none()
                                .items_center()
                                .gap(theme::GAP_SMALL)
                                .children(beside)
                                .child(fold),
                        )
                    }),
            )
            .children(under.map(|under| under.text_decoration_none()))
            .role(match self.on_fold.is_some() {
                true => gpui::Role::Button,
                false => gpui::Role::Heading,
            })
            .aria_label(self.title.clone())
            .when_some(spoken_meaning, |d, meaning| d.aria_description(meaning))
            .when_some(self.on_fold, |d, fold| {
                d.cursor_pointer()
                    .tab_index(0)
                    .capture_any_mouse_down(|_, window, _| window.prevent_default())
                    .focus_visible(|s| s.underline())
                    .on_click(fold)
            });
        div()
            // Each card its own id: siblings sharing one share their children's layout keys,
            // and gpui-fast then lays out every view around them again each frame.
            .id(card_id.clone())
            .flex()
            .flex_col()
            .w_full()
            .child(head)
            .when(!folded, |d| {
                let body = div()
                    .flex()
                    .flex_col()
                    .gap(self.spacing)
                    .pb(px(6.))
                    .children(self.body);
                match unfolded_just_now(&card_id) {
                    Some(when) => d.child(body.with_animation(
                        ("unfolding", turn_of(&card_id, when)),
                        gpui::Animation::new(UNFOLDING).with_easing(super::motion::out_cubic),
                        |body, shown| body.opacity(shown),
                    )),
                    None => d.child(body),
                }
            })
    }
}

pub fn group_title(theme: &Theme, title: String) -> AnyElement {
    use gpui::SharedString;
    let c = &theme.colours;
    let meaning = meaning(&title);
    let name = super::widgets::label(theme, theme.small(), c.ink2, title.clone())
        .font_weight(theme.semibold)
        .when(meaning.is_some(), |d| {
            d.border_b_1().border_dashed().border_color(c.ink2)
        });
    let row = div()
        .id(SharedString::from(format!("group-{title}")))
        .flex()
        .items_center()
        .gap(theme::GAP_SMALL)
        .pt(theme::GAP_WIDE)
        .pb(theme::GAP_SMALL)
        .child(name)
        .when(meaning.is_some(), |d| {
            d.child(super::widgets::icon("info", theme.sized(11.), c.ink3))
        });
    super::tip::tipped(row, meaning.unwrap_or_default().into()).into_any_element()
}

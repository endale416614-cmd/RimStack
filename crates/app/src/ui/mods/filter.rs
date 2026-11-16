use super::Mods;
use crate::ui::button::Chip;
use crate::ui::text_field::{self, Edited, TextField};
use crate::ui::theme::{self, Theme};
use crate::ui::widgets::{TextSized, halo, icon, label, measure_in};
use crate::words::put;
use crate::workspace::load_order::Side;
use gpui::{
    AppContext, Context, Entity, Focusable, FontWeight, InteractiveElement, ParentElement, Pixels,
    StatefulInteractiveElement, Styled, Window, anchored, deferred, div, prelude::FluentBuilder,
    px,
};
use rimstack_core::mods::list::flag;
use rimstack_core::mods::list::offers::{Counted, Offers, is_term};
use rimstack_core::say;

/// The filter of the load order and its mirror in the drawer: the terms shown as chips, and
/// what is typed after them. Mods hears `Changed` with the whole filter.
pub struct Filter {
    pub field: Entity<TextField>,
    pub mirror: Entity<TextField>,
    terms: Vec<String>,
    /// The offers under the filter put away until it is typed in or clicked again.
    offers_shut: bool,
}

pub struct Changed(pub String);

impl gpui::EventEmitter<Changed> for Filter {}

impl Filter {
    pub fn new(size: Pixels, cx: &mut Context<Self>) -> Self {
        let field = cx.new(|cx| TextField::new(size, cx));
        let mirror = cx.new(|cx| TextField::new(size, cx));
        for typed in [&field, &mirror] {
            cx.subscribe(typed, |filter, field, _: &Edited, cx| {
                filter.edited(&field, cx)
            })
            .detach();
        }
        Self {
            field,
            mirror,
            terms: Vec::new(),
            offers_shut: false,
        }
    }

    pub fn terms(&self) -> &[String] {
        &self.terms
    }

    pub fn offers_shut(&self) -> bool {
        self.offers_shut
    }

    pub fn shut_offers(&mut self, shut: bool, cx: &mut Context<Self>) {
        self.offers_shut = shut;
        cx.notify();
    }

    pub fn text(&self, cx: &gpui::App) -> String {
        let typed = self.field.read(cx).text().trim().to_string();
        let mut words = self.terms.clone();
        if !typed.is_empty() {
            words.push(typed);
        }
        words.join(" ")
    }

    pub fn set(&mut self, text: &str, cx: &mut Context<Self>) {
        let words: Vec<&str> = text.split(' ').filter(|w| !w.is_empty()).collect();
        self.terms = words
            .iter()
            .filter(|w| is_term(w))
            .map(|w| w.to_string())
            .collect();
        let typed: Vec<&str> = words.into_iter().filter(|w| !is_term(w)).collect();
        self.field
            .update(cx, |field, cx| field.set_text(&typed.join(" "), cx));
    }

    fn edited(&mut self, field: &Entity<TextField>, cx: &mut Context<Self>) {
        let text = field.read(cx).text().to_string();
        let words: Vec<&str> = text.split(' ').collect();
        let last = words.len().saturating_sub(1);
        let done: Vec<String> = words[..last]
            .iter()
            .filter(|w| is_term(w))
            .map(|w| w.to_string())
            .collect();
        let rest = match done.is_empty() {
            true => text.clone(),
            false => {
                let kept: Vec<&str> = words
                    .iter()
                    .enumerate()
                    .filter(|(at, w)| *at == last || !is_term(w))
                    .map(|(_, w)| *w)
                    .collect();
                kept.join(" ")
            }
        };
        self.terms.extend(done);
        for other in [self.field.clone(), self.mirror.clone()] {
            if other.read(cx).text() != rest {
                other.update(cx, |field, cx| field.set_text_quietly(&rest, cx));
            }
        }
        self.offers_shut = false;
        cx.emit(Changed(self.text(cx)));
        cx.notify();
    }

    pub fn drop_term(&mut self, at: usize, cx: &mut Context<Self>) {
        if at < self.terms.len() {
            self.terms.remove(at);
        }
        let field = self.field.clone();
        self.edited(&field, cx);
    }

    fn take_offer(&mut self, term: String, field: Entity<TextField>, cx: &mut Context<Self>) {
        let text = field.read(cx).text().to_string();
        let mut words: Vec<&str> = text.split(' ').collect();
        words.pop();
        if term.ends_with(':') {
            words.push(&term);
            let typed = words.join(" ");
            field.update(cx, |field, cx| field.set_text(&typed, cx));
            return;
        }
        if !self.terms.contains(&term) {
            self.terms.push(term.clone());
        }
        let typed = words.join(" ");
        field.update(cx, |field, cx| field.set_text(&typed, cx));
    }

    pub fn clear(&mut self, cx: &mut Context<Self>) {
        self.terms.clear();
        self.offers_shut = true;
        self.field.update(cx, |field, cx| field.set_text("", cx));
    }
}

impl Mods {
    pub fn filter_text(&self, cx: &gpui::App) -> String {
        self.filtering.read(cx).text(cx)
    }

    pub fn filter_by(&mut self, text: &str, cx: &mut Context<Self>) {
        self.filtering.update(cx, |filter, cx| filter.set(text, cx));
    }

    pub(super) fn filter_changed(&mut self, composed: String, cx: &mut Context<Self>) {
        self.edit_list(cx, |list, _| list.order.set_filter(&composed));
        self.show(cx);
        self.keep_left_filter(composed, cx);
    }

    pub(super) fn clear_filter(&mut self, cx: &mut Context<Self>) {
        self.filtering.update(cx, |filter, cx| filter.clear(cx));
    }

    fn offers(&self, field: &Entity<TextField>, cx: &gpui::App) -> Offers {
        let typed = field.read(cx).text().to_string();
        let partial = typed.rsplit(' ').next().unwrap_or_default().to_string();
        let order = &self.list.read(cx).order;
        let (errors, warnings) = order.errors_and_warnings();
        let counted = Counted {
            errors,
            warnings,
            conflicts: self.reads.conflicts,
            updated: order.list(Side::Active).flagged(flag::UPDATED),
        };
        order.filter_offers(&partial, &self.filter_text(cx), &counted)
    }

    pub(super) fn filter_field(
        &self,
        field: &Entity<TextField>,
        id: &'static str,
        room: Pixels,
        theme: &Theme,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let c = &theme.colours;
        let typed = field.read(cx).text().to_string();
        let focused = field.read(cx).is_focused(window);
        let focus = field.focus_handle(cx);
        let filtering = self.filtering.read(cx);
        let (terms, offers_shut) = (filtering.terms().to_vec(), filtering.offers_shut());
        let empty = typed.is_empty() && terms.is_empty();
        let found = (!empty).then(|| put(&self.list.read(cx).order.filter_summary()));
        let keys = put(&say!("Ctrl + F"));
        let placeholder = put(&say!("Filter active and inactive mods"));
        if field.read(cx).spoken() != placeholder.as_str() {
            field.update(cx, |field, _| field.speak_as(&placeholder));
        }
        let keys_fit = room
            > theme.icon()
                + theme::GAP
                + crate::ui::widgets::measure(
                    window,
                    theme,
                    &placeholder,
                    theme.body(),
                    FontWeight::NORMAL,
                )
                + crate::ui::widgets::measure(
                    window,
                    theme,
                    &keys,
                    theme.caption(),
                    FontWeight::NORMAL,
                )
                + theme::GAP * 4.;
        let chips = terms.iter().enumerate().map(|(at, term)| {
            Chip::new(term.clone())
                .tone(c.moved)
                .icon("x")
                .tip(put(&say!("Remove")))
                .on_click(
                    ("filter-term", at),
                    cx.listener(move |mods, _, window, cx| {
                        mods.filtering
                            .update(cx, |filter, cx| filter.drop_term(at, cx));
                        let focus = mods.filter.focus_handle(cx);
                        window.focus(&focus, cx);
                    }),
                )
        });
        let offered = match focused && !offers_shut {
            true => self.offers(field, cx),
            false => Default::default(),
        };
        let suggestions = (!offered.offers.is_empty()).then(|| {
            let wide = room.min(theme.sized(420.));
            let key_room = measure_in(
                window,
                theme,
                theme.mono.clone(),
                &offered.widest,
                theme.small(),
                FontWeight::NORMAL,
            )
            .ceil()
            .min(wide / 2.);
            let held = field.clone();
            let rows = offered.offers.into_iter().enumerate().map(|(at, offer)| {
                let (term, about) = (offer.term.clone(), put(&offer.about));
                let held = held.clone();
                div()
                    .id(("filter-offer", at))
                    .role(gpui::Role::Button)
                    .aria_label(put(&say!("Filter by {}", term.clone())))
                    .flex()
                    .items_center()
                    .gap(theme.sized(16.))
                    .h(theme.sized(26.))
                    .px(theme::GAP)
                    .rounded(theme::CORNER)
                    .cursor_pointer()
                    .hover(|s| s.bg(c.chip))
                    .child(
                        div()
                            .flex_none()
                            .w(key_room)
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_ellipsis()
                            .font_family(theme.mono.clone())
                            .sized_text(theme, theme.small())
                            .text_color(c.ink)
                            .child(term.clone()),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_ellipsis()
                            .sized_text(theme, theme.small())
                            .text_color(c.ink2)
                            .child(about),
                    )
                    .on_click(cx.listener(move |mods, _, window, cx| {
                        let (term, field) = (term.clone(), held.clone());
                        mods.filtering
                            .update(cx, |filter, cx| filter.take_offer(term, field, cx));
                        let focus = held.focus_handle(cx);
                        window.focus(&focus, cx);
                    }))
            });
            div().absolute().top_full().left_0().child(deferred(
                anchored().child(
                    div()
                        .id("filter-offers")
                        .mt(px(4.))
                        .w(wide)
                        .p(px(6.))
                        .rounded(theme::CORNER_LARGE)
                        .bg(c.bar)
                        .border_1()
                        .border_color(c.line)
                        .occlude()
                        .on_mouse_down_out(cx.listener(|mods, _, _, cx| {
                            mods.filtering
                                .update(cx, |filter, cx| filter.shut_offers(true, cx));
                        }))
                        .children(rows)
                        .child(
                            div()
                                .px(theme::GAP)
                                .pt(theme::GAP_SMALL)
                                .pb(theme::GAP_TINY)
                                .sized_text(theme, theme.small())
                                .text_color(c.ink3)
                                .child(put(&say!("Use ! to exclude, as in !tag:hud"))),
                        ),
                ),
            ))
        });
        let held = field.clone();
        div()
            .id(id)
            .relative()
            .flex()
            .items_center()
            .h(theme.field())
            .pl(theme::GAP_SMALL)
            .pr(theme::GAP)
            .gap(theme::GAP)
            .rounded(theme::CORNER)
            .bg(c.well)
            .border_1()
            .border_color(if focused { c.focus } else { c.field_line })
            .when(focused, |d| d.children(halo(c.focus, theme::CORNER)))
            .cursor_text()
            .capture_action(cx.listener(|mods, _: &text_field::Escape, _, cx| {
                mods.clear_filter(cx);
                cx.stop_propagation();
            }))
            .capture_action(cx.listener(move |mods, _: &text_field::Backspace, _, cx| {
                let terms = mods.filtering.read(cx).terms().len();
                match held.read(cx).text().is_empty() && terms > 0 {
                    true => {
                        mods.filtering
                            .update(cx, |filter, cx| filter.drop_term(terms - 1, cx));
                        cx.stop_propagation();
                    }
                    false => cx.propagate(),
                }
            }))
            .on_click(cx.listener(move |mods, _, window, cx| {
                mods.filtering
                    .update(cx, |filter, cx| filter.shut_offers(false, cx));
                window.focus(&focus, cx);
            }))
            .child(icon("funnel", theme.icon() - px(1.), c.ink3))
            .when(!terms.is_empty(), |d| {
                d.child(
                    div()
                        .flex()
                        .flex_none()
                        .gap(theme::GAP_TINY)
                        .mr(theme::GAP_LARGE - theme::GAP)
                        .children(chips),
                )
            })
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_w_0()
                    .when(empty, |d| {
                        d.child(
                            div()
                                .absolute()
                                .inset_0()
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_ellipsis()
                                .sized_text(theme, theme.body())
                                .text_color(c.ink3)
                                .child(placeholder),
                        )
                    })
                    .child(field.clone()),
            )
            .when_some(found, |d, found| {
                d.child(label(theme, theme.small(), c.ink2, found))
            })
            .when(empty && !focused && keys_fit, |d| {
                d.child(label(theme, theme.caption(), c.ink3, keys))
            })
            .when(!empty, |d| {
                d.child(crate::ui::tip::tipped(
                    div()
                        .id((id, 1usize))
                        .role(gpui::Role::Button)
                        .aria_label(put(&say!("Clear (Esc)")))
                        .cursor_pointer()
                        .child(icon("x", theme.sized(12.), c.ink3))
                        .on_click(cx.listener(|mods, _, window, cx| {
                            mods.clear_filter(cx);
                            let focus = mods.filter.focus_handle(cx);
                            window.focus(&focus, cx);
                        })),
                    put(&say!("Clear (Esc)")).into(),
                ))
            })
            .children(suggestions)
    }
}

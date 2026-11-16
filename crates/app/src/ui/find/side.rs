use super::Find;
use super::layout::Layout;
use crate::ui::field::Field;
use crate::ui::focus;
use crate::ui::rows::{bar, rows};
use crate::ui::text_field::Escape;
use crate::ui::theme::{self, Theme};
use crate::ui::widgets::{TextSized, caps, icon, label, watched, with_alpha};
use crate::words::put;
use crate::workspace::find::Choice;
use gpui::{
    AnyElement, App, Context, ElementId, InteractiveElement, IntoElement, KeyDownEvent,
    ParentElement, Pixels, RenderOnce, Role, SharedString, StatefulInteractiveElement, Styled,
    Toggled, Window, anchored, deferred, div, prelude::FluentBuilder, relative,
};
use rimstack_core::say;
use std::rc::Rc;

type Picked = Rc<dyn Fn(&mut Window, &mut App)>;

#[derive(IntoElement)]
struct Pick {
    id: ElementId,
    text: SharedString,
    count: SharedString,
    chosen: bool,
    radio: bool,
    enabled: bool,
    picked: Picked,
}

impl Pick {
    fn new(
        id: impl Into<ElementId>,
        text: impl Into<SharedString>,
        chosen: bool,
        picked: Picked,
    ) -> Self {
        Self {
            id: id.into(),
            text: text.into(),
            count: SharedString::default(),
            chosen,
            radio: false,
            enabled: true,
            picked,
        }
    }

    fn radio(mut self) -> Self {
        self.radio = true;
        self
    }

    fn count(mut self, count: impl Into<SharedString>) -> Self {
        self.count = count.into();
        self
    }

    fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}

impl RenderOnce for Pick {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let handle = focus::handle(self.id.clone(), window, cx);
        let theme = cx.global::<Theme>();
        let c = &theme.colours;
        let size = theme.icon();
        let corner = if self.radio { size / 2. } else { theme::CORNER };
        let mark = match (self.chosen, self.radio) {
            (false, _) => None,
            (true, false) => Some(icon("check", theme.sized(11.), c.accent_ink).into_any_element()),
            (true, true) => Some(
                div()
                    .size(size / 3.)
                    .rounded(size / 6.)
                    .bg(c.accent_ink)
                    .into_any_element(),
            ),
        };
        let picked = self.picked.clone();
        let control = div()
            .id(self.id)
            .role(if self.radio {
                Role::RadioButton
            } else {
                Role::CheckBox
            })
            .aria_label(self.text.clone())
            .aria_toggled(if self.chosen {
                Toggled::True
            } else {
                Toggled::False
            })
            .relative()
            .flex()
            .items_center()
            .gap(theme::GAP)
            .h(theme.control())
            .px(theme::GAP_SMALL)
            .rounded(theme::CORNER)
            .when(!self.enabled, |d| d.opacity(0.4))
            .when(self.enabled, |d| {
                d.cursor_pointer()
                    .hover(|d| d.bg(with_alpha(c.hover, 0.06)))
                    .on_click(move |_, window, cx| picked(window, cx))
            })
            .child(
                div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .justify_center()
                    .size(size)
                    .rounded(corner)
                    .bg(if self.chosen { c.accent } else { c.ctl })
                    .when(!self.chosen, |d| {
                        d.border_width(theme.outline()).border_color(c.field_line)
                    })
                    .children(mark),
            )
            .child(watched(
                div()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_ellipsis()
                    .sized_text(theme, theme.small())
                    .text_color(c.ink),
                self.text.clone(),
                self.text,
                true,
            ))
            .when(!self.count.is_empty(), |d| {
                d.child(label(theme, theme.small(), c.ink3, self.count))
            });
        focus::reachable(control, &handle, theme::CORNER, false, window, theme)
    }
}

impl Find {
    fn picked(
        &self,
        cx: &Context<Self>,
        run: impl Fn(&mut Self, &mut Context<Self>) + 'static,
    ) -> Picked {
        let me = cx.entity().downgrade();
        Rc::new(move |_, cx| {
            let _ = me.update(cx, |find, cx| run(find, cx));
        })
    }

    fn checks(&self, list: &[Choice], radio: bool, cx: &Context<Self>) -> Vec<AnyElement> {
        list.iter()
            .map(|choice| {
                let id = choice.id;
                let run = self.picked(cx, move |find, cx| match radio {
                    true => find.choose(id, cx),
                    false => find.flip(id, cx),
                });
                let pick = Pick::new(
                    ElementId::Name(format!("find-option-{id}").into()),
                    put(&choice.label),
                    choice.chosen,
                    run,
                )
                .enabled(choice.available);
                match radio {
                    true => pick.radio().into_any_element(),
                    false => pick.into_any_element(),
                }
            })
            .collect()
    }

    fn offers_shown(&self, window: &Window, cx: &App) -> bool {
        self.types.read(cx).is_focused(window) && !self.offers_shut
    }

    fn types_box(
        &self,
        theme: &Theme,
        width: Pixels,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let c = &theme.colours;
        let field = Field {
            id: "find-types",
            text: &self.types,
            icon: "",
            placeholder: put(&say!("xml, cs; json  or  .")),
            found: None,
            keys: None,
        };
        let shown = self.offers_shown(window, cx);
        let offers = self.offers(cx);
        let popup = shown.then(|| {
            let strip = theme.strip();
            let me = cx.entity().downgrade();
            let lines: Vec<AnyElement> = offers
                .iter()
                .enumerate()
                .map(|(at, offer)| {
                    let me = me.clone();
                    let hovered = me.clone();
                    div()
                        .id(("find-offer", at))
                        .role(Role::Button)
                        .aria_label(offer.label.clone())
                        .flex()
                        .flex_none()
                        .items_center()
                        .gap(theme::GAP)
                        .h(strip)
                        .px(theme::GAP)
                        .rounded(theme::CORNER)
                        .when(at == self.offer_at, |d| d.bg(c.chip))
                        .cursor_pointer()
                        .child(label(theme, theme.body(), c.ink, offer.label.clone()))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .justify_end()
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_ellipsis()
                                .sized_text(theme, theme.small())
                                .text_color(c.ink2)
                                .child(put(&offer.note)),
                        )
                        .on_hover(move |inside, _, cx| {
                            if *inside {
                                let _ = hovered.update(cx, |find, cx| {
                                    find.offer_at = at;
                                    cx.notify();
                                });
                            }
                        })
                        .on_click(move |_, _, cx| {
                            let _ = me.update(cx, |find, cx| find.complete_with(at, cx));
                        })
                        .into_any_element()
                })
                .collect();
            let none = offers.is_empty().then(|| {
                div()
                    .p(theme::GAP)
                    .sized_text(theme, theme.small())
                    .text_color(c.ink3)
                    .child(put(&say!(
                        "None of these mods has such files; it is still searched"
                    )))
            });
            deferred(
                anchored().child(
                    div()
                        .id("find-offers")
                        .mt(theme::GAP_TINY)
                        .w(width)
                        .max_h(strip * 6. + theme::GAP_TINY * 2.)
                        .overflow_y_scroll()
                        .p(theme::GAP_TINY)
                        .rounded(theme::CORNER_LARGE)
                        .bg(c.bar)
                        .border_1()
                        .border_color(c.line)
                        .occlude()
                        .children(none)
                        .children(lines),
                ),
            )
            .with_priority(1)
        });
        div()
            .id("find-types-box")
            .flex()
            .flex_col()
            .capture_action(cx.listener(|find, _: &Escape, window, cx| {
                if find.offers_shown(window, cx) {
                    find.offers_shut = true;
                    cx.stop_propagation();
                    cx.notify();
                }
            }))
            .on_action(cx.listener(|find, _: &focus::Next, window, cx| {
                if find.offers_shown(window, cx) {
                    find.complete_type(cx);
                    cx.stop_propagation();
                }
            }))
            .on_key_down(cx.listener(|find, event: &KeyDownEvent, window, cx| {
                if !find.offers_shown(window, cx) {
                    return;
                }
                match event.keystroke.key.as_str() {
                    "down" => find.step_offer(1, cx),
                    "up" => find.step_offer(-1, cx),
                    _ => return,
                }
                cx.stop_propagation();
            }))
            .on_click(cx.listener(|find, _, _, cx| {
                find.offers_shut = false;
                cx.notify();
            }))
            .child(field.render(theme, window, cx))
            .children(popup)
            .into_any_element()
    }

    pub(super) fn side(
        &mut self,
        theme: &Theme,
        layout: Layout,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let c = &theme.colours;
        let options = self.options.clone();
        let heading = |text: String| div().pt(theme::GAP).child(caps(theme, &text));
        let inner = match layout.side_fills {
            false => layout.side_width,
            true => window.viewport_size().width - theme::GAP_LARGE * 2.,
        } - theme::GAP_CARD * 2.;
        let field_width = inner - crate::ui::page::room(theme);
        let types_problem = options.types.problem.as_ref().map(put);
        let content = div()
            .flex()
            .flex_col()
            .pr(crate::ui::page::room(theme))
            .child(heading(put(&say!("Where"))))
            .children(self.checks(&options.places, true, cx))
            .child(heading(put(&say!("Match"))))
            .children(self.checks(&options.switches, false, cx))
            .child(
                label(theme, theme.small(), c.ink2, put(&say!("File types")))
                    .pt(theme::GAP_SMALL)
                    .pb(theme::GAP_TINY),
            )
            .child(self.types_box(theme, field_width, window, cx))
            .child(
                div()
                    .pt(theme::GAP_TINY)
                    .when_some(types_problem.clone(), |d, problem| {
                        d.sized_text(theme, theme.small())
                            .text_color(c.err)
                            .child(problem)
                    })
                    .when(types_problem.is_none(), |d| {
                        d.sized_text(theme, theme.caption()).text_color(c.ink3).child(put(&say!(
                            "Separate with commas, semicolons or spaces. A dot is every text file."
                        )))
                    }),
            )
            .child(heading(put(&say!("Leave out folders named"))))
            .children(self.checks(&options.leave, false, cx));
        let found = self.found.clone();
        let all = self.picked(cx, |find, cx| find.pick_mod(None, cx));
        let pick_width = inner - theme.sized(12.);
        let tops: Rc<[Pixels]> = (0..=found.rows.len())
            .map(|at| theme.control() * at as f32)
            .collect();
        let me = cx.entity().downgrade();
        let listed = found.clone();
        let list = rows(
            "find-mods",
            self.found_scroll.clone(),
            tops,
            theme.sized(40.),
            cx.entity_id(),
            move |range, _, _, _| {
                range
                    .filter_map(|at| {
                        let one = listed.rows.get(at)?;
                        let (me, index) = (me.clone(), one.index);
                        let run: Picked = Rc::new(move |_, cx| {
                            let _ = me.update(cx, |find, cx| find.pick_mod(Some(index), cx));
                        });
                        Some(
                            div()
                                .w(pick_width)
                                .child(
                                    Pick::new(
                                        ("find-mod-pick", index),
                                        put(&one.name),
                                        one.chosen,
                                        run,
                                    )
                                    .radio()
                                    .count(put(&one.counted)),
                                )
                                .into_any_element(),
                        )
                    })
                    .collect()
            },
        );
        div()
            .id("find-side")
            .relative()
            .flex()
            .flex_col()
            .flex_none()
            .h_full()
            .when(!layout.side_fills, |d| d.w(layout.side_width))
            .when(layout.side_fills, |d| d.flex_1().mr(theme::GAP_LARGE))
            .gap(theme::GAP_SMALL)
            .rounded(theme::CORNER_LARGE)
            .bg(c.panel)
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .rounded(theme::CORNER_LARGE)
                    .border_1()
                    .border_color(c.line),
            )
            .child(
                div()
                    .flex_none()
                    .mx(theme::GAP_CARD)
                    .mt(theme::GAP_CARD)
                    .max_h(relative(0.6))
                    .overflow_hidden()
                    .child(crate::ui::page::page(
                        "find-options",
                        &self.options_paging,
                        theme,
                        cx.entity_id(),
                        content,
                    )),
            )
            .child(
                div()
                    .mx(theme::GAP_CARD)
                    .pt(theme::GAP_SMALL)
                    .child(caps(theme, &put(&say!("Mods with matches")))),
            )
            .child(
                div()
                    .ml(theme::GAP_CARD)
                    .mr(theme::GAP_CARD + theme.sized(12.))
                    .child(
                        Pick::new(
                            "find-all-mods",
                            put(&found.all_label),
                            found.all_chosen,
                            all,
                        )
                        .radio()
                        .count(put(&found.all_counted)),
                    ),
            )
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .mx(theme::GAP_CARD)
                    .mb(theme::GAP_CARD)
                    .child(list)
                    .child(bar(theme, self.found_scroll.clone(), cx.entity_id())),
            )
            .into_any_element()
    }
}

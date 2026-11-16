use super::Workshop;
use crate::ui::controls::Toggle;
use crate::ui::focus;
use crate::ui::theme::{self, Theme};
use crate::ui::widgets::{TextSized, caps, icon, label, watched};
use crate::words::put;
use crate::workspace::browse::Facet;
use gpui::{
    AnyElement, App, Context, ElementId, Hsla, InteractiveElement, IntoElement, ParentElement,
    Pixels, RenderOnce, Role, SharedString, StatefulInteractiveElement, Styled, Toggled, Window,
    div, prelude::FluentBuilder,
};
use rimstack_core::say;
use std::rc::Rc;

type Pressed = Rc<dyn Fn(&mut Window, &mut App)>;

#[derive(IntoElement)]
struct Tick {
    id: ElementId,
    icon: &'static str,
    on: bool,
    tone: Option<Hsla>,
    tip: SharedString,
    pressed: Pressed,
}

impl RenderOnce for Tick {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let handle = focus::handle(self.id.clone(), window, cx);
        let theme = cx.global::<Theme>();
        let c = &theme.colours;
        let size = theme.strip();
        let pressed = self.pressed.clone();
        let control = div()
            .id(self.id)
            .role(Role::CheckBox)
            .aria_label(self.tip.clone())
            .aria_toggled(if self.on {
                Toggled::True
            } else {
                Toggled::False
            })
            .relative()
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .size(size)
            .rounded(theme::CORNER)
            .cursor_pointer()
            .when(self.on, |d| d.bg(self.tone.unwrap_or(c.accent)))
            .when(!self.on, |d| d.bg(c.ctl).hover(|d| d.bg(c.ctl_hi)))
            .child(icon(
                self.icon,
                theme.icon(),
                if self.on { c.accent_ink } else { c.ink2 },
            ))
            .on_click(move |_, window, cx| pressed(window, cx));
        let control = focus::reachable(control, &handle, theme::CORNER, false, window, theme);
        crate::ui::tip::tipped(control, self.tip)
    }
}

impl Workshop {
    fn pressed(
        &self,
        cx: &Context<Self>,
        run: impl Fn(&mut Self, &mut Context<Self>) + 'static,
    ) -> Pressed {
        let me = cx.entity().downgrade();
        Rc::new(move |_, cx| {
            let _ = me.update(cx, |workshop, cx| run(workshop, cx));
        })
    }

    fn facet(&self, theme: &Theme, facet: &Facet, dlc: bool, cx: &Context<Self>) -> AnyElement {
        let c = &theme.colours;
        let name = put(&facet.label);
        let tick = |include: bool| {
            let id = facet.id.clone();
            let pressed = self.pressed(cx, move |w, cx| match dlc {
                true => w.toggle_dlc(id.clone(), include, cx),
                false => w.toggle_tag(id.clone(), include, cx),
            });
            let key = format!("{}-{}", if include { "keep" } else { "hide" }, facet.id);
            Tick {
                id: ElementId::Name(key.into()),
                icon: if include { "plus" } else { "minus" },
                on: if include {
                    facet.include
                } else {
                    facet.exclude
                },
                tone: (!include).then_some(c.err),
                tip: match include {
                    true => put(&say!("Only show {}", name.clone())),
                    false => put(&say!("Hide {}", name.clone())),
                }
                .into(),
                pressed,
            }
        };
        div()
            .flex()
            .items_center()
            .gap(theme::GAP_SMALL)
            .child(watched(
                div()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_ellipsis()
                    .sized_text(theme, theme.body())
                    .text_color(c.ink)
                    .when(facet.exclude, |d| d.opacity(0.5).line_through()),
                name.clone().into(),
                name.clone(),
                true,
            ))
            .when_some(facet.count.as_ref(), |d, count| {
                d.child(label(theme, theme.small(), c.ink3, put(count)))
            })
            .child(tick(true))
            .child(tick(false))
            .into_any_element()
    }

    pub(super) fn side(
        &self,
        theme: &Theme,
        width: Option<Pixels>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let c = &theme.colours;
        let facets = self.sidebar.clone();
        let switch = |id: &'static str,
                      on: bool,
                      title: String,
                      set: fn(&mut crate::workspace::browse::Picked, bool)| {
            Toggle::new(id, on, title)
                .silent()
                .on_flip(crate::ui::act(cx, move |w: &mut Self, on: bool, _, cx| {
                    w.ask(|asked| set(&mut asked.picked, on), cx)
                }))
        };
        let hide_words = put(&say!("Hide mods that need an expansion I don't own"));
        let content = div()
            .flex()
            .flex_col()
            .gap(theme::GAP)
            .p(theme::GAP_CARD)
            .pr(theme::GAP_CARD + crate::ui::page::room(theme))
            .child(caps(theme, &put(&say!("Categories"))))
            .children(facets.types.iter().map(|f| self.facet(theme, f, false, cx)))
            .children(facets.versions.iter().map(|f| self.facet(theme, f, false, cx)))
            .when(!facets.dlc.is_empty(), |d| {
                d.child(div().pt(theme::GAP).child(caps(theme, &put(&say!("Required DLC")))))
            })
            .children(facets.dlc.iter().map(|f| self.facet(theme, f, true, cx)))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(theme::GAP)
                    .child(switch("hide-unowned", facets.hide_unowned, hide_words.clone(), |p, on| {
                        p.hide_unowned = on
                    }))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .sized_text(theme, theme.small())
                            .text_color(c.ink2)
                            .child(hide_words),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(theme::GAP)
                    .child(switch(
                        "show-incompatible",
                        facets.show_incompatible,
                        put(&say!("Show incompatible items")),
                        |p, on| p.show_incompatible = on,
                    ))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w_0()
                            .gap(theme::GAP_HAIR)
                            .child(
                                div()
                                    .sized_text(theme, theme.small())
                                    .text_color(c.ink2)
                                    .child(put(&say!("Show incompatible items"))),
                            )
                            .child(
                                div()
                                    .sized_text(theme, theme.caption())
                                    .text_color(c.ink3)
                                    .child(put(&say!(
                                        "Items Steam has banned or removed are left out unless this is on."
                                    ))),
                            ),
                    ),
            );
        div()
            .relative()
            .flex_none()
            .h_full()
            .when_some(width, |d, width| d.w(width))
            .when(width.is_none(), |d| d.flex_1())
            .rounded(theme::CORNER_LARGE)
            .bg(c.panel)
            .child(crate::ui::page::page(
                "workshop-filters",
                &self.side_paging,
                theme,
                cx.entity_id(),
                content,
            ))
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .rounded(theme::CORNER_LARGE)
                    .border_1()
                    .border_color(c.line),
            )
            .into_any_element()
    }
}

use super::Settings;
use super::line::{Row, group, named, note};
use crate::ui::field::Field;
use crate::ui::page::page;
use crate::ui::rows::{Scroll, bar, rows};
use crate::ui::theme::{self, Theme};
use crate::ui::widgets::{Btn, TextSized, label};
use crate::words::put;
use gpui::{
    AnyElement, Context, InteractiveElement, IntoElement, ParentElement, Pixels, Render, Role,
    SharedString, StatefulInteractiveElement, Styled, Window, anchored, deferred, div,
    prelude::FluentBuilder,
};
use rimstack_core::say;
use std::rc::Rc;

impl Settings {
    pub(super) fn parts(&self) -> [SharedString; 5] {
        [
            put(&say!("Game and folders")).into(),
            put(&say!("Appearance")).into(),
            put(&say!("Sorting")).into(),
            put(&say!("Steam Web API key")).into(),
            "RimStack".into(),
        ]
    }

    fn nav_width(theme: &Theme) -> Pixels {
        theme.sized(210.)
    }

    fn page_width(theme: &Theme, window: &Window) -> Pixels {
        let gutter = theme::GAP_PAGE + theme::GAP_SMALL;
        let room = window.viewport_size().width - Self::nav_width(theme);
        theme.sized(860.).min(room - gutter * 2.)
    }

    pub(super) fn held_room(&self, theme: &Theme, window: &Window) -> Pixels {
        Self::page_width(theme, window) - named(theme) - theme::GAP
    }

    /// The room a `Choice` has: the held room less the way back to the default, kept whether or
    /// not the line is at its default so that a pick never moves the choice.
    pub(super) fn choice_room(theme: &Theme, window: &Window) -> Pixels {
        Self::page_width(theme, window)
            - named(theme)
            - theme::GAP * 2.
            - crate::ui::widgets::btn_width(window, theme, "", true, false)
    }

    pub(super) fn lines(&self, window: &Window, cx: &mut Context<Self>) -> Vec<Row> {
        let theme = cx.global::<Theme>().clone();
        let mut rows = Vec::new();
        self.game_part(&theme, window, cx, &mut rows);
        self.look_part(&theme, window, cx, &mut rows);
        self.sort_part(&theme, window, cx, &mut rows);
        self.key_part(&theme, window, cx, &mut rows);
        self.rimstack_part(&theme, window, cx, &mut rows);
        rows
    }

    pub(super) fn keep_look(&mut self, key: &str, value: &str, cx: &mut Context<Self>) {
        crate::ui::keep_appearance(key, value, cx);
        self.prefs = crate::ui::app(cx).prefs();
        cx.notify();
    }

    fn nav(&self, theme: &Theme, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let c = &theme.colours;
        let finding = !self.finding.trim().is_empty();
        let field = Field {
            id: "settings-find",
            text: &self.fields.find,
            icon: "",
            placeholder: put(&say!("Find a setting")),
            found: None,
            keys: None,
        };
        let reached_at = if finding { 0 } else { self.part };
        let parts = self.parts().into_iter().enumerate().map(|(at, name)| {
            let on = at == self.part && !finding;
            let part = div()
                .id(("settings-part", at))
                .role(Role::Tab)
                .aria_label(name.clone())
                .flex()
                .items_center()
                .h(theme.sized(28.))
                .px(theme::GAP_MEDIUM)
                .rounded(theme::CORNER)
                .when(on, |d| d.bg(c.chosen))
                .when(!on, |d| d.hover(|s| s.bg(theme.colours.chip)))
                .cursor_pointer()
                .child(label(
                    theme,
                    theme.body(),
                    if on { c.on_selected } else { c.ink },
                    name,
                ))
                .on_click(cx.listener(move |s, _, _, cx| s.pick_part(at, cx)));
            match at == reached_at {
                true => crate::ui::focus::reachable(
                    part.on_key_down(cx.listener(move |s, event: &gpui::KeyDownEvent, _, cx| {
                        let keys = &event.keystroke;
                        if keys.modifiers.modified() {
                            return;
                        }
                        match keys.key.as_str() {
                            "up" if at > 0 => s.pick_part(at - 1, cx),
                            "down" => s.pick_part(at + 1, cx),
                            _ => return,
                        }
                        cx.stop_propagation();
                    })),
                    &self.part_keys,
                    theme::CORNER,
                    on,
                    window,
                    theme,
                ),
                false => part,
            }
        });
        div()
            .flex_none()
            .w(Self::nav_width(theme))
            .h_full()
            .bg(c.panel)
            .border_r_1()
            .border_color(c.edge)
            .px(theme::GAP)
            .pt(theme::GAP_MEDIUM)
            .child(field.render(theme, window, cx))
            .child(div().h(theme::GAP_SMALL))
            .child(div().flex().flex_col().gap(theme::GAP_HAIR).children(parts))
            .into_any_element()
    }

    pub(super) fn font_picker(
        &self,
        shown: String,
        theme: &Theme,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let c = &theme.colours;
        let button =
            Btn::new("font-menu", shown)
                .menu()
                .on_click(cx.listener(|s, _, window, cx| {
                    s.fonts_open = !s.fonts_open;
                    if s.fonts_open {
                        let names = window.text_system().all_font_names();
                        let fonts = crate::workspace::font_choices(names);
                        s.fonts = Rc::new(fonts.into_iter().map(SharedString::from).collect());
                        s.fields
                            .fonts
                            .update(cx, |field, cx| field.set_text("", cx));
                        *s.font_scroll.borrow_mut() = Scroll::default();
                        let focus = gpui::Focusable::focus_handle(s.fields.fonts.read(cx), cx);
                        window.focus(&focus, cx);
                    }
                    cx.notify();
                }));
        let picker = self.fonts_open.then(|| {
            let wanted = self.fields.fonts.read(cx).text().to_lowercase();
            let system = put(&say!("System font"));
            let shown: Rc<Vec<SharedString>> = Rc::new(
                crate::workspace::fonts_matching(&self.fonts, &wanted, &system)
                    .into_iter()
                    .map(|at| self.fonts[at].clone())
                    .collect(),
            );
            let height = theme.sized(30.);
            let tops: Rc<[Pixels]> = (0..=shown.len())
                .map(|at| height * at as f32)
                .collect::<Vec<_>>()
                .into();
            let me = cx.entity().downgrade();
            let (system_family, ink, chosen, on_selected) =
                (theme.family.clone(), c.ink, c.chosen, c.on_selected);
            let size = theme.title();
            let line = theme.line(size);
            let families = rows(
                "font-rows",
                self.font_scroll.clone(),
                tops,
                theme.sized(40.),
                cx.entity_id(),
                move |range, _, _, _| {
                    range
                        .map(|at| {
                            let name = shown[at].clone();
                            let family = if name.is_empty() {
                                system_family.clone()
                            } else {
                                name.clone()
                            };
                            let said = if name.is_empty() {
                                system.clone().into()
                            } else {
                                name.clone()
                            };
                            let me = me.clone();
                            div()
                                .id(("font", at))
                                .flex()
                                .items_center()
                                .h(height)
                                .px(theme::GAP)
                                .rounded(theme::CORNER)
                                .text_color(ink)
                                .hover(move |s| s.bg(chosen).text_color(on_selected))
                                .child(
                                    div()
                                        .overflow_hidden()
                                        .whitespace_nowrap()
                                        .text_ellipsis()
                                        .font_family(family)
                                        .text_size(size)
                                        .line_height(line)
                                        .child(said),
                                )
                                .on_click(move |_, _, cx| {
                                    let name = name.clone();
                                    let _ = me.update(cx, |s, cx| s.use_font(&name, cx));
                                })
                                .into_any_element()
                        })
                        .collect()
                },
            );
            let filter = Field {
                id: "font-filter",
                text: &self.fields.fonts,
                icon: "",
                placeholder: put(&say!("Find a font")),
                found: None,
                keys: None,
            };
            div()
                .id("font-picker")
                .occlude()
                .flex()
                .flex_col()
                .gap(theme::GAP_SMALL)
                .w(theme.sized(320.))
                .h(theme.sized(380.))
                .mt(theme::GAP_TINY)
                .p(theme::GAP)
                .rounded(theme::CORNER_LARGE)
                .bg(c.bar)
                .border_1()
                .border_color(c.line)
                .on_mouse_down_out(cx.listener(|s, _, _, cx| {
                    s.fonts_open = false;
                    cx.notify();
                }))
                .on_key_down(cx.listener(|s, event: &gpui::KeyDownEvent, _, cx| {
                    if event.keystroke.key == "escape" {
                        s.fonts_open = false;
                        cx.notify();
                    }
                }))
                .child(filter.render(theme, window, cx))
                .child(
                    div()
                        .relative()
                        .flex_1()
                        .min_h_0()
                        .child(families)
                        .child(bar(theme, self.font_scroll.clone(), cx.entity_id())),
                )
        });
        div()
            .flex()
            .flex_col()
            .child(button)
            .when_some(picker, |d, picker| {
                d.child(deferred(anchored().child(picker)).with_priority(1))
            })
            .into_any_element()
    }
}

impl Render for Settings {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.global::<Theme>().clone();
        let c = theme.colours;
        let finding = self.finding.trim().to_string();
        let lines = self.lines(window, cx);
        let marked = lines
            .iter()
            .filter(|row| matches!(row, Row::Line(line) if line.changed && self.shows(line)))
            .count();
        let mut shown: Vec<AnyElement> = Vec::new();
        for row in lines {
            match row {
                Row::Group(part, title) if finding.is_empty() && part == self.part => {
                    shown.push(group(&theme, title))
                }
                Row::Note(part, text) if finding.is_empty() && part == self.part => {
                    shown.push(note(&theme, text))
                }
                Row::Under(part, element) if !finding.is_empty() || part == self.part => {
                    shown.push(element)
                }
                Row::Line(line) if self.shows(&line) => shown.push(self.line(line, &theme, cx)),
                _ => {}
            }
        }
        let heading = match finding.is_empty() {
            true => self.parts()[self.part].to_string(),
            false => put(&say!("Settings matching “{}”", finding.clone())),
        };
        let reset = Btn::new(
            "reset-page",
            format!("{}  {marked}", put(&say!("Reset this page"))),
        )
        .icon("undo-2")
        .tip(put(&say!(
            "Every marked setting on this page goes back to its default"
        )))
        .enabled(marked > 0)
        .on_click(cx.listener(|s, _, window, cx| s.reset_page(window, cx)));
        let width = Self::page_width(&theme, window);
        let content = div()
            .flex()
            .flex_col()
            .w(width)
            .mx_auto()
            .pt(theme::GAP_CARD)
            .pb(theme::GAP_PAGE + theme::GAP_CARD * 2.)
            .gap(theme::GAP_SMALL)
            .child(
                div()
                    .flex()
                    .items_center()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .sized_text(&theme, theme.heading())
                            .font_weight(theme.semibold)
                            .text_color(c.ink)
                            .child(heading),
                    )
                    .child(div().when(marked == 0, |d| d.opacity(0.)).child(reset)),
            )
            .when_some(self.problem.clone(), |d, problem| {
                d.child(
                    div()
                        .sized_text(&theme, theme.body())
                        .text_color(c.err)
                        .child(put(&say!("Not kept: {}", problem.inside()))),
                )
            })
            .children(shown);
        div()
            .flex()
            .size_full()
            .child(self.nav(&theme, window, cx))
            .child(div().flex_1().min_w_0().h_full().child(page(
                "settings-page",
                &self.paging,
                &theme,
                cx.entity_id(),
                content,
            )))
    }
}

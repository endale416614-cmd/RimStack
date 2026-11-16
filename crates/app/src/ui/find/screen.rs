use super::Find;
use super::layout::Layout;
use crate::ui::button::{Btn, Chip};
use crate::ui::field::Field;
use crate::ui::menu::{self, Entry};
use crate::ui::rows::{bar, rows};
use crate::ui::theme::{self, Theme};
use crate::ui::widgets::{TextSized, elide_start_in, icon, label, measure, watched, with_alpha};
use crate::words::put;
use crate::workspace::find::{RowRef, Shown};
use gpui::{
    AnyElement, Context, FontWeight, HighlightStyle, InteractiveElement, IntoElement, KeyDownEvent,
    ParentElement, Pixels, Render, Role, StatefulInteractiveElement, Styled, StyledText, Window,
    div, prelude::FluentBuilder, px, relative,
};
use rimstack_core::{say, say_n};
use std::rc::Rc;

impl Find {
    fn search_row(
        &self,
        theme: &Theme,
        layout: Layout,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let typed = !self.search.read(cx).text().trim().is_empty();
        let field = Field {
            id: "find-text",
            text: &self.search,
            icon: "search",
            placeholder: put(&say!("Search the text of mods' files")),
            found: None,
            keys: Some(put(&say!("Ctrl + Shift + F"))),
        };
        let me = cx.entity().downgrade();
        let recent = self.recent.clone();
        div()
            .flex()
            .items_center()
            .gap(theme::GAP)
            .px(theme::GAP_LARGE)
            .when(!layout.wide, |d| {
                d.child(
                    Btn::new("find-filters", put(&say!("Filters")))
                        .icon("funnel")
                        .primary_if(self.filters)
                        .on_click(cx.listener(|find, _, _, cx| {
                            find.filters = !find.filters;
                            cx.notify();
                        })),
                )
            })
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .child(field.render(theme, window, cx)),
            )
            .when(!self.running, |d| {
                d.child(
                    Btn::new("find-search", put(&say!("Search")))
                        .icon("search")
                        .primary()
                        .enabled(typed)
                        .on_click(cx.listener(|find, _, _, cx| find.begin(false, cx))),
                )
            })
            .when(self.running, |d| {
                d.child(
                    Btn::new("find-stop", put(&say!("Stop")))
                        .icon("pause")
                        .on_click(cx.listener(|find, _, _, cx| find.stop(cx))),
                )
            })
            .when(!recent.is_empty(), |d| {
                d.child(
                    Btn::new("find-recent", put(&say!("Recent")))
                        .icon("history")
                        .tip(put(&say!("The searches made before")))
                        .opens(move |below, window, cx| {
                            let entries = recent
                                .iter()
                                .enumerate()
                                .map(|(at, one)| {
                                    let me = me.clone();
                                    Entry::item(one.text.clone(), move |_, cx| {
                                        let _ = me.update(cx, |find, cx| find.use_recent(at, cx));
                                    })
                                    .into()
                                })
                                .collect();
                            menu::open(below, entries, window, cx);
                        }),
                )
            })
            .into_any_element()
    }

    fn status(&self, theme: &Theme, layout: Layout, cx: &mut Context<Self>) -> Option<AnyElement> {
        let c = &theme.colours;
        if !self.searched() {
            return None;
        }
        let said = |s: &Option<rimstack_core::base::sentence::Sentence>| {
            s.as_ref().map(put).unwrap_or_default()
        };
        let text = said(&self.working) + &said(&self.said);
        let _ = (layout, cx);
        Some(
            div()
                .flex()
                .items_center()
                .gap(theme::GAP)
                .px(theme::GAP_LARGE)
                .child(
                    watched(
                        div()
                            .flex_1()
                            .min_w_0()
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_ellipsis()
                            .sized_text(theme, theme.small())
                            .text_color(c.ink2),
                        text.clone().into(),
                        text,
                        true,
                    ),
                )
                .when(self.capped, |d| {
                    d.child(
                        Chip::new(put(&say!("Results capped")))
                            .tone(c.warn)
                            .tip(put(&say!(
                                "Only the first matches are listed; narrow the search to see the rest"
                            ))),
                    )
                })
                .into_any_element(),
        )
    }

    fn progress_bar(&self, theme: &Theme) -> Option<AnyElement> {
        let c = &theme.colours;
        self.running.then(|| {
            let part = self.done as f32 / self.total.max(1) as f32;
            div()
                .mx(theme::GAP_LARGE)
                .h(theme::GAP_TINY)
                .rounded(theme::GAP_HAIR)
                .bg(c.ctl)
                .child(
                    div()
                        .h_full()
                        .w(relative(part))
                        .rounded(theme::GAP_HAIR)
                        .bg(c.accent),
                )
                .into_any_element()
        })
    }

    fn filter_row(
        &self,
        theme: &Theme,
        layout: Layout,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let resulting = !self.rows.is_empty() || !self.filter.read(cx).text().is_empty();
        if !resulting {
            return None;
        }
        let room = layout.middle - theme::GAP_LARGE * 2.;
        let field = Field {
            id: "find-filter",
            text: &self.filter,
            icon: "funnel",
            placeholder: put(&say!("Filter these results")),
            found: None,
            keys: None,
        };
        let show = put(&say_n!(
            self.count,
            "Show this mod in the list",
            "Show these {} mods in the list"
        ));
        Some(
            div()
                .flex()
                .flex_wrap()
                .gap(theme::GAP)
                .px(theme::GAP_LARGE)
                .child(
                    div()
                        .w(room.min(theme.sized(260.)))
                        .child(field.render(theme, window, cx)),
                )
                .when(self.count > 0, |d| {
                    d.child(
                        Btn::new("find-show-in-mods", show)
                            .icon("list")
                            .tip(put(&say!(
                                "Goes to Mods and keeps only the mods with matches (in:search)"
                            )))
                            .on_click(cx.listener(|find, _, _, cx| find.show_in_mods(cx))),
                    )
                })
                .into_any_element(),
        )
    }

    fn key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let secondary = event.keystroke.modifiers.secondary(cx);
        match event.keystroke.key.as_str() {
            "up" => self.step(-1, cx),
            "down" => self.step(1, cx),
            "space" => self.fold(0, cx),
            "left" => self.fold(-1, cx),
            "right" => self.fold(1, cx),
            "enter" => self.open_file(cx),
            "c" if secondary => self.copy_line(cx),
            _ => return,
        }
        let _ = window;
        cx.stop_propagation();
    }

    fn current_ref(&self) -> Option<RowRef> {
        self.current.and_then(|at| self.rows.get(at).copied())
    }

    pub(super) fn open_file(&self, cx: &mut Context<Self>) {
        if let Some(file) = self
            .current_ref()
            .and_then(|at| crate::ui::app(cx).find_file(at))
        {
            cx.open_with_system(&file);
        }
    }

    pub(super) fn open_folder(&self, cx: &mut Context<Self>) {
        if let Some(folder) = self
            .current_ref()
            .and_then(|at| crate::ui::app(cx).find_folder(at))
        {
            cx.open_with_system(&folder);
        }
    }

    pub(super) fn copy_path(&self, cx: &mut Context<Self>) {
        if let Some(file) = self
            .current_ref()
            .and_then(|at| crate::ui::app(cx).find_file(at))
        {
            let text = file.to_string_lossy().into_owned();
            cx.write_to_clipboard(gpui::ClipboardItem::new_string(text));
        }
    }

    pub(super) fn copy_line(&self, cx: &mut Context<Self>) {
        if let Some(line) = self
            .current_ref()
            .and_then(|at| crate::ui::app(cx).find_line(at))
        {
            cx.write_to_clipboard(gpui::ClipboardItem::new_string(line));
        }
    }

    fn results(
        &mut self,
        theme: &Theme,
        layout: Layout,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let tops = self.tops(theme, cx);
        let shown_rows: Rc<[RowRef]> = self.rows.clone().into();
        let (current, me, owned) = (self.current, cx.entity().downgrade(), theme.clone());
        let row_width = layout.middle - theme.sized(12.);
        let focus = self.focus.clone();
        let list = rows(
            "find-rows",
            self.scroll.clone(),
            tops,
            theme.sized(40.),
            cx.entity_id(),
            move |range, _, window, cx| {
                let theme = &owned;
                let app = crate::ui::app(cx);
                let shown: Vec<(usize, Shown<'static>)> = app.find_results(|results| {
                    range
                        .filter_map(|at| {
                            let shown = results.shown(*shown_rows.get(at)?)?;
                            Some((at, shown.into_owned()))
                        })
                        .collect()
                });
                shown
                    .into_iter()
                    .map(|(at, shown)| {
                        let row = Row {
                            at,
                            current: current == Some(at),
                            width: row_width,
                            digits: layout.digits,
                        };
                        row.draw(shown, theme, &me, &focus, window)
                    })
                    .collect()
            },
        );
        let c = &theme.colours;
        let focused = self.focus.is_focused(window);
        let searched = self.searched();
        let none = searched && !self.running && self.rows.is_empty();
        let empty_width = (layout.middle - theme::GAP_PAGE * 2.).min(theme.sized(420.));
        div()
            .id("find-results")
            .track_focus(&self.focus)
            .role(Role::List)
            .aria_label(put(&say!("Search")))
            .relative()
            .flex_1()
            .min_h_0()
            .on_key_down(cx.listener(Self::key))
            .child(list)
            .child(bar(theme, self.scroll.clone(), cx.entity_id()))
            .when(focused, |d| {
                d.child(
                    div()
                        .absolute()
                        .inset(theme::GAP_LINE)
                        .rounded(theme::CORNER)
                        .border_width(px(2.))
                        .border_color(c.focus),
                )
            })
            .when(!searched, |d| {
                d.child(empty_state(
                    theme,
                    empty_width,
                    put(&say!("Search the files of your mods")),
                    put(&say!("Finds a word or a pattern in the XML of the mods, grouped by mod and file. Ctrl + Shift + F comes here from anywhere.")),
                ))
            })
            .when(none, |d| {
                d.child(empty_state(
                    theme,
                    empty_width,
                    put(&say!("No matches")),
                    put(&say!("Nothing in these mods' files matches. Try other words, or look in more places.")),
                ))
            })
            .into_any_element()
    }
}

fn empty_state(theme: &Theme, width: Pixels, title: String, hint: String) -> AnyElement {
    div()
        .absolute()
        .inset_0()
        .flex()
        .items_center()
        .justify_center()
        .child(crate::ui::widgets::empty_state(
            theme, width, "search", title, hint,
        ))
        .into_any_element()
}

struct Row {
    at: usize,
    current: bool,
    width: Pixels,
    digits: Pixels,
}

impl Row {
    fn draw(
        &self,
        shown: Shown<'_>,
        theme: &Theme,
        me: &gpui::WeakEntity<Find>,
        focus: &gpui::FocusHandle,
        window: &Window,
    ) -> AnyElement {
        let c = &theme.colours;
        let at = self.at;
        let (toggled, chosen, focus) = (me.clone(), me.clone(), focus.clone());
        let choose = move |_: &gpui::ClickEvent, window: &mut Window, cx: &mut gpui::App| {
            window.focus(&focus, cx);
            let _ = chosen.update(cx, |find, cx| find.select(Some(at), cx));
        };
        let highlight = |d: gpui::Stateful<gpui::Div>| {
            d.when(self.current, |d| {
                d.child(
                    div()
                        .absolute()
                        .top_0()
                        .bottom_0()
                        .left(theme::GAP_SMALL)
                        .right(theme::GAP_SMALL)
                        .rounded(theme::CORNER)
                        .bg(with_alpha(c.accent, 0.22)),
                )
            })
        };
        let chevron = |collapsed: bool| {
            icon(
                if collapsed {
                    "chevron-right"
                } else {
                    "chevron-down"
                },
                theme.icon(),
                c.ink2,
            )
        };
        match shown {
            Shown::Mod {
                name,
                matches,
                files,
                collapsed,
            } => {
                let name = put(&name);
                let counted = format!(
                    "{}, {}",
                    put(&say_n!(matches, "{} match", "{} matches")),
                    put(&say_n!(files, "{} file", "{} files"))
                );
                div()
                    .id(("find-mod", at))
                    .role(Role::ListItem)
                    .aria_label(name.clone())
                    .relative()
                    .w(self.width)
                    .h_full()
                    .cursor_pointer()
                    .child(
                        div()
                            .absolute()
                            .top(theme::GAP_TINY)
                            .bottom_0()
                            .left(theme::GAP_SMALL)
                            .right(theme::GAP_SMALL)
                            .rounded(theme::CORNER)
                            .bg(c.card),
                    )
                    .child(
                        div()
                            .absolute()
                            .top(theme::GAP_TINY)
                            .bottom_0()
                            .left(theme::GAP_LARGE)
                            .right(theme::GAP_LARGE)
                            .flex()
                            .items_center()
                            .gap(theme::GAP_SMALL)
                            .child(chevron(collapsed))
                            .child(watched(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .overflow_hidden()
                                    .whitespace_nowrap()
                                    .text_ellipsis()
                                    .sized_text(theme, theme.body())
                                    .font_weight(theme.semibold)
                                    .text_color(c.ink),
                                name.clone().into(),
                                name,
                                true,
                            ))
                            .child(label(theme, theme.small(), c.ink2, counted)),
                    )
                    .on_click(move |_, _, cx| {
                        let _ = toggled.update(cx, |find, cx| find.toggle(at, cx));
                    })
                    .into_any_element()
            }
            Shown::File {
                path,
                matches,
                collapsed,
            } => {
                let counted = put(&say_n!(matches, "{} match", "{} matches"));
                let fold = theme.icon() + theme::GAP_SMALL;
                let room = self.width
                    - theme::GAP_WIDE
                    - theme::GAP_LARGE
                    - fold
                    - theme::GAP_SMALL * 2.
                    - measure(window, theme, &counted, theme.small(), FontWeight::NORMAL);
                let shown_path = elide_start_in(
                    window,
                    theme,
                    theme.mono.clone(),
                    &path,
                    theme.small(),
                    room,
                );
                let row = div()
                    .id(("find-file", at))
                    .role(Role::ListItem)
                    .aria_label(path.to_string())
                    .relative()
                    .w(self.width)
                    .h_full()
                    .cursor_pointer();
                highlight(row)
                    .child(
                        div()
                            .absolute()
                            .inset_0()
                            .pl(theme::GAP_WIDE)
                            .pr(theme::GAP_LARGE)
                            .flex()
                            .items_center()
                            .gap(theme::GAP_SMALL)
                            .child(
                                div()
                                    .id(("find-fold", at))
                                    .flex()
                                    .flex_none()
                                    .items_center()
                                    .justify_center()
                                    .w(fold)
                                    .h_full()
                                    .child(chevron(collapsed))
                                    .on_click(move |_, _, cx| {
                                        cx.stop_propagation();
                                        let _ = toggled.update(cx, |find, cx| find.toggle(at, cx));
                                    }),
                            )
                            .child(watched(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .overflow_hidden()
                                    .whitespace_nowrap()
                                    .sized_text(theme, theme.small())
                                    .font_family(theme.mono.clone())
                                    .text_color(c.ink),
                                path.to_string().into(),
                                shown_path,
                                true,
                            ))
                            .child(label(theme, theme.small(), c.ink3, counted)),
                    )
                    .on_click(choose)
                    .into_any_element()
            }
            Shown::Line {
                number,
                matched,
                gap,
                parts,
            } => {
                let indent = theme::GAP_PAGE + theme.icon() + theme::GAP_SMALL;
                let code_x = indent + self.digits + theme::GAP;
                let (text, marks) = marked(&parts, theme);
                let row = div()
                    .id(("find-line", at))
                    .role(Role::ListItem)
                    .aria_label(number.to_string())
                    .relative()
                    .w(self.width)
                    .h_full()
                    .cursor_pointer();
                highlight(row)
                    .when(!gap, |d| {
                        d.child(
                            div()
                                .absolute()
                                .top_0()
                                .left(indent)
                                .w(self.digits)
                                .flex()
                                .justify_end()
                                .child(
                                    div()
                                        .sized_text(theme, theme.small())
                                        .font_family(theme.mono.clone())
                                        .text_color(c.ink3)
                                        .child(number.to_string()),
                                ),
                        )
                    })
                    .child(
                        div()
                            .absolute()
                            .top_0()
                            .left(code_x)
                            .right(theme::GAP_LARGE)
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .sized_text(theme, theme.small())
                            .font_family(theme.mono.clone())
                            .text_color(if gap { c.ink3 } else { c.ink })
                            .when(!gap, |d| d.opacity(if matched { 1. } else { 0.6 }))
                            .child(match gap {
                                true => StyledText::new("…"),
                                false => StyledText::new(text).with_highlights(marks),
                            }),
                    )
                    .on_click(choose)
                    .into_any_element()
            }
        }
    }
}

fn marked(
    parts: &[(std::borrow::Cow<'_, str>, bool)],
    theme: &Theme,
) -> (String, Vec<(std::ops::Range<usize>, HighlightStyle)>) {
    let mark = HighlightStyle {
        background_color: Some(with_alpha(theme.colours.accent, 0.35)),
        font_weight: Some(FontWeight::BOLD),
        ..HighlightStyle::default()
    };
    let mut text = String::new();
    let mut marks = Vec::new();
    for (part, is_mark) in parts {
        let start = text.len();
        text.push_str(&part.replace('\t', "    "));
        if *is_mark && text.len() > start {
            marks.push((start..text.len(), mark));
        }
    }
    (text, marks)
}

impl Render for Find {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.global::<Theme>().clone();
        let c = theme.colours;
        let layout = self.layout(&theme, window);
        let side = layout
            .side_shown
            .then(|| self.side(&theme, layout, window, cx));
        let preview = layout
            .preview_shown
            .then(|| self.preview_pane(&theme, layout, window, cx));
        let middle = layout.results_shown.then(|| {
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_w_0()
                .gap(theme::GAP_SMALL)
                .children(self.status(&theme, layout, cx))
                .children(self.progress_bar(&theme))
                .children(self.filter_row(&theme, layout, window, cx))
                .child(self.results(&theme, layout, window, cx))
        });
        div()
            .size_full()
            .flex()
            .flex_col()
            .gap(theme::GAP_SMALL)
            .child(
                div()
                    .h(theme.header())
                    .pl(theme::GAP_CARD)
                    .flex()
                    .items_center()
                    .child(
                        label(&theme, theme.heading(), c.ink, put(&say!("Search")))
                            .font_weight(theme.semibold),
                    ),
            )
            .child(self.search_row(&theme, layout, window, cx))
            .when(!self.problem.is_empty(), |d| {
                d.child(
                    div()
                        .px(theme::GAP_LARGE)
                        .sized_text(&theme, theme.small())
                        .text_color(c.err)
                        .child(self.problem.clone()),
                )
            })
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .pl(theme::GAP_LARGE)
                    .gap(theme::GAP_LARGE)
                    .children(side)
                    .children(middle)
                    .children(preview),
            )
    }
}

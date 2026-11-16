use super::details::fill_text;
use super::{Workshop, related_said};
use crate::ui::button::{Btn, Chip};
use crate::ui::field::Field;
use crate::ui::focus;
use crate::ui::menu::{self, Entry};
use crate::ui::rows::{bar, rows};
use crate::ui::theme::{self, Theme};
use crate::ui::widgets::{TextSized, empty_state, icon, label, notice, with_alpha};
use crate::words::put;
use crate::workspace::browse::Row;
use gpui::{
    AnyElement, App, Context, ElementId, InteractiveElement, IntoElement, ObjectFit, ParentElement,
    Pixels, Render, RenderOnce, Role, StatefulInteractiveElement, Styled, StyledImage, WeakEntity,
    Window, div, img, prelude::FluentBuilder,
};
use rimstack_core::say;
use std::path::PathBuf;
use std::rc::Rc;

#[derive(Clone, Copy)]
pub(super) struct Layout {
    pub wide: bool,
    pub side_shown: bool,
    pub side_width: Pixels,
    pub column_width: Pixels,
    pub docked: bool,
    pub covered: bool,
    pub card_least: Pixels,
    pub results_width: Pixels,
}

impl Workshop {
    pub(super) fn layout(&self, theme: &Theme, window: &Window) -> Layout {
        let width = window.viewport_size().width;
        let wide = width >= theme.sized(1100.);
        let side_shown = wide || self.filters;
        let side_width = if wide {
            theme.sized(250.)
        } else {
            Pixels::ZERO
        };
        let card_least = theme.sized(369.);
        let column_width = theme.sized(360.).min(width * 0.4);
        let docked =
            width - theme::GAP_WIDE * 2. - side_width - column_width - theme::GAP_LARGE * 2.
                >= card_least;
        let covered = !docked && (self.opened() || self.list) && (wide || !self.filters);
        let results_width = width
            - theme::GAP_WIDE * 2.
            - if side_shown && wide {
                side_width + theme::GAP_LARGE
            } else {
                Pixels::ZERO
            }
            - theme::GAP_LARGE
            - if docked { column_width } else { Pixels::ZERO };
        Layout {
            wide,
            side_shown,
            side_width,
            column_width,
            docked,
            covered,
            card_least,
            results_width,
        }
    }

    fn search_row(
        &self,
        theme: &Theme,
        layout: Layout,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let field = Field {
            id: "workshop-search",
            text: &self.field,
            icon: "search",
            placeholder: put(&say!(
                "Search, or w: id  t: tag  d: expansion  m: needs a mod  a: author  or a link"
            )),
            found: None,
            keys: None,
        };
        let field = crate::ui::tip::tipped(
            field.render(theme, window, cx).h(theme.sized(30.)),
            put(&say!("Words match titles, authors and package ids. w:<id> one item, t:<tag>, d:<expansion app id>, m:<id> mods that need that mod, a:<author>. A pasted Workshop or collection link works too.")).into(),
        );
        let count = crate::ui::listed::read(cx).list.count;
        div()
            .flex()
            .items_center()
            .gap(theme::GAP)
            .when(!layout.wide, |d| {
                d.child(
                    Btn::new("workshop-filters", put(&say!("Filters")))
                        .icon("funnel")
                        .primary_if(self.filters)
                        .on_click(cx.listener(|w, _, _, cx| {
                            w.filters = !w.filters;
                            cx.notify();
                        })),
                )
            })
            .child(div().flex_1().min_w_0().child(field))
            .child(
                Btn::new("workshop-search-go", put(&say!("Search")))
                    .primary()
                    .on_click(cx.listener(|w, _, _, cx| {
                        let text = w.field.read(cx).text().to_string();
                        w.search(text, cx);
                    })),
            )
            .child(
                Btn::new("workshop-list", put(&say!("Download list ({})", count)))
                    .icon("list")
                    .primary_if(self.list || self.opened())
                    .tip(put(&say!("Show or hide the download list.")))
                    .on_click(cx.listener(|w, _, _, cx| {
                        let opened = w.opened();
                        w.select(String::new(), cx);
                        w.list = opened || !w.list;
                        cx.notify();
                    })),
            )
            .into_any_element()
    }

    fn banner(&self, theme: &Theme, cx: &mut Context<Self>) -> Option<AnyElement> {
        let (which, id, title) = self.related.as_ref()?;
        let said = put(&related_said(*which, id, title));
        Some(
            notice(theme.colours.focus)
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(theme::GAP)
                        .p(theme::GAP_SMALL)
                        .child(fill_text(theme, theme.body(), theme.colours.ink, said))
                        .child(
                            Btn::new("workshop-back", put(&say!("Back to results")))
                                .on_click(cx.listener(|w, _, _, cx| w.back(cx))),
                        ),
                )
                .into_any_element(),
        )
    }

    fn sort_button(&self, cx: &mut Context<Self>) -> Btn {
        let me = cx.entity().downgrade();
        let sorts: Vec<(&'static str, String, bool)> = self
            .sorts
            .iter()
            .map(|offer| (offer.id, put(&offer.label), offer.chosen))
            .collect();
        Btn::new("workshop-sort", self.sort_label.clone())
            .icon("arrow-up-down")
            .tip(put(&say!("Sort the results")))
            .opens(move |below, window, cx| {
                let entries = sorts
                    .iter()
                    .map(|(id, text, chosen)| {
                        let (me, id) = (me.clone(), *id);
                        let item = Entry::item(text.clone(), move |_, cx| {
                            let _ = me.update(cx, |w, cx| w.choose_sort(id, cx));
                        });
                        match chosen {
                            true => item.icon("check").into(),
                            false => item.into(),
                        }
                    })
                    .collect();
                menu::open(below, entries, window, cx);
            })
    }

    fn chips(&self, theme: &Theme, cx: &mut Context<Self>) -> AnyElement {
        let c = &theme.colours;
        let facets = self.sidebar.clone();
        let tone = |name: &str| match name {
            "err" => Some(c.err),
            "warn" => Some(c.warn),
            "ok" => Some(c.ok),
            "moved" => Some(c.moved),
            "focus" => Some(c.focus),
            _ => None,
        };
        let filters = facets.chips.iter().enumerate().map(|(at, chip)| {
            let id = chip.id.clone();
            let shown = Chip::new(put(&chip.label))
                .icon("x")
                .tip(put(&say!("Remove this filter")))
                .on_click(
                    ("filter-chip", at),
                    cx.listener(move |w, _, _, cx| w.clear_filter(id.clone(), cx)),
                );
            match tone(chip.tone) {
                Some(tone) => shown.tone(tone),
                None => shown,
            }
        });
        div()
            .flex()
            .flex_wrap()
            .gap(theme::GAP_SMALL)
            .children(filters)
            .when(facets.searched, |d| {
                d.child(Chip::new(put(&say!("Clear search"))).icon("x").on_click(
                    "clear-search",
                    cx.listener(|w, _, _, cx| w.search(String::new(), cx)),
                ))
            })
            .when(facets.filtered, |d| {
                d.child(
                    Chip::new(put(&say!("Clear filters")))
                        .icon("x")
                        .on_click("clear-filters", cx.listener(|w, _, _, cx| w.clear_all(cx))),
                )
            })
            .when(self.key_mode, |d| {
                d.child(
                    Chip::new(put(&say!("Items")))
                        .on_accent(!self.collections)
                        .on_click(
                            "show-items",
                            cx.listener(|w, _, _, cx| w.show_collections(false, cx)),
                        ),
                )
                .child(
                    Chip::new(put(&say!("Collections")))
                        .on_accent(self.collections)
                        .on_click(
                            "show-collections",
                            cx.listener(|w, _, _, cx| w.show_collections(true, cx)),
                        ),
                )
            })
            .into_any_element()
    }

    /// A card's height, as the probe card of the grid measured it: no stars.
    fn card_height(theme: &Theme, each: Pixels) -> Pixels {
        let picture = (each - theme::GAP_MEDIUM * 2.) * 9. / 16.;
        theme::GAP_MEDIUM * 2.
            + picture
            + theme.line(theme.body()) * 2.
            + theme.control()
            + theme::GAP_SMALL * 3.
    }

    fn grid(&self, theme: &Theme, layout: Layout, cx: &mut Context<Self>) -> AnyElement {
        let gap = theme::GAP_LARGE;
        let usable = layout.results_width - (theme.sized(9.) + theme::GAP_HAIR * 2.);
        let across = (((usable + gap) / (layout.card_least + gap)).floor() as usize).max(1);
        let each = (usable + gap) / across as f32 - gap;
        let cell = Self::card_height(theme, each) + gap;
        let lines = self.rows.len().div_ceil(across);
        let tops: Rc<[Pixels]> = (0..=lines).map(|at| cell * at as f32).collect();
        let rows_shown: Vec<Row> = self.rows.iter().map(|row| self.row_of(row)).collect();
        let (shown, previews) = (Rc::new(rows_shown), Rc::new(self.previews.clone()));
        let (me, selected) = (cx.entity().downgrade(), self.selected.clone());
        let thumbs = self.thumbs.clone();
        let list = rows(
            "workshop-grid",
            self.scroll.clone(),
            tops,
            theme.sized(40.),
            cx.entity_id(),
            move |range, _, _, _| {
                range
                    .map(|line| {
                        let cards =
                            (line * across..((line + 1) * across).min(shown.len())).map(|at| {
                                let row = &shown[at];
                                Card {
                                    at,
                                    row: row.clone(),
                                    preview: previews.get(&row.id).cloned(),
                                    current: row.id == selected,
                                    width: each,
                                    me: me.clone(),
                                    thumbs: thumbs.clone(),
                                }
                            });
                        div().flex().gap(gap).children(cards).into_any_element()
                    })
                    .collect()
            },
        );
        let (top, seen, total) = {
            let scroll = self.scroll.borrow();
            (scroll.top(), scroll.shown(), scroll.total())
        };
        if seen > Pixels::ZERO && top + seen * 2. >= total && self.more && !self.searching {
            let me = cx.entity().downgrade();
            cx.defer(move |cx| {
                let _ = me.update(cx, |w, cx| w.want_more(cx));
            });
        }
        div()
            .id("workshop-results")
            .role(Role::List)
            .relative()
            .size_full()
            .child(list)
            .child(bar(theme, self.scroll.clone(), cx.entity_id()))
            .into_any_element()
    }

    fn results(&self, theme: &Theme, layout: Layout, cx: &mut Context<Self>) -> AnyElement {
        let c = &theme.colours;
        let found = self.found();
        let empty_width = layout.results_width.min(theme.sized(420.));
        let centred = |child: AnyElement| {
            div()
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .child(child)
        };
        let area = div()
            .relative()
            .flex_1()
            .min_h_0()
            .when(found && self.total > 0, |d| d.child(self.grid(theme, layout, cx)))
            .when(!self.ready, |d| {
                d.child(centred(
                    div()
                        .flex()
                        .flex_col()
                        .items_center()
                        .gap(theme::GAP_LARGE)
                        .w(empty_width)
                        .child(empty_state(
                            theme,
                            empty_width,
                            "download",
                            put(&say!("The Workshop database is not downloaded")),
                            put(&say!("Browsing searches a list of the Workshop's mods that RimStack keeps on this computer. It comes from the community database, downloaded once and refreshed from Settings.")),
                        ))
                        .child(
                            Btn::new("download-community", put(&say!("Download the Workshop database")))
                                .icon("download")
                                .primary()
                                .enabled(!self.community_busy)
                                .on_click(cx.listener(|w, _, _, cx| w.download_community(cx))),
                        )
                        .when(self.community_busy, |d| {
                            d.child(label(theme, theme.small(), c.ink3, put(&say!("Downloading…"))))
                        })
                        .into_any_element(),
                ))
            })
            .when(self.ready && !self.problem.is_empty(), |d| {
                d.child(centred(
                    div()
                        .flex()
                        .flex_col()
                        .items_center()
                        .gap(theme::GAP_LARGE)
                        .w(empty_width)
                        .child(empty_state(
                            theme,
                            empty_width,
                            "circle-alert",
                            put(&say!("The search did not work")),
                            self.problem.clone(),
                        ))
                        .child(Btn::new("workshop-retry", put(&say!("Try again"))).on_click(
                            cx.listener(|w, _, _, cx| {
                                let text = w.field.read(cx).text().to_string();
                                w.search(text, cx);
                            }),
                        ))
                        .into_any_element(),
                ))
            })
            .when(found && !self.searching && self.total == 0, |d| {
                d.child(centred(
                    empty_state(
                        theme,
                        empty_width,
                        "search",
                        put(&say!("No mods match")),
                        put(&say!("Try fewer words or clear some filters.")),
                    )
                    .into_any_element(),
                ))
            });
        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .h_full()
            .gap(theme::GAP)
            .when_some(self.notice.as_ref(), |d, notice| {
                d.child(
                    div()
                        .sized_text(theme, theme.small())
                        .text_color(c.warn)
                        .child(put(notice)),
                )
            })
            .when(found, |d| {
                d.child(
                    div()
                        .flex()
                        .items_center()
                        .gap(theme::GAP)
                        .child(fill_text(
                            theme,
                            theme.small(),
                            c.ink2,
                            put(&self.sidebar.found),
                        ))
                        .when(self.searching, |d| {
                            d.child(label(
                                theme,
                                theme.small(),
                                c.ink3,
                                put(&say!("Searching…")),
                            ))
                        })
                        .child(self.sort_button(cx)),
                )
                .child(self.chips(theme, cx))
            })
            .child(area)
            .into_any_element()
    }

    fn panel(
        &self,
        theme: &Theme,
        layout: Layout,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let c = &theme.colours;
        let width = match layout.docked {
            true => layout.column_width,
            false => layout.results_width + theme::GAP_LARGE,
        };
        let inner = width - theme::GAP_CARD * 2.;
        let content = match self.opened() {
            true => self.details_pane(theme, inner, window, cx),
            false => {
                let size = crate::ui::listed::read(cx)
                    .list
                    .size_said
                    .as_ref()
                    .map(put)
                    .unwrap_or_default();
                div()
                    .flex()
                    .flex_col()
                    .size_full()
                    .gap(theme::GAP)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(theme::GAP)
                            .child(
                                label(theme, theme.small(), c.ink2, put(&say!("Download list")))
                                    .font_weight(theme.semibold),
                            )
                            .child(fill_text(theme, theme.small(), c.ink3, size)),
                    )
                    .child(self.list_view.clone())
                    .into_any_element()
            }
        };
        div()
            .relative()
            .flex_none()
            .h_full()
            .when(layout.docked, |d| d.w(layout.column_width))
            .when(!layout.docked, |d| d.flex_1())
            .p(theme::GAP_CARD)
            .rounded(theme::CORNER_LARGE)
            .bg(c.card)
            .child(content)
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

    fn download_community(&mut self, cx: &mut Context<Self>) {
        if self.community_busy || self.demo {
            return;
        }
        self.community_busy = true;
        let app = crate::ui::app(cx);
        crate::jobs::run(
            "community download",
            cx,
            move || app.download_databases(),
            |w, done, cx| {
                w.community_busy = false;
                if let Err(problem) = done {
                    w.problem = crate::words::said_of(&problem);
                }
                cx.notify();
            },
        );
        cx.notify();
    }
}

#[derive(IntoElement)]
struct Card {
    at: usize,
    row: Row,
    preview: Option<PathBuf>,
    current: bool,
    width: Pixels,
    me: WeakEntity<Workshop>,
    thumbs: crate::ui::thumbs::Shared,
}

impl RenderOnce for Card {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let id = ElementId::Name(format!("card-{}", self.row.id).into());
        let handle = focus::handle(id.clone(), window, cx);
        let theme = cx.global::<Theme>().clone();
        let c = &theme.colours;
        let row = &self.row;
        let inner = self.width - theme::GAP_MEDIUM * 2.;
        let letter: String = row
            .title
            .chars()
            .next()
            .map(|l| l.to_uppercase().collect())
            .unwrap_or_default();
        let mark = |tone, name| {
            div()
                .absolute()
                .top(theme::GAP_SMALL)
                .right(theme::GAP_SMALL)
                .flex()
                .items_center()
                .justify_center()
                .size(theme.control())
                .rounded(theme.control() / 2.)
                .bg(tone)
                .child(icon(name, theme.icon(), c.accent_ink))
        };
        let picture = self.preview.as_ref().and_then(|path| {
            let scale = window.scale_factor();
            let device = |logical: Pixels| (f32::from(logical) * scale).round() as u32;
            let size = (device(inner), device(inner * 9. / 16.));
            let path = path.to_string_lossy();
            crate::ui::thumbs::thumb(&self.thumbs, &path, size, self.me.entity_id(), cx)
        });
        let picture = div()
            .relative()
            .w(inner)
            .h(inner * 9. / 16.)
            .rounded(theme::CORNER)
            .bg(c.ctl)
            .overflow_hidden()
            .flex()
            .items_center()
            .justify_center()
            .child(label(&theme, theme.display(), c.ink3, letter).font_weight(theme.semibold))
            .when_some(picture, |d, picture| {
                d.child(
                    img(picture)
                        .absolute()
                        .inset_0()
                        .size_full()
                        .object_fit(ObjectFit::Cover),
                )
            })
            .when(row.installed, |d| d.child(mark(c.ok, "check")))
            .when(row.on_list && !row.installed, |d| {
                d.child(mark(c.moved, "list"))
            });
        let author = row
            .author
            .as_ref()
            .map(|a| put(&say!("by {}", a.clone())))
            .unwrap_or_default();
        let line = theme.line(theme.body());
        let (chosen, added, row_id) = (self.me.clone(), self.me.clone(), row.id.clone());
        let add_id = row.id.clone();
        let stars = row.stars.clone();
        let card =
            div()
                .id(id)
                .role(Role::Button)
                .aria_label(row.title.clone())
                .relative()
                .flex_none()
                .w(self.width)
                .p(theme::GAP_MEDIUM)
                .rounded(theme::CORNER_LARGE)
                .bg(c.card)
                .cursor_pointer()
                .hover(|d| d.bg(gpui::Hsla::blend(c.card, with_alpha(c.hover, 0.05))))
                .child(
                    div()
                        .absolute()
                        .inset_0()
                        .rounded(theme::CORNER_LARGE)
                        .border_1()
                        .border_color(c.line),
                )
                .when(self.current, |d| {
                    d.child(
                        div()
                            .absolute()
                            .inset_0()
                            .rounded(theme::CORNER_LARGE)
                            .border_2()
                            .border_color(c.focus),
                    )
                })
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(theme::GAP_SMALL)
                        .child(picture)
                        .child(
                            div()
                                .h(line)
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_ellipsis()
                                .sized_text(&theme, theme.body())
                                .font_weight(theme.semibold)
                                .text_color(c.ink)
                                .child(row.title.clone()),
                        )
                        .child(
                            div()
                                .h(line)
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_ellipsis()
                                .sized_text(&theme, theme.small())
                                .text_color(c.ink3)
                                .child(author),
                        )
                        .when(!stars.is_empty(), |d| {
                            d.child(div().flex().gap(theme::GAP_HAIR).children(stars.iter().map(
                                |on| icon("star", theme.icon(), if *on { c.warn } else { c.line }),
                            )))
                        })
                        .child(div().h(theme.control()).flex().items_center().when(
                            row.can_add,
                            |d| {
                                d.child(
                                    Btn::new(("card-add", self.at), put(&say!("Add")))
                                        .icon("plus")
                                        .on_click(move |_, _, cx| {
                                            cx.stop_propagation();
                                            let id = add_id.clone();
                                            let _ = added.update(cx, |w, cx| w.add(id, false, cx));
                                        }),
                                )
                            },
                        )),
                )
                .on_click(move |_, _, cx| {
                    let id = row_id.clone();
                    let _ = chosen.update(cx, |w, cx| w.select(id, cx));
                });
        focus::reachable(card, &handle, theme::CORNER_LARGE, false, window, &theme)
    }
}

impl Render for Workshop {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.global::<Theme>().clone();
        let layout = self.layout(&theme, window);
        let results_shown = (layout.wide || !self.filters) && !layout.covered;
        let panel_shown = layout.docked || layout.covered;
        let side = layout
            .side_shown
            .then(|| self.side(&theme, layout.wide.then_some(layout.side_width), cx));
        let results = results_shown.then(|| self.results(&theme, layout, cx));
        let panel = panel_shown.then(|| self.panel(&theme, layout, window, cx));
        let enlarged = self.enlarged.clone().map(|path| {
            let pictures = self.enlarged_pictures.of(&path.to_string_lossy(), cx);
            div()
                .id("enlarged")
                .absolute()
                .inset_0()
                .p(theme::GAP_WIDE)
                .bg(theme.colours.scrim)
                .occlude()
                .child(
                    pictures
                        .img(path)
                        .size_full()
                        .object_fit(ObjectFit::Contain),
                )
                .on_click(cx.listener(|w, _, _, cx| {
                    w.enlarged = None;
                    cx.notify();
                }))
        });
        div()
            .id("workshop")
            .track_focus(&self.focus)
            .relative()
            .size_full()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .size_full()
                    .p(theme::GAP_WIDE)
                    .gap(theme::GAP_LARGE)
                    .child(self.search_row(&theme, layout, window, cx))
                    .children(self.banner(&theme, cx))
                    .child(
                        div()
                            .flex()
                            .flex_1()
                            .min_h_0()
                            .gap(theme::GAP_LARGE)
                            .children(side)
                            .children(results)
                            .children(panel),
                    ),
            )
            .children(enlarged)
    }
}

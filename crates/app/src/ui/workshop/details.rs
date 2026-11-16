use super::{SCREENSHOTS, Workshop};
use crate::ui::button::{Btn, Chip};
use crate::ui::rich::{self, Link};
use crate::ui::theme::{self, Theme};
use crate::ui::widgets::{TextSized, label, watched};
use crate::words::put;
use crate::workspace::browse::{Related, Row, Shown};
use gpui::{
    AnyElement, Context, InteractiveElement, IntoElement, ObjectFit, ParentElement, Pixels,
    StatefulInteractiveElement, Styled, StyledImage, Window, div, prelude::FluentBuilder,
};
use rimstack_core::say;
use std::path::PathBuf;
use std::rc::Rc;

impl Workshop {
    pub(super) fn open_details(&mut self, id: String, cx: &mut Context<Self>) {
        let app = crate::ui::app(cx);
        self.opening.run(
            "workshop details",
            cx,
            move |_| app.browse_details(&id),
            |workshop, shown, cx| {
                workshop.opened.busy = false;
                workshop.show_details(shown, cx);
            },
        );
    }

    fn show_details(&mut self, shown: Option<Shown>, cx: &mut Context<Self>) {
        let Some(shown) = shown else {
            self.opened.shown = None;
            cx.notify();
            return;
        };
        if shown.preview_url != self.opened.preview.0 {
            self.opened.preview = (shown.preview_url.clone(), None);
            self.fetch_preview(shown.preview_url.clone(), cx);
        }
        let pictures = shown.picture_urls();
        if !pictures
            .iter()
            .all(|url| self.opened.images.contains_key(url))
        {
            self.fetch_images(pictures, cx);
        }
        if shown.screenshot_urls != self.opened.shots.0 {
            self.opened.shots = (shown.screenshot_urls.clone(), Vec::new());
            self.fetch_screenshots(shown.screenshot_urls.clone(), cx);
        }
        self.opened.shown = Some(Rc::new(shown));
        cx.notify();
    }

    fn fetch_preview(&mut self, url: String, cx: &mut Context<Self>) {
        let app = crate::ui::app(cx);
        let asked = url.clone();
        self.detail_previewing.run(
            "workshop preview",
            cx,
            move |_| app.browse_preview(&asked),
            move |workshop, found, cx| match found {
                Ok(Some(path)) if workshop.opened.preview.0 == url => {
                    workshop.opened.preview.1 = Some(path);
                    cx.notify();
                }
                Ok(_) => {}
                Err(problem) => rimstack_core::problem!("{problem}"),
            },
        );
    }

    fn fetch_images(&mut self, urls: Vec<String>, cx: &mut Context<Self>) {
        let app = crate::ui::app(cx);
        let asked = urls.clone();
        self.imaging.run(
            "workshop description pictures",
            cx,
            move |_| app.browse_images(&asked),
            move |workshop, taken, cx| {
                let current = workshop.opened.shown.as_ref().map(|s| s.picture_urls());
                if current.as_ref() == Some(&urls) {
                    for (url, path) in taken {
                        if let Some(path) = path {
                            let path = path.to_string_lossy().into_owned();
                            workshop.opened.images.insert(url, path);
                        }
                    }
                    cx.notify();
                }
            },
        );
    }

    fn fetch_screenshots(&mut self, urls: Vec<String>, cx: &mut Context<Self>) {
        if urls.is_empty() {
            self.shooting.cancel();
            return;
        }
        let app = crate::ui::app(cx);
        let asked = urls.clone();
        self.shooting.run(
            "workshop screenshots",
            cx,
            move |turn| {
                let mut paths = Vec::new();
                for url in asked.iter().take(SCREENSHOTS) {
                    if !turn.current() {
                        break;
                    }
                    match app.browse_preview(url) {
                        Ok(found) => paths.extend(found),
                        Err(problem) => rimstack_core::problem!("{problem}"),
                    }
                }
                paths
            },
            move |workshop, paths, cx| {
                if workshop.opened.shots.0 == urls {
                    workshop.opened.shots.1 = paths;
                    cx.notify();
                }
            },
        );
    }

    fn badges(theme: &Theme, row: &Row) -> Vec<AnyElement> {
        let c = &theme.colours;
        let badge = row.badge.as_ref().map(put).unwrap_or_default();
        let mut chips = Vec::new();
        if row.installed {
            chips.push(
                Chip::new(badge.clone())
                    .tone(c.ok)
                    .icon("check")
                    .into_any_element(),
            );
        } else if row.on_list {
            chips.push(
                Chip::new(badge)
                    .tone(c.moved)
                    .icon("list")
                    .into_any_element(),
            );
        }
        chips
    }

    fn heading(theme: &Theme, text: String) -> gpui::Div {
        label(theme, theme.small(), theme.colours.ink2, text).font_weight(theme.semibold)
    }

    fn about(
        &self,
        shown: &Shown,
        theme: &Theme,
        width: Pixels,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let c = &theme.colours;
        let row = &shown.row;
        let id = row.id.clone();
        let pictures = self.pictures.of(&id, cx);
        let mut parts: Vec<AnyElement> = Vec::new();
        if let Some(path) = self
            .opened
            .preview
            .1
            .clone()
            .filter(|_| self.opened.preview.0 == shown.preview_url)
        {
            parts.push(
                div()
                    .w(width)
                    .h(width * 9. / 16.)
                    .rounded(theme::CORNER)
                    .bg(c.ctl)
                    .overflow_hidden()
                    .child(
                        pictures
                            .img(path)
                            .size_full()
                            .object_fit(ObjectFit::Contain),
                    )
                    .into_any_element(),
            );
        }
        let chips = div()
            .flex()
            .flex_wrap()
            .gap(theme::GAP_TINY)
            .pt(theme::GAP_THIN)
            .children(Self::badges(theme, row))
            .when(shown.banned, |d| {
                d.child(
                    Chip::new(put(&say!("Banned")))
                        .tone(c.err)
                        .icon("circle-alert"),
                )
            })
            .children(row.versions.iter().map(|v| Chip::new(v.clone())));
        parts.push(
            div()
                .flex()
                .flex_col()
                .gap(theme::GAP_THIN)
                .child(
                    div()
                        .sized_text(theme, theme.heading())
                        .font_weight(theme.semibold)
                        .text_color(c.ink)
                        .child(row.title.clone()),
                )
                .when_some(shown.created_by.as_ref(), |d, by| {
                    d.child(
                        div()
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_ellipsis()
                            .sized_text(theme, theme.body())
                            .text_color(c.ink2)
                            .child(put(by)),
                    )
                })
                .child(chips)
                .into_any_element(),
        );
        let needs = !shown.requires.is_empty();
        let (added, with, page, notes) = (id.clone(), id.clone(), id.clone(), id.clone());
        parts.push(
            div()
                .flex()
                .flex_wrap()
                .gap(theme::GAP)
                .when(row.can_add, |d| {
                    d.child(
                        Btn::new("details-add", put(&say!("Add")))
                            .icon("plus")
                            .primary_if(!needs)
                            .on_click(
                                cx.listener(move |w, _, _, cx| w.add(added.clone(), false, cx)),
                            ),
                    )
                })
                .when(row.can_add && needs, |d| {
                    d.child(
                        Btn::new("details-add-all", put(&say!("Add with what it requires")))
                            .icon("plus")
                            .primary()
                            .on_click(
                                cx.listener(move |w, _, _, cx| w.add(with.clone(), true, cx)),
                            ),
                    )
                })
                .child(
                    Btn::new("details-page", put(&say!("Workshop page")))
                        .icon("external-link")
                        .on_click(move |_, _, cx| cx.open_url(&crate::ui::workshop_page(&page))),
                )
                .child(
                    Btn::new("details-notes", put(&say!("Change notes")))
                        .icon("history")
                        .tip(put(&say!("The item's change notes on its Workshop page.")))
                        .on_click(move |_, _, cx| cx.open_url(&crate::ui::change_notes(&notes))),
                )
                .into_any_element(),
        );
        let related = |key: &'static str, text: String, tip: Option<String>, which: Related| {
            let id = id.clone();
            let button = Btn::new(key, text)
                .on_click(cx.listener(move |w, _, _, cx| w.show_related(id.clone(), which, cx)));
            match tip {
                Some(tip) => button.tip(tip),
                None => button,
            }
        };
        parts.push(
            div()
                .flex()
                .flex_wrap()
                .gap(theme::GAP)
                .child(related(
                    "details-requires",
                    put(&say!("Requires")),
                    Some(put(&say!("The mods this one needs."))),
                    Related::Requires,
                ))
                .child(related(
                    "details-needed-by",
                    put(&say!("Needed by")),
                    Some(put(&say!("The mods that need this one."))),
                    Related::NeededBy,
                ))
                .when(row.author.is_some(), |d| {
                    d.child(related(
                        "details-same-author",
                        put(&say!("Same author")),
                        None,
                        Related::SameAuthor,
                    ))
                })
                .into_any_element(),
        );
        if let Some(problem) = &shown.problem {
            parts.push(
                div()
                    .sized_text(theme, theme.body())
                    .text_color(c.err)
                    .child(put(problem))
                    .into_any_element(),
            );
        }
        let shots = &self.opened.shots;
        if shots.0 == shown.screenshot_urls && !shots.1.is_empty() {
            let each = (width - theme::GAP_TINY * 2.) / 3.;
            let me = cx.entity().downgrade();
            parts.push(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(theme::GAP_TINY)
                    .children(shots.1.iter().enumerate().map(|(at, path)| {
                        let (me, path) = (me.clone(), path.clone());
                        div()
                            .id(("screenshot", at))
                            .role(gpui::Role::Image)
                            .aria_label(put(&say!("Screenshot")))
                            .w(each)
                            .h(each * 9. / 16.)
                            .rounded(theme::CORNER)
                            .bg(c.ctl)
                            .overflow_hidden()
                            .cursor_pointer()
                            .child(
                                pictures
                                    .img(path.clone())
                                    .size_full()
                                    .object_fit(ObjectFit::Cover),
                            )
                            .on_click(move |_, _, cx| {
                                let path = path.clone();
                                let _ = me.update(cx, |w, cx| {
                                    w.enlarged = Some(path);
                                    cx.notify();
                                });
                            })
                    }))
                    .into_any_element(),
            );
        }
        let description = shown.description(&self.opened.images);
        if shown.loaded && !description.is_empty() {
            let me = cx.entity().downgrade();
            let link: Link = Rc::new(move |url, _, cx| {
                let url = url.to_string();
                let _ = me.update(cx, |w, cx| w.followed(&url, cx));
            });
            let mut column = div()
                .flex()
                .flex_col()
                .gap(theme::GAP_SMALL)
                .child(Self::heading(theme, put(&say!("Description"))));
            for (at, part) in description.iter().enumerate() {
                let pick = {
                    let mut picks = self.description_picks.borrow_mut();
                    while picks.len() <= at {
                        picks.push(rich::Selection::new(cx));
                    }
                    picks[at].clone()
                };
                let piece = div()
                    .flex()
                    .flex_col()
                    .gap(theme::GAP_SMALL)
                    .child(
                        pick.holding(div().flex().flex_col().gap(theme::GAP_SMALL).children(
                            rich::blocks("description", theme, &part.blocks, &link, Some(&pick)),
                        )),
                    )
                    .when_some(part.note.as_ref(), |d, note| {
                        d.child(
                            div()
                                .italic()
                                .sized_text(theme, theme.small())
                                .text_color(c.ink3)
                                .child(put(note)),
                        )
                    })
                    .when(!part.image.is_empty(), |d| {
                        d.child(
                            pictures
                                .img(PathBuf::from(&part.image))
                                .id(("description-picture", at))
                                .w(width)
                                .object_fit(ObjectFit::Contain),
                        )
                    });
                column = column.child(piece);
            }
            parts.push(column.into_any_element());
        }
        if !shown.requires.is_empty() {
            let rows = shown.requires.iter().enumerate().map(|(at, need)| {
                let id = need.id.clone();
                div()
                    .flex()
                    .items_center()
                    .gap(theme::GAP)
                    .child(fill_text(theme, theme.body(), c.ink, need.title.clone()))
                    .children(Self::badges(theme, need))
                    .when(need.can_add, |d| {
                        d.child(
                            Btn::new(("need-add", at), put(&say!("Add")))
                                .icon("plus")
                                .on_click(
                                    cx.listener(move |w, _, _, cx| w.add(id.clone(), false, cx)),
                                ),
                        )
                    })
                    .into_any_element()
            });
            parts.push(
                div()
                    .flex()
                    .flex_col()
                    .gap(theme::GAP_SMALL)
                    .child(Self::heading(theme, put(&say!("Requires"))))
                    .children(rows)
                    .into_any_element(),
            );
        }
        if let Some(unknown) = &shown.unknown_said {
            parts.push(
                div()
                    .sized_text(theme, theme.small())
                    .text_color(c.warn)
                    .child(put(unknown))
                    .into_any_element(),
            );
        }
        if shown.loaded {
            let said = [
                &shown.size_said,
                &shown.created_said,
                &shown.updated_said,
                &shown.votes_said,
            ];
            parts.push(
                div()
                    .flex()
                    .flex_col()
                    .gap(theme::GAP_TINY)
                    .child(Self::heading(theme, put(&say!("Details"))))
                    .children(shown.stats.iter().map(|stat| {
                        div()
                            .flex()
                            .gap(theme::GAP)
                            .child(fill_text(theme, theme.small(), c.ink2, put(&stat.label)))
                            .child(label(theme, theme.small(), c.ink, put(&stat.value)))
                    }))
                    .children(said.into_iter().flatten().map(|said| {
                        div()
                            .sized_text(theme, theme.small())
                            .text_color(c.ink2)
                            .child(put(said))
                    }))
                    .into_any_element(),
            );
        }
        if !shown.dlc.is_empty() {
            parts.push(
                div()
                    .flex()
                    .flex_col()
                    .gap(theme::GAP_SMALL)
                    .child(Self::heading(theme, put(&say!("Needs the expansion"))))
                    .child(div().flex().flex_wrap().gap(theme::GAP_TINY).children(
                        shown.dlc.iter().map(|dlc| {
                            Chip::new(dlc.name.clone().unwrap_or_else(|| dlc.id.clone()))
                                .icon("package")
                                .max_width(width)
                        }),
                    ))
                    .into_any_element(),
            );
        }
        parts
    }

    pub(super) fn details_pane(
        &self,
        theme: &Theme,
        width: Pixels,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let c = &theme.colours;
        let _ = window;
        let inner = width - crate::ui::page::room(theme);
        let content = match self.opened.shown.clone() {
            Some(shown) => self.about(&shown, theme, inner, cx),
            None => Vec::new(),
        };
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
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(Self::heading(theme, put(&say!("Details")))),
                    )
                    .child(
                        Btn::new("close-details", "")
                            .icon("x")
                            .tip(put(&say!("Close the details")))
                            .on_click(cx.listener(|w, _, _, cx| w.select(String::new(), cx))),
                    ),
            )
            .child(
                div().flex_1().min_h_0().child(crate::ui::page::page(
                    "workshop-details",
                    &self.details_paging,
                    theme,
                    cx.entity_id(),
                    div()
                        .flex()
                        .flex_col()
                        .gap(theme::GAP_LARGE)
                        .w(inner)
                        .text_color(c.ink)
                        .children(content),
                )),
            )
            .into_any_element()
    }
}

pub(super) fn fill_text(
    theme: &Theme,
    size: Pixels,
    colour: gpui::Hsla,
    text: String,
) -> AnyElement {
    watched(
        div()
            .flex_1()
            .min_w_0()
            .overflow_hidden()
            .whitespace_nowrap()
            .text_ellipsis()
            .sized_text(theme, size)
            .text_color(colour),
        text.clone().into(),
        text,
        true,
    )
    .into_any_element()
}

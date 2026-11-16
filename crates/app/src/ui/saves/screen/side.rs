use crate::ui::button::Btn;
use crate::ui::field::Field;
use crate::ui::page::{page, room};

use crate::ui::saves::Saves;
use crate::ui::theme::{self, Theme};
use crate::ui::widgets::{TextSized, breakable, icon, label};
use crate::words::put;
use crate::workspace::logs::moment;
use crate::workspace::saves::SaveShown;
use gpui::{
    AnyElement, Context, IntoElement, ObjectFit, ParentElement, Styled, StyledImage, Window, div,
    prelude::FluentBuilder,
};
use rimstack_core::rimworld::saves::Verdict;
use rimstack_core::{say, say_n};

impl Saves {
    pub(super) fn side(
        &self,
        save: Option<&SaveShown<'_>>,
        theme: &Theme,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let c = &theme.colours;
        let width = Self::side_width(theme, window);
        let inner = width - theme::GAP_CARD * 2. - room(theme);
        let group = |text: String| {
            label(theme, theme.small(), c.ink2, text)
                .font_weight(theme.semibold)
                .pt(theme::GAP_WIDE)
                .pb(theme::GAP_SMALL)
        };
        let body = |colour, text: String| {
            div()
                .sized_text(theme, theme.body())
                .text_color(colour)
                .child(text)
        };
        let content = save.map(|save| {
            let row = save.row;
            let compared = row.compared.as_ref();
            let info = match self.report.rimsaves {
                true => put(&say!(
                    "In {}, {}, {} MB",
                    row.folder.clone(),
                    moment(row.modified),
                    format!("{:.1}", row.megabytes)
                )),
                false => put(&say!(
                    "{}, {} MB",
                    moment(row.modified),
                    format!("{:.1}", row.megabytes)
                )),
            };
            let ids = compared.map(|c| c.ids.clone());
            let breaks = row.verdict == Verdict::Breaks;
            let dir = save.dir.clone();
            let missing = compared.map(|c| c.missing.clone()).unwrap_or_default();
            let added = compared.map(|c| c.added.clone()).unwrap_or_default();
            let typed = !self.list_name.read(cx).text().trim().is_empty();
            let field = Field {
                id: "save-list-name",
                text: &self.list_name,
                icon: "",
                placeholder: put(&say!("Save its list as…")),
                found: None,
                keys: None,
            };
            let more = |n: usize, open: bool| {
                if open {
                    put(&say!("Show five"))
                } else {
                    put(&say!("{} more", n - 5))
                }
            };
            div()
                .flex()
                .flex_col()
                .when_some(row.preview.clone(), |d, preview| {
                    let pictures = self.pictures.of(&preview, cx);
                    d.child(
                        div()
                            .w(inner)
                            .h(theme.sized(190.))
                            .rounded(theme::CORNER_LARGE)
                            .bg(c.well)
                            .overflow_hidden()
                            .child(
                                pictures
                                    .img(std::path::PathBuf::from(preview))
                                    .w(inner)
                                    .h(theme.sized(190.))
                                    .object_fit(ObjectFit::Cover),
                            ),
                    )
                    .child(div().h(theme::GAP_LARGE))
                })
                .child(
                    div()
                        .sized_text(theme, theme.heading())
                        .font_weight(theme.semibold)
                        .text_color(c.ink)
                        .child(breakable(&row.name)),
                )
                .child(
                    div()
                        .pt(theme::GAP_HAIR)
                        .sized_text(theme, theme.body())
                        .text_color(c.ink2)
                        .child(info),
                )
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap(theme::GAP_SMALL)
                        .pt(theme::GAP_MEDIUM)
                        .when(save.usable, |d| {
                            let ids = ids.clone().unwrap_or_default();
                            d.child(
                                Btn::new("use-save-list", put(&say!("Use this save's mod list")))
                                    .icon("list")
                                    .primary_if(breaks)
                                    .on_click(cx.listener(move |_, _, _, cx| {
                                        crate::ui::navigate::go(
                                            crate::ui::navigate::Go::UseList(ids.clone()),
                                            cx,
                                        )
                                    })),
                            )
                        })
                        .child(
                            Btn::new("open-save-folder", put(&say!("Open folder")))
                                .icon("folder-open")
                                .on_click(move |_, _, cx| {
                                    cx.open_with_system(std::path::Path::new(&dir))
                                }),
                        ),
                )
                .when(!row.facts.is_empty(), |d| {
                    d.child(group(put(&say!("Colony"))))
                        .children(row.facts.iter().map(|(name, value)| {
                            div()
                                .flex()
                                .items_center()
                                .gap(theme::GAP)
                                .min_h(theme.sized(22.))
                                .child(
                                    div()
                                        .w(theme.sized(110.))
                                        .max_w(inner / 2.)
                                        .flex_none()
                                        .sized_text(theme, theme.body())
                                        .text_color(c.ink2)
                                        .child(put(
                                            &rimstack_core::base::sentence::Sentence::named(name),
                                        )),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .overflow_hidden()
                                        .whitespace_nowrap()
                                        .text_ellipsis()
                                        .sized_text(theme, theme.body())
                                        .text_color(c.ink)
                                        .child(value.clone()),
                                )
                        }))
                })
                .when(ids.is_some(), |d| {
                    d.child(div().h(theme::GAP_LARGE)).child(
                        div()
                            .flex()
                            .items_center()
                            .gap(theme::GAP_SMALL)
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .child(field.render(theme, window, cx)),
                            )
                            .child(
                                Btn::new("keep-save-list", put(&say!("Keep")))
                                    .enabled(typed)
                                    .on_click(cx.listener(|saves, _, _, cx| saves.keep_list(cx))),
                            ),
                    )
                })
                .when(!self.list_problem.is_empty(), |d| {
                    d.child(body(c.err, self.list_problem.clone()))
                })
                .when_some(row.problem.clone(), |d, problem| {
                    d.child(group(put(&say!("Problem"))))
                        .child(body(c.err, put(&say!("Unreadable: {}", problem))))
                })
                .when(ids.is_some(), |d| {
                    let same = row.verdict == Verdict::Same;
                    d.child(group(put(&say!("Differences from the current list"))))
                        .when(missing.is_empty() && !same, |d| {
                            d.child(body(
                                c.ink2,
                                put(&say!("Nothing the save needs is missing.")),
                            ))
                        })
                        .when(same, |d| {
                            d.child(
                                div()
                                    .spaced_text(theme, theme.body(), 1.3)
                                    .text_color(c.ink)
                                    .child(put(&say!("Same mods, same order as the load order"))),
                            )
                        })
                })
                .when(compared.is_some_and(|c| c.other_game), |d| {
                    let version = compared.map(|c| c.game_version.clone()).unwrap_or_default();
                    d.child(group(put(&say!("Other game version")))).child(
                        div()
                            .spaced_text(theme, theme.body(), 1.3)
                            .text_color(c.warn)
                            .child(put(&say!(
                                "Saved with RimWorld {}, not the installed version",
                                version
                            ))),
                    )
                })
                .when(!missing.is_empty(), |d| {
                    let shown = if self.all_missing {
                        missing.len()
                    } else {
                        missing.len().min(5)
                    };
                    d.child(
                        div()
                            .pt(theme::GAP_TINY)
                            .sized_text(theme, theme.body())
                            .text_color(if save.not_installed > 0 {
                                c.err
                            } else {
                                c.ink2
                            })
                            .child(put(&save.missing_said)),
                    )
                    .children(missing.iter().take(shown).map(|gone| {
                        div()
                            .flex()
                            .items_center()
                            .gap(theme::GAP)
                            .min_h(theme.sized(22.))
                            .child(icon(
                                if gone.installed {
                                    "minus"
                                } else {
                                    "circle-alert"
                                },
                                theme.icon(),
                                if gone.installed { c.ink3 } else { c.err },
                            ))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .overflow_hidden()
                                    .whitespace_nowrap()
                                    .text_ellipsis()
                                    .sized_text(theme, theme.body())
                                    .text_color(if gone.installed { c.ink } else { c.err })
                                    .child(gone.name.clone()),
                            )
                            .child(label(
                                theme,
                                theme.small(),
                                c.ink3,
                                if gone.installed {
                                    put(&say!("not active"))
                                } else {
                                    put(&say!("not installed"))
                                },
                            ))
                    }))
                    .when(missing.len() > 5, |d| {
                        d.child(
                            div().flex().child(
                                Btn::new("all-missing", more(missing.len(), self.all_missing))
                                    .on_click(cx.listener(|saves, _, _, cx| {
                                        saves.all_missing = !saves.all_missing;
                                        cx.notify();
                                    })),
                            ),
                        )
                    })
                })
                .when(!added.is_empty(), |d| {
                    let shown = if self.all_added {
                        added.len()
                    } else {
                        added.len().min(5)
                    };
                    d.child(
                        div()
                            .pt(theme::GAP)
                            .sized_text(theme, theme.body())
                            .text_color(c.ink2)
                            .child(put(&say_n!(
                                added.len(),
                                "{} mod added since",
                                "{} mods added since"
                            ))),
                    )
                    .children(added.iter().take(shown).map(|name| {
                        div()
                            .flex()
                            .items_center()
                            .gap(theme::GAP)
                            .min_h(theme.sized(22.))
                            .child(icon("plus", theme.icon(), c.ink3))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .overflow_hidden()
                                    .whitespace_nowrap()
                                    .text_ellipsis()
                                    .sized_text(theme, theme.body())
                                    .text_color(c.ink)
                                    .child(name.clone()),
                            )
                    }))
                    .when(added.len() > 5, |d| {
                        d.child(div().flex().child(
                            Btn::new("all-added", more(added.len(), self.all_added)).on_click(
                                cx.listener(|saves, _, _, cx| {
                                    saves.all_added = !saves.all_added;
                                    cx.notify();
                                }),
                            ),
                        ))
                    })
                })
                .when(compared.is_some_and(|c| c.reordered), |d| {
                    d.child(
                        div()
                            .pt(theme::GAP_TINY)
                            .spaced_text(theme, theme.body(), 1.3)
                            .text_color(c.ink)
                            .child(put(&say!("Shared mods are in a different order now"))),
                    )
                })
                .child(group(put(&say!("Backups"))))
                .child(body(
                    c.ink,
                    put(&say!(
                        "The newest save is copied before each launch; the last 5 are kept"
                    )),
                ))
                .child(div().h(theme::GAP_SMALL))
                .child(div().flex().child(
                    Btn::new("open-backups", put(&say!("Open the backups folder"))).on_click(
                        |_, _, cx| {
                            let folder = crate::ui::app(cx).data_dir().join("save-backups");
                            cx.open_with_system(&folder);
                        },
                    ),
                ))
        });
        div()
            .flex_none()
            .w(width)
            .h_full()
            .bg(c.panel)
            .border_l_1()
            .border_color(c.edge)
            .when_some(content, |d, content| {
                d.child(page(
                    "save-side",
                    &self.side_paging,
                    theme,
                    cx.entity_id(),
                    div()
                        .p(theme::GAP_CARD)
                        .pr(theme::GAP_CARD + room(theme))
                        .child(content),
                ))
            })
            .into_any_element()
    }
}

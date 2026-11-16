use super::{Textures, megabytes};
use crate::ui::button::{Btn, Chip};
use crate::ui::controls::Choice;
use crate::ui::rows::{bar, rows};
use crate::ui::theme::{self, Theme};
use crate::ui::widgets::{TextSized, label, measure, notice};
use crate::words::{count, put};
use crate::workspace::textures::{RowShown, TexturesShown, no_conversion, shown};
use gpui::{
    AnimationExt, AnyElement, Context, FontWeight, InteractiveElement, IntoElement, KeyDownEvent,
    ParentElement, Pixels, Render, Role, SharedString, StatefulInteractiveElement, Styled, Window,
    div, prelude::FluentBuilder, px,
};
use rimstack_core::base::sentence::Sentence;
use rimstack_core::{say, say_n};
use std::rc::Rc;

pub(super) struct Columns {
    pub(super) name: Pixels,
    pub(super) count: Pixels,
    pub(super) png: Pixels,
    pub(super) dds: Pixels,
}

impl Textures {
    fn side_width(theme: &Theme, window: &Window) -> Pixels {
        theme.sized(360.).min(window.viewport_size().width * 0.3)
    }

    fn columns(&self, theme: &Theme, width: Pixels, window: &Window) -> Columns {
        let heading = |text: &str| measure(window, theme, text, theme.caption(), theme.medium);
        let body = |text: &str| measure(window, theme, text, theme.body(), FontWeight::NORMAL);
        let widest = body(&megabytes(99999.0));
        let count = heading(&put(&say!("Textures"))).max(body(&count(99999)));
        let png = heading(&put(&say!("PNG"))).max(widest);
        let dds = heading(&put(&say!("DDS"))).max(widest).max(measure(
            window,
            theme,
            &put(&say!("Not converted")),
            theme.small(),
            FontWeight::NORMAL,
        ));
        let room = width - theme::GAP_WIDE * 2. - theme::GAP_LARGE * 4. - count - png - dds;
        Columns {
            name: room.clamp(theme.sized(80.), theme.sized(460.)),
            count,
            png,
            dds,
        }
    }

    fn summary(
        &self,
        shown: &TexturesShown<'_>,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let c = &theme.colours;
        let (total, have) = (shown.total, shown.have);
        let part = if total > 0 {
            have as f32 / total as f32
        } else {
            0.
        };
        let first_waiting = shown.first_waiting;
        let (remaining, stale, made) = (
            shown.remaining.clone(),
            shown.stale.clone(),
            shown.made.clone(),
        );
        let (busy, converts) = (self.busy, self.converts());
        let progress = match busy && self.progress.is_empty() {
            true => put(&say!("Counting textures…")),
            false => self.progress.clone(),
        };
        let dds_tip: SharedString = crate::ui::card::meaning("DDS").unwrap_or_default().into();
        let views: Vec<SharedString> = [say!("Active mods"), say!("All mods")]
            .iter()
            .map(|o| put(o).into())
            .collect();
        let counted = div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap(theme::GAP)
            .flex_1()
            .min_w_0()
            .child(
                div()
                    .flex()
                    .items_baseline()
                    .gap(theme::GAP)
                    .child(
                        label(
                            theme,
                            theme.heading(),
                            c.ink,
                            put(&say!("{} of {}", count(have), count(total))),
                        )
                        .font_weight(theme.semibold),
                    )
                    .child(crate::ui::tip::tipped(
                        div().id("have-dds").child(label(
                            theme,
                            theme.body(),
                            c.ink2,
                            put(&say!("textures have a DDS")),
                        )),
                        dds_tip,
                    )),
            )
            .when_some(shown.size_said.as_ref(), |d, said| {
                d.child(Chip::new(put(said)))
            })
            .when(shown.waiting > 0, |d| {
                d.child(
                    Chip::new(put(&say_n!(
                        shown.waiting,
                        "{} mod to convert",
                        "{} mods to convert"
                    )))
                    .tone(c.warn)
                    .tip(put(&say!("Shows the first of them")))
                    .on_click(
                        "to-convert",
                        cx.listener(move |t, _, _, cx| {
                            t.selected = first_waiting;
                            t.scroll.borrow_mut().set_top(px(0.));
                            cx.notify();
                        }),
                    ),
                )
            })
            .when(shown.out_of_date > 0, |d| {
                d.child(
                    Chip::new(put(&say!("{} out of date", count(shown.out_of_date))))
                        .tone(c.warn)
                        .icon("triangle-alert"),
                )
            });
        let actions = div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap(theme::GAP)
            .flex_1()
            .min_w_0()
            .when(!progress.is_empty(), |d| d.child(div().h(theme.control()).flex().items_center().min_w_0().overflow_hidden().whitespace_nowrap().text_ellipsis().sized_text(theme, theme.small()).text_color(c.ink2).child(progress)))
            .when(busy, |d| {
                d.child(Btn::new("cancel-textures", put(&say!("Cancel"))).tip(put(&say!("Stops after the current mod; what is already done is kept"))).on_click(cx.listener(|t, _, _, _| t.stop.store(true, std::sync::atomic::Ordering::Relaxed))))
            })
            .when(self.refuses(), |d| {
                d.child(
                        Btn::new("find-spare", put(&say!("Find spare DDS files")))
                            .enabled(!busy)
                            .tip(put(&say!("Decodes each DDS beside a PNG in the Workshop mods and finds those that show the same picture. From 1.7 the game makes its own from the PNG, so these only take room.")))
                            .on_click(cx.listener(|t, _, _, cx| t.find_spare(cx))),
                    )
            })
            .when(converts, |d| {
                d.child(
                    Btn::new("convert-remaining", put(&say!("Convert remaining")))
                        .primary_if(!remaining.is_empty())
                        .enabled(!busy && !remaining.is_empty())
                        .tip(put(&say!("Writes a .dds beside each .png that has none, inside each mod's own folder. The .png files are not changed.")))
                        .on_click(cx.listener(move |t, _, _, cx| t.work(remaining.clone(), false, cx))),
                )
                .child(
                    Btn::new("rebuild-stale", put(&say!("Rebuild out of date")))
                        .enabled(!busy && !stale.is_empty())
                        .tip(put(&say!("Converts again where the .png changed after its .dds was made.")))
                        .on_click(cx.listener(move |t, _, _, cx| t.work(stale.clone(), false, cx))),
                )
            });
        div()
            .flex()
            .flex_col()
            .gap(theme::GAP)
            .m(theme::GAP_LARGE)
            .mb(theme::GAP)
            .px(px(2.))
            .pt(theme::GAP)
            .pb(theme::GAP)
            .child(
                div()
                    .flex()
                    .items_start()
                    .gap(theme::GAP)
                    .child(counted)
                    .child(
                        Btn::new("check-all-dds", put(&say!("Check all mods")))
                            .enabled(!self.auditing)
                            .tip(put(&say!("Reads the header of every DDS file in the installed mods and finds those that are not DDS, are cut short, use a format the game cannot read, or are compressed with a size that is not a multiple of four. From 1.7 it also finds those that hide a texture the mod packed in Assets.zip.")))
                            .on_click(cx.listener(|t, _, _, cx| t.audit(cx))),
                    )
                    .child(Choice::new("textures-of", views, usize::from(!self.active_only)).title(put(&say!("Mods"))).on_pick(crate::ui::act(cx, |t: &mut Self, at: usize, _, cx| {
                        t.active_only = at == 0;
                        cx.notify();
                    }))),
            )
            .when(self.refuses(), |d| {
                d.child(div().spaced_text(theme, theme.body(), 1.3).text_color(c.ink2).child(put(&no_conversion())))
            })
            .when(converts, |d| d.child(
                div()
                    .id("dds-progress")
                    .role(Role::ProgressIndicator)
                    .aria_label(put(&say!("Textures with a DDS")))
                    .h(px(6.))
                    .rounded(px(3.))
                    .bg(c.chip)
                    .child(div().h_full().rounded(px(3.)).bg(if have == total { c.ok } else { c.accent }).with_spring(
                        "have",
                        crate::ui::motion::towards(gpui::AnimationPhase(part), 0.001, crate::ui::motion::CALM),
                        |d, part| d.w(gpui::relative(part.0.clamp(0., 1.))),
                    )),
            ))
            .child(
                div()
                    .flex()
                    .items_start()
                    .gap(theme::GAP_WIDE)
                    .child(actions)
                    .child(
                        Btn::new("remove-all-made", put(&say!("Remove all converted…")))
                            .danger()
                            .enabled(!busy && !made.is_empty())
                            .tip(put(&say!("Deletes the .dds files RimStack made, inside the mods' folders. Files it did not make are not touched.")))
                            .on_click(cx.listener(|t, _, _, cx| {
                                t.confirm_remove = true;
                                cx.notify();
                            })),
                    ),
            )
            .into_any_element()
    }

    fn confirm(
        &self,
        shown: &TexturesShown<'_>,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let c = &theme.colours;
        if !self.confirm_remove {
            return None;
        }
        let made = shown.made.clone();
        let words = put(&say_n!(
            shown.made_files,
            "Delete {} DDS file made by RimStack ({})? The PNG files stay.",
            "Delete {} DDS files made by RimStack ({})? The PNG files stay.",
            say_n!(made.len(), "{} mod", "{} mods").inside()
        ));
        Some(
            notice(c.err)
                .mx(theme::GAP_LARGE)
                .mb(theme::GAP)
                .h(theme.sized(40.))
                .flex()
                .items_center()
                .gap(theme::GAP)
                .pl(theme::GAP_LARGE)
                .pr(theme::GAP)
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_ellipsis()
                        .sized_text(theme, theme.body())
                        .text_color(c.ink)
                        .child(words),
                )
                .child(
                    Btn::new("remove-cancel", put(&say!("Cancel"))).on_click(cx.listener(
                        |t, _, _, cx| {
                            t.confirm_remove = false;
                            cx.notify();
                        },
                    )),
                )
                .child(
                    Btn::new("remove-confirm", put(&say!("Delete them")))
                        .danger()
                        .on_click(cx.listener(move |t, _, _, cx| {
                            t.confirm_remove = false;
                            t.work(made.clone(), true, cx);
                        })),
                )
                .into_any_element(),
        )
    }

    fn heading(&self, columns: &Columns, theme: &Theme) -> AnyElement {
        let c = &theme.colours;
        let cell = |text: Sentence, width: Pixels, right: bool| {
            div()
                .w(width)
                .flex_none()
                .flex()
                .when(right, |d| d.justify_end())
                .child(label(theme, theme.caption(), c.ink3, put(&text)).font_weight(theme.medium))
        };
        div()
            .relative()
            .flex()
            .items_center()
            .gap(theme::GAP_LARGE)
            .h(theme.sized(24.))
            .px(theme::GAP_WIDE)
            .child(cell(say!("Mod"), columns.name, false))
            .child(cell(say!("Textures"), columns.count, true))
            .child(cell(say!("PNG"), columns.png, true))
            .child(cell(say!("DDS"), columns.dds, true))
            .child(
                div()
                    .absolute()
                    .bottom_0()
                    .left(theme::GAP_SMALL)
                    .right(theme::GAP_SMALL)
                    .h(px(1.))
                    .bg(c.line),
            )
            .into_any_element()
    }

    fn list(
        &self,
        shown: &TexturesShown<'_>,
        columns: Columns,
        theme: &Theme,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let c = theme.colours;
        if shown.rows.is_empty() {
            let width =
                (window.viewport_size().width - theme::GAP_PAGE * 4.).min(theme.sized(460.));
            let title = if self.busy {
                put(&say!("Counting textures…"))
            } else {
                put(&say!("No textures"))
            };
            let words = if shown.all == 0 {
                String::new()
            } else {
                put(&say!("None in active mods"))
            };
            return div()
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .items_center()
                        .gap(theme::GAP_SMALL)
                        .w(width)
                        .child(
                            label(theme, theme.title(), c.ink, title).font_weight(theme.semibold),
                        )
                        .when(!self.busy, |d| {
                            d.child(
                                div()
                                    .text_center()
                                    .spaced_text(theme, theme.body(), 1.3)
                                    .text_color(c.ink2)
                                    .child(words),
                            )
                        }),
                )
                .into_any_element();
        }
        let row = theme.row();
        let tops: Rc<[Pixels]> = (0..=shown.rows.len())
            .map(|at| theme::GAP_TINY + row * at as f32)
            .collect::<Vec<_>>()
            .into();
        let ids: Vec<_> = shown.rows.iter().map(|r| r.row.id).collect();
        let current = self
            .selected
            .and_then(|id| ids.iter().position(|i| *i == id));
        let usual = shown.usual;
        let me = cx.entity().downgrade();
        let busy = self.busy;
        let theme_owned = theme.clone();
        let rows_shown: Rc<Vec<(rimstack_core::mods::textures::Row, String)>> = Rc::new(
            shown
                .rows
                .iter()
                .map(|r: &RowShown<'_>| (r.row.clone(), put(&r.said)))
                .collect(),
        );
        let (selected, converts) = (self.selected, self.converts());
        let columns = Rc::new(columns);
        let list = rows(
            "texture-rows",
            self.scroll.clone(),
            tops.clone(),
            row,
            cx.entity_id(),
            move |range, width, window, _| {
                range
                    .filter_map(|at| {
                        Some(super::row::texture_row(
                            &theme_owned,
                            at,
                            rows_shown.get(at)?,
                            usual,
                            selected,
                            busy,
                            converts,
                            &columns,
                            width,
                            me.clone(),
                            window,
                        ))
                    })
                    .collect()
            },
        );
        let count_rows = ids.len();
        div()
            .id("textures")
            .track_focus(&self.focus)
            .role(Role::List)
            .aria_label(put(&say!("Mods with textures")))
            .relative()
            .flex_1()
            .min_h_0()
            .on_key_down(cx.listener(move |t, event: &KeyDownEvent, _, cx| {
                if count_rows == 0 {
                    return;
                }
                let now = current.unwrap_or(0);
                let to = match event.keystroke.key.as_str() {
                    "up" => now.saturating_sub(1),
                    "down" => (now + 1).min(count_rows - 1),
                    "home" => 0,
                    "end" => count_rows - 1,
                    _ => return,
                };
                t.selected = Some(ids[to]);
                t.scroll.borrow_mut().contain(tops[to], tops[to + 1]);
                cx.notify();
            }))
            .child(list)
            .child(bar(theme, self.scroll.clone(), cx.entity_id()))
            .into_any_element()
    }

    fn side(
        &self,
        shown: &TexturesShown<'_>,
        theme: &Theme,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let c = &theme.colours;
        let chosen = self
            .selected
            .and_then(|id| shown.rows.iter().find(|r| r.row.id == id));
        if chosen.is_none() && self.found.is_none() && self.spare.is_none() {
            return None;
        }
        let group = |text: String| {
            label(theme, theme.small(), c.ink2, text)
                .font_weight(theme.semibold)
                .pt(theme::GAP_WIDE)
                .pb(theme::GAP_SMALL)
        };
        let (busy, converts) = (self.busy, self.converts());
        let mut side = div()
            .flex()
            .flex_col()
            .h_full()
            .min_h_0()
            .p(theme::GAP_CARD);
        if let Some(chosen) = chosen {
            let m = chosen.row.clone();
            let said = put(&chosen.said).to_lowercase();
            let about = match m.active {
                true => put(&say_n!(
                    m.textures,
                    "{} texture, {}",
                    "{} textures, {}",
                    said
                )),
                false => put(&say_n!(
                    m.textures,
                    "{} texture, {}, not active",
                    "{} textures, {}, not active",
                    said
                )),
            };
            let files = |n: usize| put(&say_n!(n, "{} file", "{} files"));
            let made = [
                files(m.converted + m.out_of_date),
                if m.made_mb > 0.0 {
                    megabytes(m.made_mb)
                } else {
                    String::new()
                },
            ]
            .into_iter()
            .filter(|p| !p.is_empty())
            .collect::<Vec<_>>()
            .join(", ");
            let facts = [
                (put(&say!("Created by RimStack")), made, false),
                (put(&say!("Not made by RimStack")), files(m.shipped), false),
                (
                    put(&say!("Source PNG changed since")),
                    files(m.out_of_date),
                    m.out_of_date > 0,
                ),
                (put(&say!("No DDS yet")), files(m.not_converted), false),
            ];
            let id = m.id;
            side = side
                .child(div().sized_text(theme, theme.heading()).font_weight(theme.semibold).text_color(c.ink).child(m.name.clone()))
                .child(div().pt(theme::GAP_HAIR).sized_text(theme, theme.body()).text_color(c.ink2).child(about))
                .child(group(put(&say!("Converted files"))))
                .children(facts.into_iter().map(|(name, value, warn)| {
                    div()
                        .flex()
                        .items_center()
                        .h(theme.sized(24.))
                        .child(div().flex_1().sized_text(theme, theme.body()).text_color(if warn { c.warn } else { c.ink }).child(name))
                        .child(label(theme, theme.body(), c.ink2, value))
                }))
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap(theme::GAP_SMALL)
                        .pt(theme::GAP)
                        .when(converts, |d| d.child(
                            Btn::new("convert-chosen", if m.out_of_date > 0 { put(&say!("Rebuild")) } else { put(&say!("Convert")) })
                                .enabled(!busy && m.not_converted + m.out_of_date > 0)
                                .on_click(cx.listener(move |t, _, _, cx| t.work(vec![id], false, cx))),
                        ))
                        .child(
                            Btn::new("remove-chosen", put(&say!("Remove converted")))
                                .enabled(!busy && m.converted + m.out_of_date > 0)
                                .tip(put(&say!("Deletes only the .dds files RimStack made and that have not changed since.")))
                                .on_click(cx.listener(move |t, _, _, cx| t.work(vec![id], true, cx))),
                        )
                        .when(converts, |d| d.child(
                            Btn::new("skip-chosen", if m.skipped { put(&say!("Stop skipping")) } else { put(&say!("Skip this mod")) })
                                .enabled(!busy)
                                .on_click(cx.listener(move |t, _, _, cx| t.skip(id, !m.skipped, cx))),
                        )),
                );
        }
        if let Some((found, list)) = &self.found {
            let said = match found.len() {
                0 => put(&say!("None")),
                n => put(&say_n!(n, "{} file", "{} files")),
            };
            side = side
                .child(div().pt(theme::GAP_WIDE).pb(theme::GAP_SMALL).sized_text(theme, theme.small()).font_weight(theme.semibold).text_color(c.ink2).child(put(&say!("All mods: DDS files to set aside"))))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .min_h_0()
                        .gap(theme::GAP_SMALL)
                        .child(
                            div()
                                .flex()
                                .gap(theme::GAP)
                                .child(Btn::new("close-audit", put(&say!("Close"))).on_click(cx.listener(|t, _, _, cx| {
                                    t.found = None;
                                    cx.notify();
                                })))
                                .when(!found.is_empty(), |d| {
                                    d.child(
                                        Btn::new("refused-aside", put(&say!("Set all aside…")))
                                            .tip(put(&say!("Renames each to .dds.rimstack-bad, so the game loads the mod's own texture instead.")))
                                            .on_click(cx.listener(|t, _, window, cx| t.ask_refused_aside(window, cx))),
                                    )
                                }),
                        )
                        .child(label(theme, theme.body(), c.ink, said))
                        .child(list.draw(super::files::Which::Refused, theme, cx)),
                );
        }
        if let Some((spare, list)) = &self.spare {
            side = side.child(self.spare_found(spare, list, theme, cx));
        }
        let quality = crate::ui::tip::tipped(
            div().id("dds-quality").spaced_text(theme, theme.body(), 1.3).text_color(c.ink).child(put(&say!("BC7 with transparency, DXT1 without; with mipmaps"))),
            put(&say!("A .dds file is written beside each .png and the game loads it instead: quicker, and less memory. The .png is not changed. Steam may delete the .dds files when it updates a mod; they then show as not converted.")).into(),
        );
        if converts {
            side = side.child(group(put(&say!("Quality")))).child(quality);
        }
        Some(
            div()
                .flex_none()
                .w(Self::side_width(theme, window))
                .h_full()
                .bg(c.panel)
                .border_l_1()
                .border_color(c.edge)
                .overflow_hidden()
                .child(side)
                .into_any_element(),
        )
    }
}

impl Render for Textures {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.global::<Theme>().clone();
        let rows = self.rows.clone();
        let shown = shown(&rows, self.active_only, self.converts());
        let side = self.side(&shown, &theme, window, cx);
        let list_width = window.viewport_size().width
            - if side.is_some() {
                Self::side_width(&theme, window) + px(1.)
            } else {
                px(0.)
            };
        let columns = self.columns(&theme, list_width, window);
        let heading = self.heading(&columns, &theme);
        div()
            .flex()
            .size_full()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .child(self.summary(&shown, &theme, cx))
                    .children(self.confirm(&shown, &theme, cx))
                    .child(heading)
                    .child(self.list(&shown, columns, &theme, window, cx)),
            )
            .children(side)
    }
}

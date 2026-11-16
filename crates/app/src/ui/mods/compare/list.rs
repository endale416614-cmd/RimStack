use super::*;

impl CompareView {
    pub(super) fn list(
        &self,
        theme: &Theme,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let shown = self.listed.clone();
        let (chosen, me, drawn) = (self.chosen, cx.entity().downgrade(), theme.clone());
        let row = Self::row_height(theme);
        let width = theme.sized(290.) - theme::GAP_WIDE * 2. - theme::GAP_HAIR;
        let names: Vec<String> = shown
            .iter()
            .map(|t| {
                elide_start_in(
                    window,
                    theme,
                    theme.family.clone(),
                    &t.file,
                    theme.body(),
                    width,
                )
            })
            .collect();
        let list = rows(
            "compare-rows",
            self.scroll.clone(),
            self.tops(theme),
            row,
            cx.entity_id(),
            move |range, list_width, _, _| {
                let theme = &drawn;
                let c = &theme.colours;
                range
                    .filter_map(|at| {
                        let texture = shown.get(at)?;
                        let current = at == chosen;
                        let me = me.clone();
                        Some(
                            div()
                                .id(("compare-texture", at))
                                .relative()
                                .w(list_width)
                                .h(row)
                                .cursor_pointer()
                                .child(
                                    div()
                                        .absolute()
                                        .top_0()
                                        .bottom_0()
                                        .left(theme::GAP_SMALL)
                                        .right(theme::GAP_SMALL)
                                        .rounded(theme::CORNER)
                                        .when(current, |d| d.bg(c.chosen))
                                        .when(!current, |d| {
                                            d.hover(|d| d.bg(with_alpha(c.hover, 0.04)))
                                        }),
                                )
                                .child(
                                    div()
                                        .absolute()
                                        .left(theme::GAP_WIDE)
                                        .right(theme::GAP_WIDE + theme::GAP_HAIR)
                                        .top_0()
                                        .bottom_0()
                                        .flex()
                                        .flex_col()
                                        .justify_center()
                                        .child(label(
                                            theme,
                                            theme.body(),
                                            if current { c.on_selected } else { c.ink },
                                            names[at].clone(),
                                        ))
                                        .child(
                                            div()
                                                .overflow_hidden()
                                                .whitespace_nowrap()
                                                .text_ellipsis()
                                                .sized_text(theme, theme.small())
                                                .text_color(if current {
                                                    c.on_selected_dim
                                                } else {
                                                    c.ink3
                                                })
                                                .child(put(&texture.used)),
                                        ),
                                )
                                .on_click(move |_, window, cx| {
                                    let _ = me.update(cx, |view, cx| {
                                        view.chosen = at;
                                        view.difference = None;
                                        window.focus(&view.focus, cx);
                                        cx.notify();
                                    });
                                })
                                .into_any_element(),
                        )
                    })
                    .collect()
            },
        );
        div()
            .relative()
            .flex_1()
            .min_h_0()
            .child(list)
            .child(bar(theme, self.scroll.clone(), cx.entity_id()))
            .into_any_element()
    }

    pub(super) fn side(
        &self,
        theme: &Theme,
        texture: &Shared,
        earlier: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let c = &theme.colours;
        let kept = texture.keeps_earlier == earlier;
        let comparison = self.comparison.clone();
        let name = comparison
            .as_ref()
            .map(|c| {
                if earlier {
                    c.earlier.name.clone()
                } else {
                    c.later.name.clone()
                }
            })
            .unwrap_or_default();
        let file = if earlier {
            texture.earlier_shown.clone()
        } else {
            texture.later_shown.clone()
        };
        let found = self.pixels_of(&file);
        let facts: Vec<String> = [
            found.map(|(w, h)| format!("{w} × {h}")),
            Some(kilobytes(if earlier {
                texture.earlier_bytes
            } else {
                texture.later_bytes
            })),
            Some(put(&say!(
                "loads {}",
                ordinal(if earlier {
                    self.places.0
                } else {
                    self.places.1
                })
            ))),
        ]
        .into_iter()
        .flatten()
        .collect();
        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .gap(theme::GAP)
            .child(self.picture(
                theme,
                if earlier {
                    "compare-earlier"
                } else {
                    "compare-later"
                },
                file,
                kept,
                cx,
            ))
            .child(label(theme, theme.body(), c.ink, name).font_weight(theme.semibold))
            .child(label(theme, theme.body(), c.ink2, facts.join(", ")))
            .child(
                div()
                    .flex()
                    .items_center()
                    .h(theme.sized(26.))
                    .when(kept, |d| {
                        d.child(
                            label(theme, theme.body(), c.ok, put(&say!("✓ Kept")))
                                .font_weight(theme.medium),
                        )
                    })
                    .when(!kept, |d| {
                        d.child(
                            Btn::new(
                                ("keep-side", usize::from(earlier)),
                                put(&say!("Keep this one")),
                            )
                            .on_click(cx.listener(move |view, _, _, cx| view.keep(earlier, cx))),
                        )
                    }),
            )
            .into_any_element()
    }
}

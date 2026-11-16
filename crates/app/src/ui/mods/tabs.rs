use super::chosen::DetailsView;
use super::notes::Picking;
use crate::ui::button::Btn;
use crate::ui::card::group_title;
use crate::ui::theme::{self, Theme};
use crate::ui::widgets::{TextSized, breakable, elide_middle_in, label};
use crate::words::put;
use crate::workspace::load_order::Details;
use gpui::InteractiveElement as _;
use gpui::{
    AnyElement, Context, IntoElement, ParentElement, Pixels, Styled, Window, div,
    prelude::FluentBuilder, px,
};
use rimstack_core::base::sentence::Sentence;
use rimstack_core::mods::details::Named;
use rimstack_core::{say, say_n};

fn names(theme: &Theme, title: String, lead: String, named: &[Named]) -> Vec<AnyElement> {
    let c = &theme.colours;
    if named.is_empty() && lead.is_empty() {
        return Vec::new();
    }
    let mut shown = vec![group_title(theme, title)];
    if !lead.is_empty() {
        shown.push(
            div()
                .pb(theme::GAP_TINY)
                .sized_text(theme, theme.body())
                .text_color(c.ink3)
                .child(lead)
                .into_any_element(),
        );
    }
    shown.extend(named.iter().map(|one| {
        let note = put(&one.note);
        div()
            .flex()
            .items_center()
            .gap(theme::GAP)
            .min_h(theme.sized(24.))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_ellipsis()
                    .sized_text(theme, theme.body())
                    .text_color(c.ink)
                    .child(one.name.clone()),
            )
            .when(!note.is_empty(), |d| {
                d.child(match one.trouble {
                    true => label(theme, theme.small(), c.err, format!("! {note}")),
                    false => label(theme, theme.small(), c.ink3, note),
                })
            })
            .into_any_element()
    }));
    shown
}

fn text(theme: &Theme, colour: gpui::Hsla, said: String) -> AnyElement {
    div()
        .w_full()
        .spaced_text(theme, theme.body(), 1.3)
        .text_color(colour)
        .child(said)
        .into_any_element()
}

impl DetailsView {
    pub(super) fn placed(
        &self,
        theme: &Theme,
        details: &Details,
        _inner: Pixels,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let c = &theme.colours;
        let active = self.shown.selected_active;
        let mut shown = vec![group_title(theme, put(&say!("Position")))];
        let position = match (active, details.category) {
            (false, _) => put(&say!("Not active")),
            (true, Some(category)) => put(&say!(
                "{} of {}, in {}",
                self.shown.position,
                self.shown.active,
                Sentence::said(category, Vec::new())
            )),
            (true, None) => put(&say!("{} of {}", self.shown.position, self.shown.active)),
        };
        shown.push(
            div()
                .sized_text(theme, theme.body())
                .text_color(c.ink)
                .child(position)
                .into_any_element(),
        );
        shown.extend(names(
            theme,
            put(&say!("Requires")),
            String::new(),
            &details.requires,
        ));
        if active {
            shown.push(group_title(theme, put(&say!("Own rules"))));
            for (at, rule) in details.your_rules.iter().enumerate() {
                let (first, then) = (rule.first, rule.then);
                shown.push(
                    div()
                        .flex()
                        .items_center()
                        .gap(theme::GAP)
                        .h(theme.sized(26.))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_ellipsis()
                                .sized_text(theme, theme.body())
                                .text_color(c.ink)
                                .child(put(&rule.said)),
                        )
                        .child(
                            Btn::new(("remove-rule", at), "")
                                .icon("x")
                                .tip(put(&say!("Remove this rule")))
                                .on_click(
                                    self.on_mods(move |mods, cx| mods.remove_rule(first, then, cx)),
                                ),
                        )
                        .into_any_element(),
                );
            }
            if let Some(picking) = self.shown.picking {
                let how = match picking {
                    Picking::After => say!("after"),
                    Picking::Before => say!("before"),
                };
                shown.push(
                    div()
                        .pb(theme::GAP_SMALL)
                        .spaced_text(theme, theme.body(), 1.3)
                        .text_color(c.moved)
                        .child(put(&say!(
                            "Click the mod this one loads {} (Esc cancels)",
                            how.inside()
                        )))
                        .into_any_element(),
                );
            }
            let picking = self.shown.picking.is_some();
            shown.push(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(theme::GAP_SMALL)
                    .when(!picking, |d| {
                        d.child(
                            Btn::new("load-after", put(&say!("Load after…")))
                                .icon("plus")
                                .on_click(self.on_mods(|mods, cx| {
                                    mods.start_picking(Some(Picking::After), cx)
                                })),
                        )
                        .child(
                            Btn::new("load-before", put(&say!("Load before…")))
                                .icon("plus")
                                .on_click(self.on_mods(|mods, cx| {
                                    mods.start_picking(Some(Picking::Before), cx)
                                })),
                        )
                    })
                    .when(picking, |d| {
                        d.child(
                            Btn::new("cancel-picking", put(&say!("Cancel")))
                                .on_click(self.on_mods(|mods, cx| mods.start_picking(None, cx))),
                        )
                    })
                    .into_any_element(),
            );
        }
        let repeated = details.required_above;
        let lead = match repeated {
            0 => String::new(),
            n => put(&say_n!(
                n,
                "{} required, listed above",
                "{} required, listed above"
            )),
        };
        shown.extend(names(
            theme,
            put(&say!("Loads after")),
            lead,
            &details.loads_after,
        ));
        shown.extend(names(
            theme,
            put(&say!("Loads before")),
            String::new(),
            &details.loads_before,
        ));
        if !details.detected.is_empty() {
            shown.push(group_title(theme, put(&say!("Detected from files"))));
            for found in &details.detected {
                let why = match found.code {
                    true => put(&found.why),
                    false => put(&say!(
                        "{}  (a hint; the game does not need this order)",
                        found.why.inside()
                    )),
                };
                shown.push(
                    div()
                        .flex()
                        .flex_col()
                        .pb(theme::GAP_SMALL)
                        .child(
                            div()
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_ellipsis()
                                .sized_text(theme, theme.body())
                                .text_color(c.ink)
                                .child(put(&say!("After {}", found.name.clone()))),
                        )
                        .child(
                            div()
                                .sized_text(theme, theme.small())
                                .text_color(c.ink3)
                                .child(breakable(&why)),
                        )
                        .into_any_element(),
                );
            }
        }
        if let Some(placement) = &details.placement {
            shown.push(group_title(theme, put(&say!("Placement"))));
            shown.push(text(theme, c.ink, put(placement)));
        }
        let _ = cx;
        shown
    }

    pub(super) fn files(
        &self,
        theme: &Theme,
        details: &Details,
        inner: Pixels,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let c = &theme.colours;
        let mut shown = Vec::new();
        if !details.patches.is_empty() {
            shown.push(group_title(theme, put(&say!("Patched methods"))));
            for patched in &details.patches {
                let note = put(&patched.note);
                shown.push(
                    div()
                        .flex()
                        .flex_col()
                        .pb(theme::GAP_TINY)
                        .child(
                            div()
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .sized_text(theme, theme.body())
                                .text_color(c.ink)
                                .child(crate::ui::widgets::elide_start_in(
                                    window,
                                    theme,
                                    theme.family.clone(),
                                    &put(&patched.method),
                                    theme.body(),
                                    inner,
                                )),
                        )
                        .when(!note.is_empty(), |d| {
                            d.child(
                                div()
                                    .sized_text(theme, theme.small())
                                    .text_color(c.ink3)
                                    .child(note),
                            )
                        })
                        .into_any_element(),
                );
            }
        }
        shown.push(group_title(theme, put(&say!("Folder"))));
        let path = elide_middle_in(
            window,
            theme,
            theme.mono.clone(),
            &details.path,
            theme.body(),
            inner - theme.icon() - theme.sized(14.) - theme::GAP_SMALL,
        );
        let copied = details.path.clone();
        shown.push(
            div()
                .flex()
                .items_center()
                .gap(theme::GAP_SMALL)
                .child(crate::ui::tip::tipped(
                    div()
                        .id("files-folder")
                        .flex_1()
                        .min_w_0()
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .font_family(theme.mono.clone())
                        .sized_text(theme, theme.body())
                        .text_color(c.ink)
                        .child(path.clone()),
                    match path == details.path {
                        true => String::new(),
                        false => details.path.clone(),
                    }
                    .into(),
                ))
                .child(
                    Btn::new("files-copy-path", "")
                        .icon("copy")
                        .tip(put(&say!("Copy path")))
                        .on_click(move |_, _, cx| {
                            cx.write_to_clipboard(gpui::ClipboardItem::new_string(copied.clone()))
                        }),
                )
                .into_any_element(),
        );
        shown.push(div().h(px(8.)).into_any_element());
        let read = self.shown.files.clone();
        if let Some(read) = &read {
            for branch in &read.1.tree {
                let room = inner - theme.sized(80.);
                shown.push(
                    div()
                        .flex()
                        .items_center()
                        .h(theme.sized(21.))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .font_family(theme.mono.clone())
                                .sized_text(theme, theme.small())
                                .text_color(c.ink)
                                .child(elide_middle_in(
                                    window,
                                    theme,
                                    theme.mono.clone(),
                                    &branch.folder,
                                    theme.small(),
                                    room,
                                )),
                        )
                        .child(label(
                            theme,
                            theme.small(),
                            c.ink3,
                            put(&say_n!(branch.files, "{} file", "{} files")),
                        ))
                        .into_any_element(),
                );
            }
        }
        shown.push(group_title(
            theme,
            put(&say!("Folders the game loads from")),
        ));
        let folders: Vec<String> = details.load_folders.iter().map(put).collect();
        let folders = match folders.is_empty() {
            true => put(&say!("None")),
            false => folders.join("\n"),
        };
        shown.push(
            div()
                .spaced_text(theme, theme.body(), 1.3)
                .text_color(c.ink)
                .child(breakable(&folders))
                .into_any_element(),
        );
        let version = self.shown.game_version.clone();
        shown.push(
            div()
                .pt(theme::GAP_SMALL)
                .spaced_text(theme, theme.small(), 1.3)
                .text_color(c.ink2)
                .child(put(&say!("For RimWorld {} and the active mods", version)))
                .into_any_element(),
        );
        let hidden = read.as_ref().map_or(0, |read| read.2);
        if hidden > 0 {
            shown.push(group_title(theme, put(&say!("Textures set aside"))));
            shown.push(text(
                theme,
                c.ink,
                put(&say_n!(
                    hidden,
                    "{} texture set aside: renamed with .rimstack-hidden, so the game skips it.",
                    "{} textures set aside: renamed with .rimstack-hidden, so the game skips them."
                )),
            ));
            shown.push(
                div()
                    .flex()
                    .pt(theme::GAP_SMALL)
                    .child(
                        Btn::new("unhide-mod", put(&say!("Bring back all")))
                            .icon("eye-off")
                            .on_click(self.on_mods(|mods, cx| mods.unhide_chosen(cx))),
                    )
                    .into_any_element(),
            );
            if !self.shown.unhide_problem.is_empty() {
                shown.push(
                    div()
                        .sized_text(theme, theme.body())
                        .text_color(c.err)
                        .child(self.shown.unhide_problem.clone())
                        .into_any_element(),
                );
            }
        }
        let (textures, assemblies, dds) = read.as_ref().map_or((0, String::new(), 0), |read| {
            (read.0.count, read.1.assemblies.clone(), read.0.dds_made)
        });
        let contents: Vec<(Sentence, String)> = [
            (say!("Kinds of content"), put(&details.contains)),
            (say!("Assemblies"), assemblies),
            (
                say!("Textures"),
                if textures > 0 {
                    textures.to_string()
                } else {
                    String::new()
                },
            ),
            (
                say!("DDS made by RimStack"),
                if dds > 0 {
                    dds.to_string()
                } else {
                    String::new()
                },
            ),
        ]
        .into_iter()
        .filter(|(_, value)| !value.is_empty())
        .collect();
        if !contents.is_empty() {
            shown.push(group_title(theme, put(&say!("Contents"))));
            for (name, value) in contents {
                shown.push(
                    div()
                        .flex()
                        .gap(px(8.))
                        .pb(px(4.))
                        .child(label(theme, theme.body(), c.ink2, put(&name)).w(theme.sized(150.)))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .sized_text(theme, theme.body())
                                .text_color(c.ink)
                                .child(breakable(&value)),
                        )
                        .into_any_element(),
                );
            }
        }
        let _ = cx;
        shown
    }
}

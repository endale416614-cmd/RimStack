use super::chosen::{DetailsView, defined};
use super::edit::acting;
use crate::ui::button::{Btn, Chip};
use crate::ui::card::Card;
use crate::ui::field::Field;
use crate::ui::problem::problem;
use crate::ui::rich::{self, Link};
use crate::ui::theme::{self, Theme};
use crate::ui::widgets::{TextSized, elide_middle_in, icon, label, measure};
use crate::words::put;
use crate::workspace::load_order::Details;
use gpui::AnimationExt;
use gpui::InteractiveElement as _;
use gpui::{
    AnyElement, Context, FontWeight, IntoElement, ParentElement, Pixels, Styled, Window, div,
    prelude::FluentBuilder, px,
};
use rimstack_core::base::sentence::Sentence;
use rimstack_core::{say, say_n};
use std::rc::Rc;

impl DetailsView {
    fn words(theme: &Theme, colour: gpui::Hsla, text: String) -> AnyElement {
        div()
            .w_full()
            .sized_text(theme, theme.body())
            .text_color(colour)
            .child(text)
            .into_any_element()
    }

    fn line_with(theme: &Theme, mark: &'static str, text: String, button: Btn) -> AnyElement {
        let c = &theme.colours;
        div()
            .flex()
            .items_center()
            .gap(theme::GAP)
            .child(icon(mark, theme.icon(), c.ink3))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .sized_text(theme, theme.body())
                    .text_color(c.ink2)
                    .child(text),
            )
            .child(button)
            .into_any_element()
    }

    fn before_play(
        &self,
        theme: &Theme,
        details: &Details,
        inner: Pixels,
        window: &Window,
    ) -> Option<AnyElement> {
        let c = &theme.colours;
        let ignored = details.problems_ignored;
        let shown = !details.problems.is_empty()
            || details.unreadable.is_some()
            || details.version_ok
            || ignored > 0;
        if !shown {
            return None;
        }
        let errors = details.problems.iter().any(|p| p.error) || details.unreadable.is_some();
        let mut lines: Vec<AnyElement> = details
            .problems
            .iter()
            .enumerate()
            .map(|(at, said)| {
                problem(
                    theme,
                    at,
                    said,
                    Some((inner, window)),
                    (!self.shown.previewing).then(|| acting(self.mods.clone(), said)),
                )
            })
            .collect();
        let alike = self.shown.overview.as_ref().map_or(0, |o| o.version_only);
        if alike > 1 && details.offer_trust && !self.shown.previewing {
            let version = self.shown.game_version.clone();
            lines.push(
                div()
                    .flex()
                    .child(
                        Btn::new("ignore-all-version", put(&say!("Ignore all {} for RimWorld {}", alike, version.clone())))
                            .tip(put(&say!("No warning about the game version while RimWorld stays on this version.")))
                            .on_click(self.on_mods(move |mods, cx| mods.set_version_aside(version.clone(), cx))),
                    )
                    .into_any_element(),
            );
        }
        if let Some(unreadable) = &details.unreadable {
            lines.push(Self::words(
                theme,
                c.err,
                put(&say!("Unreadable: {}", unreadable.inside())),
            ));
        }
        if details.version_ok {
            let made_for = details.made_for.clone().unwrap_or_default();
            lines.push(Self::line_with(
                theme,
                "eye-off",
                put(&say!(
                    "Made for RimWorld {}; the warning is ignored.",
                    made_for
                )),
                Btn::new("trust-again", put(&say!("Warn again"))).on_click(self.on_mods(
                    |mods, cx| {
                        let ids = mods.selected_id(cx).into_iter().collect();
                        mods.trust(ids, false, cx)
                    },
                )),
            ));
        }
        if ignored > 0 {
            lines.push(Self::line_with(
                theme,
                "eye-off",
                put(&say_n!(
                    ignored,
                    "{} problem is ignored.",
                    "{} problems are ignored."
                )),
                Btn::new("warn-again", put(&say!("Warn again")))
                    .on_click(self.on_mods(|mods, cx| mods.warn_again(cx))),
            ));
        }
        Some(
            Card::new("about-problems", put(&say!("Before you play")))
                .tone(if errors { c.err } else { c.warn })
                .spacing(theme::GAP)
                .body(lines)
                .render(theme, None)
                .into_any_element(),
        )
    }

    fn clashes(
        &self,
        theme: &Theme,
        details: &Details,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let c = &theme.colours;
        let all: Vec<_> = details
            .loses_to
            .iter()
            .map(|clash| (true, clash))
            .chain(details.wins_over.iter().map(|clash| (false, clash)))
            .collect();
        if all.is_empty() {
            return None;
        }
        let count = all.len();
        let lines = all.into_iter().enumerate().map(|(at, (loses, clash))| {
            let other = clash.mod_id;
            let chip = match loses {
                true => Chip::new(put(&say!("loses {}", clash.textures)))
                    .tone(c.warn)
                    .icon("arrow-down"),
                false => Chip::new(put(&say!("wins {}", clash.textures)))
                    .tone(c.ok)
                    .icon("arrow-up"),
            };
            div()
                .flex()
                .flex_col()
                .gap(theme::GAP_TINY)
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(theme::GAP_SMALL)
                        .child(chip)
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_ellipsis()
                                .sized_text(theme, theme.body())
                                .text_color(c.ink)
                                .child(clash.name.clone()),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .gap(theme::GAP_SMALL)
                        .child(
                            Btn::new(("compare", at), put(&say!("Compare")))
                                .tip(put(&say!(
                                    "Both mods' versions of each shared texture, side by side"
                                )))
                                .on_click(
                                    self.on_mods(move |mods, cx| mods.compare_with(other, cx)),
                                ),
                        )
                        .when(loses, |d| {
                            d.child(
                                Btn::new(("win-all", at), put(&say!("Win all")))
                                    .enabled(!self.shown.previewing)
                                    .tip(put(&say!(
                                        "Adds a rule of your own: this mod loads after the other."
                                    )))
                                    .on_click(
                                        self.on_mods(move |mods, cx| mods.win_over(other, cx)),
                                    ),
                            )
                        }),
                )
                .into_any_element()
        });
        let lines: Vec<AnyElement> = lines.collect();
        let (folded, fold) = self.fold("about-conflicts", false, cx);
        Some(
            Card::new("about-conflicts", put(&say!("Texture conflicts")))
                .tone(c.warn)
                .note(count.to_string())
                .spacing(theme::GAP)
                .folds(folded, fold)
                .body(lines)
                .render(theme, None)
                .into_any_element(),
        )
    }

    fn replaced(&self, theme: &Theme, details: &Details) -> Option<AnyElement> {
        let c = &theme.colours;
        let by = details.replaced_by.clone()?;
        let said = match &details.replaced_by_versions {
            Some(versions) => put(&say!("{}, for RimWorld {}", by, versions.clone())),
            None => by,
        };
        let id = details.replaced_by_id.clone().unwrap_or_default();
        let active = self.shown.selected_active && !self.shown.previewing;
        let body = vec![
            div()
                .flex()
                .items_center()
                .gap(theme::GAP)
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .spaced_text(theme, theme.body(), 1.3)
                        .text_color(c.ink)
                        .child(said),
                )
                .child(
                    Btn::new("replacement-open", put(&say!("Open")))
                        .icon("external-link")
                        .on_click(move |_, _, cx| cx.open_url(&crate::ui::workshop_page(&id))),
                )
                .into_any_element(),
            div()
                .flex()
                .gap(theme::GAP_SMALL)
                .when(active, |d| {
                    d.child(
                        Btn::new("deactivate-replaced", put(&say!("Deactivate this one")))
                            .icon("minus")
                            .on_click(self.on_mods(|mods, cx| mods.toggle_selected(cx))),
                    )
                })
                .when(details.workshop, |d| {
                    d.child(
                        Btn::new("unsubscribe-replaced", put(&say!("Unsubscribe…")))
                            .danger()
                            .on_click({
                                let mods = self.mods.clone();
                                move |_, window, cx| {
                                    let _ = mods.update(cx, |mods, cx| {
                                        if let Some(id) = mods.selected_id(cx) {
                                            mods.ask_unsubscribe(id, window, cx)
                                        }
                                    });
                                }
                            }),
                    )
                })
                .into_any_element(),
        ];
        Some(
            Card::new("about-replaced", put(&say!("Use this instead")))
                .spacing(theme::GAP)
                .body(body)
                .render(theme, None)
                .into_any_element(),
        )
    }

    fn copies(&self, theme: &Theme, details: &Details) -> Option<AnyElement> {
        let c = &theme.colours;
        let copies = details.copies.as_ref()?;
        let mut body = vec![
            div()
                .spaced_text(theme, theme.body(), 1.3)
                .text_color(c.ink)
                .child(put(copies))
                .into_any_element(),
        ];
        if let Some(local) = &details.bin_local_copy {
            let (name, path, id) = (details.name.clone(), local.path.clone(), local.mod_id);
            let mods = self.mods.clone();
            body.push(
                div()
                    .flex()
                    .child(
                        Btn::new("bin-local", put(&say!("Move the local copy to the bin…")))
                            .danger()
                            .icon("trash-2")
                            .tip(put(&say!(
                                "The Workshop copy is then the one the game uses."
                            )))
                            .on_click(move |_, window, cx| {
                                let (name, path) = (name.clone(), path.clone());
                                let _ = mods.update(cx, |mods, cx| {
                                    mods.ask_bin(&name, &path, id, window, cx)
                                });
                            }),
                    )
                    .into_any_element(),
            );
        }
        Some(
            Card::new("about-copies", put(&say!("Two copies")))
                .spacing(theme::GAP)
                .body(body)
                .render(theme, None)
                .into_any_element(),
        )
    }

    fn saved_mark(&self, theme: &Theme) -> Vec<AnyElement> {
        let saved = self.shown.notes_saved;
        vec![
            label(theme, theme.small(), theme.colours.ok, put(&say!("Saved")))
                .with_spring(
                    "saved-fade",
                    crate::ui::motion::towards(
                        gpui::AnimationPhase(f32::from(u8::from(saved))),
                        0.01,
                        crate::ui::motion::QUICK,
                    ),
                    |label, shown| label.opacity(shown.0.clamp(0., 1.)),
                )
                .into_any_element(),
        ]
    }

    fn facts(
        &self,
        theme: &Theme,
        details: &Details,
        inner: Pixels,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let c = &theme.colours;
        let labels = [
            say!("Contains"),
            say!("Code"),
            say!("Load time"),
            say!("Package id"),
            say!("Folder"),
            say!("Filed under"),
            say!("Group"),
        ];
        let label_width = labels
            .iter()
            .map(|name| measure(window, theme, &put(name), theme.body(), FontWeight::NORMAL))
            .fold(px(0.), Pixels::max)
            + theme::GAP_SMALL
            + theme.sized(12.)
            + theme::GAP;
        let textures = self.shown.files.as_ref().map_or(0, |read| read.0.count);
        let contains = [
            put(&details.contains),
            match textures {
                0 => String::new(),
                n => put(&say_n!(n, "{} texture", "{} textures")),
            },
        ]
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("; ");
        let facts = [
            ("fact-contains", say!("Contains"), contains),
            ("fact-code", say!("Code"), put(&details.code)),
            ("fact-load", say!("Load time"), details.load_time.clone()),
            (
                "fact-package",
                say!("Package id"),
                details.package_id.clone(),
            ),
        ];
        let mut lines: Vec<AnyElement> = facts
            .into_iter()
            .filter(|(_, _, value)| !value.is_empty())
            .map(|(key, name, value)| {
                let value = self.copyable(key, &value, theme.body(), FontWeight::NORMAL, c.ink, cx);
                div()
                    .flex()
                    .gap(theme::GAP_TINY)
                    .child(label(theme, theme.body(), c.ink2, put(&name)).w(label_width))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .sized_text(theme, theme.body())
                            .text_color(c.ink)
                            .child(value),
                    )
                    .into_any_element()
            })
            .collect();
        if !details.path.is_empty() {
            let button = theme.icon() + theme.sized(14.);
            let room = inner - label_width - theme::GAP - (button + theme::GAP_SMALL) * 2.;
            let shown = elide_middle_in(
                window,
                theme,
                theme.mono.clone(),
                &details.path,
                theme.small(),
                room,
            );
            let (copied, opened) = (details.path.clone(), details.path.clone());
            lines.push(
                div()
                    .flex()
                    .items_center()
                    .gap(theme::GAP)
                    .child(label(theme, theme.body(), c.ink2, put(&say!("Folder"))).w(label_width))
                    .child(
                        div()
                            .flex()
                            .flex_1()
                            .min_w_0()
                            .items_center()
                            .gap(theme::GAP_SMALL)
                            .child(crate::ui::tip::tipped(
                                div()
                                    .id("facts-folder")
                                    .flex_1()
                                    .min_w_0()
                                    .overflow_hidden()
                                    .whitespace_nowrap()
                                    .font_family(theme.mono.clone())
                                    .sized_text(theme, theme.small())
                                    .text_color(c.ink)
                                    .child(shown.clone()),
                                match shown == details.path {
                                    true => String::new(),
                                    false => details.path.clone(),
                                }
                                .into(),
                            ))
                            .child(
                                Btn::new("copy-path", "")
                                    .icon("copy")
                                    .tip(put(&say!("Copy path")))
                                    .on_click(move |_, _, cx| {
                                        cx.write_to_clipboard(gpui::ClipboardItem::new_string(
                                            copied.clone(),
                                        ))
                                    }),
                            )
                            .child(
                                Btn::new("open-path", "")
                                    .icon("folder-open")
                                    .tip(put(&say!("Open folder")))
                                    .on_click(move |_, _, cx| {
                                        cx.open_with_system(std::path::Path::new(&opened))
                                    }),
                            ),
                    )
                    .into_any_element(),
            );
        }
        let category = details.category.unwrap_or_default();
        let at = theme::CATEGORY_NAMES
            .iter()
            .position(|name| *name == category);
        let filed = put(&Sentence::said(category, Vec::new()));
        let filed = match details.category_chosen.is_empty() {
            true => filed,
            false => put(&say!("{} (chosen)", filed)),
        };
        let mods = self.mods.clone();
        lines.push(
            div()
                .flex()
                .items_center()
                .gap(theme::GAP)
                .child(defined(theme, put(&say!("Filed under"))).w(label_width))
                .child(icon(
                    at.and_then(|at| theme::CATEGORY_ICONS.get(at)).copied().unwrap_or("package"),
                    theme.icon(),
                    at.map_or(c.ink2, |at| theme.category_colour(at)),
                ))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_ellipsis()
                        .sized_text(theme, theme.body())
                        .text_color(c.ink)
                        .child(filed),
                )
                .child(
                    Btn::new("file-under", put(&say!("Change")))
                        .tip(put(&say!("RimStack picks a mod's category from how other mods relate to it, what its folders hold and what its code does. A category chosen here is used instead.")))
                        .pops(move |below, window, cx| {
                            let at = gpui::point(below.left(), below.bottom() + px(2.));
                            let _ = mods.update(cx, |mods, cx| mods.category_choices(at, window, cx));
                        }),
                )
                .into_any_element(),
        );
        let held = put(&details.held);
        if !held.is_empty() {
            lines.push(
                div()
                    .pl(label_width + theme::GAP)
                    .sized_text(theme, theme.body())
                    .text_color(c.ink2)
                    .child(held)
                    .into_any_element(),
            );
        }
        lines.push(
            div()
                .flex()
                .items_center()
                .gap(theme::GAP)
                .child(defined(theme, put(&say!("Group"))).w(label_width))
                .child(
                    div().flex_1().min_w_0().child(
                        Field {
                            id: "group-field",
                            text: &self.group,
                            icon: "",
                            placeholder: put(&say!("A tag to group under")),
                            found: None,
                            keys: None,
                        }
                        .render(theme, window, cx),
                    ),
                )
                .into_any_element(),
        );
        let (folded, fold) = self.fold("about-facts", false, cx);
        Card::new("about-facts", put(&say!("Facts")))
            .actions(self.saved_mark(theme), theme.sized(60.))
            .folds(folded, fold)
            .tall()
            .body(lines)
            .render(theme, None)
            .into_any_element()
    }

    fn notes(
        &self,
        theme: &Theme,
        details: &Details,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let c = &theme.colours;
        let tags: Vec<String> = details
            .tags
            .split(',')
            .map(|tag| tag.trim().to_string())
            .filter(|tag| !tag.is_empty())
            .collect();
        let group_meaning = crate::ui::card::meaning(&put(&say!("Group"))).unwrap_or_default();
        let chips = tags.into_iter().enumerate().map(|(at, tag)| {
            let chip = match at {
                0 => Chip::new(put(&say!("{}  group", tag.clone())))
                    .tone(c.moved)
                    .icon("layers")
                    .tip(group_meaning.clone()),
                _ => Chip::new(tag.clone())
                    .tone(c.ink2)
                    .tip(put(&say!("Group under this tag: it becomes the first"))),
            };
            match at {
                0 => chip.into_any_element(),
                _ => chip
                    .on_click(
                        ("tag-chip", at),
                        self.on_mods(move |mods, cx| {
                            let ids = mods.selected_id(cx).into_iter().collect();
                            mods.group(ids, &tag, cx)
                        }),
                    )
                    .into_any_element(),
            }
        });
        let field = |id: &'static str, text, placeholder: String| {
            Field {
                id,
                text,
                icon: "",
                placeholder,
                found: None,
                keys: None,
            }
            .render(theme, window, cx)
        };
        let chips: Vec<AnyElement> = chips.collect();
        let mut body = vec![
            field(
                "tags-field",
                &self.tags,
                put(&say!("Tags, separated by commas")),
            )
            .into_any_element(),
        ];
        if !chips.is_empty() {
            body.push(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(theme::GAP_SMALL)
                    .children(chips)
                    .into_any_element(),
            );
        }
        body.push(
            Field {
                id: "note-field",
                text: &self.note,
                icon: "",
                placeholder: put(&say!("Note")),
                found: None,
                keys: None,
            }
            .area(theme, window, cx)
            .into_any_element(),
        );
        let (folded, fold) = self.fold("about-notes", false, cx);
        Card::new("about-notes", put(&say!("Tags and note")))
            .actions(self.saved_mark(theme), theme.sized(60.))
            .folds(folded, fold)
            .tall()
            .body(body)
            .render(theme, None)
            .into_any_element()
    }

    pub(super) fn about(
        &self,
        theme: &Theme,
        details: &Details,
        inner: Pixels,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let mut shown = Vec::new();
        shown.extend(self.before_play(theme, details, inner, window));
        if !details.description_blocks.is_empty() {
            let link: Link = Rc::new(|url, _, cx| cx.open_url(url));
            let text =
                self.description
                    .holding(
                        div()
                            .flex()
                            .flex_col()
                            .gap(theme::GAP_SMALL)
                            .children(rich::blocks(
                                "mod-description",
                                theme,
                                &details.description_blocks,
                                &link,
                                Some(&self.description),
                            )),
                    );
            shown.push(
                Card::new("about-description", put(&say!("Description")))
                    .body(vec![text.into_any_element()])
                    .render(theme, None)
                    .into_any_element(),
            );
        }
        shown.extend(self.clashes(theme, details, cx));
        shown.extend(self.replaced(theme, details));
        shown.extend(self.copies(theme, details));
        shown.push(self.facts(theme, details, inner, window, cx));
        shown.push(self.notes(theme, details, window, cx));
        shown.extend(self.latest(theme, details, window, cx));
        shown
    }
}

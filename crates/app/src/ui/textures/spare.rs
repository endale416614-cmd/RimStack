use super::Textures;
use super::files::{FileList, Which};
use crate::ui::button::Btn;
use crate::ui::theme::{self, Theme};
use crate::ui::widgets::{TextSized, label};
use crate::words::put;
use crate::workspace::textures::SpareFound;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, div, prelude::FluentBuilder};
use rimstack_core::base::sizes;
use rimstack_core::{say, say_n};

impl Textures {
    pub(super) fn spare_found(
        &self,
        found: &SpareFound,
        list: &FileList,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let c = &theme.colours;
        let said = match list.files() {
            0 => put(&say!("None")),
            n => put(&say_n!(
                n,
                "{} file, {}",
                "{} files, {}",
                sizes::of_bytes(found.bytes()).inside()
            )),
        };
        let heading = put(&say!("Workshop mods: DDS files that repeat their PNG"));
        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .gap(theme::GAP_SMALL)
            .child(
                div()
                    .pt(theme::GAP_WIDE)
                    .sized_text(theme, theme.small())
                    .font_weight(theme.semibold)
                    .text_color(c.ink2)
                    .child(heading),
            )
            .child(
                div()
                    .flex()
                    .gap(theme::GAP)
                    .child(
                        Btn::new("close-spare", put(&say!("Close"))).on_click(cx.listener(
                            |t, _, _, cx| {
                                t.spare = None;
                                cx.notify();
                            },
                        )),
                    )
                    .when(found.files() > 0, |d| {
                        d.child(
                            Btn::new("delete-spare", put(&say!("Delete them…")))
                                .danger()
                                .enabled(!self.busy)
                                .on_click(
                                    cx.listener(|t, _, window, cx| t.ask_delete_spare(window, cx)),
                                ),
                        )
                    }),
            )
            .child(label(theme, theme.body(), c.ink, said))
            .child(list.draw(Which::Spare, theme, cx))
            .into_any_element()
    }
}

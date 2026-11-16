use super::super::Mods;
use super::super::chosen::DetailsView;
use crate::ui::button::Btn;
use crate::ui::card::group_title;
use crate::ui::theme::{self, Theme};
use crate::ui::widgets::{TextSized, label};
use crate::words::put;
use crate::workspace::textures::Comparison;
use gpui::{AnyElement, Context, InteractiveElement, IntoElement, ParentElement, Styled, div};
use rimstack_core::{say, say_n};

impl DetailsView {
    pub(in crate::ui::mods) fn compare_summary(
        &self,
        theme: &Theme,
        comparison: &Comparison,
    ) -> AnyElement {
        let c = &theme.colours;
        let (earlier, later) = (
            comparison.earlier.name.clone(),
            comparison.later.name.clone(),
        );
        let tally = [
            (
                put(&say!("{} kept", later.clone())),
                (comparison.total - comparison.kept_earlier).to_string(),
            ),
            (
                put(&say!("{} kept", earlier.clone())),
                comparison.kept_earlier.to_string(),
            ),
            (
                put(&say!("Chosen by hand")),
                put(&say!("{} of {}", comparison.yours, comparison.total)),
            ),
        ];
        let rows = tally.into_iter().map(|(name, value)| {
            div()
                .flex()
                .items_center()
                .h(theme.sized(24.))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_ellipsis()
                        .sized_text(theme, theme.body())
                        .text_color(c.ink)
                        .child(name),
                )
                .child(label(theme, theme.body(), c.ink2, value))
        });
        let total = comparison.total;
        div()
            .flex()
            .flex_col()
            .child(label(theme, theme.heading(), c.ink, put(&say!("Shared textures"))).font_weight(theme.semibold))
            .child(div().pt(theme::GAP_HAIR).sized_text(theme, theme.body()).text_color(c.ink2).child(put(&say!("{} and {}", earlier.clone(), later.clone()))))
            .child(group_title(theme, put(&say!("So far"))))
            .children(rows)
            .child(group_title(theme, put(&say!("All at once"))))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(theme::GAP_SMALL)
                    .child(Btn::new("keep-all-later", put(&say!("Keep all from {}", later.clone()))).on_click(self.on_mods(|mods, cx| mods.keep_all_compared(false, cx))))
                    .child(
                        Btn::new("keep-all-earlier", put(&say!("Keep all from {}", earlier)))
                            .tip(put(&say_n!(total, "Sets aside {} file in {} by renaming it; Undo renames it back.", "Sets aside {} files in {} by renaming them; Undo renames them back.", later.clone())))
                            .on_click(self.on_mods(|mods, cx| mods.keep_all_compared(true, cx))),
                    ),
            )
            .child(group_title(theme, put(&say!("Default"))))
            .child(crate::ui::tip::tipped(
                div().id("compare-default").sized_text(theme, theme.body()).text_color(c.ink).child(put(&say!("{} wins: it loads later.", later))),
                put(&say!("Choosing against the load order sets the losing file aside: it is renamed inside its mod, with .rimstack-hidden added, so the game skips it. Nothing is deleted; choosing the other version renames it back. Steam restores renamed files when it updates a mod.")).into(),
            ))
            .into_any_element()
    }
}

impl Mods {
    pub(in crate::ui::mods) fn keep_all_compared(&mut self, earlier: bool, cx: &mut Context<Self>) {
        if let Some(view) = self.comparing.clone() {
            view.update(cx, |view, cx| view.keep_all(earlier, cx));
        }
    }
}

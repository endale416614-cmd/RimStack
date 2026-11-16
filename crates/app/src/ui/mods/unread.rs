use super::Mods;
use crate::ui::button::Btn;
use crate::ui::theme::{self, Theme};
use crate::ui::widgets::{TextSized, label};
use crate::words::put;
use gpui::{
    Context, ParentElement, PathPromptOptions, Styled, Window, div, prelude::FluentBuilder,
};
use rimstack_core::say;

impl Mods {
    /// What `unread` shows beside the problem, read when the mods fail to be read or the
    /// settings change: it looks at the disk.
    pub(super) fn judge_unread(&mut self, cx: &mut Context<Self>) {
        let order = &self.list.read(cx).order;
        if self.demo || order.problem.is_empty() || order.is_installed() {
            self.why_unread = None;
            return;
        }
        let app = crate::ui::app(cx);
        let no_folders = app.settings_report().no_mod_folders;
        let chosen = app.settings().get("game").unwrap_or_default().to_string();
        let standing = (!chosen.is_empty())
            .then(|| crate::workspace::judge_folder("game", &chosen))
            .filter(|verdict| !verdict.ok)
            .map(|verdict| verdict.said);
        self.why_unread = Some((no_folders, standing));
    }

    /// Why nothing could be read; `None` while the mods are read or were.
    pub(super) fn unread(&self, theme: &Theme, cx: &mut Context<Self>) -> Option<gpui::Div> {
        let order = &self.list.read(cx).order;
        if self.demo || order.problem.is_empty() || order.is_installed() {
            return None;
        }
        let c = &theme.colours;
        let problem = put(&order.problem);
        let (no_folders, standing) = self.why_unread.clone()?;
        let standing = standing.map(|said| put(&said));
        let text = |colour, said: String| {
            div()
                .w_full()
                .text_center()
                .spaced_text(theme, theme.body(), 1.3)
                .text_color(colour)
                .child(said)
        };
        Some(
            div()
                .flex()
                .flex_1()
                .min_h_0()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .items_center()
                        .gap(theme::GAP_SMALL)
                        .w(theme.sized(520.))
                        .max_w_full()
                        .px(theme.sized(40.))
                        .child(
                            label(
                                theme,
                                theme.title(),
                                c.ink,
                                put(&say!("Mods could not be read")),
                            )
                            .font_weight(theme.semibold),
                        )
                        .when(!no_folders, |d| d.child(text(c.ink2, problem.clone())))
                        .when_some(
                            standing.filter(|_| self.status.folder_problem.is_empty()),
                            |d, said| d.child(text(c.err, said)),
                        )
                        .when(!self.status.folder_problem.is_empty(), |d| {
                            d.child(text(c.err, self.status.folder_problem.clone()))
                        })
                        .when(no_folders, |d| {
                            d.child(
                                div()
                                    .flex()
                                    .gap(theme::GAP)
                                    .child(
                                        Btn::new(
                                            "choose-game",
                                            put(&say!("Choose the game folder…")),
                                        )
                                        .primary()
                                        .icon("folder-open")
                                        .on_click(
                                            cx.listener(|mods, _, window, cx| {
                                                mods.choose_game_folder(window, cx)
                                            }),
                                        ),
                                    )
                                    .child(
                                        Btn::new("unread-settings", put(&say!("Settings")))
                                            .on_click(cx.listener(|_, _, _, cx| {
                                                crate::ui::navigate::go(
                                                    crate::ui::navigate::Go::Section(
                                                        crate::ui::section::Section::Settings,
                                                    ),
                                                    cx,
                                                )
                                            })),
                                    ),
                            )
                            .child(
                                div()
                                    .w_full()
                                    .text_center()
                                    .sized_text(theme, theme.small())
                                    .text_color(c.ink3)
                                    .child(problem),
                            )
                        }),
                ),
        )
    }

    /// The game folder chosen in the system's dialog, kept when it is the game's.
    fn choose_game_folder(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let chosen = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some(put(&say!("Choose the game folder…")).into()),
        });
        cx.spawn_in(window, async move |mods, cx| {
            let Ok(Ok(Some(paths))) = chosen.await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            let typed = path.to_string_lossy().into_owned();
            let verdict = crate::workspace::judge_folder("game", &typed);
            let _ = mods.update(cx, |mods, cx| {
                mods.status.folder_problem = match verdict.ok {
                    true => crate::words::why_not(&crate::ui::app(cx).keep_setting("game", &typed)),
                    false => put(&verdict.said),
                };
                cx.notify();
            });
        })
        .detach();
    }
}

use super::button::Btn;
use super::lists::Which;
use super::shell::Shell;
use super::theme::{self, Theme};
use super::widgets::{icon, label};
use crate::ui::widgets::TextSized;
use crate::words::put;
use gpui::{
    AnyElement, Context, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled, div, prelude::FluentBuilder, px,
};
use rimstack_core::say;

impl Shell {
    /// The work under way across the window, each with how it is stopped when it can be.
    fn tasks(&self, cx: &gpui::App) -> Vec<(String, Option<Stop>)> {
        let around = super::around::around(cx).read(cx);
        let mut tasks: Vec<(String, Option<Stop>)> = around
            .mods_work
            .iter()
            .chain(&around.downloads_work)
            .map(|said| (said.clone(), None))
            .collect();
        if around.community_busy {
            tasks.push((put(&say!("Downloading community data…")), None));
        }
        if let Some(said) = &around.textures_work
            && let Some(textures) = self.sections.made::<super::textures::Textures>()
        {
            let stop = textures.read(cx).stop.clone();
            let stop: Stop =
                std::rc::Rc::new(move || stop.store(true, std::sync::atomic::Ordering::Relaxed));
            tasks.push((said.clone(), Some(stop)));
        }
        tasks
    }

    pub(crate) fn status_bar(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.global::<Theme>().clone();
        let c = theme.colours;
        let height = theme.strip() - px(2.);
        let tasks = self.tasks(cx);
        let lists = self.lists.clone();
        let tray = (!tasks.is_empty() || self.tray_open || self.tray_fading).then(|| {
            let said = match tasks.len() {
                0 => String::new(),
                1 => tasks[0].0.clone(),
                n => put(&say!("{} tasks running", n)),
            };
            div()
                .id("tray")
                .relative()
                .flex()
                .items_center()
                .gap(theme::GAP_SMALL)
                .cursor_pointer()
                .role(gpui::Role::Button)
                .aria_label(put(&say!("Background work")))
                .child(self.spinner.clone())
                .child(label(&theme, theme.small(), c.ink, said))
                .on_click(cx.listener(|shell, _, _, cx| match shell.tray_open {
                    true => shell.close_tray(cx),
                    false => {
                        shell.tray_open = true;
                        shell.tray_fading = false;
                        shell.tray_turn = super::motion::opening();
                        cx.notify();
                    }
                }))
                .when(self.tray_open || self.tray_fading, |d| {
                    d.child(self.tray_popup(&theme, &tasks, cx))
                })
        });
        let release = self.release.clone().map(|(version, page)| {
            Btn::new("get-release", put(&say!("Get {}", version)))
                .icon("external-link")
                .height(height)
                .tip(put(&say!("A newer version is published; opens its page")))
                .on_click(move |_, _, cx| cx.open_url(&page))
        });
        div()
            .flex()
            .flex_none()
            .items_center()
            .w_full()
            .h(theme.strip())
            .pl(theme::GAP_CARD)
            .pr(theme::GAP_MEDIUM)
            .bg(c.bar)
            .border_t_1()
            .border_color(c.edge)
            .child(self.note.clone())
            .child(
                div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .gap(theme::GAP_LARGE)
                    .children(tray)
                    .children(release)
                    .child(
                        Btn::new("support", "")
                            .icon("heart")
                            .icon_tone(c.err)
                            .height(height)
                            .tip(put(&say!("Support RimStack")))
                            .pops(|over, window, cx| {
                                super::menu::open_above(over, super::support::menu(), window, cx)
                            }),
                    )
                    .child(
                        Btn::new("keys", put(&say!("? Keys")))
                            .height(height)
                            .spoken(put(&say!("Keyboard shortcuts (F1)")))
                            .tip(put(&say!("Keyboard shortcuts (F1)")))
                            .on_click(
                                cx.listener(|shell, _, window, cx| shell.open_keys(window, cx)),
                            ),
                    )
                    .when(!self.demo, |d| {
                        d.child(
                            Btn::new("history", put(&say!("History")))
                                .icon("history")
                                .height(height)
                                .tip(put(&say!("What RimStack changed, and how to take it back")))
                                .pops(move |below, window, cx| {
                                    lists.update(cx, |lists, cx| {
                                        lists.toggle(Which::History, below, window, cx)
                                    })
                                }),
                        )
                    }),
            )
            .into_any_element()
    }

    fn tray_popup(
        &self,
        theme: &Theme,
        tasks: &[(String, Option<Stop>)],
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let c = &theme.colours;
        let rows = tasks.iter().enumerate().map(|(at, (said, stop))| {
            div()
                .flex()
                .items_center()
                .gap(theme::GAP)
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .sized_text(theme, theme.body())
                        .text_color(c.ink)
                        .child(said.clone()),
                )
                .when_some(stop.clone(), |d, stop| {
                    d.child(
                        Btn::new(("tray-cancel", at), put(&say!("Cancel")))
                            .tip(put(&say!(
                                "Stops after the current mod; what is already done is kept"
                            )))
                            .on_click(move |_, _, _| stop()),
                    )
                })
        });
        let popup = div()
            .id("tray-popup")
            .absolute()
            .bottom(theme.strip())
            .right_0()
            .w(theme.sized(380.))
            .p(theme::GAP_LARGE)
            .flex()
            .flex_col()
            .gap(theme::GAP)
            .rounded(theme::CORNER_LARGE)
            .bg(c.bar)
            .border_1()
            .border_color(c.line)
            .when(!self.tray_fading, |d| d.occlude())
            .when(self.tray_open, |d| {
                d.on_mouse_down_out(cx.listener(|shell, _, _, cx| shell.close_tray(cx)))
            })
            .child(
                label(theme, theme.title(), c.ink, put(&say!("Background work")))
                    .font_weight(theme.semibold),
            )
            .when(tasks.is_empty(), |d| {
                d.child(label(
                    theme,
                    theme.body(),
                    c.ink2,
                    put(&say!("Nothing running")),
                ))
            })
            .children(rows);
        let popup = match self.tray_fading {
            true => super::motion::pop_out(popup.child(super::motion::shield()), self.tray_turn)
                .into_any_element(),
            false => super::motion::pop_in(popup, self.tray_turn).into_any_element(),
        };
        gpui::deferred(popup).into_any_element()
    }

    pub(crate) fn close_tray(&mut self, cx: &mut Context<Self>) {
        self.tray_open = false;
        if super::motion::fades_out(cx) {
            self.tray_fading = true;
            let turn = self.tray_turn;
            super::motion::after_fade(cx, move |shell: &mut Self, cx| {
                if shell.tray_turn == turn && shell.tray_fading {
                    shell.tray_fading = false;
                    cx.notify();
                }
            });
        }
        cx.notify();
    }
}

pub(crate) struct Spinner;

impl gpui::Render for Spinner {
    fn render(&mut self, _: &mut gpui::Window, cx: &mut Context<Self>) -> impl IntoElement {
        use gpui::AnimationExt;
        #[cfg(test)]
        TURNS.with(|turns| turns.set(turns.get() + 1));
        let theme = cx.global::<Theme>();
        icon("refresh-cw", theme.sized(12.), theme.colours.moved).with_animation(
            "turning",
            gpui::Animation::new(std::time::Duration::from_millis(1400)).repeat(),
            |icon, part| {
                icon.with_transformation(gpui::Transformation::rotate(gpui::percentage(part)))
            },
        )
    }
}

#[cfg(test)]
thread_local! {
    pub(crate) static TURNS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

type Stop = std::rc::Rc<dyn Fn()>;

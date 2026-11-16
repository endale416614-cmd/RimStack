use super::field::Field;
use super::shell::Shell;
use super::text_field::{Edited, Entered, Escape, TextField};
use super::theme::{self, Theme};
use super::widgets::{TextSized, label, with_alpha};
use crate::words::put;
use gpui::{
    AppContext, Context, Entity, Focusable, InteractiveElement, IntoElement, KeyDownEvent,
    ParentElement, Render, StatefulInteractiveElement, Styled, WeakEntity, Window, div,
    prelude::FluentBuilder,
};
use rimstack_core::say;
use std::rc::Rc;

pub type Run = Rc<dyn Fn(&mut Shell, &mut Window, &mut Context<Shell>)>;

#[derive(Clone)]
pub struct Command {
    pub name: String,
    pub group: String,
    pub keys: String,
    pub run: Run,
}

pub fn matching<'a>(commands: &'a [Command], typed: &str, settings: &str) -> Vec<&'a Command> {
    let typed = typed.to_lowercase();
    let words: Vec<&str> = typed.split(' ').filter(|w| !w.is_empty()).collect();
    commands
        .iter()
        .filter(|c| {
            let said = format!("{} {}", c.name, c.group).to_lowercase();
            words.iter().all(|w| said.contains(w)) && (!words.is_empty() || c.group != settings)
        })
        .collect()
}

pub struct Commands {
    shell: WeakEntity<Shell>,
    commands: Vec<Command>,
    found: Vec<Command>,
    field: Entity<TextField>,
    chosen: usize,
    scroll: super::rows::Scrolling,
    turn: usize,
    /// Closed, and drawn only while it fades out.
    pub(crate) closing: bool,
}

impl Commands {
    pub fn new(
        shell: WeakEntity<Shell>,
        commands: Vec<Command>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let theme = cx.global::<Theme>().clone();
        let field = cx.new(|cx| TextField::new(theme.body(), cx));
        cx.subscribe(&field, |commands, _, _: &Edited, cx| commands.typed(cx))
            .detach();
        cx.subscribe_in(&field, window, |commands, _, _: &Entered, window, cx| {
            commands.run_chosen(window, cx)
        })
        .detach();
        window.focus(&field.focus_handle(cx), cx);
        let scroll = super::rows::Scrolling::default();
        scroll
            .borrow_mut()
            .set_reduce_motion(theme.appearance.reduce_motion);
        Self {
            shell,
            commands,
            found: Vec::new(),
            field,
            chosen: 0,
            scroll,
            turn: super::motion::opening(),
            closing: false,
        }
    }

    fn typed(&mut self, cx: &mut Context<Self>) {
        self.chosen = 0;
        let text = self.field.read(cx).text().trim().to_string();
        self.found = match (text.chars().count() >= 2, self.shell.upgrade()) {
            (true, Some(shell)) => shell.read(cx).found_mods(&text, cx),
            _ => Vec::new(),
        };
        cx.notify();
    }

    fn shown(&self, cx: &gpui::App) -> Vec<Command> {
        let settings = put(&say!("Setting"));
        let typed = self.field.read(cx).text().to_string();
        let mut shown: Vec<Command> = matching(&self.commands, &typed, &settings)
            .into_iter()
            .cloned()
            .collect();
        shown.extend(self.found.iter().cloned());
        shown
    }

    fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.closing {
            return;
        }
        if self.field.focus_handle(cx).contains_focused(window, cx) {
            window.blur(cx);
        }
        self.closing = true;
        cx.notify();
        let _ = self.shell.update(cx, |shell, cx| shell.close_commands(cx));
    }

    fn run_chosen(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.closing {
            return;
        }
        let chosen = self.shown(cx).into_iter().nth(self.chosen);
        self.close(window, cx);
        if let Some(command) = chosen {
            let _ = self
                .shell
                .update(cx, |shell, cx| (command.run)(shell, window, cx));
        }
    }

    #[cfg(all(test, target_os = "linux"))]
    pub(crate) fn type_text(&mut self, text: &str, cx: &mut Context<Self>) {
        self.field.update(cx, |field, cx| field.set_text(text, cx));
    }

    #[cfg(all(test, target_os = "linux"))]
    pub(crate) fn run_first(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.chosen = 0;
        self.run_chosen(window, cx);
    }

    fn step(&mut self, by: isize, cx: &mut Context<Self>) {
        let last = self.shown(cx).len().saturating_sub(1) as isize;
        self.chosen = (self.chosen as isize + by).clamp(0, last.max(0)) as usize;
        let theme = cx.global::<Theme>().clone();
        let row = theme.sized(30.);
        let top = row * self.chosen as f32;
        self.scroll.borrow_mut().contain(top, top + row);
        cx.notify();
    }
}

impl Render for Commands {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.global::<Theme>().clone();
        let c = theme.colours;
        let shown = self.shown(cx);
        let row = theme.sized(30.);
        let width = theme
            .sized(600.)
            .min(window.viewport_size().width - theme::GAP_PAGE * 2.);
        let tops: Rc<[gpui::Pixels]> = (0..=shown.len()).map(|at| row * at as f32).collect();
        let height = (row * shown.len() as f32).min(theme.sized(390.));
        let (chosen, me, drawn) = (self.chosen, cx.entity().downgrade(), theme.clone());
        let rows_shown = shown.clone();
        let list = super::rows::rows(
            "commands-rows",
            self.scroll.clone(),
            tops,
            row,
            cx.entity_id(),
            move |range, width, _, _| {
                let theme = &drawn;
                let c = &theme.colours;
                range
                    .filter_map(|at| {
                        let command = rows_shown.get(at)?;
                        let current = at == chosen;
                        let me = me.clone();
                        let beside = match command.keys.is_empty() {
                            true => command.group.clone(),
                            false => command.keys.clone(),
                        };
                        Some(
                            div()
                                .id(("command", at))
                                .flex()
                                .items_center()
                                .gap(theme::GAP)
                                .w(width)
                                .h(row)
                                .px(theme::GAP_MEDIUM)
                                .rounded(theme::CORNER)
                                .when(current, |d| d.bg(c.chosen))
                                .when(!current, |d| d.hover(|d| d.bg(with_alpha(c.hover, 0.05))))
                                .cursor_pointer()
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .overflow_hidden()
                                        .whitespace_nowrap()
                                        .text_ellipsis()
                                        .sized_text(theme, theme.body())
                                        .text_color(if current { c.on_selected } else { c.ink })
                                        .child(command.name.clone()),
                                )
                                .child(label(
                                    theme,
                                    theme.small(),
                                    if current { c.on_selected_dim } else { c.ink3 },
                                    beside,
                                ))
                                .on_click(move |_, window, cx| {
                                    let _ = me.update(cx, |commands, cx| {
                                        commands.chosen = at;
                                        commands.run_chosen(window, cx)
                                    });
                                })
                                .into_any_element(),
                        )
                    })
                    .collect()
            },
        );
        let popup = div()
            .id("commands")
            .absolute()
            .top(theme.sized(90.))
            .left((window.viewport_size().width - width) / 2.)
            .w(width)
            .flex()
            .flex_col()
            .gap(theme::GAP_SMALL)
            .p(theme::GAP)
            .rounded(theme::CORNER_LARGE)
            .bg(c.bar)
            .border_1()
            .border_color(c.line)
            .when(!self.closing, |d| {
                d.occlude()
                    .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
            })
            .capture_action(cx.listener(|commands, _: &Escape, window, cx| {
                cx.stop_propagation();
                commands.close(window, cx);
            }))
            .on_key_down(cx.listener(|commands, event: &KeyDownEvent, _, cx| {
                let by = match event.keystroke.key.as_str() {
                    "down" => 1,
                    "up" => -1,
                    "pagedown" => 10,
                    "pageup" => -10,
                    _ => return,
                };
                cx.stop_propagation();
                commands.step(by, cx);
            }))
            .child(
                Field {
                    id: "commands-field",
                    text: &self.field,
                    icon: "",
                    placeholder: put(&say!("A mod, a command or a setting")),
                    found: None,
                    keys: None,
                }
                .render(&theme, window, cx),
            )
            .when(!shown.is_empty(), |d| {
                d.child(div().relative().h(height).child(list))
            })
            .when(shown.is_empty(), |d| {
                d.child(
                    label(&theme, theme.body(), c.ink2, put(&say!("No match")))
                        .pl(theme::GAP_MEDIUM)
                        .pb(theme::GAP_SMALL),
                )
            });
        let scrim = div().id("commands-scrim").absolute().inset_0().bg(c.scrim);
        match self.closing {
            true => super::motion::pop_out(scrim.child(popup), self.turn).into_any_element(),
            false => super::motion::pop_in(
                scrim
                    .occlude()
                    .on_mouse_down(
                        gpui::MouseButton::Left,
                        cx.listener(|commands, _, window, cx| commands.close(window, cx)),
                    )
                    .child(popup),
                self.turn,
            )
            .into_any_element(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn command(name: &str, group: &str) -> Command {
        Command {
            name: name.into(),
            group: group.into(),
            keys: String::new(),
            run: Rc::new(|_, _, _| {}),
        }
    }

    #[test]
    fn every_word_is_found_in_the_name_or_the_group_and_settings_wait_for_a_word() {
        let all = [
            command("Save", "Load order"),
            command("Theme", "Setting"),
            command("Sort", "Load order"),
        ];
        let names = |typed: &str| -> Vec<String> {
            matching(&all, typed, "Setting")
                .iter()
                .map(|c| c.name.clone())
                .collect()
        };
        assert_eq!(names(""), ["Save", "Sort"]);
        assert_eq!(names("load s"), ["Save", "Sort"]);
        assert_eq!(names("SORT order"), ["Sort"]);
        assert_eq!(names("theme"), ["Theme"]);
        assert!(names("nothing").is_empty());
    }
}

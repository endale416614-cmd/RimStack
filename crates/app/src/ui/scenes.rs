use super::Options;
use super::mods::{ListState, Menu, Mods, Progress};
use super::shell::Shell;
use crate::workspace::load_order::Side;
use gpui::{Context, Entity, Subscription, Window};
use std::cell::RefCell;
use std::rc::Rc;

pub fn stage(shell: &mut Shell, options: &Options, window: &mut Window, cx: &mut Context<Shell>) {
    let mods = shell.sections.mods.clone();
    if let Some(text) = &options.filter {
        mods.update(cx, |mods, cx| mods.filter_by(text, cx));
    }
    if let Some(text) = &options.typing {
        let field = mods.read(cx).filter.clone();
        field.update(cx, |field, cx| field.set_text(text, cx));
        window.focus(&gpui::Focusable::focus_handle(field.read(cx), cx), cx);
    }
    if options.still {
        mods.update(cx, |mods, _| mods.leave_off());
    }
    if let Some(step) = options.hints {
        mods.update(cx, |mods, cx| mods.show_hint(step, cx));
    }
    if options.drawer {
        mods.update(cx, |mods, cx| mods.set_drawer(true, cx));
    }
    let listed = |list: &ListState| list.count(Side::Active) > 0;
    if let Some(row) = options.pick {
        let tab = options.tab;
        once(
            &mods,
            listed,
            move |mods, cx| mods.pick_row(Side::Active, row, tab, cx),
            cx,
        );
    }
    if options.several {
        once(&mods, listed, |mods, cx| mods.choose_several(cx), cx);
    }
    if options.comparing {
        once(
            &mods,
            ListState::files_read,
            |mods, cx| mods.compare_first(cx),
            cx,
        );
    }
    if options.sorting {
        once(
            &mods,
            ListState::files_read,
            |mods, cx| mods.preview_sort(cx),
            cx,
        );
    }
    if options.bench {
        let ready = |list: &ListState| list.count(Side::Active) > 0 && !list.reading_files();
        once(&mods, ready, |mods, cx| mods.start_bench(cx), cx);
    }
    if let Some(asked) = &options.browse {
        let workshop = shell.workshop(cx);
        workshop.update(cx, |workshop, cx| workshop.show_asked(asked, cx));
    }
    if let Some(text) = &options.find {
        let find = shell.find(cx);
        find.update(cx, |find, cx| find.search_for(text, Some(2), cx));
    }
    if options.notices {
        show_notices(cx);
    }
    open(shell, options.open.as_deref(), window, cx);
}

fn once(
    mods: &Entity<Mods>,
    ready: impl Fn(&ListState) -> bool + 'static,
    act: impl FnOnce(&mut Mods, &mut Context<Mods>) + 'static,
    cx: &mut Context<Shell>,
) {
    if ready(mods.read(cx).list.read(cx)) {
        mods.update(cx, act);
        return;
    }
    let held: Rc<RefCell<Option<Subscription>>> = Rc::default();
    let kept = held.clone();
    let mut act = Some(act);
    let heard = cx.subscribe(mods, move |_, mods, _: &Progress, cx| {
        if !ready(mods.read(cx).list.read(cx)) {
            return;
        }
        if let Some(act) = act.take() {
            mods.update(cx, act);
        }
        kept.borrow_mut().take();
    });
    *held.borrow_mut() = Some(heard);
}

fn show_notices(cx: &mut Context<Shell>) {
    use super::notices::{self, Tone};
    use crate::words::put;
    use rimstack_core::{say, say_n};
    notices::say(put(&say!("Saved")), Tone::Ok, cx);
    notices::offer(
        put(&say_n!(3, "Deactivated {} mod", "Deactivated {} mods")),
        put(&say!("Undo")),
        |_, _| {},
        cx,
    );
}

fn open(shell: &mut Shell, open: Option<&str>, window: &mut Window, cx: &mut Context<Shell>) {
    match open {
        Some("play") => shell.before_play = true,
        Some("problems") => shell
            .sections
            .mods
            .update(cx, |mods, cx| mods.open_problems(cx)),
        Some("keys") => shell.open_keys(window, cx),
        Some("commands") => shell.open_commands(window, cx),
        Some("lists") => shell.open_popup(super::lists::Which::Lists, window, cx),
        Some("history") => shell.open_popup(super::lists::Which::History, window, cx),
        Some("tray") => shell.tray_open = true,
        Some("view") => open_menu(&shell.sections.mods, Menu::View, cx),
        Some("order") => open_menu(&shell.sections.mods, Menu::InactiveOrder, cx),
        Some("ask") => ask(window, cx),
        Some("support") => {
            let theme = cx.global::<super::theme::Theme>().clone();
            let room = window.viewport_size();
            let heart = gpui::Bounds::new(
                gpui::point(room.width * 0.6, room.height - theme.strip()),
                gpui::Size::default(),
            );
            super::menu::open_above(heart, super::support::menu(), window, cx);
        }
        _ => {}
    }
}

fn open_menu(mods: &Entity<Mods>, menu: Menu, cx: &mut Context<Shell>) {
    let listed = |list: &ListState| list.count(Side::Active) > 0;
    once(
        mods,
        listed,
        move |mods, cx| {
            mods.when_drawn(
                Box::new(move |mods, window, cx| mods.open_menu(menu, window, cx)),
                cx,
            )
        },
        cx,
    );
}

fn ask(window: &mut Window, cx: &mut Context<Shell>) {
    use super::prompt::{self, Line, Question};
    use crate::words::put;
    use rimstack_core::say;
    let question = Question {
        title: put(&say!("Set the mods' settings aside")),
        lines: vec![
            Line::Text(put(&say!("Every Mod_*.xml is moved out of"))),
            Line::Path(
                "/home/example/.config/unity3d/Ludeon Studios/RimWorld by Ludeon Studios/Config"
                    .into(),
            ),
            Line::Text(put(&say!(
                "Mods start from their defaults. Moving the files back restores them."
            ))),
        ],
        label: put(&say!("Set aside")),
        danger: false,
        run: Box::new(|_, _| {}),
    };
    prompt::ask(question, window, cx);
}

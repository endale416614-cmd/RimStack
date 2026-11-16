use super::Settings;
use crate::App;
use crate::ui::layout_checks::check_scene;
use crate::ui::shell::Shell;
use crate::ui::theme::{Theme, ThemeName};
use crate::ui::widgets::watch::{self, Seen};
use crate::ui::{Options, install};
use gpui::{
    AppContext, Entity, HeadlessAppContext, Pixels, Size, TestAppContext, VisualTestContext, size,
};
use std::sync::Arc;

fn settings(
    cx: &mut TestAppContext,
    part: usize,
) -> (tempfile::TempDir, Entity<Settings>, &mut VisualTestContext) {
    let root = tempfile::tempdir().unwrap();
    let path = root.path();
    let app = App::new(
        path.join("home"),
        &path.join("settings"),
        &path.join("data"),
        Default::default(),
    );
    let options = Options {
        demo: true,
        section: crate::ui::section::Section::Settings,
        part,
        ..Options::default()
    };
    cx.update(|cx| install(Arc::new(app), &options, cx));
    let (shell, cx) = cx.add_window_view(|window, cx| Shell::new(&options, window, cx));
    let settings = shell.read_with(cx, |shell, _| {
        shell
            .sections
            .made::<crate::ui::settings::Settings>()
            .unwrap()
    });
    cx.run_until_parked();
    (root, settings, cx)
}

fn shown_titles(settings: &Entity<Settings>, cx: &mut VisualTestContext) -> Vec<String> {
    cx.update(|window, cx| {
        settings.update(cx, |s, cx| {
            let lines = s.lines(window, cx);
            lines
                .into_iter()
                .filter_map(|row| match row {
                    super::line::Row::Line(line) if s.shows(&line) => Some(line.title),
                    _ => None,
                })
                .collect()
        })
    })
}

#[gpui::test]
fn a_search_shows_the_settings_it_names_from_every_part_and_the_arrows_go_from_part_to_part(
    cx: &mut TestAppContext,
) {
    let (_root, settings, cx) = settings(cx, 0);
    assert!(shown_titles(&settings, cx).contains(&"Launch arguments".to_string()));
    let field = settings.read_with(cx, |s, _| s.fields.find.clone());
    field.update(cx, |field, cx| field.set_text("motion", cx));
    settings.update(cx, |s, cx| {
        s.finding = "motion".into();
        cx.notify();
    });
    assert_eq!(shown_titles(&settings, cx), ["Motion"]);
    cx.update(|window, cx| {
        let keys = settings.read(cx).part_keys.clone();
        window.focus(&keys, cx);
    });
    cx.run_until_parked();
    cx.simulate_keystrokes("down down up");
    assert_eq!(settings.read_with(cx, |s, _| s.part), 1);
}

#[gpui::test]
fn a_theme_chosen_is_kept_in_rimstack_conf_and_shown_at_once(cx: &mut TestAppContext) {
    let (root, settings, cx) = settings(cx, 1);
    settings.update(cx, |s, cx| s.keep_look("theme", "light", cx));
    cx.run_until_parked();
    let kept =
        std::fs::read_to_string(root.path().join("settings/RimStack/RimStack.conf")).unwrap();
    assert!(kept.contains("[Appearance]\ntheme=light"), "{kept}");
    assert_eq!(
        cx.update(|_, cx| cx.global::<Theme>().appearance.theme),
        ThemeName::Light
    );
    cx.update(|window, cx| settings.update(cx, |s, cx| s.reset_page(window, cx)));
    assert_eq!(
        cx.update(|_, cx| cx.global::<Theme>().appearance.theme),
        ThemeName::Rim,
        "Reset this page brings it back"
    );
}

type Shape<'a> = &'a dyn Fn(&Theme) -> Size<Pixels>;

fn drawn(part: usize) -> impl Fn(f32, Shape<'_>) -> Vec<Seen> {
    drawn_with(part, "{}", "")
}

fn drawn_with(
    part: usize,
    kept: &'static str,
    finding: &'static str,
) -> impl Fn(f32, Shape<'_>) -> Vec<Seen> {
    move |text_scale, window| {
        let root = tempfile::tempdir().unwrap();
        let path = root.path();
        std::fs::create_dir_all(path.join("settings/RimStack")).unwrap();
        std::fs::write(path.join("settings/RimStack/settings.json"), kept).unwrap();
        let app = App::new(
            path.join("home"),
            &path.join("settings"),
            &path.join("data"),
            Default::default(),
        );
        let text = crate::ui::made_up::text_system();
        let mut cx =
            HeadlessAppContext::with_asset_source(text, Arc::new(crate::ui::assets::Assets));
        let options = Options {
            demo: true,
            section: crate::ui::section::Section::Settings,
            part,
            text_scale: Some(text_scale),
            ..Options::default()
        };
        cx.update(|cx| install(Arc::new(app), &options, cx));
        let shape = cx.update(|cx| window(cx.global::<Theme>()));
        let opened = cx
            .open_window(shape, |window, cx| {
                cx.new(|cx| {
                    let shell = Shell::new(&options, window, cx);
                    if let Some(settings) = shell
                        .sections
                        .made::<crate::ui::settings::Settings>()
                        .filter(|_| !finding.is_empty())
                    {
                        settings.update(cx, |s, _| s.finding = finding.into());
                    }
                    shell
                })
            })
            .unwrap();
        cx.run_until_parked();
        watch::start();
        cx.update_window(opened.into(), |_, window, cx| {
            window.refresh();
            window.draw(cx).clear(cx)
        })
        .unwrap();
        watch::take()
    }
}

#[test]
fn every_part_of_settings_fits_at_every_text_size() {
    for part in 0..5 {
        check_scene(
            &format!("Settings part {part}, first window"),
            15,
            &|_| size(crate::ui::WINDOW.0, crate::ui::WINDOW.1),
            drawn(part),
        );
        check_scene(
            &format!("Settings part {part}, smallest window"),
            15,
            &|theme| size(theme.sized(1000.), theme.sized(560.)),
            drawn(part),
        );
    }
}

#[test]
fn a_choice_of_many_ends_before_the_way_back_to_its_default() {
    let options = [
        "Every minute",
        "Every 5 minutes",
        "Every 30 minutes",
        "Every hour",
    ];
    let windows: [Shape<'_>; 2] = [
        &|_| size(crate::ui::WINDOW.0, crate::ui::WINDOW.1),
        &|theme| size(theme.sized(1000.), theme.sized(560.)),
    ];
    for window in windows {
        for text_scale in crate::ui::layout_checks::TEXT_SIZES {
            let seen = drawn_with(0, r#"{ "updateEvery": "1" }"#, "how often")(text_scale, window);
            let page_right = seen
                .iter()
                .find(|s| s.text.starts_with("Reset this page"))
                .map(|s| s.bounds.right())
                .expect("the page's reset is drawn");
            let reset = gpui::px(28. * text_scale);
            for option in options {
                let drawn = seen.iter().find(|s| s.text == option).expect(option);
                assert!(
                    drawn.bounds.right() <= page_right - reset,
                    "{option} at {text_scale} ends at {:?}, the line's reset begins by {:?}",
                    drawn.bounds.right(),
                    page_right - reset
                );
            }
        }
    }
}

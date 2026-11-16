use super::Textures;
use crate::ui::layout_checks::check_scene;
use crate::ui::made_up::made_up;
use crate::ui::shell::Shell;
use crate::ui::theme::Theme;
use crate::ui::widgets::watch::{self, Seen};
use crate::ui::{Options, install};
use crate::workspace::textures::shown;
use gpui::{AppContext, Entity, HeadlessAppContext, Pixels, Size, WindowHandle, size};
use std::sync::Arc;
use std::time::{Duration, Instant};

type Shape<'a> = &'a dyn Fn(&Theme) -> Size<Pixels>;

fn opened(
    text_scale: f32,
    window: Shape<'_>,
    version: Option<&str>,
) -> (
    tempfile::TempDir,
    HeadlessAppContext,
    WindowHandle<Shell>,
    Entity<Textures>,
) {
    let (tmp, app) = made_up();
    if let Some(version) = version {
        std::fs::write(tmp.path().join("fx/game/Version.txt"), version).unwrap();
        workshop_mod_with_spare_dds(&tmp.path().join("fx/steam/workshop/content/294100/900"));
        crate::jobs::waiting_here(|| app.rescan());
    }
    let text = crate::ui::made_up::text_system();
    let mut cx = HeadlessAppContext::with_asset_source(text, Arc::new(crate::ui::assets::Assets));
    let options = Options {
        section: crate::ui::section::Section::Textures,
        text_scale: Some(text_scale),
        ..Options::default()
    };
    cx.update(|cx| install(Arc::new(app), &options, cx));
    let shape = cx.update(|cx| window(cx.global::<Theme>()));
    let shell = cx
        .open_window(shape, |window, cx| {
            cx.new(|cx| Shell::new(&options, window, cx))
        })
        .unwrap();
    let textures = cx
        .update(|cx| {
            shell.read(cx).map(|shell| {
                shell
                    .sections
                    .made::<crate::ui::textures::Textures>()
                    .unwrap()
            })
        })
        .unwrap();
    let started = Instant::now();
    while cx.update(|cx| textures.read(cx).rows.is_empty()) {
        assert!(
            started.elapsed() < Duration::from_secs(20),
            "the made-up textures were not counted"
        );
        crate::ui::made_up::step(&cx);
    }
    cx.run_until_parked();
    (tmp, cx, shell, textures)
}

#[test]
fn the_made_up_install_counts_its_textures_and_none_has_a_dds() {
    let (_tmp, mut cx, _shell, textures) = opened(
        1.0,
        &|_| size(crate::ui::WINDOW.0, crate::ui::WINDOW.1),
        None,
    );
    let (have, total, remaining, converts) = cx.update(|cx| {
        let textures = textures.read(cx);
        let shown = shown(&textures.rows, true, textures.converts());
        (
            shown.have,
            shown.total,
            shown.remaining.len(),
            textures.dds_faster,
        )
    });
    assert_eq!((have, total, remaining, converts), (0, 13, 13, Some(true)));
}

#[test]
fn from_1_7_nothing_waits_to_be_converted() {
    let (_tmp, mut cx, _shell, textures) = opened(
        1.0,
        &|_| size(crate::ui::WINDOW.0, crate::ui::WINDOW.1),
        Some("1.7.5005 rev1082"),
    );
    let (waiting, converts) = cx.update(|cx| {
        let textures = textures.read(cx);
        let shown = shown(&textures.rows, true, textures.converts());
        (shown.waiting, textures.dds_faster)
    });
    assert_eq!((waiting, converts), (0, Some(false)));
}

fn workshop_mod_with_spare_dds(root: &std::path::Path) {
    let about = root.join("About");
    std::fs::create_dir_all(&about).unwrap();
    std::fs::write(
        about.join("About.xml"),
        "<ModMetaData><packageId>Fixture.Hats</packageId><name>Fixture Hats With A Rather Long Name</name></ModMetaData>",
    )
    .unwrap();
    let textures = root.join("Textures/Things/Apparel");
    std::fs::create_dir_all(&textures).unwrap();
    let picture = image::RgbaImage::from_fn(32, 32, |x, y| {
        image::Rgba([(x * 4) as u8, (y * 4) as u8, 90, 255])
    });
    for n in 0..5 {
        let png = textures.join(format!("WideBrimmedFeatheredHat_{n}_south.png"));
        picture.save(&png).unwrap();
        let dds = rimstack_core::mods::textures::encode(&std::fs::read(&png).unwrap()).unwrap();
        std::fs::write(png.with_extension("dds"), dds).unwrap();
    }
}

fn spare_listed(cx: &mut HeadlessAppContext, textures: &Entity<Textures>) {
    cx.update(|cx| textures.update(cx, |t, cx| t.find_spare(cx)));
    let started = Instant::now();
    while cx.update(|cx| textures.read(cx).spare.is_none()) {
        assert!(
            started.elapsed() < Duration::from_secs(20),
            "no spare DDS listed"
        );
        crate::ui::made_up::step(cx);
    }
    cx.run_until_parked();
}

#[test]
fn from_1_7_the_workshop_mod_s_spare_dds_are_listed() {
    let (_tmp, mut cx, _shell, textures) = opened(
        1.0,
        &|_| size(crate::ui::WINDOW.0, crate::ui::WINDOW.1),
        Some("1.7.5005 rev1082"),
    );
    spare_listed(&mut cx, &textures);
    let listed = cx.update(|cx| {
        textures
            .read(cx)
            .spare
            .as_ref()
            .map(|(found, list)| (found.files(), found.mods.len(), list.files()))
    });
    assert_eq!(listed, Some((5, 1, 5)));
}

fn drawn_spare(text_scale: f32, window: Shape<'_>) -> Vec<Seen> {
    let (_tmp, mut cx, shell, textures) = opened(text_scale, window, Some("1.7.5005 rev1082"));
    spare_listed(&mut cx, &textures);
    cx.update(|cx| {
        textures.update(cx, |t, cx| {
            t.toggle_files(super::files::Which::Spare, 0, cx)
        })
    });
    drawn_once(&mut cx, shell, &textures)
}

fn drawn_once(
    cx: &mut HeadlessAppContext,
    shell: WindowHandle<Shell>,
    textures: &Entity<Textures>,
) -> Vec<Seen> {
    watch::start();
    cx.update(|cx| textures.update(cx, |_, cx| cx.notify()));
    cx.update_window(shell.into(), |_, window, cx| window.draw(cx).clear(cx))
        .unwrap();
    watch::take()
}

fn drawn_at(version: Option<&str>, text_scale: f32, window: Shape<'_>) -> Vec<Seen> {
    let (_tmp, mut cx, shell, textures) = opened(text_scale, window, version);
    drawn_once(&mut cx, shell, &textures)
}

fn drawn(text_scale: f32, window: Shape<'_>) -> Vec<Seen> {
    drawn_at(None, text_scale, window)
}

fn drawn_from_1_7(text_scale: f32, window: Shape<'_>) -> Vec<Seen> {
    drawn_at(Some("1.7.5005 rev1082"), text_scale, window)
}

#[test]
fn textures_fits_at_every_text_size() {
    check_scene(
        "Textures, first window",
        20,
        &|_| size(crate::ui::WINDOW.0, crate::ui::WINDOW.1),
        drawn,
    );
    check_scene(
        "Textures, smallest window",
        20,
        &|theme| size(theme.sized(1000.), theme.sized(560.)),
        drawn,
    );
}

#[test]
fn textures_from_1_7_fits_at_every_text_size() {
    check_scene(
        "Textures 1.7, first window",
        20,
        &|_| size(crate::ui::WINDOW.0, crate::ui::WINDOW.1),
        drawn_from_1_7,
    );
    check_scene(
        "Textures 1.7, smallest window",
        20,
        &|theme| size(theme.sized(1000.), theme.sized(560.)),
        drawn_from_1_7,
    );
    check_scene(
        "Textures 1.7, spare DDS listed",
        20,
        &|theme| size(theme.sized(1000.), theme.sized(560.)),
        drawn_spare,
    );
}

pub mod around;
pub mod assets;
pub mod button;
pub mod card;
pub mod command_list;
pub mod commands;
pub mod controls;
pub mod counts;
pub mod divider;
pub mod downloads;
pub mod field;
pub mod find;
pub mod first_run;
pub mod focus;
pub mod game;
pub mod hunt;
pub mod keys;
pub mod listed;
pub mod lists;
pub mod logs;
pub mod menu;
pub mod mods;
pub mod motion;
pub mod navigate;
pub mod notices;
pub mod order;
pub mod outside;
pub mod page;
pub mod pictures;
pub mod play;
#[cfg(all(test, target_os = "linux"))]
mod play_tests;
pub mod problem;
pub mod prompt;
pub mod rich;
pub mod rows;
pub mod saves;
pub mod scenes;
pub mod section;
pub mod sections;
pub mod settings;
pub mod shell;
pub mod status_bar;
pub mod status_note;
pub mod text_field;
pub mod textures;
pub mod theme;
pub mod thumbs;
pub mod tip;
pub mod top_bar;
pub mod widgets;
pub mod workshop;

use crate::App;
use gpui::{
    App as Cx, AppContext, Bounds, Global, TitlebarOptions, WindowBounds, WindowOptions, font, size,
};
use std::sync::Arc;
use theme::{Appearance, Theme, ThemeName};

pub struct Shared(pub Arc<App>);

impl Global for Shared {}

pub fn app(cx: &Cx) -> Arc<App> {
    cx.global::<Shared>().0.clone()
}

#[derive(Clone, Debug, Default)]
pub struct Options {
    pub demo: bool,
    pub section: section::Section,
    pub theme: Option<ThemeName>,
    pub text_scale: Option<f32>,
    pub several: bool,
    pub comparing: bool,
    pub drawer: bool,
    pub sorting: bool,
    pub hints: Option<u8>,
    /// Typed into the Mods filter, which has the keys, as `--typing=` asks.
    pub typing: Option<String>,
    pub filter: Option<String>,
    /// A row of the load order chosen, and the tab shown, as `--pick=` and `--tab=` give them.
    pub pick: Option<usize>,
    pub tab: usize,
    /// The scripted scroll of `--selftest`.
    pub bench: bool,
    pub notices: bool,
    /// What is open at the start, as `--open=` names it, for pictures.
    pub open: Option<String>,
    pub part: usize,
    pub view: usize,
    pub find: Option<String>,
    /// What the Workshop shows at the start, as `--browse=` names it: `list`, `details` or
    /// `filters`, for pictures.
    pub browse: Option<String>,
    /// For a picture: the disk is not watched, so nothing changes while it is taken.
    pub still: bool,
    /// The first run shown, as `--first` asks, on the step `--step=` gives.
    pub first: bool,
    pub step: usize,
}

impl Options {
    pub fn from_args(args: impl IntoIterator<Item = String>) -> Self {
        let mut options = Self::default();
        for arg in args {
            let (name, value) = arg.split_once('=').unwrap_or((&arg, ""));
            match name {
                "--demo" => options.demo = true,
                "--selftest" => (options.demo, options.bench) = (true, true),
                "--bench" => options.bench = true,
                "--section" => options.section = section::Section::at(value.parse().unwrap_or(0)),
                "--theme" => options.theme = ThemeName::named(value),
                "--text-scale" => options.text_scale = value.parse().ok(),
                "--several" => options.several = true,
                "--comparing" => options.comparing = true,
                "--drawer" => options.drawer = true,
                "--sorting" => options.sorting = true,
                "--hints" => options.hints = value.parse().ok(),
                "--typing" => options.typing = Some(value.to_string()),
                "--filter" => options.filter = Some(value.to_string()),
                "--pick" => options.pick = value.parse().ok(),
                "--tab" => options.tab = value.parse().unwrap_or(0),
                "--notices" => options.notices = true,
                "--open" => options.open = Some(value.to_string()),
                "--part" => options.part = value.parse().unwrap_or(0),
                "--view" => options.view = value.parse().unwrap_or(0),
                "--find" => options.find = Some(value.to_string()),
                "--browse" => options.browse = Some(value.to_string()),
                "--first" => options.first = true,
                "--step" => options.step = value.parse().unwrap_or(0),
                _ => {}
            }
        }
        options
    }
}

pub use theme::WINDOW;

/// The weights fontconfig has for a family, as CSS numbers.
#[cfg(target_os = "linux")]
fn weights_of(family: &str) -> Vec<f32> {
    let mut asked = rimstack_core::base::program::command("fc-list");
    asked.args([family, "weight"]);
    let Ok(told) = rimstack_core::base::program::run(&mut asked, std::time::Duration::from_secs(2))
    else {
        return Vec::new();
    };
    // fontconfig's scale: 50 light, 80 regular, 100 medium, 180 semibold, 200 bold, 210 black.
    const SCALE: [(f32, f32); 9] = [
        (0., 100.),
        (40., 200.),
        (50., 300.),
        (80., 400.),
        (100., 500.),
        (180., 600.),
        (200., 700.),
        (205., 800.),
        (210., 900.),
    ];
    let mut found: Vec<f32> = String::from_utf8_lossy(&told.stdout)
        .lines()
        .filter_map(|line| line.trim().strip_prefix(":weight=")?.parse::<f32>().ok())
        .filter_map(|fc| SCALE.iter().find(|(at, _)| *at == fc).map(|(_, css)| *css))
        .collect();
    found.sort_by(f32::total_cmp);
    found.dedup();
    found
}

#[cfg(not(target_os = "linux"))]
fn weights_of(_family: &str) -> Vec<f32> {
    Vec::new()
}

fn system_family() -> String {
    system_family_for("sans-serif").unwrap_or_else(|| ".SystemUIFont".to_string())
}

fn system_family_for(generic: &str) -> Option<String> {
    #[cfg(target_os = "linux")]
    {
        let mut asked = rimstack_core::base::program::command("fc-match");
        asked.args(["-f", "%{family[0]}", generic]);
        let told = rimstack_core::base::program::run(&mut asked, std::time::Duration::from_secs(2));
        if let Ok(told) = told
            && told.status.success()
            && let Ok(family) = String::from_utf8(told.stdout)
            && !family.trim().is_empty()
        {
            return Some(family.trim().to_string());
        }
    }
    let _ = generic;
    None
}

pub fn workshop_page(id: &str) -> String {
    format!("https://steamcommunity.com/sharedfiles/filedetails/?id={id}")
}

pub fn change_notes(id: &str) -> String {
    format!("https://steamcommunity.com/sharedfiles/filedetails/changelog/{id}")
}

/// The `App`'s news among what the view `reads`, handed to the window's thread: what came
/// meanwhile is told at once. A view is told only what it reads, and woken for nothing else.
pub fn hear<V: 'static>(
    cx: &mut gpui::Context<V>,
    reads: crate::changes::Changes,
    heard: impl Fn(&mut V, crate::changes::Changes, &mut gpui::Context<V>) + 'static,
) {
    let (tell, mut told) = futures::channel::mpsc::unbounded();
    let closing = tell.clone();
    cx.on_release(move |_, _| closing.close_channel()).detach();
    app(cx).listen_while(move |changes| match changes.and(reads) {
        read if read.is_empty() => !tell.is_closed(),
        read => tell.unbounded_send(read).is_ok(),
    });
    cx.spawn(async move |view, cx| {
        use futures::StreamExt;
        while let Some(mut changes) = told.next().await {
            while let Ok(more) = told.try_recv() {
                changes |= more;
            }
            if view
                .update(cx, |view, cx| heard(view, changes, cx))
                .is_err()
            {
                return;
            }
        }
    })
    .detach();
}

/// `view` drawn again once this frame is done: `window.refresh()` while a frame is drawn does
/// nothing in gpui-fast.
pub fn redraw_after(view: gpui::EntityId, cx: &mut Cx) {
    cx.defer(move |cx| cx.notify(view));
}

/// `view` filling its place, drawn again only when it or what it read is notified: a view the
/// one around it hands what to show. Placed as a plain child, it is drawn again with the view
/// around it whenever an entity it read was merely updated, as an `observe` updates its observer.
pub fn cached<V: gpui::Render>(view: gpui::Entity<V>) -> impl gpui::IntoElement {
    use gpui::Styled;
    view.cached(gpui::StyleRefinement::default().size_full())
}

pub fn act<V: 'static, T: 'static>(
    cx: &gpui::Context<V>,
    f: impl Fn(&mut V, T, &mut gpui::Window, &mut gpui::Context<V>) + 'static,
) -> impl Fn(T, &mut gpui::Window, &mut Cx) + 'static {
    let me = cx.entity().downgrade();
    move |value, window, cx| {
        let _ = me.update(cx, |view, cx| f(view, value, window, cx));
    }
}

fn kept_appearance(app: &App) -> Appearance {
    Appearance::of(&app.prefs(), |theme| {
        let own = app.own_colours(theme);
        own.into_iter()
            .filter_map(|(key, value)| Some((key, value.as_str()?.to_string())))
            .collect()
    })
}

pub fn retheme(cx: &mut Cx) {
    let appearance = kept_appearance(&app(cx));
    let theme = cx.global::<Theme>().with_appearance(appearance);
    set_motion(&theme, cx);
    cx.set_global(theme);
    cx.refresh_windows();
}

pub fn keep_appearance(key: &str, value: &str, cx: &mut Cx) {
    match app(cx).keep_pref("Appearance", key, value) {
        Ok(()) => retheme(cx),
        Err(problem) => rimstack_core::problem!("{problem}"),
    }
}

pub fn appearance(app: &App, options: &Options) -> Appearance {
    let mut appearance = kept_appearance(app);
    if let Some(theme) = options.theme {
        appearance.theme = theme;
    }
    if let Some(scale) = options.text_scale {
        appearance.text_scale = scale;
    }
    appearance
}

/// The family texts are drawn in and its weights, and the monospace family, as fontconfig gives
/// them: about 15 ms of programs run, so `run` asks while the platform starts.
pub struct Fonts {
    family: String,
    weights: Vec<f32>,
    mono: Option<String>,
}

impl Fonts {
    fn probe(chosen: Option<String>) -> Self {
        let family = chosen.unwrap_or_else(system_family);
        let weights = weights_of(&family);
        Self {
            family,
            weights,
            mono: system_family_for("monospace"),
        }
    }
}

/// The font chosen in Settings, none for the system's.
fn chosen_font(app: &App) -> Option<String> {
    app.settings()
        .get("font")
        .map(str::to_string)
        .filter(|f| !f.is_empty())
}

fn weighted(theme: Theme, family: String, weights: &[f32], cx: &Cx) -> Theme {
    let mut theme = Theme::new(theme.appearance.clone(), family).with_weights(weights);
    let text = cx.text_system();
    let id = text.resolve_font(&font(theme.family.clone()));
    let at = theme::PROBE;
    theme.ascent = text.ascent(id, at) / at;
    theme.descent = text.descent(id, at).abs() / at;
    theme
}

/// Every text in `family` from now on; the system's font when it is empty. fontconfig is asked
/// off the window's thread, and the windows drawn again once it answers.
pub fn use_font(family: &str, cx: &mut Cx) {
    let chosen = (!family.is_empty()).then(|| family.to_string());
    let probing = crate::jobs::off_thread("fonts", move || Fonts::probe(chosen));
    cx.spawn(async move |cx| {
        let Ok(fonts) = probing.await else {
            return;
        };
        cx.update(|cx| {
            let old = cx.global::<Theme>().clone();
            let mut theme = weighted(old.clone(), fonts.family, &fonts.weights, cx);
            theme.mono = old.mono;
            set_motion(&theme, cx);
            cx.set_global(theme);
            cx.refresh_windows();
        });
    })
    .detach();
}

/// A picture is taken: nothing moves, so it shows each thing where it comes to rest.
struct Still;

impl gpui::Global for Still {}

fn set_motion(theme: &Theme, cx: &mut Cx) {
    cx.set_reduce_motion(theme.appearance.reduce_motion || cx.has_global::<Still>());
}

pub fn install(app: Arc<App>, options: &Options, cx: &mut Cx) {
    let fonts = crate::jobs::waiting_here(|| Fonts::probe(chosen_font(&app)));
    install_with(app, options, fonts, cx);
}

fn install_with(app: Arc<App>, options: &Options, fonts: Fonts, cx: &mut Cx) {
    #[cfg(test)]
    crate::jobs::run_on(cx.background_executor().clone());
    crate::jobs::keep_window_thread();
    if let Err(problem) = app.carry_interface_scale() {
        rimstack_core::problem!("{problem}");
    }
    let Fonts {
        family,
        weights,
        mono,
    } = fonts;
    let mut theme = weighted(
        Theme::new(appearance(&app, options), ""),
        family,
        &weights,
        cx,
    );
    if let Some(mono) = mono {
        theme.mono = mono.into();
    }
    if options.still {
        cx.set_global(Still);
    }
    set_motion(&theme, cx);
    cx.set_global(theme);
    cx.set_global(Shared(app));
    counts::install(cx);
    navigate::install(cx);
    around::install(cx);
    order::install(cx);
    listed::install(cx);
    tip::install(cx);
    outside::install(options.demo, cx);
    text_field::bind_keys(cx);
    focus::bind_keys(cx);
    keys::bind_keys(cx);
    cx.bind_keys([
        gpui::KeyBinding::new(crate::command!("f"), mods::FocusFilter, None),
        gpui::KeyBinding::new(crate::command!("z"), mods::Undo, None),
        gpui::KeyBinding::new(crate::command!("shift-z"), mods::Redo, None),
        #[cfg(not(target_os = "macos"))]
        gpui::KeyBinding::new("ctrl-y", mods::Redo, None),
        gpui::KeyBinding::new(crate::command!("s"), mods::Save, None),
    ]);
    cx.bind_keys([gpui::KeyBinding::new(
        crate::command!("shift-f"),
        find::FocusSearch,
        None,
    )]);
}

/// Where the window opens: where it was last closed, kept inside the screens there are now.
fn window_start(app: &App, cx: &Cx) -> WindowBounds {
    use rimstack_core::base::window::Area;
    let screens: Vec<Area> = cx
        .displays()
        .iter()
        .map(|display| {
            let b = display.bounds();
            Area {
                x: f32::from(b.origin.x) as i32,
                y: f32::from(b.origin.y) as i32,
                width: f32::from(b.size.width) as i32,
                height: f32::from(b.size.height) as i32,
            }
        })
        .collect();
    let (area, maximized) = app.window_start(&screens);
    let bounds = Bounds::new(
        gpui::point(gpui::px(area.x as f32), gpui::px(area.y as f32)),
        size(gpui::px(area.width as f32), gpui::px(area.height as f32)),
    );
    match maximized {
        true => WindowBounds::Maximized(bounds),
        false => WindowBounds::Windowed(bounds),
    }
}

pub(crate) fn keep_window(window: &gpui::Window, cx: &Cx) {
    use rimstack_core::base::window::Area;
    let (bounds, maximized) = match window.window_bounds() {
        WindowBounds::Windowed(bounds) => (bounds, false),
        WindowBounds::Maximized(bounds) | WindowBounds::Fullscreen(bounds) => (bounds, true),
    };
    let area = Area {
        x: f32::from(bounds.origin.x) as i32,
        y: f32::from(bounds.origin.y) as i32,
        width: f32::from(bounds.size.width) as i32,
        height: f32::from(bounds.size.height) as i32,
    };
    if let Err(problem) = app(cx).keep_window(area, maximized) {
        rimstack_core::problem!("{problem}");
    }
}

pub fn run(app: Arc<App>, options: Options) {
    let chosen = chosen_font(&app);
    let fonts = crate::jobs::off_thread("fonts", move || Fonts::probe(chosen));
    gpui_platform::application()
        .with_assets(assets::Assets)
        .run(move |cx: &mut Cx| {
            let started = window_start(&app, cx);
            let fonts = futures::executor::block_on(fonts)
                .unwrap_or_else(|_| Fonts::probe(chosen_font(&app)));
            install_with(app, &options, fonts, cx);
            let least = {
                let theme = cx.global::<theme::Theme>();
                size(theme.sized(1000.), theme.sized(560.))
            };
            let opened = cx.open_window(
                WindowOptions {
                    window_bounds: Some(started),
                    window_min_size: Some(least),
                    titlebar: Some(TitlebarOptions {
                        title: Some("RimStack".into()),
                        ..Default::default()
                    }),
                    app_id: Some(
                        rimstack_core::base::sandbox::flatpak_id()
                            .unwrap_or_else(|| "rimstack".to_string()),
                    ),
                    ..Default::default()
                },
                |window, cx| cx.new(|cx| shell::Shell::new(&options, window, cx)),
            );
            if let Err(problem) = opened {
                rimstack_core::problem!("the window did not open: {problem}");
                cx.quit();
            }
            cx.activate(true);
        });
}

#[cfg(test)]
mod fork_tests;
#[cfg(test)]
mod gates;
#[cfg(test)]
pub(crate) mod layout_checks;
#[cfg(test)]
pub(crate) mod made_up;
#[cfg(all(test, target_os = "linux"))]
mod motion_tests;
#[cfg(test)]
mod widget_tests;

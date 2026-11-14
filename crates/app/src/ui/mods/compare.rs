mod list;
mod picture;
mod summary;

use super::Mods;
use crate::changes::Changes;
use crate::ui::button::Btn;
use crate::ui::controls::Choice;
use crate::ui::field::Field;
use crate::ui::notices;
use crate::ui::rows::{Scrolling, bar, rows};
use crate::ui::text_field::{Edited, TextField};
use crate::ui::theme::{self, Theme};
use crate::ui::widgets::{TextSized, elide_start_in, label, with_alpha};
use crate::words::{count, put};
use crate::workspace::textures::{Comparison, Difference, Shared};
use gpui::{
    AnyElement, AppContext, Context, Entity, FocusHandle, InteractiveElement, IntoElement,
    KeyDownEvent, ParentElement, Pixels, Point, Render, SharedString, Size,
    StatefulInteractiveElement, Styled, Window, div, point, prelude::FluentBuilder, px,
};
use rimstack_core::mods::library::ModId;
use rimstack_core::say;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

struct Cutting;

/// Told to the Mods tab, once the view's own update is over.
pub enum Told {
    Compared(Option<Rc<Comparison>>),
    Done,
}

impl gpui::EventEmitter<Told> for CompareView {}

/// The zooms offered: 0 fits the picture to its box.
const ZOOMS: [f32; 4] = [0., 2., 4., 8.];

/// A picture's pixels, read from its header whatever its name ends with.
fn pixels(file: &str) -> Option<(u32, u32)> {
    image::ImageReader::open(file)
        .ok()?
        .with_guessed_format()
        .ok()?
        .into_dimensions()
        .ok()
}

fn kilobytes(n: u64) -> String {
    match n >= 1024 * 1024 {
        true => put(&say!("{} MB", format!("{:.1}", n as f64 / 1_048_576.))),
        false => put(&say!("{} KB", (n as f64 / 1024.).round().max(1.) as u64)),
    }
}

fn ordinal(n: usize) -> String {
    let (last, last_two) = (n % 10, n % 100);
    match (n, last, last_two) {
        (1, ..) => put(&say!("1st")),
        (_, 1, t) if t != 11 => put(&say!("{}st", n)),
        (_, 2, t) if t != 12 => put(&say!("{}nd", n)),
        (_, 3, t) if t != 13 => put(&say!("{}rd", n)),
        _ => put(&say!("{}th", n)),
    }
}

pub struct CompareView {
    sides: (ModId, ModId),
    places: (usize, usize),
    comparison: Option<Rc<Comparison>>,
    listed: Rc<Vec<Shared>>,
    /// Each picture's pixels, read once until the textures are read again.
    sizes: RefCell<HashMap<String, Option<(u32, u32)>>>,
    /// The keys come here when it is first shown.
    focused_once: bool,
    pub(super) filter: Entity<TextField>,
    chosen: usize,
    /// 0 side by side, 1 a slider, 2 the difference.
    mode: usize,
    zoom: f32,
    /// Where both pictures are scrolled to when zoomed, as a share of the way: 0 to 1.
    pan: Point<f32>,
    cut: f32,
    problem: String,
    difference: Option<(usize, Result<Difference, String>)>,
    differing: crate::jobs::Lane,
    dragged: Option<(Point<Pixels>, Point<f32>)>,
    boxes: Rc<RefCell<HashMap<&'static str, Size<Pixels>>>>,
    /// Each box's texture, let go once the box shows another.
    pictures: RefCell<HashMap<&'static str, crate::ui::pictures::Pictures>>,
    scroll: Scrolling,
    pub(super) focus: FocusHandle,
    /// Every texture is being taken from one side: a second press waits for it.
    keeping: bool,
}

impl CompareView {
    pub fn new(sides: (ModId, ModId), places: (usize, usize), cx: &mut Context<Self>) -> Self {
        let theme = cx.global::<Theme>().clone();
        let filter = cx.new(|cx| TextField::new(theme.body(), cx));
        cx.subscribe(&filter, |view, _, _: &Edited, cx| {
            view.chosen = 0;
            view.refilter(cx);
            cx.notify();
        })
        .detach();
        let scroll = Scrolling::default();
        scroll
            .borrow_mut()
            .set_reduce_motion(theme.appearance.reduce_motion);
        let mut view = Self {
            sides,
            places,
            comparison: None,
            listed: Rc::default(),
            sizes: RefCell::default(),
            focused_once: false,
            filter,
            chosen: 0,
            mode: 0,
            zoom: 0.,
            pan: point(0.5, 0.5),
            cut: 0.5,
            problem: String::new(),
            difference: None,
            differing: crate::jobs::Lane::default(),
            dragged: None,
            boxes: Rc::default(),
            pictures: Default::default(),
            scroll,
            focus: cx.focus_handle(),
            keeping: false,
        };
        view.comparison = crate::ui::app(cx)
            .compare_textures(sides.0, sides.1)
            .map(Rc::new);
        view.refilter(cx);
        view
    }

    fn refilter(&mut self, cx: &gpui::App) {
        let filter = self.filter.read(cx).text().to_string();
        self.listed = Rc::new(
            self.comparison
                .as_ref()
                .map(|c| c.named(&filter).into_iter().cloned().collect())
                .unwrap_or_default(),
        );
    }

    /// A picture's pixels, its header read the first time it is asked for.
    fn pixels_of(&self, file: &str) -> Option<(u32, u32)> {
        *self
            .sizes
            .borrow_mut()
            .entry(file.to_string())
            .or_insert_with(|| pixels(file))
    }

    pub fn comparison(&self) -> Option<Rc<Comparison>> {
        self.comparison.clone()
    }

    fn reload(&mut self, cx: &mut Context<Self>) {
        let (earlier, later) = self.sides;
        self.comparison = crate::ui::app(cx)
            .compare_textures(earlier, later)
            .map(Rc::new);
        self.refilter(cx);
        self.sizes.borrow_mut().clear();
        self.chosen = self.chosen.min(self.listed.len().saturating_sub(1));
        self.difference = None;
        cx.emit(Told::Compared(self.comparison.clone()));
        cx.notify();
    }

    fn texture(&self) -> Option<Shared> {
        self.listed.get(self.chosen).cloned()
    }

    fn keep(&mut self, earlier: bool, cx: &mut Context<Self>) {
        let Some(texture) = self.texture() else {
            return;
        };
        let done = crate::ui::app(cx).keep_textures(&[&texture], earlier);
        self.problem = crate::words::why_not(&done);
        self.reload(cx);
    }

    /// Every shared texture taken from one side; keeping the earlier mod's says so, with Undo.
    pub(super) fn keep_all(&mut self, earlier: bool, cx: &mut Context<Self>) {
        let Some(comparison) = self.comparison.clone().filter(|_| !self.keeping) else {
            return;
        };
        self.keeping = true;
        let app = crate::ui::app(cx);
        let later = comparison.later.id;
        let comparison = (*comparison).clone();
        crate::jobs::run(
            "keep textures",
            cx,
            move || app.keep_all_textures(&comparison, earlier),
            move |view, kept, cx| {
                view.keeping = false;
                match kept {
                    Ok(said) if earlier => {
                        let me = cx.entity().downgrade();
                        notices::offer(
                            put(&said),
                            put(&say!("Undo")),
                            move |_, cx| {
                                if let Err(problem) = crate::ui::app(cx).unhide_mod(later) {
                                    notices::say(
                                        crate::words::said_of(&problem),
                                        notices::Tone::Err,
                                        cx,
                                    );
                                }
                                let _ = me.update(cx, |view, cx| view.reload(cx));
                            },
                            cx,
                        );
                        view.problem.clear();
                    }
                    Ok(_) => view.problem.clear(),
                    Err(problem) => view.problem = crate::words::said_of(&problem),
                }
                view.reload(cx);
            },
        );
    }

    fn step(&mut self, by: isize, cx: &mut Context<Self>) {
        let last = self.listed.len().saturating_sub(1) as isize;
        self.chosen = (self.chosen as isize + by).clamp(0, last.max(0)) as usize;
        self.difference = None;
        let theme = cx.global::<Theme>().clone();
        let tops = self.tops(&theme);
        if self.chosen + 1 < tops.len() {
            self.scroll
                .borrow_mut()
                .contain(tops[self.chosen], tops[self.chosen + 1]);
        }
        cx.notify();
    }

    fn done(&mut self, cx: &mut Context<Self>) {
        cx.emit(Told::Done);
    }

    fn key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let typing = self.filter.read(cx).is_focused(window);
        let key = event.keystroke.key.as_str();
        if typing && !matches!(key, "up" | "down") {
            return;
        }
        match key {
            "up" | "left" => self.step(-1, cx),
            "down" | "right" => self.step(1, cx),
            "escape" => self.done(cx),
            "1" => self.keep(true, cx),
            "2" => self.keep(false, cx),
            _ => return,
        }
        cx.stop_propagation();
    }

    fn row_height(theme: &Theme) -> Pixels {
        theme
            .sized(40.)
            .max(theme.line(theme.body()) + theme.line(theme.small()) + theme::GAP_TINY * 2.)
    }

    fn tops(&self, theme: &Theme) -> Rc<[Pixels]> {
        let row = Self::row_height(theme);
        (0..=self.listed.len()).map(|at| row * at as f32).collect()
    }

    /// The difference of the pair chosen, made off the window's thread when first shown.
    fn want_difference(&mut self, texture: &Shared, cx: &mut Context<Self>) {
        if self
            .difference
            .as_ref()
            .is_some_and(|(at, _)| *at == texture.at)
        {
            return;
        }
        let (app, texture) = (crate::ui::app(cx), texture.clone());
        let at = texture.at;
        self.differing.run(
            "texture difference",
            cx,
            move |_| {
                app.texture_difference(&texture)
                    .map_err(|p| crate::words::said_of(&p))
            },
            move |view, made, cx| {
                view.difference = Some((at, made));
                cx.notify();
            },
        );
    }
}

impl Render for CompareView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.global::<Theme>().clone();
        let c = theme.colours;
        if !std::mem::replace(&mut self.focused_once, true) {
            window.focus(&self.focus, cx);
        }
        let texture = self.texture();
        let same_size = texture
            .as_ref()
            .and_then(|t| {
                Some(self.pixels_of(&t.earlier_shown)? == self.pixels_of(&t.later_shown)?)
            })
            .unwrap_or(false);
        if !same_size && self.mode == 2 {
            self.mode = 0;
        }
        if let (Some(texture), 2) = (&texture, self.mode) {
            self.want_difference(texture, cx);
        }
        let total = self.comparison.as_ref().map_or(0, |c| c.total);
        let views: Vec<SharedString> = match same_size {
            true => vec![
                put(&say!("Side by side")).into(),
                put(&say!("Slider")).into(),
                put(&say!("Difference")).into(),
            ],
            false => vec![
                put(&say!("Side by side")).into(),
                put(&say!("Slider")).into(),
            ],
        };
        let zoom_at = ZOOMS.iter().position(|z| *z == self.zoom).unwrap_or(0);
        let title = texture
            .as_ref()
            .map(|t| t.name.clone())
            .unwrap_or_else(|| put(&say!("These two mods share no textures now.")));
        let body = match (&texture, self.mode) {
            (None, _) => div().flex_1(),
            (Some(texture), 0) => div()
                .flex()
                .flex_1()
                .min_h_0()
                .gap(theme::GAP_WIDE)
                .p(theme::GAP_WIDE)
                .pt(theme::GAP_TINY)
                .child(self.side(&theme, texture, true, cx))
                .child(self.side(&theme, texture, false, cx)),
            (Some(texture), 1) => self.slider(&theme, texture, cx),
            (Some(_), _) => {
                let said = match &self.difference {
                    Some((_, Ok(found))) if found.differing == 0 => {
                        put(&say!("The two pictures are the same"))
                    }
                    Some((_, Ok(found))) => put(&say!(
                        "{} of {} pixels differ ({}%), shown as in {}",
                        count(found.differing as usize),
                        count(found.total as usize),
                        format!(
                            "{:.1}",
                            100. * found.differing as f64 / found.total.max(1) as f64
                        ),
                        self.comparison
                            .as_ref()
                            .map(|c| c.later.name.clone())
                            .unwrap_or_default()
                    )),
                    Some((_, Err(problem))) => problem.clone(),
                    None => String::new(),
                };
                let failed = matches!(self.difference, Some((_, Err(_))));
                let file = match &self.difference {
                    Some((_, Ok(found))) => found.file.clone(),
                    _ => String::new(),
                };
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .gap(theme::GAP)
                    .p(theme::GAP_WIDE)
                    .pt(theme::GAP_TINY)
                    .child(self.picture(&theme, "compare-difference", file, false, cx))
                    .child(
                        div()
                            .sized_text(&theme, theme.body())
                            .text_color(if failed { c.err } else { c.ink2 })
                            .child(said),
                    )
            }
        };
        let hint = match self.zoom > 0. {
            true => put(&say!("Dragging moves both sides together")),
            false => put(&say!("Zoom in to look closer")),
        };
        div()
            .id("compare")
            .track_focus(&self.focus)
            .key_context("Compare")
            .on_key_down(cx.listener(Self::key))
            .flex()
            .size_full()
            .bg(c.win)
            .child(
                div()
                    .relative()
                    .flex()
                    .flex_col()
                    .flex_none()
                    .w(theme.sized(290.))
                    .h_full()
                    .bg(c.panel)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(theme::GAP)
                            .h(theme.header())
                            .pl(theme::GAP_LARGE)
                            .pr(theme::GAP_CARD)
                            .child(
                                Btn::new("compare-back", put(&say!("‹ Load order")))
                                    .on_click(cx.listener(|view, _, _, cx| view.done(cx))),
                            )
                            .child(div().flex_1())
                            .child(label(
                                &theme,
                                theme.small(),
                                c.ink2,
                                put(&say!("{} shared", total)),
                            )),
                    )
                    .child(
                        div().px(theme::GAP_LARGE).pb(theme::GAP).child(
                            Field {
                                id: "compare-filter",
                                text: &self.filter,
                                icon: "",
                                placeholder: put(&say!("Filter textures")),
                                found: None,
                                keys: None,
                            }
                            .render(&theme, window, cx),
                        ),
                    )
                    .child(self.list(&theme, window, cx))
                    .child(
                        div()
                            .absolute()
                            .top_0()
                            .bottom_0()
                            .right_0()
                            .w(px(1.))
                            .bg(c.edge),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(theme::GAP)
                            .h(theme.header())
                            .pl(theme::GAP_CARD)
                            .pr(theme::GAP_LARGE)
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .overflow_hidden()
                                    .whitespace_nowrap()
                                    .sized_text(&theme, theme.title())
                                    .font_weight(theme.semibold)
                                    .text_color(c.ink)
                                    .child(elide_start_in(
                                        window,
                                        &theme,
                                        theme.family.clone(),
                                        &title,
                                        theme.title(),
                                        crate::ui::rows::bar_room(&theme) * 30.,
                                    )),
                            )
                            .child(
                                Choice::new("compare-view", views, self.mode)
                                    .title(put(&say!("View")))
                                    .on_pick(crate::ui::act(cx, |view, at: usize, _, cx| {
                                        view.mode = at;
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Choice::new(
                                    "compare-zoom",
                                    ["Fit", "2×", "4×", "8×"].map(SharedString::from).to_vec(),
                                    zoom_at,
                                )
                                .title(put(&say!("Zoom")))
                                .on_pick(crate::ui::act(
                                    cx,
                                    |view, at: usize, _, cx| {
                                        view.zoom = ZOOMS[at];
                                        view.pan = point(0.5, 0.5);
                                        cx.notify();
                                    },
                                )),
                            ),
                    )
                    .when(!self.problem.is_empty(), |d| {
                        d.child(
                            div()
                                .px(theme::GAP_WIDE)
                                .sized_text(&theme, theme.body())
                                .text_color(c.err)
                                .child(put(&say!("Failed: {}", self.problem.clone()))),
                        )
                    })
                    .child(body)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(theme::GAP_WIDE)
                            .h(theme.sized(28.))
                            .px(theme::GAP_CARD)
                            .bg(c.panel)
                            .child(label(
                                &theme,
                                theme.small(),
                                c.ink2,
                                put(&say!("← → next texture")),
                            ))
                            .child(label(
                                &theme,
                                theme.small(),
                                c.ink2,
                                put(&say!("1 2 keep left or right")),
                            ))
                            .child(div().flex_1())
                            .child(label(&theme, theme.small(), c.ink2, hint)),
                    ),
            )
    }
}

impl Mods {
    pub(super) fn open_compare(&mut self, earlier: ModId, later: ModId, cx: &mut Context<Self>) {
        let list = self
            .list
            .read(cx)
            .order
            .list(crate::workspace::load_order::Side::Active);
        let place = |id| list.position_of(id).map_or(0, |at| at + 1);
        let places = (place(earlier), place(later));
        let view = cx.new(|cx| CompareView::new((earlier, later), places, cx));
        cx.subscribe(&view, |mods, _, told: &Told, cx| match told {
            Told::Compared(compared) => mods.show_compared(compared.clone(), cx),
            Told::Done => mods.close_compare(cx),
        })
        .detach();
        self.compared = view.read(cx).comparison();
        self.comparing = Some(view);
        self.sync_details(cx);
        cx.notify();
    }

    /// A reading of the mods voids the ids compared.
    pub(super) fn follow_compare(&mut self, changes: Changes, cx: &mut Context<Self>) {
        let Some(view) = self.comparing.clone() else {
            return;
        };
        if changes.contains(Changes::LIBRARY) {
            self.close_compare(cx);
        } else if changes.contains(Changes::TEXTURES | Changes::LOAD_ORDER) {
            view.update(cx, |view, cx| view.reload(cx));
        }
    }

    /// What a reading of the mods would void: a change seen on disk waits for it.
    pub fn held(&self, cx: &gpui::App) -> bool {
        self.comparing.is_some() || self.list.read(cx).held()
    }

    pub(super) fn close_compare(&mut self, cx: &mut Context<Self>) {
        self.comparing = None;
        self.compared = None;
        self.focus_wanted = true;
        self.sync_details(cx);
        cx.notify();
    }

    pub fn compare_first(&mut self, cx: &mut Context<Self>) {
        if !self.list.read(cx).files_read {
            return;
        }
        let app = crate::ui::app(cx);
        let ids: Vec<ModId> = self
            .active
            .rows
            .iter()
            .filter(|r| !r.is_header)
            .filter_map(|r| r.mod_id)
            .collect();
        for id in ids {
            let details = self.list.update(cx, |list, _| list.order.details(&app, id));
            if let Some(other) = details
                .loses_to
                .iter()
                .chain(&details.wins_over)
                .next()
                .map(|clash| clash.mod_id)
            {
                self.choose(crate::workspace::load_order::Side::Active, id, cx);
                self.compare_with(other, cx);
                return;
            }
        }
    }

    fn show_compared(&mut self, compared: Option<Rc<Comparison>>, cx: &mut Context<Self>) {
        self.compared = compared;
        self.sync_details(cx);
    }
}

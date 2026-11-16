//! `Theme.quick` and `Theme.calm`, and the motions `PopIn`, `PopOut` and `Appear`
//! give: what is opened fades in, what closes fades out, a section shown fades in. gpui-fast
//! scales no `div`, so `PopIn`'s growth from 0.97 is left out. Reduce motion stills them all
//! (`AnimationExt` reads `App::reduce_motion`).

use gpui::{Animation, AnimationExt, ElementId, InteractiveElement, IntoElement, Styled};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

pub const QUICK: Duration = Duration::from_millis(140);
pub const CALM: Duration = Duration::from_millis(220);

pub fn out_cubic(t: f32) -> f32 {
    1. - (1. - t).powi(3)
}

static OPENINGS: AtomicUsize = AtomicUsize::new(0);

/// A turn of its own for each opening, so that its motion starts again.
pub fn opening() -> usize {
    OPENINGS.fetch_add(1, Ordering::Relaxed) + 1
}

pub fn pop_in<E: Styled + IntoElement + 'static>(element: E, turn: usize) -> impl IntoElement {
    element.with_animation(("pop-in", turn), Animation::new(QUICK), |e, t| e.opacity(t))
}

pub fn pop_out<E: Styled + IntoElement + 'static>(element: E, turn: usize) -> impl IntoElement {
    element.with_animation(("pop-out", turn), Animation::new(QUICK), |e, t| {
        e.opacity(1. - t)
    })
}

pub fn appear<E: Styled + IntoElement + 'static>(
    element: E,
    id: impl Into<ElementId>,
) -> impl IntoElement {
    element.with_animation(id, Animation::new(CALM).with_easing(out_cubic), |e, t| {
        e.opacity(t)
    })
}

/// A spring that settles in about `settled`, as `Behavior on x` with `Easing.OutCubic` did, to
/// within `epsilon` of `to` in its own measure; it starts from where it was, so a change met
/// halfway turns back smoothly, and a new one starts at its end.
pub fn towards<T: gpui::SpringTarget>(
    to: T,
    epsilon: f32,
    settled: Duration,
) -> gpui::SpringAnimation<T> {
    // Critically damped: (1 + 7) e^-7, under 1% of the way, is left once `settled` is over.
    let frequency = 7. / settled.as_secs_f32();
    gpui::SpringAnimation::new(gpui::SpringConfig::new(
        frequency * frequency,
        2. * frequency,
        1.,
    ))
    .to(to)
    .with_epsilon(epsilon)
}

thread_local! {
    /// The controls pressed, and when: only their springs run, so a value that arrives after a
    /// control is shown shows at once.
    static PRESSED: std::cell::RefCell<std::collections::HashMap<ElementId, std::time::Instant>> =
        Default::default();
}

pub fn pressed(id: &ElementId) {
    PRESSED.with_borrow_mut(|pressed| {
        pressed.insert(id.clone(), std::time::Instant::now());
    });
}

/// Running for a control pressed within the last second, at its end otherwise: a pick may come
/// back through a read off the window's thread (Settings reads its report again, Git's version
/// among it), so the second is a bound on that read, not on the motion.
pub fn playback(id: &ElementId) -> gpui::SpringPlayback {
    let running = PRESSED.with_borrow_mut(|pressed| {
        pressed.retain(|_, when| when.elapsed() < Duration::from_secs(1));
        pressed.contains_key(id)
    });
    match running {
        true => gpui::SpringPlayback::Running,
        false => gpui::SpringPlayback::Completed,
    }
}

/// Laid over what fades out: it stops the presses, so nothing in it is pressed while it goes,
/// and lets the wheel and the pointer through to what is under it.
pub fn shield() -> gpui::Div {
    gpui::div()
        .absolute()
        .inset_0()
        .on_any_mouse_down(|_, _, cx| cx.stop_propagation())
}

pub fn fades_out(cx: &gpui::App) -> bool {
    !cx.reduce_motion()
}

/// `done` once a fade-out begun now is over.
pub fn after_fade<V: 'static>(
    cx: &mut gpui::Context<V>,
    done: impl FnOnce(&mut V, &mut gpui::Context<V>) + 'static,
) {
    cx.spawn(async move |view, cx| {
        cx.background_executor().timer(QUICK).await;
        let _ = view.update(cx, done);
    })
    .detach();
}

/// Whether the pointer is on the control `id`, kept between renders, and what its `on_hover`
/// is given to keep it: the view drawing it is drawn again when it changes.
pub fn hovered(
    id: &ElementId,
    window: &mut gpui::Window,
    cx: &mut gpui::App,
) -> (
    bool,
    impl Fn(&bool, &mut gpui::Window, &mut gpui::App) + 'static,
) {
    let kept = window.use_keyed_state(ElementId::from((id.clone(), "hovered")), cx, |_, _| false);
    let now = *kept.read(cx);
    let keep = move |on: &bool, _: &mut gpui::Window, cx: &mut gpui::App| {
        let on = *on;
        kept.update(cx, |kept, cx| {
            if *kept != on {
                *kept = on;
                cx.notify();
            }
        });
    };
    (now, keep)
}

/// `Behavior on color` for a ground that changes with a state (pointed at, focused): `over`
/// laid on it, fading in as `on` becomes true and out as it becomes false, in `QUICK`.
pub fn layer(over: gpui::Div, on: bool) -> impl IntoElement {
    over.with_spring(
        "layer",
        towards(gpui::AnimationPhase(f32::from(u8::from(on))), 0.01, QUICK),
        |d, phase| d.opacity(phase.0.clamp(0., 1.)),
    )
}

pub fn over(ground: gpui::Hsla, wanted: gpui::Hsla) -> gpui::Hsla {
    match wanted.a >= 1. || ground.a >= 1. {
        true => wanted,
        false => gpui::Hsla {
            a: ((wanted.a - ground.a) / (1. - ground.a)).clamp(0., 1.),
            ..wanted
        },
    }
}

pub fn slid_in<E: Styled + IntoElement + 'static>(
    element: E,
    width: gpui::Pixels,
    id: impl Into<ElementId>,
) -> impl IntoElement {
    element.with_animation(
        id,
        Animation::new(CALM).with_easing(out_cubic),
        move |e, t| e.left(-width * (1. - t)),
    )
}

struct Coloured {
    now: gpui::Hsla,
    before: gpui::Hsla,
    turn: usize,
    when: std::time::Instant,
}

/// `Behavior on color` for a colour that changes from outside (a chip's tone): the colour shown
/// before the last change, and the turn of that change, while it is under `QUICK` or so old.
pub fn colour_change(
    id: &ElementId,
    now: gpui::Hsla,
    window: &mut gpui::Window,
) -> Option<(gpui::Hsla, usize)> {
    window.with_global_id(ElementId::from((id.clone(), "colour")), |id, window| {
        window.with_element_state(id, |kept: Option<Coloured>, _| {
            let mut kept = kept.unwrap_or(Coloured {
                now,
                before: now,
                turn: 0,
                when: std::time::Instant::now() - QUICK * 3,
            });
            if kept.now != now {
                kept.before = kept.now;
                kept.now = now;
                kept.turn = opening();
                kept.when = std::time::Instant::now();
            }
            let recent = kept.when.elapsed() < QUICK * 3;
            (recent.then_some((kept.before, kept.turn)), kept)
        })
    })
}

pub fn colour_fading(was: gpui::Div, turn: usize) -> impl IntoElement {
    was.with_animation(("colour-fading", turn), Animation::new(QUICK), |d, t| {
        d.opacity(1. - t)
    })
}

//! `--selftest`: the load order scrolled from top to bottom in ten seconds.
//! Each frame moves the list and draws it again, as a wheel's glide does.

use super::list::ModList;
use gpui::{Context, Window};
use std::time::{Duration, Instant};

const LENGTH: Duration = Duration::from_secs(10);
/// A frame slower than this counts as slow.
const SLOW: Duration = Duration::from_millis(20);

pub struct Bench {
    started: Instant,
    last: Instant,
    frames: usize,
    slow: usize,
    worst: Duration,
    cpu: Duration,
}

fn cpu_time() -> Duration {
    let stat = std::fs::read_to_string("/proc/self/stat").unwrap_or_default();
    let fields: Vec<&str> = stat
        .rsplit(')')
        .next()
        .unwrap_or_default()
        .split_whitespace()
        .collect();
    let ticks = |at: usize| {
        fields
            .get(at)
            .and_then(|t| t.parse::<u64>().ok())
            .unwrap_or(0)
    };
    // Fields 14 and 15 of `proc(5)`, counted after the name; ticks of 1/100 s on Linux.
    Duration::from_millis((ticks(11) + ticks(12)) * 10)
}

impl ModList {
    pub fn start_bench(&mut self, cx: &mut Context<Self>) {
        let now = Instant::now();
        self.bench = Some(Bench {
            started: now,
            last: now,
            frames: 0,
            slow: 0,
            worst: Duration::ZERO,
            cpu: cpu_time(),
        });
        cx.notify();
    }

    pub(super) fn bench_frame(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let scroll = self.scroll().clone();
        let Some(bench) = &mut self.bench else {
            return;
        };
        let now = Instant::now();
        let took = now - bench.last;
        if bench.frames > 0 {
            bench.slow += usize::from(took > SLOW);
            bench.worst = bench.worst.max(took);
        }
        if bench.frames == 0 {
            // A pointer resting on the list, as a hand on the wheel leaves it: what follows the
            // pointer is drawn as the rows pass under it.
            let size = window.viewport_size();
            let at = gpui::point(size.width * 0.6, size.height / 2.);
            window.on_next_frame(move |window, cx| {
                window.dispatch_event(
                    gpui::PlatformInput::MouseMove(gpui::MouseMoveEvent {
                        position: at,
                        pressed_button: None,
                        modifiers: gpui::Modifiers::default(),
                    }),
                    cx,
                );
            });
        }
        (bench.frames, bench.last) = (bench.frames + 1, now);
        let part = ((now - bench.started).as_secs_f32() / LENGTH.as_secs_f32()).min(1.);
        let most = scroll.borrow().most();
        scroll.borrow_mut().set_top(most * part);
        if part < 1. {
            window.request_animation_frame();
            return;
        }
        let cpu = cpu_time() - bench.cpu;
        println!(
            "scroll frames={} slow={} worst_ms={:.1} cpu_ms={}",
            bench.frames,
            bench.slow,
            bench.worst.as_secs_f64() * 1000.,
            cpu.as_millis()
        );
        self.bench = None;
        cx.quit();
    }
}

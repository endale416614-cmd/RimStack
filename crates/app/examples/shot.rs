//! The window's picture with no window shown: `just shot <out.png> [--theme=… --text-scale=… --size=1000x560]`.

use gpui::{AppContext, HeadlessAppContext, size};
use rimstack_app::{App, ui};
use std::sync::Arc;
use std::time::{Duration, Instant};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let out = args
        .iter()
        .find(|a| !a.starts_with("--"))
        .cloned()
        .unwrap_or("shot.png".into());
    let wait = args
        .iter()
        .find_map(|a| a.strip_prefix("--shot-delay="))
        .and_then(|v| v.parse().ok())
        .unwrap_or(400);
    let (width, height) = args
        .iter()
        .find_map(|a| a.strip_prefix("--size="))
        .and_then(|v| v.split_once('x'))
        .and_then(|(w, h)| Some((w.parse::<f32>().ok()?, h.parse::<f32>().ok()?)))
        .map_or(ui::WINDOW, |(w, h)| (gpui::px(w), gpui::px(h)));
    let mut options = ui::Options::from_args(args.iter().cloned());
    options.still = !args.iter().any(|a| a == "--live");
    let app = Arc::new(App::from_environment());
    rimstack_app::words::show_in(rimstack_app::words::Words::of(&app.speak()));
    let text = gpui_platform::current_platform(true).text_system();
    let mut cx = HeadlessAppContext::with_platform(
        text,
        Arc::new(ui::assets::Assets),
        gpui_platform::current_headless_renderer,
    );
    cx.update(|cx| ui::install(app, &options, cx));
    let window = cx
        .open_window(size(width, height), |window, cx| {
            cx.new(|cx| ui::shell::Shell::new(&options, window, cx))
        })
        .expect("a window opens with no display");
    let started = Instant::now();
    while started.elapsed() < Duration::from_millis(wait) {
        cx.run_until_parked();
        std::thread::sleep(Duration::from_millis(10));
    }
    cx.run_until_parked();
    let picture = cx
        .capture_screenshot(window.into())
        .expect("the window is drawn");
    picture.save(&out).expect("the picture is written");
    println!("{out} {}x{}", picture.width(), picture.height());
}

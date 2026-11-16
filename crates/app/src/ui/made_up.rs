#[cfg(target_os = "linux")]
pub(crate) fn made_up() -> (tempfile::TempDir, crate::App) {
    use crate::App;
    use rimstack_core::base::program;
    use rimstack_core::rimworld::paths::Overrides;
    use std::time::Duration;
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let tools = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools");
    let fx = root.join("fx");
    let run = |script: &str, args: &[&std::path::Path]| {
        let mut command = program::command(tools.join(script));
        command.args(args);
        let done = crate::jobs::waiting_here(|| {
            program::run(&mut command, Duration::from_secs(60)).unwrap()
        });
        assert!(
            done.status.success(),
            "{script}: {}",
            String::from_utf8_lossy(&done.stderr)
        );
    };
    run("fixture-game.sh", &[&fx]);
    run("fixture-downloads.sh", &[&fx, &root.join("data/RimStack")]);
    let given = Overrides {
        game: Some(fx.join("game")),
        config: Some(fx.join("config")),
        workshop: Some(fx.join("steam/workshop/content/294100")),
    };
    let app = App::new(
        root.join("home"),
        &root.join("settings"),
        &root.join("data"),
        given,
    );
    // As `just shot-fx` writes it: the first run is behind, as on an install in use.
    app.keep_setting("setupDone", "yes").unwrap();
    (tmp, app)
}

/// The platform's text system, made once for every test: a platform made per test leaves its
/// sixteen worker threads behind, 4,700 threads by the end of a run (2026-11-12).
pub(crate) fn text_system() -> std::sync::Arc<dyn gpui::PlatformTextSystem> {
    static MADE: std::sync::OnceLock<std::sync::Arc<dyn gpui::PlatformTextSystem>> =
        std::sync::OnceLock::new();
    MADE.get_or_init(|| gpui_platform::current_platform(true).text_system())
        .clone()
}

/// One step of a test waiting on the made-up install: the window's waits move on by as much as
/// the `App`'s own threads are given.
#[cfg(target_os = "linux")]
pub(crate) fn step(cx: &gpui::HeadlessAppContext) {
    const STEP: std::time::Duration = std::time::Duration::from_millis(10);
    cx.advance_clock(STEP);
    cx.run_until_parked();
    std::thread::sleep(STEP);
}

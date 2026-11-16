#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use rimstack_app::{App, STARTED, jobs, ui, words};
use rimstack_core::said;
use std::sync::Arc;
use std::time::Instant;

/// Written where the desktop looks for them.
#[cfg(target_os = "linux")]
fn put_among_applications() {
    let home = std::env::var_os("XDG_DATA_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| Some(std::path::PathBuf::from(std::env::var_os("HOME")?).join(".local/share")));
    let (Some(home), Ok(exe)) = (home, std::env::current_exe()) else {
        return;
    };
    let Some(exe) =
        rimstack_core::kept::desktop::program_to_list(&exe, |name| std::env::var_os(name))
    else {
        return;
    };
    let entry = include_str!("../assets/rimstack.desktop");
    let icon = include_bytes!("../assets/rimstack.svg");
    match rimstack_core::kept::desktop::install(&home, &exe, entry, icon) {
        // KDE reads icons again on this signal; a running desktop holds them otherwise.
        Ok(true) => {
            let told = std::process::Command::new("dbus-send")
                .args(["--session", "--type=signal", "/KIconLoader"])
                .args(["org.kde.KIconLoader.iconChanged", "int32:0"])
                .status();
            if let Err(problem) = told {
                rimstack_core::problem!("icons changed, desktop not told: {problem}");
            }
        }
        Ok(false) => {}
        Err(problem) => rimstack_core::problem!("desktop entry: {problem}"),
    }
}

/// `rimstack --steamworks=subscribe:1,2`; exits 0 when Steam took the requests.
fn steamworks(app: &App, request: &str) -> std::process::ExitCode {
    use rimstack_core::base::error::Error;
    use rimstack_core::rimworld::steamworks;
    let asked = steamworks::parse(request).ok_or_else(|| Error::refused("not a request"));
    let library = app.paths().game.as_deref().and_then(steamworks::library);
    let done = asked.and_then(|(action, ids)| {
        let library = library
            .ok_or_else(|| Error::program(said!("the game's Steam library was not found")))?;
        steamworks::act(&library, action, &ids)
    });
    match done {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(problem) => {
            eprintln!("{problem}");
            std::process::ExitCode::FAILURE
        }
    }
}

fn main() -> std::process::ExitCode {
    let steam_request =
        std::env::args().find_map(|a| a.strip_prefix("--steamworks=").map(String::from));
    // Before any thread is started, as changing the environment must be.
    if steam_request.is_some() {
        for (name, value) in rimstack_core::rimworld::steamworks::environment() {
            unsafe { std::env::set_var(name, value) };
        }
    }
    let app = Arc::new(App::from_environment());
    if let Some(request) = steam_request {
        return steamworks(&app, &request);
    }
    STARTED.get_or_init(Instant::now);
    if let Err(problem) = rimstack_core::base::diagnostics::start(&app.diagnostics_file()) {
        rimstack_core::problem!("{}: {problem}", app.diagnostics_file().display());
    }
    rimstack_core::base::diagnostics::record_panics();
    rimstack_core::note!(
        "RimStack {} started on {}",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS
    );
    // Before the mods are read: their problems are put in words as they are.
    words::show_in(words::Words::of(&app.speak()));
    if !std::env::args().any(|a| a == "--demo" || a == "--selftest") {
        let early = app.clone();
        jobs::spawn("early read", move || {
            early.settle_downloads();
            early.community();
            early.library();
        });
    }
    #[cfg(target_os = "linux")]
    if !std::env::args().any(|a| a.starts_with("--shot")) {
        put_among_applications();
    }
    ui::run(app.clone(), ui::Options::from_args(std::env::args()));
    app.settle_writes();
    std::process::ExitCode::SUCCESS
}

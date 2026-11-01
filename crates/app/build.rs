use cxx_qt_build::{CxxQtBuilder, QmlFile, QmlModule};

/// The commit this program is built from, for the nightly check. The two variables set
/// in the environment stand in where there is no `.git` (a source archive).
fn commit_of_build() {
    let git = |args: &[&str]| {
        let out = std::process::Command::new("git").args(args).output().ok()?;
        let text = String::from_utf8(out.stdout).ok()?.trim().to_string();
        (out.status.success() && !text.is_empty()).then_some(text)
    };
    let asked = |name: &str| std::env::var(name).ok().filter(|v| !v.is_empty());
    let commit = asked("RIMSTACK_COMMIT").or_else(|| git(&["rev-parse", "--short=7", "HEAD"]));
    let made =
        asked("RIMSTACK_COMMIT_TIME").or_else(|| git(&["show", "-s", "--format=%ct", "HEAD"]));
    if let (Some(commit), Some(made)) = (commit, made) {
        println!("cargo::rustc-env=RIMSTACK_COMMIT={commit}");
        println!("cargo::rustc-env=RIMSTACK_COMMIT_TIME={made}");
    }
    println!("cargo::rerun-if-env-changed=RIMSTACK_COMMIT");
    println!("cargo::rerun-if-env-changed=RIMSTACK_COMMIT_TIME");
    println!("cargo::rerun-if-changed=../../.git/HEAD");
}

fn main() {
    commit_of_build();
    let builder = CxxQtBuilder::new_qml_module(
        QmlModule::new("RimStack.Ui")
            .qml_file(QmlFile::from("qml/Theme.qml").singleton(true))
            .qml_file(QmlFile::from("qml/mods/Mods.qml").singleton(true))
            .qml_file(QmlFile::from("qml/Notices.qml").singleton(true))
            .qml_files([
                "qml/Main.qml",
                "qml/Appear.qml",
                "qml/PopIn.qml",
                "qml/PopOut.qml",
                "qml/TopBar.qml",
                "qml/StatusBar.qml",
                "qml/AppPrefs.qml",
                "qml/Icon.qml",
                "qml/Btn.qml",
                "qml/AskSteam.qml",
                "qml/FocusRing.qml",
                "qml/Tip.qml",
                "qml/Chip.qml",
                "qml/Card.qml",
                "qml/StatusCard.qml",
                "qml/Dots.qml",
                "qml/Slide.qml",
                "qml/Hint.qml",
                "qml/Scroller.qml",
                "qml/KeyCap.qml",
                "qml/KeysSheet.qml",
                "qml/Toasts.qml",
                "qml/HistoryMenu.qml",
                "qml/HuntBar.qml",
                "qml/mods/ProblemLine.qml",
                "qml/mods/BulkBar.qml",
                "qml/mods/ProblemsPanel.qml",
                "qml/CopyText.qml",
                "qml/PathText.qml",
                "qml/Defined.qml",
                "qml/Choice.qml",
                "qml/Toggle.qml",
                "qml/LaunchRecord.qml",
                "qml/RowKeys.qml",
                "qml/Wheel.qml",
                "qml/PopMenu.qml",
                "qml/FilterField.qml",
                "qml/GroupTitle.qml",
                "qml/NameList.qml",
                "qml/mods/ModsScreen.qml",
                "qml/mods/ModsPrefs.qml",
                "qml/mods/ModList.qml",
                "qml/mods/ModRow.qml",
                "qml/mods/RowMenu.qml",
                "qml/mods/InactivePane.qml",
                "qml/mods/ModsFilter.qml",
                "qml/mods/ActivePane.qml",
                "qml/mods/DetailsPane.qml",
                "qml/mods/CompareSummary.qml",
                "qml/mods/SelectionSummary.qml",
                "qml/mods/ModHeader.qml",
                "qml/mods/AboutTab.qml",
                "qml/mods/Columns.qml",
                "qml/mods/Dragging.qml",
                "qml/mods/LoadOrderTab.qml",
                "qml/mods/FilesTab.qml",
                "qml/mods/ListSummary.qml",
                "qml/EmptyState.qml",
                "qml/Prompt.qml",
                "qml/Scrolled.qml",
                "qml/mods/SortMoves.qml",
                "qml/checks/ModsChecks.qml",
                "qml/checks/PointerChecks.qml",
                "qml/checks/ScreensChecks.qml",
                "qml/checks/LayoutChecks.qml",
                "qml/SavesScreen.qml",
                "qml/LogsScreen.qml",
                "qml/TexturesScreen.qml",
                "qml/DownloadsScreen.qml",
                "qml/SettingsScreen.qml",
                "qml/ListsMenu.qml",
                "qml/FirstRun.qml",
                "qml/Commands.qml",
                "qml/CompareView.qml",
            ]),
    )
    .qrc_resources(["assets/rimstack.svg"])
    .qt_module("Quick")
    .qt_module("QuickControls2")
    .crate_include_root(None)
    .include_dir(std::env::var("CARGO_MANIFEST_DIR").expect("cargo sets the manifest folder"))
    .files([
        "src/hub.rs",
        "src/settings_data.rs",
        "src/mod_rows.rs",
        "src/mods_data.rs",
        "src/saves_data.rs",
        "src/logs_data.rs",
        "src/game_data.rs",
        "src/lists_data.rs",
        "src/search_data.rs",
        "src/textures_data.rs",
        "src/downloads_data.rs",
    ]);
    let msvc = std::env::var("CARGO_CFG_TARGET_ENV").is_ok_and(|env| env == "msvc");
    // SAFETY: one more source for the same compiler, nothing set that the generated code relies on.
    let builder = unsafe {
        builder.cc_builder(|cc| {
            cc.file("src/application.cpp");
            // qmlcachegen writes the QML's texts into C++ as UTF-8; without this MSVC
            // reads them in the system's code page and a ✓ shows as âœ“. Qt's CMake sets it.
            if msvc {
                cc.flag("/utf-8");
            }
        })
    };
    println!("cargo::rerun-if-changed=src/application.cpp");
    println!("cargo::rerun-if-changed=src/application.h");
    builder.build();
    // The program's own icon, which Windows shows for its shortcuts and on the taskbar.
    embed_resource::compile("assets/rimstack.rc", embed_resource::NONE)
        .manifest_required()
        .expect("the icon is compiled into the Windows program");
    println!("cargo::rerun-if-changed=assets/rimstack.rc");
    println!("cargo::rerun-if-changed=assets/rimstack.ico");
}

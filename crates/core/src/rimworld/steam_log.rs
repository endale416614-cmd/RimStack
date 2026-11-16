use super::paths::{self, APP_ID};
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

/// Steam keeps appending to the file and only clears it when it restarts; a game's start
/// and end are the last lines it wrote for it.
const TAIL: u64 = 256 * 1024;

/// Steam's own record of the games it started, in `logs/gameprocess_log.txt`: the game is
/// running from `AppID 294100 adding PID … as a tracked process` until
/// `Remove 294100 from running list`.
pub fn game_running(home: &Path) -> bool {
    paths::steam_folders(home)
        .iter()
        .filter_map(|steam| tail(&steam.join("logs/gameprocess_log.txt")))
        .any(|text| running_in(&text))
}

fn tail(file: &Path) -> Option<String> {
    let mut file = std::fs::File::open(file).ok()?;
    let length = file.metadata().ok()?.len();
    file.seek(SeekFrom::Start(length.saturating_sub(TAIL)))
        .ok()?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).ok()?;
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

fn running_in(text: &str) -> bool {
    let started = format!("AppID {APP_ID} adding PID ");
    let ended = format!("Remove {APP_ID} from running list");
    let last = text.lines().rev().find_map(|line| {
        if line.contains(&started) {
            Some(true)
        } else if line.contains(&ended) {
            Some(false)
        } else {
            None
        }
    });
    last.unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    const STARTED: &str =
        "[2026-11-15 00:02:13] AppID 294100 adding PID 2639419 as a tracked process
[2026-11-15 00:02:13] SSGL: InternalUpdateClientGame indicates change to games list
[2026-11-15 00:02:14] SSGL: change [294100] LCT 1073852875->0
";
    const ENDED: &str =
        "[2026-11-15 00:02:16] AppID 294100 no longer tracking PID 2639419, exit code -1
[2026-11-15 00:02:16] Remove 294100 from running list
";

    #[test]
    fn the_last_start_or_end_steam_wrote_for_the_game_decides() {
        assert!(running_in(STARTED));
        assert!(!running_in(&format!("{STARTED}{ENDED}")));
        assert!(running_in(&format!("{STARTED}{ENDED}{STARTED}")));
        assert!(!running_in(""));
        let other = "[2026-11-15 00:03:00] AppID 730 adding PID 12 as a tracked process\n";
        assert!(!running_in(&format!("{STARTED}{ENDED}{other}")));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn a_game_started_by_steam_is_seen_from_its_log_in_the_home() {
        let tmp = tempfile::tempdir().unwrap();
        let logs = tmp.path().join(".local/share/Steam/logs");
        std::fs::create_dir_all(&logs).unwrap();
        assert!(!game_running(tmp.path()));
        let padding = "[2026-11-15 00:00:00] SSGL: change\n".repeat(20_000);
        std::fs::write(
            logs.join("gameprocess_log.txt"),
            format!("{padding}{STARTED}"),
        )
        .unwrap();
        assert!(game_running(tmp.path()));
        std::fs::write(
            logs.join("gameprocess_log.txt"),
            format!("{STARTED}{ENDED}"),
        )
        .unwrap();
        assert!(!game_running(tmp.path()));
    }
}

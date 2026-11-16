use crate::base::locks::Held;
use crate::say_n;

use std::ffi::OsStr;
use std::io::Read;
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub const LOCAL: Duration = Duration::from_secs(30);
pub const NETWORK: Duration = Duration::from_secs(10 * 60);
pub const DOWNLOAD: Duration = Duration::from_secs(2 * 60 * 60);

const AFTER_END: Duration = Duration::from_secs(1);

pub fn command(program: impl AsRef<OsStr>) -> Command {
    let mut command = Command::new(program);
    without_window(&mut command);
    command
}

#[cfg(windows)]
fn without_window(command: &mut Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    command.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
fn without_window(_command: &mut Command) {}

type Gathered = Arc<(Mutex<Vec<u8>>, AtomicBool)>;

fn collected(pipe: Option<impl Read + Send + 'static>) -> Gathered {
    let gathered: Gathered = Arc::default();
    let into = gathered.clone();
    std::thread::spawn(move || {
        if let Some(mut pipe) = pipe {
            let mut chunk = [0; 8192];
            loop {
                match pipe.read(&mut chunk) {
                    Ok(0) => break,
                    Ok(read) => into.0.held().extend_from_slice(&chunk[..read]),
                    Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
                    Err(_) => break,
                }
            }
        }
        into.1.store(true, Ordering::Release);
    });
    gathered
}

/// Its stdin is empty. Past `limit` the program is killed, not what it started,
/// and the error is `TimedOut`.
pub fn run(command: &mut Command, limit: Duration) -> std::io::Result<Output> {
    crate::base::waits::slow(&format!(
        "the program {}",
        command.get_program().to_string_lossy()
    ));
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let (stdout, stderr) = (
        collected(child.stdout.take()),
        collected(child.stderr.take()),
    );
    let started = Instant::now();
    let mut pause = Duration::from_millis(1);
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if started.elapsed() >= limit {
            child.kill()?;
            child.wait()?;
            let seconds = usize::try_from(limit.as_secs()).unwrap_or(usize::MAX);
            return Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                say_n!(
                    seconds,
                    "no answer after {} second",
                    "no answer after {} seconds"
                )
                .to_string(),
            ));
        }
        std::thread::sleep(pause);
        pause = (pause * 2).min(Duration::from_millis(50));
    };
    let ended = Instant::now();
    let until = (started + limit).min(ended + AFTER_END);
    let done = |pipe: &Gathered| pipe.1.load(Ordering::Acquire);
    while !(done(&stdout) && done(&stderr)) && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(5));
    }
    let taken = |pipe: &Gathered| std::mem::take(&mut *pipe.0.held());
    Ok(Output {
        status,
        stdout: taken(&stdout),
        stderr: taken(&stderr),
    })
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn what_a_program_wrote_and_how_it_ended_come_back() {
        let ran = run(
            Command::new("sh").args(["-c", "echo out; echo err >&2; exit 3"]),
            LOCAL,
        )
        .unwrap();
        assert_eq!(ran.status.code(), Some(3));
        assert_eq!(
            (ran.stdout.as_slice(), ran.stderr.as_slice()),
            (&b"out\n"[..], &b"err\n"[..])
        );
    }

    #[test]
    fn more_than_a_pipe_holds_is_read_while_the_program_runs() {
        let ran = run(
            Command::new("sh").args(["-c", "head -c 300000 /dev/zero"]),
            LOCAL,
        )
        .unwrap();
        assert_eq!(ran.stdout.len(), 300_000);
    }

    #[test]
    fn a_program_that_does_not_end_is_stopped_at_the_limit() {
        let started = Instant::now();
        let ran = run(Command::new("sleep").arg("30"), Duration::from_millis(150));
        assert_eq!(ran.unwrap_err().kind(), std::io::ErrorKind::TimedOut);
        assert!(started.elapsed() < Duration::from_secs(5));
    }

    #[test]
    fn a_program_it_started_that_keeps_the_output_open_is_not_waited_for() {
        let started = Instant::now();
        let ran = run(
            Command::new("sh").args(["-c", "sleep 6 & echo started"]),
            LOCAL,
        )
        .unwrap();
        assert!(ran.status.success());
        assert_eq!(ran.stdout, b"started\n");
        assert!(
            started.elapsed() < Duration::from_secs(3),
            "{:?}",
            started.elapsed()
        );
    }
}

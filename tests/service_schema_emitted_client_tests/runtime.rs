//! What the runtime groups share: a workspace per run, the run itself, and the stand-down a
//! machine with no such runtime takes.

use core::sync::atomic::{AtomicU32, Ordering};
use std::env;
use std::env::temp_dir;
use std::fs;
use std::io::Write as _;
use std::io::stderr;
use std::path::PathBuf;
use std::process::{Command, id};
use std::sync::Mutex;

/// The process id alone does not separate two tests running beside each other.
static RUNS: AtomicU32 = AtomicU32::new(0);

/// The runtimes already reported absent: once per runtime, so two silent groups are not one.
static STOOD_DOWN: Mutex<Vec<&'static str>> = Mutex::new(Vec::new());

/// A directory of its own per run.
fn workspace(named: &str) -> PathBuf {
    let nth = RUNS.fetch_add(1, Ordering::Relaxed);
    let at = temp_dir().join(format!("tixschema-emitted-{named}-{run}-{nth}", run = id()));
    if at.exists() {
        fs::remove_dir_all(&at).unwrap();
    }
    fs::create_dir_all(&at).unwrap();
    at
}

/// Writes `entry` into a workspace of its own, runs it under the named runtime, and answers what
/// the process wrote to stdout.
///
/// `None` says no runtime was reachable and nothing ran — never that a run passed. A runtime named
/// explicitly in `var` that cannot be started is a failure instead.
pub fn ran(
    named: &str,
    var: &str,
    fallback: &'static str,
    entry: &str,
    source: &str,
) -> Option<String> {
    let chosen = env::var(var).ok();
    let runtime = chosen.clone().unwrap_or_else(|| fallback.to_owned());
    let at = workspace(named);
    fs::write(at.join(entry), source).unwrap();
    let run = Command::new(&runtime).arg(entry).current_dir(&at).output();
    let Ok(reported) = run else {
        assert!(
            chosen.is_none(),
            "{var} names `{runtime}`, and no runtime could be started there: {}",
            run.unwrap_err()
        );
        stand_down(var, fallback);
        fs::remove_dir_all(&at).unwrap();
        return None;
    };
    fs::remove_dir_all(&at).unwrap();
    assert!(
        reported.status.success(),
        "`{runtime} {entry}` failed.\n--- stdout ---\n{}\n--- stderr ---\n{}\n--- source ---\n{source}",
        String::from_utf8_lossy(&reported.stdout),
        String::from_utf8_lossy(&reported.stderr)
    );
    Some(String::from_utf8_lossy(&reported.stdout).into_owned())
}

/// Said on the process's own stderr, which `cargo test` does not capture, so a run that proved
/// nothing says so on the terminal.
fn stand_down(var: &str, fallback: &'static str) {
    let already = {
        let mut said = STOOD_DOWN.lock().unwrap();
        let seen = said.contains(&fallback);
        if !seen {
            said.push(fallback);
        }
        seen
    };
    if already {
        return;
    }
    let notice = format!(
        "\ntixschema: no `{fallback}` is reachable, so the emitted client was NOT run.\n  That \
         group stood down. Put `{fallback}` on PATH, or name one in {var}, and run `just \
         test-emitted`, which refuses to stand down.\n\n"
    );
    drop(stderr().write_all(notice.as_bytes()));
}

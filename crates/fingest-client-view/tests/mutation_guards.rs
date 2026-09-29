//! Every mutation a shell spawns goes through the busy helpers in `fingest_client_view`.
//!
//! A source scan, like `dependency_rule.rs`: components need a renderer to run, but the
//! rule that matters — resolve the session and claim the flag *before* `spawn` — is visible
//! in the text. Written after #38, where row actions skipped both and so sent duplicate
//! requests on a double click and did nothing, silently, without a session.

use std::{
    fs,
    path::{Path, PathBuf},
};

const SHELLS: &[&str] = &["fingest-web-shell", "fingest-mobile-shell"];

/// Calls that claim the busy flag, and for `start_action`/`begin` the session too.
const GUARDS: &[&str] = &["start_action(", "begin(", "claim(", "hold("];

/// How far above a `spawn` the guard may sit. Generous for a handler, far too short to
/// borrow a guard from a neighbouring one.
const WINDOW: usize = 12;

fn shell_sources() -> Vec<PathBuf> {
    let crates = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates/ is the parent of this crate")
        .to_path_buf();

    let mut files = Vec::new();
    for shell in SHELLS {
        collect(&crates.join(shell).join("src"), &mut files);
    }
    files.sort();
    assert!(!files.is_empty(), "no shell sources found");
    files
}

fn collect(dir: &Path, files: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir)
        .expect("shell src/ should exist")
        .flatten()
    {
        let path = entry.path();
        if path.is_dir() {
            collect(&path, files);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            files.push(path);
        }
    }
}

#[test]
fn no_spawned_task_bails_out_silently_without_a_session() {
    let offenders: Vec<String> = shell_sources()
        .into_iter()
        .filter(|path| {
            fs::read_to_string(path)
                .unwrap()
                .contains("let Some(session) = session else { return };")
        })
        .map(|path| path.display().to_string())
        .collect();

    assert!(
        offenders.is_empty(),
        "resolve the session before `spawn` with `start_action`: {offenders:?}"
    );
}

#[test]
fn every_spawned_mutation_claims_its_busy_flag_first() {
    let mut spawns = 0;
    let mut unguarded = Vec::new();

    for path in shell_sources() {
        let source = fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = source.lines().collect();

        for (index, line) in lines.iter().enumerate() {
            if !line.contains("spawn(async move") {
                continue;
            }
            spawns += 1;

            let window = &lines[index.saturating_sub(WINDOW)..index];
            if !window
                .iter()
                .any(|above| GUARDS.iter().any(|guard| above.contains(guard)))
            {
                unguarded.push(format!("{}:{}", path.display(), index + 1));
            }
        }
    }

    assert!(spawns > 0, "no spawned tasks found — the scan is wrong");
    assert!(
        unguarded.is_empty(),
        "spawned without a busy guard: {unguarded:?}"
    );
}

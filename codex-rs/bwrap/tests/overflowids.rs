#![cfg(target_os = "linux")]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

struct TemporaryDirectory(PathBuf);

impl TemporaryDirectory {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock is before Unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "codex-bwrap-overflowids-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir(&path).expect("create temporary test directory");
        Self(path)
    }
}

impl Drop for TemporaryDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn run_outer_bwrap(arguments: &[&str]) -> Option<Output> {
    Command::new("bwrap").args(arguments).output().ok()
}

fn outer_bwrap_supports_nested_bwrap(inner_bwrap: &str) -> bool {
    let Some(output) = run_outer_bwrap(&[
        "--bind",
        "/",
        "/",
        "--dev-bind",
        "/dev",
        "/dev",
        "--",
        inner_bwrap,
        "--bind",
        "/",
        "/",
        "--dev-bind",
        "/dev",
        "/dev",
        "--",
        "/bin/true",
    ]) else {
        return false;
    };
    output.status.success()
}

fn path_text(path: &Path) -> &str {
    path.to_str().expect("temporary path is valid UTF-8")
}

#[test]
fn unprivileged_bwrap_does_not_require_overflow_ids() {
    let inner_bwrap = env!("CARGO_BIN_EXE_bwrap");
    if !outer_bwrap_supports_nested_bwrap(inner_bwrap) {
        // Some Linux CI and developer hosts do not permit nested user
        // namespaces. The regression only applies where bwrap can launch.
        return;
    }

    let temporary_directory = TemporaryDirectory::new();
    let overflow_uid = temporary_directory.0.join("overflowuid");
    let overflow_gid = temporary_directory.0.join("overflowgid");
    fs::write(&overflow_uid, "").expect("write empty overflowuid fixture");
    fs::write(&overflow_gid, "").expect("write empty overflowgid fixture");

    let output = run_outer_bwrap(&[
        "--bind",
        "/",
        "/",
        "--dev-bind",
        "/dev",
        "/dev",
        "--ro-bind",
        path_text(&overflow_uid),
        "/proc/sys/kernel/overflowuid",
        "--ro-bind",
        path_text(&overflow_gid),
        "/proc/sys/kernel/overflowgid",
        "--",
        inner_bwrap,
        "--bind",
        "/",
        "/",
        "--dev-bind",
        "/dev",
        "/dev",
        "--",
        "/bin/true",
    ])
    .expect("run outer bubblewrap");

    assert!(
        output.status.success(),
        "nested unprivileged bwrap should not read overflow IDs: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

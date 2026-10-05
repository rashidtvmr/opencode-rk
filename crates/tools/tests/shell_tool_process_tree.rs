#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command as StdCommand;
use std::time::{Duration, Instant};

use opencode_rk_tools::shell_tool::{ShellConfig, ShellTool};
use tempfile::TempDir;
use tokio::process::Command;

const FIXTURE: &str = "#!/bin/sh\nprintf '%s\\n' \"$$\" > \"$1\"\n\"$5\" \"$2\" \"$4\" &\ndescendant=$!\nwait \"$descendant\"\nif [ ! -e \"$4\" ]; then printf '%s\\n' sentinel > \"$3\"; fi\n";
const DESCENDANT_FIXTURE: &str = "#!/bin/sh\nprintf '%s\\n' \"$$\" > \"$1\"\nwhile [ ! -e \"$2\" ]; do sleep 0.05; done\n";

struct FixtureGuard {
    dir: TempDir,
    parent_pid_path: PathBuf,
    descendant_pid_path: PathBuf,
    release_path: PathBuf,
}

impl Drop for FixtureGuard {
    fn drop(&mut self) {
        let _ = fs::write(&self.release_path, "release");
        let deadline = Instant::now() + Duration::from_secs(2);
        while fixture_process_alive(&self.parent_pid_path, self.dir.path())
            || fixture_process_alive(&self.descendant_pid_path, self.dir.path())
        {
            if Instant::now() >= deadline {
                eprintln!("process-tree fixture cleanup did not finish");
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}

fn fixture_process_alive(pid_path: &Path, dir: &Path) -> bool {
    read_pid(pid_path).is_some_and(|pid| {
        let Ok(output) = StdCommand::new("/bin/ps")
            .env_clear()
            .args(["-p", &pid.to_string(), "-o", "state=", "-o", "command="])
            .output()
        else {
            return true;
        };
        if output.stdout.len() > 4096 {
            return true;
        }
        if !output.status.success() && output.stdout.is_empty() {
            return false;
        }
        let text = String::from_utf8_lossy(&output.stdout);
        let mut fields = text.split_whitespace();
        let Some(state) = fields.next() else {
            return true;
        };
        !state.to_ascii_lowercase().starts_with('z')
            && fields.collect::<Vec<_>>().join(" ").contains(&dir.to_string_lossy().to_string())
    })
}

fn executable(dir: &TempDir, name: &str, contents: &str) -> PathBuf {
    let path = dir.path().join(name);
    fs::write(&path, contents).expect("write fixture");
    let mut permissions = fs::metadata(&path).expect("fixture metadata").permissions();
    permissions.set_mode(0o700);
    fs::set_permissions(&path, permissions).expect("fixture executable");
    path
}

fn read_pid(path: &Path) -> Option<u32> {
    let text = fs::read_to_string(path).ok()?;
    let text = text.trim();
    if text.is_empty() || text.len() > 20 || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    text.parse().ok()
}

async fn is_alive(pid: u32) -> bool {
    Command::new("/bin/kill")
        .args(["-0", &pid.to_string()])
        .output()
        .await
        .expect("kill -0 runner")
        .status
        .success()
}

#[tokio::test]
async fn aborting_execute_kills_process_tree_and_prevents_delayed_side_effect() {
    let dir = tempfile::tempdir().expect("tempdir");
    let script = executable(&dir, "process-tree.sh", FIXTURE);
    let descendant_script = executable(&dir, "descendant.sh", DESCENDANT_FIXTURE);
    let parent_pid_path = dir.path().join("parent.pid");
    let descendant_pid_path = dir.path().join("descendant.pid");
    let sentinel_path = dir.path().join("sentinel");
    let release_path = dir.path().join("release");
    let guard = FixtureGuard {
        dir,
        parent_pid_path: parent_pid_path.clone(),
        descendant_pid_path: descendant_pid_path.clone(),
        release_path: release_path.clone(),
    };
    let config = ShellConfig {
        timeout_secs: 10,
        max_output_mb: 1,
        allowed_commands: vec![script.to_string_lossy().into_owned()],
    };
    let tool = ShellTool::new(
        script.to_string_lossy().into_owned(),
        vec![
            parent_pid_path.to_string_lossy().into_owned(),
            descendant_pid_path.to_string_lossy().into_owned(),
            sentinel_path.to_string_lossy().into_owned(),
            release_path.to_string_lossy().into_owned(),
            descendant_script.to_string_lossy().into_owned(),
        ],
    );
    let handle = tokio::spawn(tool.execute(config));

    let pid_deadline = Instant::now() + Duration::from_secs(2);
    let (parent_pid, descendant_pid) = loop {
        if let (Some(parent), Some(descendant)) =
            (read_pid(&parent_pid_path), read_pid(&descendant_pid_path))
        {
            break (parent, descendant);
        }
        assert!(
            Instant::now() < pid_deadline,
            "fixture did not publish both bounded numeric PIDs"
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    };

    handle.abort();
    let _ = handle.await;

    let kill_deadline = Instant::now() + Duration::from_millis(750);
    loop {
        let parent_alive = is_alive(parent_pid).await;
        let descendant_alive = is_alive(descendant_pid).await;
        if !parent_alive && !descendant_alive {
            break;
        }
        assert!(
            Instant::now() < kill_deadline,
            "process-tree cancellation leaked parent PID {parent_pid} or descendant PID {descendant_pid}"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }

    tokio::time::timeout(
        Duration::from_secs(1),
        tokio::time::sleep(Duration::from_millis(250)),
    )
    .await
    .expect("bounded post-cancellation side-effect wait");
    assert!(
        !sentinel_path.exists(),
        "descendant PID {descendant_pid} created delayed sentinel"
    );
    drop(guard);
}

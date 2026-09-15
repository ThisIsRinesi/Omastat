use rusqlite::{Connection, OpenFlags};
use std::{
    fs,
    os::unix::{fs::PermissionsExt, net::UnixListener},
    path::PathBuf,
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

// All IPC, executables and telemetry are isolated from the user's desktop.
struct Desktop {
    root: tempfile::TempDir,
    db: PathBuf,
}

impl Desktop {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let db = root.path().join("activity.db");
        fs::create_dir(root.path().join("bin")).unwrap();
        fs::create_dir_all(root.path().join("hypr/test")).unwrap();
        let script = root.path().join("bin/hyprctl");
        fs::write(
            &script,
            r#"#!/usr/bin/python3
import os, sys, time
from pathlib import Path
root = Path(os.environ['OMASTAT_TEST_ROOT'])
(root / 'query-pid').write_text(str(os.getpid()))
if (root / 'stall').exists(): time.sleep(30)
if (root / 'fail').exists(): sys.exit(1)
window = '{"address":"0x1","class":"firefox","title":"test","pid":1}'
print('[' + window + ']' if sys.argv[-1] == 'clients' else window)
"#,
        )
        .unwrap();
        fs::set_permissions(script, fs::Permissions::from_mode(0o755)).unwrap();
        Self { root, db }
    }

    fn command(&self, binary: &str) -> Command {
        let mut cmd = Command::new(binary);
        cmd.args(["--config", "/dev/null", "--database"])
            .arg(&self.db)
            .env("PATH", self.root.path().join("bin"))
            .env("OMASTAT_TEST_ROOT", self.root.path())
            .env("XDG_RUNTIME_DIR", self.root.path())
            .env("HYPRLAND_INSTANCE_SIGNATURE", "test")
            .env("WAYLAND_DISPLAY", "absent")
            .env(
                "DBUS_SYSTEM_BUS_ADDRESS",
                format!("unix:path={}/absent", self.root.path().display()),
            )
            .env(
                "DBUS_SESSION_BUS_ADDRESS",
                format!("unix:path={}/absent", self.root.path().display()),
            );
        cmd
    }

    fn daemon(&self) -> Daemon {
        Daemon(
            self.command(env!("CARGO_BIN_EXE_omastatd"))
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .unwrap(),
        )
    }

    fn scalar(&self, sql: &str) -> Option<i64> {
        Connection::open_with_flags(&self.db, OpenFlags::SQLITE_OPEN_READ_ONLY)
            .ok()?
            .query_row(sql, [], |r| r.get(0))
            .ok()
    }

    fn listener(&self) -> UnixListener {
        let socket = UnixListener::bind(self.root.path().join("hypr/test/.socket2.sock")).unwrap();
        socket.set_nonblocking(true).unwrap();
        socket
    }
}

struct Daemon(Child);

impl Daemon {
    fn terminate(&mut self) {
        assert!(
            Command::new("/usr/bin/kill")
                .args(["-TERM", &self.0.id().to_string()])
                .status()
                .unwrap()
                .success()
        );
        wait_until(Duration::from_secs(2), || {
            self.0.try_wait().unwrap().is_some()
        });
        assert!(self.0.wait().unwrap().success());
    }
}

impl Drop for Daemon {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn wait_until(timeout: Duration, mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + timeout;
    while !condition() {
        assert!(
            Instant::now() < deadline,
            "condition did not complete within {timeout:?}"
        );
        thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn simultaneous_startup_creates_exactly_one_daemon_run() {
    let desktop = Desktop::new();
    let mut first = desktop.daemon();
    let mut second = desktop.daemon();
    wait_until(Duration::from_secs(5), || {
        first.0.try_wait().unwrap().is_some() || second.0.try_wait().unwrap().is_some()
    });
    wait_until(Duration::from_secs(5), || {
        desktop.scalar("SELECT COUNT(*) FROM unobserved_intervals WHERE ended_at IS NULL")
            == Some(1)
    });
    assert_eq!(desktop.scalar("SELECT COUNT(*) FROM daemon_runs"), Some(1));
    if let Some(status) = first.0.try_wait().unwrap() {
        assert!(!status.success());
        second.terminate();
    } else {
        assert!(!second.0.wait().unwrap().success());
        first.terminate();
    }
}

#[test]
fn disconnected_daemon_rejects_competitors_and_purge_but_allows_preview_and_shutdown() {
    let desktop = Desktop::new();
    let mut daemon = desktop.daemon();
    wait_until(Duration::from_secs(5), || {
        desktop.scalar("SELECT COUNT(*) FROM unobserved_intervals WHERE ended_at IS NULL")
            == Some(1)
    });
    let mut competitor = desktop.daemon();
    wait_until(Duration::from_secs(2), || {
        competitor.0.try_wait().unwrap().is_some()
    });
    assert!(!competitor.0.wait().unwrap().success());
    assert_eq!(desktop.scalar("SELECT COUNT(*) FROM daemon_runs"), Some(1));

    let purge = desktop
        .command(env!("CARGO_BIN_EXE_omastat"))
        .args(["purge", "--all", "--confirm"])
        .output()
        .unwrap();
    assert!(!purge.status.success());
    assert!(String::from_utf8_lossy(&purge.stderr).contains("stop omastatd"));
    let preview = desktop
        .command(env!("CARGO_BIN_EXE_omastat"))
        .args(["purge", "--all", "--dry-run", "--json"])
        .output()
        .unwrap();
    assert!(
        preview.status.success(),
        "{}",
        String::from_utf8_lossy(&preview.stderr)
    );
    assert_eq!(desktop.scalar("SELECT COUNT(*) FROM daemon_runs"), Some(1));

    daemon.terminate();
    assert_eq!(
        desktop.scalar("SELECT COUNT(*) FROM daemon_runs WHERE stop_kind = 'clean'"),
        Some(1)
    );
    assert_eq!(
        desktop.scalar("SELECT COUNT(*) FROM unobserved_intervals WHERE ended_at IS NULL"),
        Some(0)
    );
    let purge = desktop
        .command(env!("CARGO_BIN_EXE_omastat"))
        .args(["purge", "--all", "--confirm"])
        .output()
        .unwrap();
    assert!(purge.status.success());
    let mut restarted = desktop.daemon();
    wait_until(Duration::from_secs(5), || {
        desktop.scalar("SELECT COUNT(*) FROM daemon_runs") == Some(1)
    });
    // Wait until startup has installed its signal handler.
    wait_until(Duration::from_secs(5), || {
        desktop.scalar("SELECT COUNT(*) FROM unobserved_intervals WHERE ended_at IS NULL")
            == Some(1)
    });
    restarted.terminate();
}

#[test]
fn disconnect_and_failed_snapshot_stop_focus_until_reconciliation_succeeds() {
    let desktop = Desktop::new();
    let listener = desktop.listener();
    let mut daemon = desktop.daemon();
    let mut connection = None;
    wait_until(Duration::from_secs(5), || {
        connection = listener.accept().ok();
        connection.is_some()
    });
    wait_until(Duration::from_secs(5), || {
        desktop.scalar("SELECT COUNT(*) FROM intervals WHERE kind = 'focused' AND ended_at IS NULL")
            == Some(1)
    });
    fs::write(desktop.root.path().join("fail"), "").unwrap();
    drop(connection);
    wait_until(Duration::from_secs(5), || {
        desktop.scalar("SELECT COUNT(*) FROM unobserved_intervals WHERE ended_at IS NULL")
            == Some(1)
    });
    assert_eq!(
        desktop.scalar("SELECT COUNT(*) FROM intervals WHERE ended_at IS NULL"),
        Some(0)
    );
    let heartbeat = desktop.scalar("SELECT last_heartbeat_at FROM daemon_runs");
    thread::sleep(Duration::from_millis(1100));
    assert_eq!(
        desktop.scalar("SELECT last_heartbeat_at FROM daemon_runs"),
        heartbeat
    );
    assert_eq!(
        desktop.scalar("SELECT COUNT(*) FROM intervals WHERE ended_at IS NULL"),
        Some(0)
    );
    fs::remove_file(desktop.root.path().join("fail")).unwrap();
    wait_until(Duration::from_secs(8), || {
        desktop.scalar("SELECT COUNT(*) FROM intervals WHERE kind = 'focused' AND ended_at IS NULL")
            == Some(1)
    });
    assert_eq!(
        desktop.scalar("SELECT COUNT(*) FROM unobserved_intervals WHERE ended_at IS NULL"),
        Some(0)
    );
    assert_eq!(desktop.scalar("SELECT COUNT(*) FROM intervals i JOIN unobserved_intervals g ON i.started_at < g.ended_at AND COALESCE(i.ended_at, unixepoch()) > g.started_at WHERE i.kind = 'focused' AND g.ended_at > g.started_at"), Some(0));
    daemon.terminate();
}

fn query_pid(desktop: &Desktop) -> Option<u32> {
    fs::read_to_string(desktop.root.path().join("query-pid"))
        .ok()?
        .parse()
        .ok()
}

fn process_terminated(pid: u32) -> bool {
    // A zombie has terminated even if its new parent has not reaped it yet.
    fs::read_to_string(format!("/proc/{pid}/status"))
        .map(|s| {
            s.lines()
                .any(|l| l.starts_with("State:") && l.contains('Z'))
        })
        .unwrap_or(true)
}

#[test]
fn stalled_snapshot_is_killed_on_timeout_and_on_shutdown() {
    for shutdown in [false, true] {
        let desktop = Desktop::new();
        let _listener = desktop.listener();
        fs::write(desktop.root.path().join("stall"), "").unwrap();
        let mut daemon = desktop.daemon();
        wait_until(Duration::from_secs(5), || query_pid(&desktop).is_some());
        let pid = query_pid(&desktop).unwrap();
        if shutdown {
            daemon.terminate();
        }
        wait_until(Duration::from_secs(4), || process_terminated(pid));
        assert_eq!(desktop.scalar("SELECT COUNT(*) FROM intervals"), Some(0));
        if !shutdown {
            assert_eq!(
                desktop.scalar("SELECT COUNT(*) FROM unobserved_intervals WHERE ended_at IS NULL"),
                Some(1)
            );
            daemon.terminate();
        }
    }
}

#[test]
fn killed_daemon_releases_lock_and_recovers_unfinished_gap() {
    let desktop = Desktop::new();
    let mut daemon = desktop.daemon();
    wait_until(Duration::from_secs(5), || {
        desktop.scalar("SELECT COUNT(*) FROM unobserved_intervals WHERE ended_at IS NULL")
            == Some(1)
    });
    daemon.0.kill().unwrap();
    daemon.0.wait().unwrap();
    let mut restarted = desktop.daemon();
    wait_until(Duration::from_secs(5), || {
        desktop.scalar("SELECT COUNT(*) FROM daemon_runs WHERE stop_kind = 'recovered'") == Some(1)
    });
    wait_until(Duration::from_secs(5), || {
        desktop.scalar("SELECT COUNT(*) FROM unobserved_intervals WHERE ended_at IS NULL")
            == Some(1)
    });
    assert_eq!(
        desktop.scalar("SELECT COUNT(*) FROM daemon_runs WHERE stopped_at IS NULL"),
        Some(1)
    );
    restarted.terminate();
}

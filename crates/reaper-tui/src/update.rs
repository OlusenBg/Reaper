//! Self-update: Reaper is installed from a git clone, so updating means
//! `git pull` in that clone followed by a rebuild/reinstall.

use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const FETCH_TIMEOUT: Duration = Duration::from_secs(5);
const LOCAL_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, PartialEq, Eq)]
pub struct UpdateInfo {
    pub upstream: String,
    pub behind: u32,
    /// Subjects of the newest incoming commits.
    pub summary: Vec<String>,
}

pub struct Repo {
    root: PathBuf,
}

/// Run git with a hard timeout; stdout is captured, stderr discarded.
fn git(dir: &Path, args: &[&str], timeout: Duration) -> Result<String> {
    let mut child = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("LC_ALL", "C")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .context("failed to run git (is it installed?)")?;
    let deadline = Instant::now() + timeout;
    loop {
        if child.try_wait()?.is_some() {
            break;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            bail!("git {} timed out", args.first().copied().unwrap_or(""));
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let out = child.wait_with_output()?;
    if !out.status.success() {
        bail!("git {} failed", args.join(" "));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_owned())
}

impl Repo {
    /// The clone this binary was built from, if it still exists.
    pub fn locate() -> Option<Self> {
        Self::at(Path::new(env!("CARGO_MANIFEST_DIR")))
    }

    pub fn at(dir: &Path) -> Option<Self> {
        if !dir.exists() {
            return None;
        }
        let root = git(dir, &["rev-parse", "--show-toplevel"], LOCAL_TIMEOUT).ok()?;
        Some(Self {
            root: PathBuf::from(root),
        })
    }

    fn git(&self, args: &[&str], timeout: Duration) -> Result<String> {
        git(&self.root, args, timeout)
    }

    /// Fetch and report whether the tracked upstream has new commits. `Ok(None)` = up to date.
    pub fn check(&self) -> Result<Option<UpdateInfo>> {
        let upstream = self
            .git(
                &["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"],
                LOCAL_TIMEOUT,
            )
            .context("current branch has no upstream to update from")?;
        let remote = upstream.split('/').next().unwrap_or("origin");
        self.git(&["fetch", "--quiet", remote], FETCH_TIMEOUT)
            .context("could not reach the remote")?;
        let behind: u32 = self
            .git(&["rev-list", "--count", "HEAD..@{u}"], LOCAL_TIMEOUT)?
            .parse()?;
        if behind == 0 {
            return Ok(None);
        }
        let summary = self
            .git(
                &["log", "--format=%s", "-n", "5", "HEAD..@{u}"],
                LOCAL_TIMEOUT,
            )?
            .lines()
            .map(str::to_owned)
            .collect();
        Ok(Some(UpdateInfo {
            upstream,
            behind,
            summary,
        }))
    }

    /// Fast-forward the clone. Never rewrites local work: fails if it has diverged.
    pub fn pull(&self) -> Result<()> {
        let status = Command::new("git")
            .arg("-C")
            .arg(&self.root)
            .args(["pull", "--ff-only"])
            .status()
            .context("failed to run git")?;
        if !status.success() {
            bail!(
                "git pull --ff-only failed (local changes or diverged history in {}). Resolve it manually.",
                self.root.display()
            );
        }
        Ok(())
    }

    /// Rebuild and reinstall the binary from the clone.
    pub fn reinstall(&self) -> Result<()> {
        println!("Rebuilding Reaper (this can take a minute)...");
        let status = Command::new("cargo")
            .current_dir(&self.root)
            .args([
                "install",
                "--path",
                "crates/reaper-tui",
                "--locked",
                "--force",
            ])
            .status()
            .context("failed to run cargo (is Rust installed? try: sudo pacman -S rust)")?;
        if !status.success() {
            bail!("cargo install failed");
        }
        Ok(())
    }

    pub fn update(&self) -> Result<()> {
        self.pull()?;
        self.reinstall()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    fn sh(dir: &Path, args: &[&str]) {
        let ok = Command::new("git")
            .current_dir(dir)
            .args([
                "-c",
                "user.name=t",
                "-c",
                "user.email=t@t",
                "-c",
                "commit.gpgsign=false",
            ])
            .args(args)
            .status()
            .unwrap()
            .success();
        assert!(ok, "git {args:?} failed");
    }

    /// bare origin + a clone tracking it + a second clone used to push new commits.
    fn setup() -> (tempfile::TempDir, PathBuf, PathBuf) {
        let t = tempfile::tempdir().unwrap();
        let origin = t.path().join("origin.git");
        let a = t.path().join("a");
        let b = t.path().join("b");
        std::fs::create_dir(&origin).unwrap();
        sh(&origin, &["init", "--bare", "-b", "main"]);
        sh(t.path(), &["clone", origin.to_str().unwrap(), "a"]);
        std::fs::write(a.join("f"), "1").unwrap();
        sh(&a, &["add", "."]);
        sh(&a, &["commit", "-m", "first"]);
        sh(&a, &["push", "-u", "origin", "HEAD:main"]);
        sh(t.path(), &["clone", origin.to_str().unwrap(), "b"]);
        (t, a, b)
    }

    #[test]
    fn detects_and_pulls_updates() {
        let (_t, a, b) = setup();
        let repo = Repo::at(&b).unwrap();
        assert_eq!(repo.check().unwrap(), None);

        std::fs::write(a.join("f"), "2").unwrap();
        sh(&a, &["commit", "-am", "second change"]);
        sh(&a, &["push"]);

        let info = repo.check().unwrap().expect("update expected");
        assert_eq!(info.behind, 1);
        assert_eq!(info.upstream, "origin/main");
        assert_eq!(info.summary, vec!["second change"]);

        repo.pull().unwrap();
        assert_eq!(std::fs::read_to_string(b.join("f")).unwrap(), "2");
        assert_eq!(repo.check().unwrap(), None);
    }

    #[test]
    fn pull_refuses_diverged_history() {
        let (_t, a, b) = setup();
        std::fs::write(a.join("f"), "remote").unwrap();
        sh(&a, &["commit", "-am", "remote"]);
        sh(&a, &["push"]);
        std::fs::write(b.join("f"), "local").unwrap();
        sh(&b, &["commit", "-am", "local"]);
        let repo = Repo::at(&b).unwrap();
        assert!(repo.pull().is_err());
        assert_eq!(std::fs::read_to_string(b.join("f")).unwrap(), "local");
    }

    #[test]
    fn no_upstream_is_an_error_not_a_panic() {
        let t = tempfile::tempdir().unwrap();
        sh(t.path(), &["init", "-b", "main"]);
        let repo = Repo::at(t.path()).unwrap();
        assert!(repo.check().is_err());
    }

    #[test]
    fn missing_dir_means_no_repo() {
        assert!(Repo::at(Path::new("/nonexistent/reaper")).is_none());
    }
}

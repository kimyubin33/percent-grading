use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

struct TempDir {
    root: PathBuf,
    path: PathBuf,
}

impl TempDir {
    fn new() -> Self {
        let root = fs::canonicalize(std::env::temp_dir()).expect("temporary directory exists");
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = root.join(format!(
            "percent-grading-cli-{}-{stamp}-{}",
            std::process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).expect("create unique owned test directory");
        Self { root, path }
    }

    fn join(&self, name: &str) -> PathBuf {
        self.path.join(name)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        // Remove only this test's exact directory, directly under the canonical temp root.
        if let Ok(resolved) = fs::canonicalize(&self.path)
            && resolved == self.path
            && resolved.parent() == Some(self.root.as_path())
            && fs::symlink_metadata(&self.path).is_ok_and(|m| !m.file_type().is_symlink())
        {
            let _ = fs::remove_dir_all(&resolved);
        }
    }
}

fn cli() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_percent-grading"));
    command
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .env_remove("OPENAI_API_KEY")
        .env_remove("ANTHROPIC_API_KEY");
    command
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn compile(lock: &Path) -> Output {
    cli()
        .args(["compile", "policies/apple.aha", "--out"])
        .arg(lock)
        .output()
        .expect("execute compile CLI")
}

fn checked_compile(lock: &Path) {
    let output = compile(lock);
    assert!(output.status.success(), "{}", stderr(&output));
    let result: Value = serde_json::from_slice(&output.stdout).expect("compile emits JSON");
    assert_eq!(result["status"], "compiled");
    assert!(lock.is_file());
}

fn grade(lock: &Path, runs: &Path) -> Command {
    let mut command = cli();
    command
        .arg("grade")
        .arg(lock)
        .args([
            "examples/rust/apple-blue.txt",
            "--openai-model",
            "offline-openai",
            "--claude-model",
            "offline-claude",
            "--max-attempts",
            "1",
            "--timeout-seconds",
            "1",
            "--runs-dir",
        ])
        .arg(runs);
    command
}

#[test]
fn compiled_policy_and_blue_demo_preserve_partial_credit_and_critical_failure() {
    let temp = TempDir::new();
    let lock = temp.join("apple.lock.json");
    checked_compile(&lock);
    let output = cli()
        .arg("demo")
        .arg(&lock)
        .arg("examples/rust/apple-blue.fixture.json")
        .output()
        .expect("execute demo CLI");
    assert!(output.status.success(), "{}", stderr(&output));
    let result: Value = serde_json::from_slice(&output.stdout).expect("demo emits JSON");
    assert_eq!(result["origin"], "manual_fixture");
    assert_eq!(result["live_api_tested"], false);
    assert_eq!(result["report"]["current"]["percent"], 46.25);
    assert_eq!(result["report"]["future"]["percent"], 0.0);
    assert_eq!(result["report"]["status"], "blocked");
    assert_eq!(
        result["report"]["critical_failures"],
        serde_json::json!(["intent"])
    );
}

#[test]
fn recompiling_to_existing_lock_fails_without_changing_its_bytes() {
    let temp = TempDir::new();
    let lock = temp.join("apple.lock.json");
    checked_compile(&lock);
    let original = fs::read(&lock).unwrap();
    let output = compile(&lock);
    assert_eq!(output.status.code(), Some(2));
    assert!(stderr(&output).contains("existing files are never replaced"));
    assert_eq!(fs::read(&lock).unwrap(), original);
}

#[test]
fn missing_or_invalid_claude_credentials_fail_before_creating_a_run() {
    let temp = TempDir::new();
    let lock = temp.join("apple.lock.json");
    checked_compile(&lock);
    for (name, key, expected) in [
        ("missing-key", None, "ANTHROPIC_API_KEY is required"),
        (
            "invalid-key",
            Some(" invalid "),
            "without surrounding whitespace",
        ),
    ] {
        let runs = temp.join(name);
        let mut command = grade(&lock, &runs);
        command.env("OPENAI_API_KEY", "fake");
        if let Some(key) = key {
            command.env("ANTHROPIC_API_KEY", key);
        }
        let output = command.output().expect("execute credential preflight CLI");
        assert_eq!(output.status.code(), Some(2), "{name}: {}", stderr(&output));
        assert!(
            stderr(&output).contains(expected),
            "{name}: {}",
            stderr(&output)
        );
        assert!(output.stdout.is_empty());
        assert!(
            !runs.exists(),
            "{name}: preflight must precede run creation and API calls"
        );
    }
}

#[test]
fn unsupported_provider_option_is_rejected_before_loading_inputs() {
    let temp = TempDir::new();
    let runs = temp.join("runs");
    let output = grade(&temp.join("absent.lock.json"), &runs)
        .args(["--provider", "gemini"])
        .output()
        .expect("execute invalid-option CLI");
    assert_eq!(output.status.code(), Some(2));
    assert!(stderr(&output).contains("Unknown or duplicate option: --provider"));
    assert!(!runs.exists());
}

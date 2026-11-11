//! End-to-end offline research workflow; no test makes an HTTP or model request.
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
        let root = fs::canonicalize(std::env::temp_dir()).unwrap();
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = root.join(format!(
            "percent-grading-research-cli-{}-{stamp}-{}",
            std::process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self { root, path }
    }

    fn join(&self, name: &str) -> PathBuf {
        self.path.join(name)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        if let Ok(resolved) = fs::canonicalize(&self.path)
            && resolved == self.path
            && resolved.parent() == Some(self.root.as_path())
            && fs::symlink_metadata(&self.path).is_ok_and(|m| !m.file_type().is_symlink())
        {
            let _ = fs::remove_dir_all(resolved);
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

fn success(output: Output) -> Value {
    assert!(output.status.success(), "{}", stderr(&output));
    serde_json::from_slice(&output.stdout).unwrap()
}

fn lock(temp: &TempDir) -> PathBuf {
    let lock = temp.join("apple.lock.json");
    success(
        cli()
            .args(["compile", "policies/apple.aha", "--out"])
            .arg(&lock)
            .output()
            .unwrap(),
    );
    lock
}

fn select(lock: &Path, pack: &Path) -> Output {
    cli()
        .arg("select")
        .arg(lock)
        .args(["catalogs/apple.json", "--out"])
        .arg(pack)
        .output()
        .unwrap()
}

fn pack(temp: &TempDir, lock: &Path) -> PathBuf {
    let pack = temp.join("research.json");
    success(select(lock, &pack));
    pack
}

fn prompt(lock: &Path, pack: &Path, answer: &str) -> Output {
    cli()
        .arg("prompt")
        .arg(lock)
        .arg(answer)
        .arg("--research")
        .arg(pack)
        .output()
        .unwrap()
}

#[test]
fn selected_research_is_inspectable_and_identical_for_different_answers() {
    let temp = TempDir::new();
    let lock = lock(&temp);
    let original = fs::read(&lock).unwrap();
    let pack = pack(&temp, &lock);
    let inspected = success(
        cli()
            .arg("inspect-research")
            .arg(&lock)
            .arg(&pack)
            .output()
            .unwrap(),
    );
    assert!(inspected["diagnostics"]["selected_count"].as_u64().unwrap() > 0);
    assert!(inspected["diagnostics"]["context_bytes"].as_u64().unwrap() <= 16000);
    let red = success(prompt(&lock, &pack, "examples/rust/apple-red.txt"));
    let blue = success(prompt(&lock, &pack, "examples/rust/apple-blue.txt"));
    let red_data: Value = serde_json::from_str(red["user"].as_str().unwrap()).unwrap();
    let blue_data: Value = serde_json::from_str(blue["user"].as_str().unwrap()).unwrap();
    assert_ne!(red_data["answer"], blue_data["answer"]);
    assert_ne!(red_data["answer_hash"], blue_data["answer_hash"]);
    assert_eq!(red_data["research"], blue_data["research"]);
    assert_eq!(red_data["research"], inspected["context"]);
    assert_eq!(red_data["research_hash"], inspected["pack_hash"]);
    assert_eq!(red_data["contract"], blue_data["contract"]);
    assert_eq!(fs::read(lock).unwrap(), original);
}

#[test]
fn tampered_research_is_rejected_in_the_actual_prompt_command() {
    let temp = TempDir::new();
    let lock = lock(&temp);
    let pack = pack(&temp, &lock);
    let mut modified: Value = serde_json::from_slice(&fs::read(&pack).unwrap()).unwrap();
    modified["selected"][0]["perspective"]["description"] =
        Value::String("Tampered interpretation".into());
    fs::write(&pack, serde_json::to_vec(&modified).unwrap()).unwrap();
    let output = prompt(&lock, &pack, "examples/rust/apple-red.txt");
    assert_eq!(output.status.code(), Some(2));
    assert!(stderr(&output).contains("Research pack differs"));
    assert!(output.stdout.is_empty());
}

#[test]
fn selecting_to_an_existing_path_preserves_its_contents() {
    let temp = TempDir::new();
    let lock = lock(&temp);
    let pack = pack(&temp, &lock);
    let original = fs::read(&pack).unwrap();
    let output = select(&lock, &pack);
    assert_eq!(output.status.code(), Some(2));
    assert!(stderr(&output).contains("existing files are never replaced"));
    assert_eq!(fs::read(pack).unwrap(), original);
}

#[test]
fn another_contracts_research_is_rejected_before_credentials_or_run_creation() {
    let temp = TempDir::new();
    let lock = lock(&temp);
    let pack = pack(&temp, &lock);
    let source =
        fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("policies/apple.aha"))
            .unwrap();
    let revised = source.replace("version 1,", "version 2,");
    assert_ne!(source, revised);
    let policy = temp.join("revised.aha");
    let other_lock = temp.join("revised.lock.json");
    let runs = temp.join("runs");
    fs::write(&policy, revised).unwrap();
    success(
        cli()
            .arg("compile")
            .arg(policy)
            .arg("--out")
            .arg(&other_lock)
            .output()
            .unwrap(),
    );
    let output = cli()
        .arg("grade")
        .arg(other_lock)
        .args([
            "examples/rust/apple-blue.txt",
            "--openai-model",
            "offline-openai",
            "--claude-model",
            "offline-claude",
            "--research",
        ])
        .arg(pack)
        .arg("--runs-dir")
        .arg(&runs)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(
        stderr(&output).contains("Research pack differs"),
        "{}",
        stderr(&output)
    );
    assert!(!stderr(&output).contains("API_KEY"));
    assert!(!runs.exists());
}

#[test]
fn discovery_refuses_existing_output_before_any_http_request() {
    let temp = TempDir::new();
    let lock = lock(&temp);
    let target = temp.join("search.json");
    fs::write(&target, b"preserve this result").unwrap();
    let output = cli()
        .arg("discover")
        .arg(lock)
        .arg("--out")
        .arg(&target)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(stderr(&output).contains("Discovery output already exists"));
    assert_eq!(fs::read(target).unwrap(), b"preserve this result");
}

#[test]
fn invalid_discovery_queries_and_limits_fail_before_http_or_output_creation() {
    let temp = TempDir::new();
    let lock = lock(&temp);
    for (index, options, expected) in [
        (0, vec!["--rows", "0"], "rows must be"),
        (1, vec!["--rows", "21"], "rows must be"),
        (2, vec!["--query", ""], "query must contain"),
        (
            3,
            vec!["--query", "optics\ninjection"],
            "query must contain",
        ),
    ] {
        let target = temp.join(&format!("invalid-{index}.json"));
        let output = cli()
            .arg("discover")
            .arg(&lock)
            .arg("--out")
            .arg(&target)
            .args(options)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2));
        assert!(stderr(&output).contains(expected), "{}", stderr(&output));
        assert!(!target.exists());
        assert!(output.stdout.is_empty());
    }
}

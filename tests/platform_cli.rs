//! Offline integration checks for the public adapter and experiment interfaces.
//! The judgments below are authored fixtures, never live provider responses.
use percent_grading::{benchmark, contract};
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

static NEXT: AtomicU64 = AtomicU64::new(0);

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
            "percent-grading-platform-{}-{stamp}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
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
        .env_remove("ANTHROPIC_API_KEY")
        .env_remove("PERCENT_GRADING_API_TOKEN");
    command
}

fn success(output: Output) -> Value {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

fn write(path: &Path, value: &Value) {
    fs::write(path, serde_json::to_vec_pretty(value).unwrap()).unwrap();
}

fn read(path: &Path) -> Value {
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

fn policy() -> String {
    "\u{b0b1}\u{b9d0} Policy: version*, question*, context*, reference*\n\
     \u{b0b1}\u{b9d0} Criterion: dimension*, weight*(\u{c815}\u{c218}), description*, critical_min*\n\
     \u{b0b1}\u{b9d0} Anchor: criterion*, points*(\u{c815}\u{c218}), description*\n\
     \u{b0b1}\u{b9d0} Perspective: criteria*, kind*, source*, assumptions*, description*, overlap*\n\
     \u{c7a5}\u{ba74} Addition\n\
     \x20\x20Policy addition: version 1, question Compute two plus three., context Ordinary integer addition. Only the result is requested., reference Five.\n\
     \x20\x20Criterion result: dimension intent, weight 100, description Give five or an equivalent representation., critical_min 100\n\
     \x20\x20Anchor wrong: criterion result, points 0, description Five is absent or another result is asserted.\n\
     \x20\x20Anchor correct: criterion result, points 100, description Five is clearly identified as the sum.\n\
     \x20\x20Perspective arithmetic: criteria result, kind method, source Definition of integer addition, assumptions Ordinary integers., description Add the two quantities., overlap arithmetic\n"
        .into()
}

fn judgment(lock: &contract::LockedContract, answer: &str, anchor: &str) -> Value {
    json!({
        "contract_hash":lock.contract_hash,
        "answer_hash":contract::hash(answer.as_bytes()),
        "judgments":[{"criterion_id":"result","anchor_id":anchor,"evidence":[answer],"reason":"Authored protocol fixture, not an AI judgment.","conditions":[]}]
    })
}

fn suite_and_results() -> (Value, Value) {
    let source = policy();
    let lock = contract::compile(&source).unwrap();
    let suite = json!({
        "format":"percent-grading-benchmark-v1","id":"platform_fixture","version":"1",
        "tasks":[{"id":"addition","split":"development","policy_source":source,
            "cases":[
                {"id":"five","category":"minimal_correct","answer":"5","expected":[{"criterion_id":"result","allowed_anchor_ids":["correct"]}]},
                {"id":"six","category":"plausible_wrong","answer":"6","expected":[{"criterion_id":"result","allowed_anchor_ids":["wrong"]}]}
            ],
            "relations":[{"id":"incorrect_not_higher","left_case_id":"five","right_case_id":"six","criterion_id":"result","kind":"not_higher"}]
        }]
    });
    let parsed: benchmark::BenchmarkSuite = serde_json::from_value(suite.clone()).unwrap();
    let suite_hash = benchmark::suite_hash(&parsed).unwrap();
    let results = json!({
        "format":"percent-grading-benchmark-results-v1","suite_hash":suite_hash,"origin":"manual_fixture","split":"all","repeats":1,
        "trials":[
            {"task_id":"addition","case_id":"five","repeat":1,"openai":judgment(&lock,"5","correct"),"claude":judgment(&lock,"5","correct"),"run_id":null},
            {"task_id":"addition","case_id":"six","repeat":1,"openai":judgment(&lock,"6","correct"),"claude":judgment(&lock,"6","correct"),"run_id":null}
        ]
    });
    (suite, results)
}

#[test]
fn api_compile_and_evaluate_preserve_the_engine_contract_and_report() {
    let temp = TempDir::new();
    let request_path = temp.join("request.json");
    write(
        &request_path,
        &json!({"api_version":"percent-grading-api-v1","request_id":"compile-1","payload":{"operation":"compile","source":policy()}}),
    );
    let compiled = success(cli().arg("api").arg(&request_path).output().unwrap());
    assert_eq!(compiled["request_id"], "compile-1");
    assert!(compiled.get("error").is_none());
    let expected = contract::compile(&policy()).unwrap();
    assert_eq!(
        compiled["result"]["lock"],
        serde_json::to_value(&expected).unwrap()
    );

    let lock_path = temp.join("contract.json");
    let answer_path = temp.join("answer.txt");
    let judgment_path = temp.join("judgment.json");
    write(&lock_path, &compiled["result"]["lock"]);
    fs::write(&answer_path, "5").unwrap();
    let row = judgment(&expected, "5", "correct");
    write(&judgment_path, &row);
    let existing_cli = success(
        cli()
            .arg("evaluate")
            .arg(&lock_path)
            .arg(&answer_path)
            .arg(&judgment_path)
            .arg(&judgment_path)
            .output()
            .unwrap(),
    );
    write(
        &request_path,
        &json!({"api_version":"percent-grading-api-v1","request_id":"evaluate-1","payload":{"operation":"evaluate","lock":compiled["result"]["lock"],"answer":"5","openai":row,"claude":row}}),
    );
    let response = success(cli().arg("api").arg(&request_path).output().unwrap());
    assert_eq!(response["result"]["report"], existing_cli["report"]);
}

#[test]
fn api_rejects_versions_unknown_fields_and_client_secrets_as_structured_errors() {
    let temp = TempDir::new();
    let path = temp.join("request.json");
    let valid = json!({"api_version":"percent-grading-api-v1","request_id":"invalid-1","payload":{"operation":"compile","source":policy()}});
    let mut bad_version = valid.clone();
    bad_version["api_version"] = json!("percent-grading-api-v999");
    let mut extra = valid.clone();
    extra["payload"]["expected"] = json!({"hidden_answer":"must not enter a prompt"});
    let mut secret = valid;
    secret["payload"]["openai_api_key"] = json!("secret-canary-never-echo-this");
    for request in [bad_version, extra, secret] {
        write(&path, &request);
        let output = cli().arg("api").arg(&path).output().unwrap();
        assert_eq!(output.status.code(), Some(2));
        let response: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert!(response.get("error").is_some());
        assert!(response.get("result").is_none());
        assert!(!String::from_utf8_lossy(&output.stdout).contains("secret-canary"));
        assert!(!String::from_utf8_lossy(&output.stderr).contains("secret-canary"));
    }
}

#[test]
fn api_live_grading_is_disabled_before_any_credentials_or_run_directory() {
    let temp = TempDir::new();
    let request = temp.join("grade.json");
    let runs = temp.join("must-not-exist");
    write(
        &request,
        &json!({"api_version":"percent-grading-api-v1","request_id":"grade-1","payload":{"operation":"grade","lock":contract::compile(&policy()).unwrap(),"answer":"5","config":{"openai_model":"test-openai","claude_model":"test-claude","limits":{"timeout_seconds":60,"max_output_tokens":4096,"max_response_bytes":2097152,"max_attempts":1}}}}),
    );
    let output = cli()
        .arg("api")
        .arg(&request)
        .arg("--runs-dir")
        .arg(&runs)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    let response: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(response["error"]["code"], "live_disabled");
    assert!(!runs.exists());
}

#[test]
fn benchmark_cli_matches_library_and_refuses_to_replace_reports() {
    let temp = TempDir::new();
    let (suite, results) = suite_and_results();
    let suite_path = temp.join("suite.json");
    let results_path = temp.join("results.json");
    let report_path = temp.join("report.json");
    write(&suite_path, &suite);
    write(&results_path, &results);
    success(
        cli()
            .arg("benchmark-validate")
            .arg(&suite_path)
            .output()
            .unwrap(),
    );
    success(
        cli()
            .arg("benchmark")
            .arg(&suite_path)
            .arg(&results_path)
            .arg("--out")
            .arg(&report_path)
            .output()
            .unwrap(),
    );
    let expected = benchmark::evaluate(
        &serde_json::from_value(suite).unwrap(),
        &serde_json::from_value(results).unwrap(),
    )
    .unwrap();
    assert_eq!(read(&report_path), serde_json::to_value(expected).unwrap());
    let original = fs::read(&report_path).unwrap();
    let output = cli()
        .arg("benchmark")
        .arg(&suite_path)
        .arg(&results_path)
        .arg("--out")
        .arg(&report_path)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(fs::read(&report_path).unwrap(), original);
}

#[test]
fn benchmark_cli_rejects_wrong_identity_and_incomplete_trials() {
    let temp = TempDir::new();
    let (suite, results) = suite_and_results();
    let suite_path = temp.join("suite.json");
    let results_path = temp.join("results.json");
    let report_path = temp.join("must-not-exist.json");
    write(&suite_path, &suite);
    let mut bad_hash = results.clone();
    bad_hash["suite_hash"] = json!("0".repeat(64));
    let mut incomplete = results;
    incomplete["trials"].as_array_mut().unwrap().pop();
    for value in [bad_hash, incomplete] {
        write(&results_path, &value);
        let output = cli()
            .arg("benchmark")
            .arg(&suite_path)
            .arg(&results_path)
            .arg("--out")
            .arg(&report_path)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert!(!report_path.exists());
    }
}

#[test]
fn batch_call_budget_fails_before_credentials_or_experiment_creation() {
    let temp = TempDir::new();
    let (suite, _) = suite_and_results();
    let suite_path = temp.join("suite.json");
    let runs = temp.join("must-not-exist");
    write(&suite_path, &suite);
    let output = cli()
        .arg("benchmark-run")
        .arg(&suite_path)
        .args([
            "--openai-model",
            "test-openai",
            "--claude-model",
            "test-claude",
            "--split",
            "all",
            "--repeats",
            "1",
            "--max-api-calls",
            "1",
            "--runs-dir",
        ])
        .arg(&runs)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let error = String::from_utf8_lossy(&output.stderr).to_lowercase();
    assert!(
        error.contains("budget") || error.contains("max_api_calls"),
        "{error}"
    );
    assert!(!error.contains("openai_api_key"));
    assert!(!runs.exists());
}

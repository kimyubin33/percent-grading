//! Offline command/API checks for the score ceiling, proof badge and regression gate.
use percent_grading::{contract, proof, service};
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
            "percent-grading-v06-{}-{stamp}-{}",
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
fn successful(output: Output) -> Value {
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
fn fixture(path: &str) -> Value {
    serde_json::from_slice(&fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join(path)).unwrap())
        .unwrap()
}
fn proof_payload() -> Value {
    let lock = contract::compile(include_str!("../examples/proof/grading.aha")).unwrap();
    let policy =
        proof::compile_policy(&lock, include_str!("../examples/proof/challenge.aha")).unwrap();
    json!({
        "operation":"proof_evaluate", "lock":lock, "policy":policy,
        "answer":include_str!("../examples/proof/answer.txt"),
        "certificate":fixture("examples/proof/certificate.json"),
        "openai":fixture("examples/proof/judgment.json"),
        "claude":fixture("examples/proof/judgment.json")
    })
}

#[test]
fn proof_cli_verifies_the_certificate_and_keeps_celebration_outside_the_grade() {
    let temp = TempDir::new();
    let contract_file = temp.join("contract.json");
    let policy_file = temp.join("proof-policy.json");
    successful(
        cli()
            .args(["compile", "examples/proof/grading.aha", "--out"])
            .arg(&contract_file)
            .output()
            .unwrap(),
    );
    successful(
        cli()
            .arg("proof-policy")
            .arg(&contract_file)
            .arg("examples/proof/challenge.aha")
            .arg("--out")
            .arg(&policy_file)
            .output()
            .unwrap(),
    );
    let prompt = successful(
        cli()
            .arg("proof-prompt")
            .arg(&contract_file)
            .arg(&policy_file)
            .arg("examples/proof/answer.txt")
            .output()
            .unwrap(),
    );
    assert!(prompt["system"].as_str().is_some_and(|s| !s.is_empty()));
    assert!(prompt["schema"].is_object());
    let result = successful(
        cli()
            .arg("proof-evaluate")
            .arg(&contract_file)
            .arg(&policy_file)
            .args([
                "examples/proof/answer.txt",
                "examples/proof/certificate.json",
                "examples/proof/judgment.json",
                "examples/proof/judgment.json",
            ])
            .output()
            .unwrap(),
    );
    assert_eq!(result["report"]["current"]["percent"], 99.9);
    assert_eq!(result["celebration"]["display_percent"], 10000);
    assert_eq!(result["celebration"]["official_percent"], 99.9);
    assert!(
        result["celebration"]["message"]
            .as_str()
            .unwrap()
            .contains("celebration")
    );
    assert_eq!(
        result["verification"]["checker"],
        "finite-boolean-entailment-v1"
    );
    assert!(result["report"].get("display_percent").is_none());

    let mut certificate = fixture("examples/proof/certificate.json");
    certificate["rows"].as_array_mut().unwrap().pop();
    let invalid = temp.join("incomplete.json");
    write(&invalid, &certificate);
    let output = cli()
        .arg("proof-evaluate")
        .arg(&contract_file)
        .arg(&policy_file)
        .arg("examples/proof/answer.txt")
        .arg(invalid)
        .args([
            "examples/proof/judgment.json",
            "examples/proof/judgment.json",
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
}

#[test]
fn proof_api_recomputes_the_report_and_rejects_forged_reports_and_certificates() {
    let payload = proof_payload();
    let invoke = |payload: Value| {
        service::handle_json(&service::ServiceContext::default(),
        &serde_json::to_vec(&json!({"api_version":service::API_VERSION,"request_id":"proof-check","payload":payload})).unwrap())
    };
    let valid = invoke(payload.clone());
    assert_eq!(valid.http_status(), 200);
    assert_eq!(valid.result.unwrap()["report"]["current"]["percent"], 99.9);

    let mut forged = payload.clone();
    forged["report"] = json!({"current":{"percent":10000}});
    assert_eq!(invoke(forged).http_status(), 400);

    let mut changed = payload;
    let original = changed["certificate"]["rows"][0]["conclusion"]
        .as_bool()
        .unwrap();
    changed["certificate"]["rows"][0]["conclusion"] = json!(!original);
    let invalid = invoke(changed);
    assert_eq!(invalid.http_status(), 422);
    assert!(invalid.result.is_none());
}

#[test]
fn failed_regression_gate_writes_reviewable_report_and_exits_three() {
    let temp = TempDir::new();
    let destination = temp.join("gate.json");
    let output = cli()
        .args([
            "benchmark-gate",
            "benchmarks/adversarial/suite.json",
            "benchmarks/adversarial/manual-results.json",
            "benchmarks/adversarial/gate-policy.json",
            "--out",
        ])
        .arg(&destination)
        .output()
        .unwrap();
    assert_eq!(
        output.status.code(),
        Some(3),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let summary: Value = serde_json::from_slice(&output.stdout).unwrap();
    let saved: Value = serde_json::from_slice(&fs::read(&destination).unwrap()).unwrap();
    assert_eq!(summary["gate"], saved);
    assert_eq!(saved["passed"], false);
    assert!(!saved["violations"].as_array().unwrap().is_empty());

    let overwritten = cli()
        .args([
            "benchmark-gate",
            "benchmarks/adversarial/suite.json",
            "benchmarks/adversarial/manual-results.json",
            "benchmarks/adversarial/gate-policy.json",
            "--out",
        ])
        .arg(&destination)
        .output()
        .unwrap();
    assert_eq!(overwritten.status.code(), Some(2));
    assert_eq!(
        serde_json::from_slice::<Value>(&fs::read(destination).unwrap()).unwrap(),
        saved
    );
}

#[test]
fn api_reports_regression_failure_as_data_but_rejects_policy_hash_tampering() {
    let mut request = json!({"api_version":service::API_VERSION,"request_id":"gate-check","payload":{
        "operation":"benchmark_gate","suite":fixture("benchmarks/adversarial/suite.json"),
        "results":fixture("benchmarks/adversarial/manual-results.json"),"policy":fixture("benchmarks/adversarial/gate-policy.json")
    }});
    let context = service::ServiceContext::default();
    let result = service::handle_json(&context, &serde_json::to_vec(&request).unwrap());
    assert_eq!(result.http_status(), 200);
    assert_eq!(result.result.unwrap()["gate"]["passed"], false);
    request["payload"]["policy"]["suite_hash"] = json!("0".repeat(64));
    let invalid = service::handle_json(&context, &serde_json::to_vec(&request).unwrap());
    assert_eq!(invalid.http_status(), 422);
    assert!(invalid.result.is_none());
}

#[test]
fn passing_manual_protocol_fixture_exits_zero_without_claiming_live_validation() {
    let temp = TempDir::new();
    let suite = fixture("benchmarks/adversarial/suite.json");
    let mut results = fixture("benchmarks/adversarial/manual-results.json");
    // This is deliberately authored protocol data, not replacement model evidence.
    for trial in results["trials"].as_array_mut().unwrap() {
        let task = suite["tasks"]
            .as_array()
            .unwrap()
            .iter()
            .find(|task| task["id"] == trial["task_id"])
            .unwrap();
        let case = task["cases"]
            .as_array()
            .unwrap()
            .iter()
            .find(|case| case["id"] == trial["case_id"])
            .unwrap();
        for provider in ["openai", "claude"] {
            for row in trial[provider]["judgments"].as_array_mut().unwrap() {
                let expected = case["expected"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|expected| expected["criterion_id"] == row["criterion_id"])
                    .unwrap();
                row["anchor_id"] = expected["allowed_anchor_ids"][0].clone();
                row["evidence"] = json!([case["answer"]]);
            }
        }
    }
    let input = temp.join("authored-results.json");
    let report = temp.join("passed-gate.json");
    write(&input, &results);
    let summary = successful(
        cli()
            .args(["benchmark-gate", "benchmarks/adversarial/suite.json"])
            .arg(input)
            .arg("benchmarks/adversarial/gate-policy.json")
            .arg("--out")
            .arg(report)
            .output()
            .unwrap(),
    );
    assert_eq!(summary["gate"]["passed"], true);
    assert_eq!(summary["gate"]["benchmark"]["origin"], "manual_fixture");
    assert_eq!(summary["gate"]["policy"]["mode"], "development_smoke");
    assert!(
        summary["gate"]["interpretation"]
            .as_str()
            .unwrap()
            .contains("Manual fixtures test the software only")
    );
}

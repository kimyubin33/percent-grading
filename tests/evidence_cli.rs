//! Offline adversarial integration tests. All documents and reviews are synthetic.
//! No test calls an AI provider or fetches a source.
use percent_grading::{contract, evidence};
use serde_json::{Value, json};
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
            "percent-grading-evidence-cli-{}-{stamp}-{}",
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

fn success(output: Output) -> Value {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

fn failure(output: Output) {
    assert_eq!(
        output.status.code(),
        Some(2),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(output.stdout.is_empty());
}

fn read(path: &Path) -> Value {
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}
fn write(path: &Path, value: &Value) {
    fs::write(path, serde_json::to_vec_pretty(value).unwrap()).unwrap();
}

struct Fixture {
    temp: TempDir,
    lock: PathBuf,
    policy: PathBuf,
    dossier: Value,
}

impl Fixture {
    fn new(version: &str, min_independent: usize) -> Self {
        let temp = TempDir::new();
        let lock = temp.join("contract.json");
        success(
            cli()
                .args(["compile", "policies/apple.aha", "--out"])
                .arg(&lock)
                .output()
                .unwrap(),
        );
        let contract = read(&lock);
        let mut source = format!(
            "\u{b0b1}\u{b9d0} EvidencePolicy: version*, domain*, inclusion*, exclusion*, query_plan*, cutoff*, max_sources*(\u{c815}\u{c218}), min_independent*(\u{c815}\u{c218}), max_context_bytes*(\u{c815}\u{c218})\n\
             \u{b0b1}\u{b9d0} Claim: criterion*, statement*, conditions*, kind*\n\
             \u{c7a5}\u{ba74} EvidenceTest\n  EvidencePolicy test: version {version}, domain Synthetic integration fixture, inclusion Inspect direct source support, exclusion Exclude inapplicable material, query_plan Search support and counterevidence, cutoff 2026-11-11, max_sources 10, min_independent {min_independent}, max_context_bytes 65536\n"
        );
        let mut links = Vec::new();
        for criterion in contract["contract"]["criteria"].as_array().unwrap() {
            let id = criterion["id"].as_str().unwrap();
            let kind = if criterion["dimension"] == "future" {
                "hypothesis"
            } else {
                "established"
            };
            source.push_str(&format!("  Claim claim_{id}: criterion {id}, statement Synthetic fixture claim for {id}, conditions Fixture conditions only, kind {kind}\n"));
            links.push(json!({"id":format!("link_{id}"),"claim_id":format!("claim_{id}"),"source_id":"fixture_source","quote":"Synthetic test evidence.","location":"Fixture paragraph 1"}));
        }
        let source_path = temp.join("policy.aha");
        fs::write(&source_path, source).unwrap();
        let policy = temp.join("policy.json");
        success(
            cli()
                .arg("evidence-policy")
                .arg(&lock)
                .arg(source_path)
                .arg("--out")
                .arg(&policy)
                .output()
                .unwrap(),
        );
        let dossier = json!({
            "format":evidence::DOSSIER_FORMAT,
            "sources":[{"id":"fixture_source","title":"Synthetic integration fixture; not scientific evidence","url":"https://example.org/synthetic-fixture","origin_id":"fixture_origin","kind":"original","locator":"Fixture paragraph 1","content":"Synthetic test evidence."}],
            "links":links,
            "searches":[{"query":"synthetic fixture support","purpose":"support","outcome":"Synthetic source supplied by this test."},{"query":"synthetic fixture counterevidence","purpose":"counter","outcome":"Synthetic search record only; no network search performed."}]
        });
        Self {
            temp,
            lock,
            policy,
            dossier,
        }
    }

    fn prepare(&self, label: &str, dossier: &Value) -> PathBuf {
        let path = self.temp.join(&format!("{label}.dossier.json"));
        write(&path, dossier);
        let input = self.temp.join(&format!("{label}.input.json"));
        success(
            cli()
                .arg("evidence-prepare")
                .arg(&self.lock)
                .arg(&self.policy)
                .arg(path)
                .arg("--out")
                .arg(&input)
                .output()
                .unwrap(),
        );
        input
    }

    fn reviews(input: &Path) -> Value {
        let input = read(input);
        let rows: Vec<_> = input["dossier"]["links"].as_array().unwrap().iter().map(|link| json!({
            "link_id":link["id"],"relation":"supports","applicable":true,"quality":"eligible","reason":"Synthetic review fixture; not a scientific finding."
        })).collect();
        json!({"input_hash":input["input_hash"],"reviews":rows})
    }

    fn import(&self, label: &str, input: &Path, openai: &Value, claude: &Value) -> PathBuf {
        let a = self.temp.join(&format!("{label}.openai.json"));
        let b = self.temp.join(&format!("{label}.claude.json"));
        let pack = self.temp.join(&format!("{label}.evidence.json"));
        write(&a, openai);
        write(&b, claude);
        success(
            cli()
                .arg("evidence-import")
                .arg(&self.lock)
                .arg(input)
                .arg(a)
                .arg(b)
                .arg("--out")
                .arg(&pack)
                .output()
                .unwrap(),
        );
        pack
    }

    fn evaluate(&self, pack: &Path) -> Value {
        let answer = "Synthetic learner answer.";
        let answer_path = self.temp.join("answer.txt");
        fs::write(&answer_path, answer).unwrap();
        let lock = read(&self.lock);
        let judgments: Vec<_> = lock["contract"]["criteria"].as_array().unwrap().iter().map(|criterion| {
            let full = criterion["anchors"].as_array().unwrap().iter().find(|anchor| anchor["points"] == 100).unwrap();
            json!({"criterion_id":criterion["id"],"anchor_id":full["id"],"evidence":[answer],"reason":"Synthetic full-credit grading assertion to exercise the independent evidence gate.","conditions":["Fixture conditions only"]})
        }).collect();
        let judgment = json!({"contract_hash":lock["contract_hash"],"answer_hash":contract::hash(answer.as_bytes()),"judgments":judgments});
        let judge_path = self.temp.join("judgment.json");
        write(&judge_path, &judgment);
        success(
            cli()
                .arg("evaluate")
                .arg(&self.lock)
                .arg(answer_path)
                .arg(&judge_path)
                .arg(judge_path)
                .arg("--evidence")
                .arg(pack)
                .output()
                .unwrap(),
        )
    }
}

fn principle(report: &Value) -> &Value {
    report["report"]["criteria"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["criterion_id"] == "principle")
        .unwrap()
}

#[test]
fn missing_source_support_cannot_be_overridden_by_two_full_credit_judges() {
    let mut fixture = Fixture::new("1", 1);
    fixture.dossier["links"]
        .as_array_mut()
        .unwrap()
        .retain(|link| link["id"] != "link_principle");
    let input = fixture.prepare("missing", &fixture.dossier);
    let review = Fixture::reviews(&input);
    let pack = fixture.import("missing", &input, &review, &review);
    let report = fixture.evaluate(&pack);
    assert!(report["report"]["current"]["percent"].is_null());
    assert_eq!(report["report"]["current"]["coverage_percent"], 70.0);
    assert_eq!(report["report"]["current"]["lower_percent"], 70.0);
    assert_eq!(report["report"]["current"]["upper_percent"], 99.9);
    assert_eq!(principle(&report)["status"], "evidence_unresolved");
    assert!(principle(&report)["awarded_points"].is_null());
    assert_eq!(principle(&report)["openai"]["anchor_id"], "principle_sound");
    assert_eq!(report["report"]["future"]["percent"], 99.9);
}

#[test]
fn contradicted_or_disagreed_evidence_withholds_grade_without_assigning_zero() {
    let fixture = Fixture::new("1", 1);
    let input = fixture.prepare("base", &fixture.dossier);
    let support = Fixture::reviews(&input);
    let mut contrary = support.clone();
    contrary["reviews"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|row| row["link_id"] == "link_principle")
        .unwrap()["relation"] = json!("contradicts");
    for (label, a, b) in [
        ("contradicted", &contrary, &contrary),
        ("disagreed", &support, &contrary),
    ] {
        let pack = fixture.import(label, &input, a, b);
        let report = fixture.evaluate(&pack);
        assert!(report["report"]["current"]["percent"].is_null());
        assert!(principle(&report)["awarded_points"].is_null());
        assert_eq!(principle(&report)["lower_points"], 0);
        assert_eq!(principle(&report)["upper_points"], 100);
    }
}

#[test]
fn repeated_document_with_fresh_origin_ids_is_not_independent_corroboration() {
    let mut fixture = Fixture::new("1", 2);
    let mut copy = fixture.dossier["sources"][0].clone();
    copy["id"] = json!("copy_source");
    copy["origin_id"] = json!("invented_other_origin");
    copy["url"] = json!("https://example.org/another-fixture-url");
    fixture.dossier["sources"]
        .as_array_mut()
        .unwrap()
        .push(copy);
    let copies: Vec<_> = fixture.dossier["links"]
        .as_array()
        .unwrap()
        .iter()
        .map(|link| {
            let mut copy = link.clone();
            copy["id"] = json!(format!("copy_{}", link["id"].as_str().unwrap()));
            copy["source_id"] = json!("copy_source");
            copy
        })
        .collect();
    fixture.dossier["links"]
        .as_array_mut()
        .unwrap()
        .extend(copies);
    let input = fixture.prepare("duplicated", &fixture.dossier);
    let review = Fixture::reviews(&input);
    let pack = fixture.import("duplicated", &input, &review, &review);
    assert!(
        read(&pack)["blocked_criteria"]
            .as_array()
            .unwrap()
            .iter()
            .any(|id| id == "principle")
    );
    assert!(fixture.evaluate(&pack)["report"]["current"]["percent"].is_null());
}

#[test]
fn fabricated_quote_and_modified_pack_are_rejected_before_grading() {
    let fixture = Fixture::new("1", 1);
    let mut bad = fixture.dossier.clone();
    bad["links"][0]["quote"] = json!("This sentence was never in the source.");
    let path = fixture.temp.join("fabricated.dossier.json");
    write(&path, &bad);
    let target = fixture.temp.join("fabricated.input.json");
    failure(
        cli()
            .arg("evidence-prepare")
            .arg(&fixture.lock)
            .arg(&fixture.policy)
            .arg(path)
            .arg("--out")
            .arg(&target)
            .output()
            .unwrap(),
    );
    assert!(!target.exists());
    let input = fixture.prepare("valid", &fixture.dossier);
    let review = Fixture::reviews(&input);
    let pack = fixture.import("valid", &input, &review, &review);
    let mut changed = read(&pack);
    changed["claims"][0]["status"] = json!("contradicted");
    write(&pack, &changed);
    failure(
        cli()
            .arg("inspect-evidence")
            .arg(&fixture.lock)
            .arg(&pack)
            .output()
            .unwrap(),
    );
    let runs = fixture.temp.join("runs");
    let output = cli()
        .arg("grade")
        .arg(&fixture.lock)
        .args([
            "examples/rust/apple-red.txt",
            "--openai-model",
            "offline-openai",
            "--claude-model",
            "offline-claude",
            "--evidence",
        ])
        .arg(pack)
        .arg("--runs-dir")
        .arg(&runs)
        .output()
        .unwrap();
    assert!(!String::from_utf8_lossy(&output.stderr).contains("API_KEY"));
    failure(output);
    assert!(!runs.exists());
}

#[test]
fn evidence_import_never_replaces_an_existing_pack() {
    let fixture = Fixture::new("1", 1);
    let input = fixture.prepare("valid", &fixture.dossier);
    let review = Fixture::reviews(&input);
    let pack = fixture.import("valid", &input, &review, &review);
    let original = fs::read(&pack).unwrap();
    failure(
        cli()
            .arg("evidence-import")
            .arg(&fixture.lock)
            .arg(input)
            .arg(fixture.temp.join("valid.openai.json"))
            .arg(fixture.temp.join("valid.claude.json"))
            .arg("--out")
            .arg(&pack)
            .output()
            .unwrap(),
    );
    assert_eq!(fs::read(pack).unwrap(), original);
}

#[test]
fn revision_preserves_distinct_submissions_with_identical_answers() {
    let old = Fixture::new("1", 1);
    let new = Fixture::new("2", 1);
    let old_input = old.prepare("old", &old.dossier);
    let new_input = new.prepare("new", &new.dossier);
    let old_review = Fixture::reviews(&old_input);
    let new_review = Fixture::reviews(&new_input);
    let old_pack = old.import("old", &old_input, &old_review, &old_review);
    let new_pack = new.import("new", &new_input, &new_review, &new_review);
    let old_bytes = fs::read(&old_pack).unwrap();
    let hash = contract::hash(b"same answer by two students");
    let cohort = old.temp.join("cohort.json");
    write(
        &cohort,
        &json!({"format":"percent-grading-cohort-v1","task_id":"fixture-task","members":[{"submission_id":"student-a","answer_hash":hash,"previous_run_id":null},{"submission_id":"student-b","answer_hash":hash,"previous_run_id":null}]}),
    );
    let target = old.temp.join("revision.json");
    success(
        cli()
            .arg("revise-evidence")
            .arg(&old.lock)
            .arg(&old_pack)
            .arg(&new_pack)
            .arg(cohort)
            .args(["--reason", "Revised source eligibility policy", "--out"])
            .arg(&target)
            .output()
            .unwrap(),
    );
    let plan = read(&target);
    assert_eq!(plan["pending"].as_array().unwrap().len(), 2);
    for row in plan["pending"].as_array().unwrap() {
        assert_eq!(row["status"], "pending");
        assert_eq!(row["required_evidence_hash"], read(&new_pack)["pack_hash"]);
    }
    assert_eq!(fs::read(&old_pack).unwrap(), old_bytes);
}

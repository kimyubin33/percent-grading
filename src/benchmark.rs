//! Evaluation against declared expectations, not proof of grading accuracy.
//!
//! Expectations never enter the grading prompt. Complete selected cohorts and
//! all requested repeats are required, so omission cannot improve the report.
use crate::{
    VERSION,
    contract::{self, LockedContract, hash},
    evidence::EvidencePack,
    grading::{self, Judgment, Report},
    runner::validate_answer,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const SUITE_FORMAT: &str = "percent-grading-benchmark-v1";
pub const RESULTS_FORMAT: &str = "percent-grading-benchmark-results-v1";
pub const REPORT_FORMAT: &str = "percent-grading-benchmark-report-v1";
pub const MAX_TASKS: usize = 100;
pub const MAX_CASES: usize = 1000;
pub const MAX_RELATIONS: usize = 4000;
pub const MAX_SUITE_BYTES: usize = 16 * 1024 * 1024;
const NOTICE: &str = "Metrics compare submitted judgments with authored expectations, not independently established truth or educational validity. Manual fixtures are not model benchmarks. Origin labels and run IDs do not authenticate provider execution. Accepted accuracy excludes withheld criteria; always inspect coverage and unexpected withholding alongside it. Repeated trial records measure observed categorical variation, not replay determinism or a guarantee of future stability. Splits apply to whole declared tasks; semantic leakage between differently written tasks still requires review.";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct BenchmarkSuite {
    pub format: String,
    pub id: String,
    pub version: String,
    pub tasks: Vec<BenchmarkTask>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct BenchmarkTask {
    pub id: String,
    pub split: String,
    pub policy_source: String,
    pub cases: Vec<BenchmarkCase>,
    pub relations: Vec<Relation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence: Option<EvidencePack>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct BenchmarkCase {
    pub id: String,
    pub category: String,
    pub answer: String,
    pub expected: Vec<Expectation>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Expectation {
    pub criterion_id: String,
    /// "unresolved" permits a withheld result, including an evidence gate.
    pub allowed_anchor_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Relation {
    pub id: String,
    pub left_case_id: String,
    pub right_case_id: String,
    pub criterion_id: String,
    /// Compare RIGHT to LEFT: equal, not_lower, or not_higher.
    pub kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Results {
    pub format: String,
    pub suite_hash: String,
    pub origin: String,
    pub split: String,
    pub repeats: u32,
    pub trials: Vec<Trial>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Trial {
    pub task_id: String,
    pub case_id: String,
    /// One-based repetition number. A repeated API trial requires its own run.
    pub repeat: u32,
    pub openai: Judgment,
    pub claude: Judgment,
    pub run_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(deny_unknown_fields)]
pub struct Metrics {
    pub criteria_total: u64,
    pub model_agreements: u64,
    pub two_judge_wrong_agreements: u64,
    pub accepted: u64,
    pub accepted_correct: u64,
    pub wrong_accepted: u64,
    pub withheld: u64,
    pub expected_withheld: u64,
    pub unexpected_withheld: u64,
    pub accepted_accuracy_percent: Option<f64>,
    pub settled_coverage_percent: Option<f64>,
    pub expectation_match_percent: Option<f64>,
    pub wrong_agreement_percent: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ExpectationCheck {
    pub criterion_id: String,
    pub allowed_anchor_ids: Vec<String>,
    pub openai_anchor_id: String,
    pub claude_anchor_id: String,
    pub settled_anchor_id: Option<String>,
    pub status: String,
    pub matches_expectation: bool,
    pub two_judge_wrong_agreement: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct TrialReport {
    pub task_id: String,
    pub split: String,
    pub case_id: String,
    pub category: String,
    pub repeat: u32,
    pub run_id: Option<String>,
    pub checks: Vec<ExpectationCheck>,
    pub report: Report,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RelationOutcome {
    pub task_id: String,
    pub split: String,
    pub relation_id: String,
    pub repeat: u32,
    pub criterion_id: String,
    pub kind: String,
    pub left_case_id: String,
    pub right_case_id: String,
    pub left_points: Option<u32>,
    pub right_points: Option<u32>,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct RelationMetrics {
    pub total: u64,
    pub passed: u64,
    pub failed: u64,
    pub indeterminate: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct StabilityObservation {
    pub task_id: String,
    pub case_id: String,
    pub criterion_id: String,
    pub changed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(deny_unknown_fields)]
pub struct StabilityReport {
    pub comparable_criteria: u64,
    pub changed_criteria: u64,
    pub change_percent: Option<f64>,
    pub observations: Vec<StabilityObservation>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct BenchmarkReport {
    pub format: String,
    pub engine_version: String,
    pub suite_hash: String,
    pub results_hash: String,
    pub origin: String,
    pub split: String,
    pub repeats: u32,
    pub metrics: Metrics,
    pub by_split: BTreeMap<String, Metrics>,
    pub by_category: BTreeMap<String, Metrics>,
    pub trials: Vec<TrialReport>,
    pub relations: Vec<RelationOutcome>,
    pub relation_metrics: RelationMetrics,
    pub stability: StabilityReport,
    pub interpretation: String,
    pub report_hash: String,
}

fn identifier(value: &str, label: &str) -> Result<(), String> {
    if value.is_empty()
        || value.len() > 64
        || !value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
    {
        return Err(format!(
            "{label}: use 1..64 ASCII letters, digits, underscores or hyphens"
        ));
    }
    Ok(())
}

fn digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn bytes<T: Serialize>(value: &T) -> Result<Vec<u8>, String> {
    serde_json::to_vec(value).map_err(|e| e.to_string())
}

fn compile_suite(suite: &BenchmarkSuite) -> Result<BTreeMap<String, LockedContract>, String> {
    if suite.format != SUITE_FORMAT {
        return Err(format!("Unsupported suite format; expected {SUITE_FORMAT}"));
    }
    identifier(&suite.id, "suite id")?;
    if suite.version.trim().is_empty() || suite.version.len() > 128 || suite.version.contains('\0')
    {
        return Err("Suite version must contain 1..128 bytes without NUL".into());
    }
    if suite.tasks.is_empty()
        || suite.tasks.len() > MAX_TASKS
        || bytes(suite)?.len() > MAX_SUITE_BYTES
    {
        return Err("Benchmark requires 1..100 tasks within 16 MiB".into());
    }
    let mut compiled = BTreeMap::new();
    let mut contract_splits = BTreeMap::new();
    let mut cases = 0usize;
    let mut answer_bytes = 0usize;
    let mut relation_count = 0usize;
    for task in &suite.tasks {
        identifier(&task.id, "task id")?;
        if !["development", "holdout"].contains(&task.split.as_str()) {
            return Err("Task split must be development or holdout".into());
        }
        if compiled.contains_key(&task.id) {
            return Err(format!("Duplicate task id: {}", task.id));
        }
        let lock = contract::compile(&task.policy_source)?;
        let normalized = |text: &str| {
            text.split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
                .to_lowercase()
        };
        let task_identity = hash(&bytes(&(
            normalized(&lock.contract.question),
            normalized(&lock.contract.context),
        ))?);
        if let Some(other_split) = contract_splits.insert(task_identity, task.split.clone())
            && other_split != task.split
        {
            return Err(
                "The same task question and context cannot occur in both development and holdout"
                    .into(),
            );
        }
        if let Some(pack) = &task.evidence {
            pack.verify(&lock)?;
        }
        relation_count += task.relations.len();
        if task.cases.is_empty() || task.relations.len() > 2000 || relation_count > MAX_RELATIONS {
            return Err(
                "Each task needs cases; relations are limited to 2000 per task and 4000 per suite"
                    .into(),
            );
        }
        let mut case_ids = BTreeSet::new();
        for case in &task.cases {
            identifier(&case.id, "case id")?;
            identifier(&case.category, "category")?;
            if !case_ids.insert(case.id.as_str()) {
                return Err(format!("{}: duplicate case id", task.id));
            }
            validate_answer(&case.answer)?;
            answer_bytes += case.answer.len();
            cases += 1;
            if cases > MAX_CASES || answer_bytes > 8 * 1024 * 1024 {
                return Err("Benchmark exceeds 1000 cases or 8 MiB of answers".into());
            }
            if case.expected.len() != lock.contract.criteria.len() {
                return Err("Every case must declare exactly one expectation per criterion".into());
            }
            let mut seen = BTreeSet::new();
            for expected in &case.expected {
                let criterion = lock
                    .contract
                    .criteria
                    .iter()
                    .find(|c| c.id == expected.criterion_id)
                    .ok_or("Expectation references an unknown criterion")?;
                if !seen.insert(expected.criterion_id.as_str())
                    || expected.allowed_anchor_ids.is_empty()
                    || expected.allowed_anchor_ids.len() > criterion.anchors.len() + 1
                {
                    return Err(
                        "Duplicate expectation or empty/oversized allowed anchor list".into(),
                    );
                }
                let mut anchors = BTreeSet::new();
                for anchor in &expected.allowed_anchor_ids {
                    if !anchors.insert(anchor.as_str())
                        || (anchor != "unresolved"
                            && !criterion.anchors.iter().any(|a| a.id == *anchor))
                    {
                        return Err(
                            "Expected anchors must be unique and belong to their criterion".into(),
                        );
                    }
                }
            }
        }
        let mut relation_ids = BTreeSet::new();
        for relation in &task.relations {
            identifier(&relation.id, "relation id")?;
            if !relation_ids.insert(&relation.id)
                || !case_ids.contains(relation.left_case_id.as_str())
                || !case_ids.contains(relation.right_case_id.as_str())
                || relation.left_case_id == relation.right_case_id
                || !lock
                    .contract
                    .criteria
                    .iter()
                    .any(|c| c.id == relation.criterion_id)
                || !["equal", "not_lower", "not_higher"].contains(&relation.kind.as_str())
            {
                return Err("Invalid relation: use distinct cases in the same task, a known criterion and comparison".into());
            }
        }
        compiled.insert(task.id.clone(), lock);
    }
    Ok(compiled)
}

pub fn validate(suite: &BenchmarkSuite) -> Result<(), String> {
    compile_suite(suite).map(|_| ())
}

/// Includes expectations and declared splits; changing them starts a new suite.
pub fn suite_hash(suite: &BenchmarkSuite) -> Result<String, String> {
    validate(suite)?;
    Ok(hash(&bytes(suite)?))
}

fn percent(numerator: u64, denominator: u64) -> Option<f64> {
    (denominator != 0).then(|| 100.0 * numerator as f64 / denominator as f64)
}

impl Metrics {
    fn add(&mut self, check: &ExpectationCheck) {
        self.criteria_total += 1;
        let agrees = check.openai_anchor_id == check.claude_anchor_id
            && check.openai_anchor_id != "unresolved";
        self.model_agreements += u64::from(agrees);
        self.two_judge_wrong_agreements += u64::from(check.two_judge_wrong_agreement);
        if check.settled_anchor_id.is_some() {
            self.accepted += 1;
            if check.matches_expectation {
                self.accepted_correct += 1;
            } else {
                self.wrong_accepted += 1;
            }
        } else {
            self.withheld += 1;
            if check.matches_expectation {
                self.expected_withheld += 1;
            } else {
                self.unexpected_withheld += 1;
            }
        }
    }

    fn finish(&mut self) {
        self.accepted_accuracy_percent = percent(self.accepted_correct, self.accepted);
        self.settled_coverage_percent = percent(self.accepted, self.criteria_total);
        self.expectation_match_percent = percent(
            self.accepted_correct + self.expected_withheld,
            self.criteria_total,
        );
        self.wrong_agreement_percent =
            percent(self.two_judge_wrong_agreements, self.model_agreements);
    }
}

pub fn evaluate(suite: &BenchmarkSuite, results: &Results) -> Result<BenchmarkReport, String> {
    let compiled = compile_suite(suite)?;
    let expected_hash = hash(&bytes(suite)?);
    if results.format != RESULTS_FORMAT || results.suite_hash != expected_hash {
        return Err("Results format or suite hash does not match this benchmark".into());
    }
    if !["manual_fixture", "api_run"].contains(&results.origin.as_str())
        || !["all", "development", "holdout"].contains(&results.split.as_str())
        || !(1..=5).contains(&results.repeats)
    {
        return Err("Invalid results origin, split or repeat count (1..5)".into());
    }
    let tasks: BTreeMap<_, _> = suite
        .tasks
        .iter()
        .filter(|t| results.split == "all" || results.split == t.split)
        .map(|t| (t.id.as_str(), t))
        .collect();
    if tasks.is_empty() {
        return Err("The requested split contains no tasks".into());
    }
    let expected_count: usize =
        tasks.values().map(|t| t.cases.len()).sum::<usize>() * results.repeats as usize;
    if results.trials.len() != expected_count {
        return Err(
            "Results must contain every selected case for every requested repetition".into(),
        );
    }
    let mut trial_map = BTreeMap::new();
    let mut run_ids = BTreeSet::new();
    for trial in &results.trials {
        let task = tasks
            .get(trial.task_id.as_str())
            .ok_or("Trial references an unselected task")?;
        if !task.cases.iter().any(|c| c.id == trial.case_id)
            || !(1..=results.repeats).contains(&trial.repeat)
            || trial_map
                .insert(
                    (trial.task_id.as_str(), trial.case_id.as_str(), trial.repeat),
                    trial,
                )
                .is_some()
        {
            return Err("Unknown case, out-of-range repetition or duplicate trial".into());
        }
        if let Some(id) = &trial.run_id {
            if !digest(id) || (results.origin == "api_run" && !run_ids.insert(id.as_str())) {
                return Err("API trials require distinct lowercase SHA-256 run IDs; cached trials are not repeats".into());
            }
        } else if results.origin == "api_run" {
            return Err("An API trial must identify its archived run".into());
        }
    }
    let mut metrics = Metrics::default();
    let mut by_split = BTreeMap::from([
        ("development".into(), Metrics::default()),
        ("holdout".into(), Metrics::default()),
    ]);
    let mut by_category: BTreeMap<String, Metrics> = BTreeMap::new();
    let mut trials = Vec::with_capacity(trial_map.len());
    for ((task_id, case_id, repeat), trial) in &trial_map {
        let task = tasks[task_id];
        let case = task
            .cases
            .iter()
            .find(|c| c.id == *case_id)
            .expect("validated case");
        let lock = &compiled[*task_id];
        let mut report = grading::aggregate(
            &lock.contract,
            &lock.contract_hash,
            &case.answer,
            &hash(case.answer.as_bytes()),
            &trial.openai,
            &trial.claude,
        )?;
        if let Some(pack) = &task.evidence {
            grading::apply_evidence_gate(&mut report, &pack.blocked_criteria)?;
        }
        let mut checks = Vec::new();
        for row in &report.criteria {
            let expected = case
                .expected
                .iter()
                .find(|e| e.criterion_id == row.criterion_id)
                .expect("validated expectation");
            let settled = row.awarded_points.map(|_| row.openai.anchor_id.clone());
            let raw_agrees = row.openai.anchor_id == row.claude.anchor_id
                && row.openai.anchor_id != "unresolved";
            let check = ExpectationCheck {
                criterion_id: row.criterion_id.clone(),
                allowed_anchor_ids: expected.allowed_anchor_ids.clone(),
                openai_anchor_id: row.openai.anchor_id.clone(),
                claude_anchor_id: row.claude.anchor_id.clone(),
                matches_expectation: expected
                    .allowed_anchor_ids
                    .iter()
                    .any(|a| a == settled.as_deref().unwrap_or("unresolved")),
                settled_anchor_id: settled,
                status: row.status.clone(),
                two_judge_wrong_agreement: raw_agrees
                    && !expected.allowed_anchor_ids.contains(&row.openai.anchor_id),
            };
            metrics.add(&check);
            by_split
                .get_mut(&task.split)
                .expect("valid split")
                .add(&check);
            by_category
                .entry(case.category.clone())
                .or_default()
                .add(&check);
            checks.push(check);
        }
        trials.push(TrialReport {
            task_id: (*task_id).into(),
            split: task.split.clone(),
            case_id: (*case_id).into(),
            category: case.category.clone(),
            repeat: *repeat,
            run_id: trial.run_id.clone(),
            checks,
            report,
        });
    }
    metrics.finish();
    for item in by_split.values_mut().chain(by_category.values_mut()) {
        item.finish();
    }
    let reports: BTreeMap<_, _> = trials
        .iter()
        .map(|t| ((t.task_id.as_str(), t.case_id.as_str(), t.repeat), t))
        .collect();
    let mut relations = Vec::new();
    let mut relation_metrics = RelationMetrics::default();
    for (task_id, task) in &tasks {
        let mut declared: Vec<_> = task.relations.iter().collect();
        declared.sort_by(|a, b| a.id.cmp(&b.id));
        for relation in declared {
            for repeat in 1..=results.repeats {
                let points = |case: &str| {
                    reports[&(*task_id, case, repeat)]
                        .report
                        .criteria
                        .iter()
                        .find(|c| c.criterion_id == relation.criterion_id)
                        .expect("validated relation")
                        .awarded_points
                };
                let left = points(&relation.left_case_id);
                let right = points(&relation.right_case_id);
                let status = match (left, right) {
                    (Some(a), Some(b)) => {
                        let pass = match relation.kind.as_str() {
                            "equal" => a == b,
                            "not_lower" => b >= a,
                            "not_higher" => b <= a,
                            _ => unreachable!("validated kind"),
                        };
                        if pass { "pass" } else { "fail" }
                    }
                    _ => "indeterminate",
                };
                relation_metrics.total += 1;
                match status {
                    "pass" => relation_metrics.passed += 1,
                    "fail" => relation_metrics.failed += 1,
                    _ => relation_metrics.indeterminate += 1,
                }
                relations.push(RelationOutcome {
                    task_id: (*task_id).into(),
                    split: task.split.clone(),
                    relation_id: relation.id.clone(),
                    repeat,
                    criterion_id: relation.criterion_id.clone(),
                    kind: relation.kind.clone(),
                    left_case_id: relation.left_case_id.clone(),
                    right_case_id: relation.right_case_id.clone(),
                    left_points: left,
                    right_points: right,
                    status: status.into(),
                });
            }
        }
    }
    let mut stability = StabilityReport::default();
    if results.repeats > 1 {
        for (task_id, task) in &tasks {
            let mut cases: Vec<_> = task.cases.iter().collect();
            cases.sort_by(|a, b| a.id.cmp(&b.id));
            for case in cases {
                let first = &reports[&(*task_id, case.id.as_str(), 1)].report;
                for baseline in &first.criteria {
                    let changed = (2..=results.repeats).any(|repeat| {
                        let current = reports[&(*task_id, case.id.as_str(), repeat)]
                            .report
                            .criteria
                            .iter()
                            .find(|c| c.criterion_id == baseline.criterion_id)
                            .expect("same contract");
                        current.status != baseline.status
                            || current.awarded_points != baseline.awarded_points
                            || current.openai.anchor_id != baseline.openai.anchor_id
                            || current.claude.anchor_id != baseline.claude.anchor_id
                    });
                    stability.comparable_criteria += 1;
                    stability.changed_criteria += u64::from(changed);
                    stability.observations.push(StabilityObservation {
                        task_id: (*task_id).into(),
                        case_id: case.id.clone(),
                        criterion_id: baseline.criterion_id.clone(),
                        changed,
                    });
                }
            }
        }
        stability.change_percent =
            percent(stability.changed_criteria, stability.comparable_criteria);
    }
    let mut canonical_results = results.clone();
    canonical_results.trials.sort_by(|a, b| {
        (&a.task_id, &a.case_id, a.repeat).cmp(&(&b.task_id, &b.case_id, b.repeat))
    });
    let mut report = BenchmarkReport {
        format: REPORT_FORMAT.into(),
        engine_version: VERSION.into(),
        suite_hash: expected_hash,
        results_hash: hash(&bytes(&canonical_results)?),
        origin: results.origin.clone(),
        split: results.split.clone(),
        repeats: results.repeats,
        metrics,
        by_split,
        by_category,
        trials,
        relations,
        relation_metrics,
        stability,
        interpretation: NOTICE.into(),
        report_hash: String::new(),
    };
    report.report_hash = hash(&bytes(&report)?);
    Ok(report)
}

impl BenchmarkReport {
    pub fn verify(&self, suite: &BenchmarkSuite, results: &Results) -> Result<(), String> {
        if *self != evaluate(suite, results)? {
            return Err("Benchmark report differs from deterministic recomputation".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grading::Row;

    fn suite() -> BenchmarkSuite {
        let source = include_str!("../policies/apple.aha").to_owned();
        let lock = contract::compile(&source).unwrap();
        let expected: Vec<_> = lock
            .contract
            .criteria
            .iter()
            .map(|criterion| Expectation {
                criterion_id: criterion.id.clone(),
                allowed_anchor_ids: vec![
                    criterion
                        .anchors
                        .iter()
                        .find(|a| a.points == 100)
                        .unwrap()
                        .id
                        .clone(),
                ],
            })
            .collect();
        BenchmarkSuite {
            format: SUITE_FORMAT.into(),
            id: "synthetic".into(),
            version: "1".into(),
            tasks: vec![BenchmarkTask {
                id: "apple".into(),
                split: "development".into(),
                policy_source: source,
                cases: vec![
                    BenchmarkCase {
                        id: "short".into(),
                        category: "concise".into(),
                        answer: "red apple".into(),
                        expected: expected.clone(),
                    },
                    BenchmarkCase {
                        id: "long".into(),
                        category: "paraphrase".into(),
                        answer: "red apple with synthetic extra words".into(),
                        expected,
                    },
                ],
                relations: vec![Relation {
                    id: "same_intent".into(),
                    left_case_id: "short".into(),
                    right_case_id: "long".into(),
                    criterion_id: "intent".into(),
                    kind: "equal".into(),
                }],
                evidence: None,
            }],
        }
    }

    fn results(suite: &BenchmarkSuite, split: &str, repeats: u32) -> Results {
        let mut trials = Vec::new();
        for task in &suite.tasks {
            if split != "all" && split != task.split {
                continue;
            }
            let lock = contract::compile(&task.policy_source).unwrap();
            for case in &task.cases {
                for repeat in 1..=repeats {
                    let judgment = Judgment {
                        contract_hash: lock.contract_hash.clone(),
                        answer_hash: hash(case.answer.as_bytes()),
                        judgments: lock
                            .contract
                            .criteria
                            .iter()
                            .map(|criterion| Row {
                                criterion_id: criterion.id.clone(),
                                anchor_id: criterion
                                    .anchors
                                    .iter()
                                    .find(|a| a.points == 100)
                                    .unwrap()
                                    .id
                                    .clone(),
                                evidence: vec![case.answer.clone()],
                                reason: "Synthetic software-test judgment; no provider call."
                                    .into(),
                                conditions: vec![],
                            })
                            .collect(),
                    };
                    trials.push(Trial {
                        task_id: task.id.clone(),
                        case_id: case.id.clone(),
                        repeat,
                        openai: judgment.clone(),
                        claude: judgment,
                        run_id: None,
                    });
                }
            }
        }
        Results {
            format: RESULTS_FORMAT.into(),
            suite_hash: suite_hash(suite).unwrap(),
            origin: "manual_fixture".into(),
            split: split.into(),
            repeats,
            trials,
        }
    }

    fn set_anchor(trial: &mut Trial, criterion_id: &str, anchor: &str) {
        for judgment in [&mut trial.openai, &mut trial.claude] {
            judgment
                .judgments
                .iter_mut()
                .find(|row| row.criterion_id == criterion_id)
                .unwrap()
                .anchor_id = anchor.into();
        }
    }

    #[test]
    fn authored_reference_matches_are_not_presented_as_live_accuracy() {
        let suite = suite();
        let input = results(&suite, "all", 1);
        let report = evaluate(&suite, &input).unwrap();
        assert_eq!(report.metrics.accepted_correct, 10);
        assert_eq!(report.metrics.accepted_accuracy_percent, Some(100.0));
        assert_eq!(report.metrics.settled_coverage_percent, Some(100.0));
        assert_eq!(report.by_split["holdout"].criteria_total, 0);
        assert_eq!(report.by_split["holdout"].accepted_accuracy_percent, None);
        assert_eq!(report.relation_metrics.passed, 1);
        assert_eq!(report.stability.change_percent, None);
        assert_eq!(report.origin, "manual_fixture");
        assert!(
            report
                .interpretation
                .contains("Manual fixtures are not model benchmarks")
        );
        report.verify(&suite, &input).unwrap();
        let mut changed = report;
        changed.metrics.accepted_correct = 9;
        assert!(changed.verify(&suite, &input).is_err());
    }

    #[test]
    fn agreeing_wrong_judges_are_reported_separately_from_coverage() {
        let suite = suite();
        let mut input = results(&suite, "all", 1);
        set_anchor(&mut input.trials[1], "intent", "intent_wrong");
        let report = evaluate(&suite, &input).unwrap();
        assert_eq!(report.metrics.two_judge_wrong_agreements, 1);
        assert_eq!(report.metrics.wrong_accepted, 1);
        assert_eq!(report.metrics.accepted_accuracy_percent, Some(90.0));
        assert_eq!(report.metrics.settled_coverage_percent, Some(100.0));
        assert_eq!(report.relation_metrics.failed, 1);
        assert!(
            report
                .trials
                .iter()
                .any(|t| t.checks.iter().any(|c| !c.matches_expectation))
        );
    }

    #[test]
    fn abstaining_everywhere_cannot_claim_perfect_accepted_accuracy() {
        let mut suite = suite();
        for expectation in &mut suite.tasks[0].cases[0].expected {
            expectation.allowed_anchor_ids = vec!["unresolved".into()];
        }
        let mut input = results(&suite, "all", 1);
        for trial in &mut input.trials {
            for row in trial
                .openai
                .judgments
                .iter_mut()
                .chain(trial.claude.judgments.iter_mut())
            {
                row.anchor_id = "unresolved".into();
            }
        }
        let report = evaluate(&suite, &input).unwrap();
        assert_eq!(report.metrics.accepted_accuracy_percent, None);
        assert_eq!(report.metrics.settled_coverage_percent, Some(0.0));
        assert_eq!(report.metrics.expected_withheld, 5);
        assert_eq!(report.metrics.unexpected_withheld, 5);
        assert_eq!(report.metrics.expectation_match_percent, Some(50.0));
        assert_eq!(report.relation_metrics.indeterminate, 1);
        assert_eq!(report.relation_metrics.passed, 0);
    }

    #[test]
    fn missing_duplicate_and_unselected_trials_cannot_improve_metrics() {
        let suite = suite();
        let original = results(&suite, "all", 1);
        let mut changed = original.clone();
        changed.trials.pop();
        assert!(evaluate(&suite, &changed).is_err());
        changed = original.clone();
        changed.trials[1] = changed.trials[0].clone();
        assert!(evaluate(&suite, &changed).is_err());
        changed = original.clone();
        changed.trials[1].task_id = "unselected".into();
        assert!(evaluate(&suite, &changed).is_err());
        changed = original.clone();
        changed.trials[1].repeat = 0;
        assert!(evaluate(&suite, &changed).is_err());
        changed = original;
        changed.suite_hash = hash(b"another suite");
        assert!(evaluate(&suite, &changed).is_err());
    }

    #[test]
    fn expected_anchors_must_cover_the_actual_compiled_contract() {
        let original = suite();
        let mut changed = original.clone();
        changed.tasks[0].cases[0].expected.pop();
        assert!(validate(&changed).is_err());
        changed = original.clone();
        changed.tasks[0].cases[0].expected[0].allowed_anchor_ids = vec!["invented".into()];
        assert!(validate(&changed).is_err());
        changed = original.clone();
        let duplicate = changed.tasks[0].cases[0].expected[0].clone();
        changed.tasks[0].cases[0].expected[1] = duplicate;
        assert!(validate(&changed).is_err());
        changed = original.clone();
        changed.tasks[0].relations[0].right_case_id = "outside_task".into();
        assert!(validate(&changed).is_err());
        let mut value = serde_json::to_value(original).unwrap();
        value["trusted_accuracy"] = serde_json::json!(100);
        assert!(serde_json::from_value::<BenchmarkSuite>(value).is_err());
    }

    #[test]
    fn split_filter_is_complete_and_duplicate_contract_leakage_is_rejected() {
        let mut suite = suite();
        let mut held = suite.tasks[0].clone();
        held.id = "second_task".into();
        held.split = "holdout".into();
        suite.tasks.push(held);
        assert!(
            validate(&suite)
                .unwrap_err()
                .contains("both development and holdout")
        );
        suite.tasks[1].policy_source = suite.tasks[1]
            .policy_source
            .replace("version 1,", "version 2,");
        assert!(validate(&suite).is_err());
        suite.tasks[1].policy_source = suite.tasks[1]
            .policy_source
            .replace("specified apple", "specified pear");
        validate(&suite).unwrap();
        let input = results(&suite, "development", 1);
        let report = evaluate(&suite, &input).unwrap();
        assert_eq!(report.trials.len(), 2);
        assert_eq!(report.by_split["holdout"].criteria_total, 0);
        let mut changed = input;
        changed.split = "all".into();
        assert!(evaluate(&suite, &changed).is_err());
    }

    #[test]
    fn fresh_repeat_ids_are_distinct_and_variation_is_measured() {
        let suite = suite();
        let mut input = results(&suite, "all", 2);
        input.origin = "api_run".into();
        assert!(evaluate(&suite, &input).is_err());
        for (i, trial) in input.trials.iter_mut().enumerate() {
            trial.run_id = Some(hash(format!("synthetic unique run {i}").as_bytes()));
        }
        assert_eq!(
            evaluate(&suite, &input).unwrap().stability.change_percent,
            Some(0.0)
        );
        set_anchor(&mut input.trials[1], "intent", "intent_wrong");
        let report = evaluate(&suite, &input).unwrap();
        assert_eq!(report.stability.comparable_criteria, 10);
        assert_eq!(report.stability.changed_criteria, 1);
        assert_eq!(report.stability.change_percent, Some(10.0));
        input.trials[1].run_id = input.trials[0].run_id.clone();
        assert!(
            evaluate(&suite, &input)
                .unwrap_err()
                .contains("cached trials are not repeats")
        );
    }

    #[test]
    fn relations_compare_right_to_left_and_withholding_is_indeterminate() {
        let mut suite = suite();
        suite.tasks[0].relations[0].kind = "not_higher".into();
        let mut input = results(&suite, "all", 1);
        set_anchor(&mut input.trials[1], "intent", "intent_wrong");
        assert_eq!(evaluate(&suite, &input).unwrap().relation_metrics.passed, 1);
        suite.tasks[0].relations[0].kind = "not_lower".into();
        input.suite_hash = suite_hash(&suite).unwrap();
        assert_eq!(evaluate(&suite, &input).unwrap().relation_metrics.failed, 1);
        input.trials[1]
            .claude
            .judgments
            .iter_mut()
            .find(|r| r.criterion_id == "intent")
            .unwrap()
            .anchor_id = "unresolved".into();
        assert_eq!(
            evaluate(&suite, &input)
                .unwrap()
                .relation_metrics
                .indeterminate,
            1
        );
    }

    #[test]
    fn evidence_gate_cannot_be_bypassed_by_benchmark_aggregation() {
        let (lock, mut input) = crate::evidence::test_input();
        input.dossier.links.clear();
        let input = crate::evidence::prepare(&lock, &input.policy, &input.dossier).unwrap();
        let review = crate::evidence::test_review(&input);
        let pack = crate::evidence::aggregate(&lock, &input, &review, &review).unwrap();
        let mut suite = suite();
        let task = &mut suite.tasks[0];
        task.policy_source = lock.source.clone();
        task.relations.clear();
        task.evidence = Some(pack);
        for case in &mut task.cases {
            case.expected = lock
                .contract
                .criteria
                .iter()
                .map(|c| Expectation {
                    criterion_id: c.id.clone(),
                    allowed_anchor_ids: vec!["unresolved".into()],
                })
                .collect();
        }
        let input = results(&suite, "all", 1);
        let report = evaluate(&suite, &input).unwrap();
        assert_eq!(report.metrics.accepted, 0);
        assert_eq!(
            report.metrics.expected_withheld,
            report.metrics.criteria_total
        );
        assert!(
            report
                .trials
                .iter()
                .flat_map(|t| &t.report.criteria)
                .all(|c| c.status == "evidence_unresolved")
        );
    }

    #[test]
    fn result_order_does_not_change_recomputed_report() {
        let suite = suite();
        let original = results(&suite, "all", 2);
        let mut reordered = original.clone();
        reordered.trials.reverse();
        assert_eq!(
            evaluate(&suite, &original).unwrap(),
            evaluate(&suite, &reordered).unwrap()
        );
    }
}

//! Frozen acceptance checks over recomputed benchmark results.
//!
//! Passing this gate means only that the declared thresholds were met on the
//! supplied corpus. It neither authenticates API execution nor establishes
//! educational validity, population fairness, or deployment readiness.
use crate::{
    VERSION,
    benchmark::{self, BenchmarkReport, BenchmarkSuite, Metrics, Results},
    contract::hash,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const POLICY_FORMAT: &str = "percent-grading-benchmark-gate-v1";
pub const REPORT_FORMAT: &str = "percent-grading-benchmark-gate-report-v1";
const NOTICE: &str = "This gate checks a frozen policy against recomputed authored benchmark expectations. A pass is not deployment approval, educational validity, population fairness, proof of semantic correctness, or verified provider execution. Manual fixtures test the software only. The api_run label and unique run IDs do not authenticate real API calls; inspect the archived provider records separately. Policy and suite hashes detect changes but do not prove that thresholds were chosen before results were seen. Public holdout tasks are not a concealed independent evaluation. Coverage and accuracy are checked separately for every declared split and observed category, not only the aggregate.";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CategoryRequirement {
    pub name: String,
    /// Distinct cases, not case repetitions or individual criterion awards.
    pub minimum_cases: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GatePolicy {
    pub format: String,
    pub id: String,
    pub version: String,
    /// Includes task definitions, expectations, categories, splits and relations.
    pub suite_hash: String,
    /// development_smoke explicitly permits fixtures; release_candidate does not.
    pub mode: String,
    pub minimum_cases: u32,
    pub minimum_repeats: u32,
    pub minimum_coverage_percent: f64,
    pub minimum_accepted_accuracy_percent: f64,
    pub max_wrong_settled: u64,
    pub max_wrong_agreements: u64,
    pub max_relation_failures: u64,
    pub max_relation_indeterminate: u64,
    pub required_categories: Vec<CategoryRequirement>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GateFailure {
    pub scope: String,
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GateReport {
    pub format: String,
    pub engine_version: String,
    pub passed: bool,
    pub status: String,
    pub policy: GatePolicy,
    pub policy_hash: String,
    pub unique_cases: u32,
    pub cases_by_category: BTreeMap<String, u32>,
    pub violations: Vec<GateFailure>,
    pub benchmark: BenchmarkReport,
    pub interpretation: String,
    pub report_hash: String,
}

fn identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
}

fn serialized<T: Serialize>(value: &T) -> Result<Vec<u8>, String> {
    serde_json::to_vec(value).map_err(|e| e.to_string())
}

impl GatePolicy {
    /// A starting policy for protocol checks, explicitly not a live-model claim.
    /// Inspect and freeze thresholds before collecting the corresponding results.
    pub fn development_smoke(suite: &BenchmarkSuite) -> Result<Self, String> {
        benchmark::validate(suite)?;
        let mut categories = BTreeMap::<String, u32>::new();
        let mut count = 0;
        for task in &suite.tasks {
            for case in &task.cases {
                *categories.entry(case.category.clone()).or_default() += 1;
                count += 1;
            }
        }
        Ok(Self {
            format: POLICY_FORMAT.into(),
            id: "development_smoke".into(),
            version: "1".into(),
            suite_hash: benchmark::suite_hash(suite)?,
            mode: "development_smoke".into(),
            minimum_cases: count,
            minimum_repeats: 1,
            minimum_coverage_percent: 95.0,
            minimum_accepted_accuracy_percent: 100.0,
            max_wrong_settled: 0,
            max_wrong_agreements: 0,
            max_relation_failures: 0,
            max_relation_indeterminate: 0,
            required_categories: categories
                .into_iter()
                .map(|(name, minimum_cases)| CategoryRequirement {
                    name,
                    minimum_cases,
                })
                .collect(),
        })
    }

    /// Adds structural prerequisites for a candidate experiment: both splits,
    /// fresh repeats and API-origin records. This does not attest their origin.
    pub fn release_candidate(suite: &BenchmarkSuite) -> Result<Self, String> {
        let mut policy = Self::development_smoke(suite)?;
        policy.id = "release_candidate".into();
        policy.mode = "release_candidate".into();
        policy.minimum_repeats = 2;
        Ok(policy)
    }

    pub fn validate(&self, suite: &BenchmarkSuite) -> Result<(), String> {
        if self.format != POLICY_FORMAT {
            return Err(format!(
                "Unsupported gate policy format; expected {POLICY_FORMAT}"
            ));
        }
        if !identifier(&self.id)
            || self.version.trim().is_empty()
            || self.version.len() > 128
            || self.version.contains('\0')
        {
            return Err("Gate policy needs a valid identifier and a nonempty version".into());
        }
        if !["development_smoke", "release_candidate"].contains(&self.mode.as_str()) {
            return Err("Gate mode must be development_smoke or release_candidate".into());
        }
        if self.suite_hash != benchmark::suite_hash(suite)? {
            return Err("Gate policy does not bind this exact benchmark suite".into());
        }
        if !(1..=1000).contains(&self.minimum_cases)
            || !(1..=5).contains(&self.minimum_repeats)
            || (self.mode == "release_candidate" && self.minimum_repeats < 2)
        {
            return Err("Gate requires 1..1000 cases and 1..5 repeats; release_candidate needs at least 2 repeats".into());
        }
        for percent in [
            self.minimum_coverage_percent,
            self.minimum_accepted_accuracy_percent,
        ] {
            if !percent.is_finite() || !(0.0..=100.0).contains(&percent) {
                return Err("Gate percentages must be finite values within 0..100".into());
            }
        }
        if self.required_categories.is_empty() || self.required_categories.len() > 1000 {
            return Err("Gate requires 1..1000 explicit category requirements".into());
        }
        let mut seen = BTreeSet::new();
        for category in &self.required_categories {
            if !identifier(&category.name)
                || !(1..=1000).contains(&category.minimum_cases)
                || !seen.insert(&category.name)
            {
                return Err(
                    "Gate category requirements need unique identifiers and 1..1000 cases".into(),
                );
            }
        }
        Ok(())
    }
}

fn failure(failures: &mut Vec<GateFailure>, scope: &str, code: &str, message: String) {
    failures.push(GateFailure {
        scope: scope.into(),
        code: code.into(),
        message,
    });
}

fn minimum_percent(
    failures: &mut Vec<GateFailure>,
    scope: &str,
    code: &str,
    actual: Option<f64>,
    minimum: f64,
) {
    match actual {
        Some(value) if value.is_finite() && value >= minimum => {}
        Some(value) => failure(
            failures,
            scope,
            code,
            format!("Observed {value}% is below the required {minimum}%"),
        ),
        None => failure(
            failures,
            scope,
            code,
            "The metric has no denominator; absence is not a passing result".into(),
        ),
    }
}

fn check_metrics(
    failures: &mut Vec<GateFailure>,
    scope: &str,
    metrics: &Metrics,
    policy: &GatePolicy,
) {
    minimum_percent(
        failures,
        scope,
        "coverage_below_minimum",
        metrics.settled_coverage_percent,
        policy.minimum_coverage_percent,
    );
    minimum_percent(
        failures,
        scope,
        "accuracy_below_minimum",
        metrics.accepted_accuracy_percent,
        policy.minimum_accepted_accuracy_percent,
    );
    for (code, actual, maximum) in [
        (
            "wrong_settled_exceeded",
            metrics.wrong_accepted,
            policy.max_wrong_settled,
        ),
        (
            "wrong_agreements_exceeded",
            metrics.two_judge_wrong_agreements,
            policy.max_wrong_agreements,
        ),
    ] {
        if actual > maximum {
            failure(
                failures,
                scope,
                code,
                format!("Observed {actual} exceeds the allowed maximum {maximum}"),
            );
        }
    }
}

/// Recompute judgments, evidence gates and metrics from the actual inputs before
/// applying the policy. No caller-supplied aggregate report is trusted.
pub fn evaluate(
    suite: &BenchmarkSuite,
    results: &Results,
    policy: &GatePolicy,
) -> Result<GateReport, String> {
    policy.validate(suite)?;
    let benchmark = benchmark::evaluate(suite, results)?;
    let mut violations = Vec::new();
    if results.split != "all" {
        failure(
            &mut violations,
            "experiment",
            "incomplete_split",
            "Gate evaluation requires split=all; selected subsets cannot stand in for the frozen suite".into(),
        );
    }
    if results.repeats < policy.minimum_repeats {
        failure(
            &mut violations,
            "experiment",
            "insufficient_repeats",
            format!(
                "Observed {} repeats; at least {} are required",
                results.repeats, policy.minimum_repeats
            ),
        );
    }
    if policy.mode == "release_candidate" {
        if results.origin != "api_run" {
            failure(
                &mut violations,
                "experiment",
                "api_origin_required",
                "release_candidate requires api_run records; manual fixtures cannot satisfy it"
                    .into(),
            );
        }
        for split in ["development", "holdout"] {
            if !suite.tasks.iter().any(|task| task.split == split) {
                failure(
                    &mut violations,
                    "experiment",
                    "missing_required_split",
                    format!("release_candidate requires distinct tasks in the {split} split"),
                );
            }
        }
        if benchmark.relation_metrics.total == 0 {
            failure(
                &mut violations,
                "relations",
                "missing_relation_checks",
                "release_candidate requires at least one declared relation check".into(),
            );
        }
    }
    let mut seen = BTreeSet::new();
    let mut cases_by_category = BTreeMap::<String, u32>::new();
    for trial in &benchmark.trials {
        if seen.insert((&trial.task_id, &trial.case_id)) {
            *cases_by_category.entry(trial.category.clone()).or_default() += 1;
        }
    }
    let unique_cases = seen.len() as u32;
    if unique_cases < policy.minimum_cases {
        failure(
            &mut violations,
            "experiment",
            "insufficient_cases",
            format!(
                "Observed {unique_cases} distinct cases; at least {} are required. Repeats do not add distinct cases",
                policy.minimum_cases
            ),
        );
    }
    for required in &policy.required_categories {
        let actual = cases_by_category.get(&required.name).copied().unwrap_or(0);
        if actual < required.minimum_cases {
            failure(
                &mut violations,
                &format!("category:{}", required.name),
                "insufficient_category_cases",
                format!(
                    "Observed {actual} distinct cases; at least {} are required",
                    required.minimum_cases
                ),
            );
        }
    }
    check_metrics(&mut violations, "overall", &benchmark.metrics, policy);
    // A weak holdout cannot be concealed by a larger development partition.
    // Do not invent an empty holdout for a development-only smoke corpus.
    let declared_splits: BTreeSet<_> = suite.tasks.iter().map(|task| &task.split).collect();
    for split in declared_splits {
        if let Some(metrics) = benchmark.by_split.get(split) {
            check_metrics(&mut violations, &format!("split:{split}"), metrics, policy);
        }
    }
    // Apply thresholds even to observed categories not named as required. This
    // prevents a good overall average from concealing a weak attack category.
    for (name, metrics) in &benchmark.by_category {
        check_metrics(
            &mut violations,
            &format!("category:{name}"),
            metrics,
            policy,
        );
    }
    for (code, actual, maximum) in [
        (
            "relation_failures_exceeded",
            benchmark.relation_metrics.failed,
            policy.max_relation_failures,
        ),
        (
            "relation_indeterminate_exceeded",
            benchmark.relation_metrics.indeterminate,
            policy.max_relation_indeterminate,
        ),
    ] {
        if actual > maximum {
            failure(
                &mut violations,
                "relations",
                code,
                format!("Observed {actual} exceeds the allowed maximum {maximum}"),
            );
        }
    }
    let passed = violations.is_empty();
    let mut report = GateReport {
        format: REPORT_FORMAT.into(),
        engine_version: VERSION.into(),
        passed,
        status: if passed { "passed" } else { "failed" }.into(),
        policy: policy.clone(),
        policy_hash: hash(&serialized(policy)?),
        unique_cases,
        cases_by_category,
        violations,
        benchmark,
        interpretation: NOTICE.into(),
        report_hash: String::new(),
    };
    report.report_hash = hash(&serialized(&report)?);
    Ok(report)
}

impl GateReport {
    pub fn verify(
        &self,
        suite: &BenchmarkSuite,
        results: &Results,
        policy: &GatePolicy,
    ) -> Result<(), String> {
        if *self != evaluate(suite, results, policy)? {
            return Err("Gate report does not match recomputed inputs and frozen policy".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        benchmark::{RESULTS_FORMAT, Trial},
        contract,
        grading::{Judgment, Row},
    };

    fn suite() -> BenchmarkSuite {
        serde_json::from_str(include_str!("../benchmarks/suite.json")).unwrap()
    }

    fn matching_results(suite: &BenchmarkSuite, repeats: u32) -> Results {
        let mut trials = Vec::new();
        for task in &suite.tasks {
            let lock = contract::compile(&task.policy_source).unwrap();
            for case in &task.cases {
                let judgment = Judgment {
                    contract_hash: lock.contract_hash.clone(),
                    answer_hash: hash(case.answer.as_bytes()),
                    judgments: case
                        .expected
                        .iter()
                        .map(|expectation| Row {
                            criterion_id: expectation.criterion_id.clone(),
                            anchor_id: expectation.allowed_anchor_ids[0].clone(),
                            evidence: vec![case.answer.clone()],
                            reason: "Synthetic matching fixture; no provider call occurred".into(),
                            conditions: Vec::new(),
                        })
                        .collect(),
                };
                for repeat in 1..=repeats {
                    trials.push(Trial {
                        task_id: task.id.clone(),
                        case_id: case.id.clone(),
                        repeat,
                        openai: judgment.clone(),
                        claude: judgment.clone(),
                        run_id: None,
                    });
                }
            }
        }
        Results {
            format: RESULTS_FORMAT.into(),
            suite_hash: benchmark::suite_hash(suite).unwrap(),
            origin: "manual_fixture".into(),
            split: "all".into(),
            repeats,
            trials,
        }
    }

    fn wrong_first_intent(results: &mut Results) {
        let trial = &mut results.trials[0];
        for judgment in [&mut trial.openai, &mut trial.claude] {
            judgment
                .judgments
                .iter_mut()
                .find(|row| row.criterion_id == "intent")
                .unwrap()
                .anchor_id = "intent_zero".into();
        }
    }

    fn has(report: &GateReport, scope: &str, code: &str) -> bool {
        report
            .violations
            .iter()
            .any(|failure| failure.scope == scope && failure.code == code)
    }

    #[test]
    fn explicit_smoke_policy_passes_matching_fixtures_without_claiming_live_validation() {
        let suite = suite();
        let results = matching_results(&suite, 1);
        let policy = GatePolicy::development_smoke(&suite).unwrap();
        let report = evaluate(&suite, &results, &policy).unwrap();
        assert!(report.passed);
        assert_eq!(report.unique_cases, 60);
        assert_eq!(report.status, "passed");
        assert!(
            report
                .interpretation
                .contains("Manual fixtures test the software only")
        );
        assert!(report.interpretation.contains("not deployment approval"));
        report.verify(&suite, &results, &policy).unwrap();
    }

    #[test]
    fn two_wrong_judges_and_tampered_report_cannot_turn_failure_into_pass() {
        let suite = suite();
        let mut results = matching_results(&suite, 1);
        let policy = GatePolicy::development_smoke(&suite).unwrap();
        wrong_first_intent(&mut results);
        let mut report = evaluate(&suite, &results, &policy).unwrap();
        assert!(!report.passed);
        assert!(has(&report, "overall", "wrong_agreements_exceeded"));
        assert!(has(&report, "overall", "wrong_settled_exceeded"));
        assert!(has(&report, "relations", "relation_failures_exceeded"));
        report.passed = true;
        report.violations.clear();
        assert!(report.verify(&suite, &results, &policy).is_err());
    }

    #[test]
    fn every_observed_category_is_checked_even_when_overall_accuracy_passes() {
        let suite = suite();
        let mut results = matching_results(&suite, 1);
        let mut policy = GatePolicy::development_smoke(&suite).unwrap();
        wrong_first_intent(&mut results);
        policy.minimum_accepted_accuracy_percent = 99.0;
        policy.max_wrong_settled = 10;
        policy.max_wrong_agreements = 10;
        policy.max_relation_failures = 10;
        policy
            .required_categories
            .retain(|c| c.name != "minimal_correct");
        let report = evaluate(&suite, &results, &policy).unwrap();
        assert!(report.benchmark.metrics.accepted_accuracy_percent.unwrap() > 99.0);
        assert!(!has(&report, "overall", "accuracy_below_minimum"));
        assert!(has(
            &report,
            "category:minimal_correct",
            "accuracy_below_minimum"
        ));
        assert!(!report.passed);
    }

    #[test]
    fn development_cases_cannot_dilute_a_weak_holdout_with_the_same_category() {
        let mut suite = suite();
        for task in &mut suite.tasks {
            for case in &mut task.cases {
                case.category = "shared_category".into();
            }
        }
        let holdout_id = suite
            .tasks
            .iter()
            .find(|t| t.split == "holdout")
            .unwrap()
            .id
            .clone();
        let mut results = matching_results(&suite, 1);
        let trial = results
            .trials
            .iter_mut()
            .find(|t| t.task_id == holdout_id)
            .unwrap();
        for judgment in [&mut trial.openai, &mut trial.claude] {
            judgment
                .judgments
                .iter_mut()
                .find(|row| row.criterion_id == "intent")
                .unwrap()
                .anchor_id = "intent_zero".into();
        }
        let mut policy = GatePolicy::development_smoke(&suite).unwrap();
        policy.minimum_accepted_accuracy_percent = 99.0;
        policy.max_wrong_settled = 1;
        policy.max_wrong_agreements = 1;
        policy.max_relation_failures = 10;
        let report = evaluate(&suite, &results, &policy).unwrap();
        assert!(report.benchmark.metrics.accepted_accuracy_percent.unwrap() > 99.0);
        assert!(!has(&report, "overall", "accuracy_below_minimum"));
        assert!(!has(
            &report,
            "category:shared_category",
            "accuracy_below_minimum"
        ));
        assert!(has(&report, "split:holdout", "accuracy_below_minimum"));
        assert!(!report.passed);
    }

    #[test]
    fn missing_categories_and_repeated_cases_do_not_meet_sample_requirements() {
        let suite = suite();
        let results = matching_results(&suite, 2);
        let mut policy = GatePolicy::development_smoke(&suite).unwrap();
        policy.minimum_cases = 61;
        policy.required_categories.push(CategoryRequirement {
            name: "absent_attack_category".into(),
            minimum_cases: 1,
        });
        let report = evaluate(&suite, &results, &policy).unwrap();
        assert_eq!(report.unique_cases, 60);
        assert!(has(&report, "experiment", "insufficient_cases"));
        assert!(has(
            &report,
            "category:absent_attack_category",
            "insufficient_category_cases"
        ));
    }

    #[test]
    fn withholding_everything_cannot_pass_even_with_zero_accuracy_threshold() {
        let suite = suite();
        let mut results = matching_results(&suite, 1);
        for trial in &mut results.trials {
            for judgment in [&mut trial.openai, &mut trial.claude] {
                for row in &mut judgment.judgments {
                    row.anchor_id = "unresolved".into();
                }
            }
        }
        let mut policy = GatePolicy::development_smoke(&suite).unwrap();
        policy.minimum_coverage_percent = 0.0;
        policy.minimum_accepted_accuracy_percent = 0.0;
        let report = evaluate(&suite, &results, &policy).unwrap();
        assert!(!report.passed);
        assert_eq!(report.benchmark.metrics.accepted_accuracy_percent, None);
        assert!(has(&report, "overall", "accuracy_below_minimum"));
        assert!(has(&report, "relations", "relation_indeterminate_exceeded"));
    }

    #[test]
    fn complete_selected_subset_cannot_stand_in_for_complete_frozen_suite() {
        let suite = suite();
        let mut results = matching_results(&suite, 1);
        let policy = GatePolicy::development_smoke(&suite).unwrap();
        let development_ids: BTreeSet<_> = suite
            .tasks
            .iter()
            .filter(|t| t.split == "development")
            .map(|t| t.id.as_str())
            .collect();
        results.split = "development".into();
        results
            .trials
            .retain(|t| development_ids.contains(t.task_id.as_str()));
        let report = evaluate(&suite, &results, &policy).unwrap();
        assert!(!report.passed);
        assert!(has(&report, "experiment", "incomplete_split"));
        let mut missing = matching_results(&suite, 1);
        missing.trials.pop();
        assert!(evaluate(&suite, &missing, &policy).is_err());
    }

    #[test]
    fn release_mode_requires_api_records_repeats_and_both_splits() {
        let suite = suite();
        let results = matching_results(&suite, 1);
        let policy = GatePolicy::release_candidate(&suite).unwrap();
        let report = evaluate(&suite, &results, &policy).unwrap();
        assert!(has(&report, "experiment", "api_origin_required"));
        assert!(has(&report, "experiment", "insufficient_repeats"));
        let mut development = suite.clone();
        development.tasks.retain(|t| t.split == "development");
        let policy = GatePolicy::release_candidate(&development).unwrap();
        let results = matching_results(&development, 2);
        let report = evaluate(&development, &results, &policy).unwrap();
        assert!(has(&report, "experiment", "missing_required_split"));
    }

    #[test]
    fn api_origin_label_can_pass_structure_but_is_not_attested_execution() {
        let suite = suite();
        let mut results = matching_results(&suite, 2);
        results.origin = "api_run".into();
        for (index, trial) in results.trials.iter_mut().enumerate() {
            trial.run_id = Some(hash(format!("synthetic record {index}").as_bytes()));
        }
        let policy = GatePolicy::release_candidate(&suite).unwrap();
        let report = evaluate(&suite, &results, &policy).unwrap();
        assert!(report.passed);
        assert!(
            report
                .interpretation
                .contains("do not authenticate real API calls")
        );
        results.trials[1].run_id = results.trials[0].run_id.clone();
        assert!(evaluate(&suite, &results, &policy).is_err());
    }

    #[test]
    fn malformed_policies_fail_closed_and_bind_the_authored_expectations() {
        let suite = suite();
        let policy = GatePolicy::development_smoke(&suite).unwrap();
        let mut bad = policy.clone();
        bad.minimum_coverage_percent = f64::NAN;
        assert!(bad.validate(&suite).is_err());
        bad = policy.clone();
        bad.required_categories
            .push(bad.required_categories[0].clone());
        assert!(bad.validate(&suite).is_err());
        bad = policy.clone();
        bad.mode = "release_candidate".into();
        assert!(bad.validate(&suite).is_err());
        bad = policy.clone();
        bad.required_categories.clear();
        assert!(bad.validate(&suite).is_err());
        let mut changed = suite.clone();
        changed.tasks[0].cases[0].expected[0].allowed_anchor_ids = vec!["intent_zero".into()];
        assert!(policy.validate(&changed).is_err());
        let mut value = serde_json::to_value(policy).unwrap();
        value["override_pass"] = serde_json::json!(true);
        assert!(serde_json::from_value::<GatePolicy>(value).is_err());
    }
}

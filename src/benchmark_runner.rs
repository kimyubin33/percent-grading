//! Bounded experiments with independent trials and immutable provider records.
//!
//! Authored expectations stay in the experiment manifest, never model prompts.
//! Attempt reservations are conservative upper bounds, not monetary estimates.
use crate::{
    VERSION,
    benchmark::{self, BenchmarkSuite, Results, Trial},
    contract::{self, LockedContract, hash},
    providers::{self, Limits, Provider, Response},
    runner::{self, RunConfig, read_json, write_new},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

pub const PLAN_FORMAT: &str = "percent-grading-benchmark-plan-v1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Options {
    pub split: String,
    pub repeats: u32,
    pub max_api_calls: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PlannedTrial {
    pub task_id: String,
    pub case_id: String,
    pub repeat: u32,
    pub trial_id: String,
    pub run_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Plan {
    pub format: String,
    pub engine_version: String,
    pub suite_hash: String,
    pub config: RunConfig,
    pub options: Options,
    pub trial_count: usize,
    pub max_initial_api_attempts: u32,
    pub plan_id: String,
    pub trials: Vec<PlannedTrial>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    plan: Plan,
    suite: BenchmarkSuite,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Reservation {
    plan_id: String,
    sequence: u32,
    trial_id: String,
    provider: Provider,
    reserved_attempts: u32,
}

struct Lease(PathBuf);
impl Drop for Lease {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

fn same<T: Serialize>(a: &T, b: &T) -> Result<bool, String> {
    Ok(serde_json::to_value(a).map_err(|e| e.to_string())?
        == serde_json::to_value(b).map_err(|e| e.to_string())?)
}

fn digest<T: Serialize>(value: &T) -> Result<String, String> {
    Ok(hash(&serde_json::to_vec(value).map_err(|e| e.to_string())?))
}

/// Review the full experiment and upper request bound before any credentials or writes.
pub fn prepare_plan(
    suite: &BenchmarkSuite,
    config: &RunConfig,
    options: &Options,
) -> Result<Plan, String> {
    let suite_hash = benchmark::suite_hash(suite)?;
    if !["all", "development", "holdout"].contains(&options.split.as_str())
        || !(1..=5).contains(&options.repeats)
        || !(1..=100_000).contains(&options.max_api_calls)
    {
        return Err("Benchmark split must be all/development/holdout, repeats 1..5 and max_api_calls 1..100000".into());
    }
    let selected: Vec<_> = suite
        .tasks
        .iter()
        .filter(|task| options.split == "all" || task.split == options.split)
        .collect();
    let count =
        selected.iter().map(|task| task.cases.len()).sum::<usize>() * options.repeats as usize;
    if count == 0 {
        return Err("The selected benchmark split contains no cases".into());
    }
    let maximum = u32::try_from(count)
        .ok()
        .and_then(|n| n.checked_mul(2))
        .and_then(|n| n.checked_mul(u32::from(config.limits.max_attempts)))
        .ok_or("Benchmark request bound overflow")?;
    if maximum > options.max_api_calls {
        return Err(format!(
            "Benchmark needs at most {maximum} initial API attempts; max_api_calls is {}. Reduce cases/repeats/attempts or explicitly choose a sufficient budget.",
            options.max_api_calls
        ));
    }
    let plan_id = digest(&json!({
        "format":PLAN_FORMAT,"engine_version":VERSION,"suite_hash":suite_hash,
        "config":config,"options":options
    }))?;
    let mut trials = Vec::with_capacity(count);
    for task in selected {
        let lock = contract::compile(&task.policy_source)?;
        for case in &task.cases {
            for repeat in 1..=options.repeats {
                let trial_id = digest(&json!({
                    "plan_id":plan_id,"task_id":task.id,"case_id":case.id,"repeat":repeat
                }))?;
                let run_id = runner::trial_identity(
                    &lock,
                    &case.answer,
                    config,
                    task.evidence.as_ref(),
                    &trial_id,
                )?
                .run_id()?;
                trials.push(PlannedTrial {
                    task_id: task.id.clone(),
                    case_id: case.id.clone(),
                    repeat,
                    trial_id,
                    run_id,
                });
            }
        }
    }
    Ok(Plan {
        format: PLAN_FORMAT.into(),
        engine_version: VERSION.into(),
        suite_hash,
        config: config.clone(),
        options: options.clone(),
        trial_count: count,
        max_initial_api_attempts: maximum,
        plan_id,
        trials,
    })
}

fn reservation_state(dir: &Path, plan: &Plan) -> Result<(u32, u32), String> {
    let path = dir.join("reservations");
    let mut entries: Vec<_> = if path.exists() {
        fs::read_dir(&path)
            .map_err(|e| e.to_string())?
            .collect::<Result<_, _>>()
            .map_err(|e| e.to_string())?
    } else {
        Vec::new()
    };
    if entries.len() > plan.options.max_api_calls as usize {
        return Err("Benchmark reservation count exceeds its request budget".into());
    }
    entries.sort_by_key(|entry| entry.file_name());
    let mut reserved = 0u32;
    let mut requested = BTreeSet::new();
    for (index, entry) in entries.iter().enumerate() {
        let sequence = index as u32 + 1;
        if entry.file_name().to_str() != Some(&format!("{sequence:06}.json")) {
            return Err(
                "Benchmark reservation ledger must be contiguous and contain only numbered records"
                    .into(),
            );
        }
        let record: Reservation = read_json(&entry.path())?;
        if record.sequence != sequence
            || record.plan_id != plan.plan_id
            || record.reserved_attempts != u32::from(plan.config.limits.max_attempts)
            || !plan
                .trials
                .iter()
                .any(|trial| trial.trial_id == record.trial_id)
        {
            return Err("Benchmark reservation does not match its experiment".into());
        }
        reserved = reserved
            .checked_add(record.reserved_attempts)
            .ok_or("Benchmark reservation sum overflow")?;
        requested.insert((record.trial_id, record.provider.as_str()));
    }
    if reserved > plan.options.max_api_calls {
        return Err("Benchmark reservations exceed max_api_calls".into());
    }
    for trial in &plan.trials {
        for provider in [Provider::Openai, Provider::Claude] {
            if dir
                .join("trials")
                .join(&trial.run_id)
                .join(format!("{provider}.response.json"))
                .exists()
                && !requested.contains(&(trial.trial_id.clone(), provider.as_str()))
            {
                return Err(
                    "Archived benchmark response has no matching API attempt reservation".into(),
                );
            }
        }
    }
    Ok((entries.len() as u32, reserved))
}

fn validate_manifest(dir: &Path, expected: &Manifest) -> Result<(), String> {
    if dir.join("manifest.json").exists() {
        let saved: Manifest = read_json(&dir.join("manifest.json"))?;
        if !same(&saved, expected)? {
            return Err("Stored benchmark manifest differs from the planned experiment".into());
        }
    } else if dir.exists()
        && fs::read_dir(dir)
            .map_err(|e| e.to_string())?
            .next()
            .is_some()
    {
        return Err("Benchmark artifacts exist without a frozen manifest".into());
    }
    Ok(())
}

fn collect_results(dir: &Path, plan: &Plan) -> Result<Results, String> {
    let mut trials = Vec::with_capacity(plan.trials.len());
    for trial in &plan.trials {
        let (openai, claude) = runner::read_judgments(&dir.join("trials").join(&trial.run_id))?;
        trials.push(Trial {
            task_id: trial.task_id.clone(),
            case_id: trial.case_id.clone(),
            repeat: trial.repeat,
            openai,
            claude,
            run_id: Some(trial.run_id.clone()),
        });
    }
    Ok(Results {
        format: benchmark::RESULTS_FORMAT.into(),
        suite_hash: plan.suite_hash.clone(),
        origin: "api_run".into(),
        split: plan.options.split.clone(),
        repeats: plan.options.repeats,
        trials,
    })
}

fn output(dir: &Path, plan: &Plan, report: Value, reserved: u32, cached: bool) -> Value {
    json!({
        "origin":"benchmark_api_run","plan_id":plan.plan_id,"run_directory":dir,
        "cached":cached,"reserved_api_attempts":reserved,
        "results_file":dir.join("results.json"),"report_file":dir.join("report.json"),
        "report":report,
        "interpretation":"Expectations are authored references, not measured scientific truth. Reserved attempts are conservative request limits, not token usage or a monetary cap."
    })
}

fn completed(dir: &Path, manifest: &Manifest) -> Result<Value, String> {
    validate_manifest(dir, manifest)?;
    let (_, reserved) = reservation_state(dir, &manifest.plan)?;
    let results = collect_results(dir, &manifest.plan)?;
    let saved: Results = read_json(&dir.join("results.json"))?;
    if !same(&saved, &results)? {
        return Err("Saved benchmark results differ from verified provider trial records".into());
    }
    let report = serde_json::to_value(benchmark::evaluate(&manifest.suite, &results)?)
        .map_err(|e| e.to_string())?;
    let saved_report: Value = read_json(&dir.join("report.json"))?;
    if report != saved_report {
        return Err("Saved benchmark report differs from deterministic evaluation".into());
    }
    Ok(output(dir, &manifest.plan, report, reserved, true))
}

/// Calls exactly the existing OpenAI and Claude adapters, with persisted budgets.
pub fn run(
    suite: &BenchmarkSuite,
    config: &RunConfig,
    options: &Options,
    runs: &Path,
) -> Result<Value, String> {
    run_with(suite, config, options, runs, true, providers::judge)
}

fn run_with<F>(
    suite: &BenchmarkSuite,
    config: &RunConfig,
    options: &Options,
    runs: &Path,
    check_credentials: bool,
    mut judge: F,
) -> Result<Value, String>
where
    F: FnMut(Provider, &str, &str, &str, &Value, &Limits) -> Result<Response, String>,
{
    let plan = prepare_plan(suite, config, options)?;
    let manifest = Manifest {
        plan,
        suite: suite.clone(),
    };
    let plan = &manifest.plan;
    let dir = runs.join(&plan.plan_id);
    validate_manifest(&dir, &manifest)?;
    if dir.join("report.json").exists() {
        return completed(&dir, &manifest);
    }
    if dir.join("results.json").exists() {
        let saved: Results = read_json(&dir.join("results.json"))?;
        let reconstructed = collect_results(&dir, plan)?;
        if !same(&saved, &reconstructed)? {
            return Err(
                "Existing benchmark results differ from verified trials; no new API calls made"
                    .into(),
            );
        }
    }
    let locks: BTreeMap<String, LockedContract> = suite
        .tasks
        .iter()
        .map(|task| contract::compile(&task.policy_source).map(|lock| (task.id.clone(), lock)))
        .collect::<Result<_, _>>()?;
    let mut pending = Vec::new();
    for trial in &plan.trials {
        let task = suite
            .tasks
            .iter()
            .find(|task| task.id == trial.task_id)
            .unwrap();
        let case = task
            .cases
            .iter()
            .find(|case| case.id == trial.case_id)
            .unwrap();
        pending.extend(runner::trial_pending_providers(
            &locks[&task.id],
            &case.answer,
            config,
            &dir.join("trials"),
            task.evidence.as_ref(),
            &trial.trial_id,
        )?);
    }
    let (_, reserved_before) = reservation_state(&dir, plan)?;
    let remaining_bound = pending.len() as u32 * u32::from(config.limits.max_attempts);
    if reserved_before.saturating_add(remaining_bound) > options.max_api_calls {
        return Err(format!(
            "Benchmark has reserved {reserved_before} API attempts and needs up to {remaining_bound} more; the persisted budget is {}. No new calls made.",
            options.max_api_calls
        ));
    }
    if check_credentials {
        for provider in [Provider::Openai, Provider::Claude] {
            if pending.contains(&provider) {
                providers::validate_credentials(provider)?;
            }
        }
    }
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let lease_path = dir.join("running.lock");
    let mut lease = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&lease_path)
        .map_err(
            |_| "Benchmark is locked; inspect for another process before removing a stale lock",
        )?;
    let _lease = Lease(lease_path);
    writeln!(lease, "process {}", std::process::id()).map_err(|e| e.to_string())?;
    drop(lease);
    if dir.join("report.json").exists() {
        return completed(&dir, &manifest);
    }
    if dir.join("manifest.json").exists() {
        validate_manifest(&dir, &manifest)?;
    } else {
        write_new(&dir.join("manifest.json"), &manifest)?;
    }
    // Re-read under the experiment lease so concurrent starts cannot share a budget.
    let (mut sequence, mut reserved) = reservation_state(&dir, plan)?;
    for trial in &plan.trials {
        let task = suite
            .tasks
            .iter()
            .find(|task| task.id == trial.task_id)
            .unwrap();
        let case = task
            .cases
            .iter()
            .find(|case| case.id == trial.case_id)
            .unwrap();
        runner::run_trial_with(
            &locks[&task.id],
            &case.answer,
            config,
            &dir.join("trials"),
            task.evidence.as_ref(),
            &trial.trial_id,
            |provider, model, system, user, schema, limits| {
                let next = reserved
                    .checked_add(u32::from(limits.max_attempts))
                    .ok_or("API attempt reservation overflow")?;
                if next > options.max_api_calls {
                    return Err(
                        "Benchmark request budget is exhausted; no API call was made".into(),
                    );
                }
                sequence += 1;
                let reservation = Reservation {
                    plan_id: plan.plan_id.clone(),
                    sequence,
                    trial_id: trial.trial_id.clone(),
                    provider,
                    reserved_attempts: u32::from(limits.max_attempts),
                };
                write_new(
                    &dir.join("reservations").join(format!("{sequence:06}.json")),
                    &reservation,
                )?;
                reserved = next;
                judge(provider, model, system, user, schema, limits)
            },
        )
        .map_err(|error| format!("{error}; experiment preserved at {}", dir.display()))?;
    }
    let results = collect_results(&dir, plan)?;
    let report =
        serde_json::to_value(benchmark::evaluate(suite, &results)?).map_err(|e| e.to_string())?;
    if dir.join("results.json").exists() {
        let saved: Results = read_json(&dir.join("results.json"))?;
        if !same(&saved, &results)? {
            return Err("Existing benchmark results differ from completed trials".into());
        }
    } else {
        write_new(&dir.join("results.json"), &results)?;
    }
    write_new(&dir.join("report.json"), &report)?;
    Ok(output(&dir, plan, report, reserved, false))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        benchmark::{BenchmarkCase, BenchmarkTask, Expectation},
        grading::{Judgment, Row},
    };

    struct Temp(PathBuf);
    impl Temp {
        fn new() -> Self {
            Self(std::env::temp_dir().join(format!(
                    "percent-grading-benchmark-{}-{}",
                    std::process::id(),
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_nanos()
                )))
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            if let (Ok(path), Ok(root)) = (
                fs::canonicalize(&self.0),
                fs::canonicalize(std::env::temp_dir()),
            ) && path.parent() == Some(root.as_path())
                && fs::symlink_metadata(&self.0).is_ok_and(|m| !m.file_type().is_symlink())
                && path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .is_some_and(|s| s.starts_with("percent-grading-benchmark-"))
            {
                let _ = fs::remove_dir_all(path);
            }
        }
    }

    fn setup() -> (BenchmarkSuite, RunConfig, Options, Temp) {
        let source = include_str!("../policies/apple.aha");
        let lock = contract::compile(source).unwrap();
        let expected = lock
            .contract
            .criteria
            .iter()
            .map(|c| Expectation {
                criterion_id: c.id.clone(),
                allowed_anchor_ids: vec![
                    c.anchors
                        .iter()
                        .find(|a| a.points == 100)
                        .unwrap()
                        .id
                        .clone(),
                ],
            })
            .collect();
        let suite = BenchmarkSuite {
            format: benchmark::SUITE_FORMAT.into(),
            id: "runner-fixture".into(),
            version: "1".into(),
            tasks: vec![BenchmarkTask {
                id: "apple".into(),
                split: "development".into(),
                policy_source: source.into(),
                evidence: None,
                cases: vec![BenchmarkCase {
                    id: "red".into(),
                    category: "synthetic".into(),
                    answer: "A red apple.".into(),
                    expected,
                }],
                relations: vec![],
            }],
        };
        let config = RunConfig {
            openai_model: "fixture-openai".into(),
            claude_model: "fixture-claude".into(),
            limits: Limits::default(),
        };
        let options = Options {
            split: "development".into(),
            repeats: 2,
            max_api_calls: 6,
        };
        (suite, config, options, Temp::new())
    }

    fn response(provider: Provider, user: &str) -> Response {
        let data: Value = serde_json::from_str(user).unwrap();
        assert!(data.get("expected").is_none());
        assert!(data.get("relations").is_none());
        assert!(data.get("experiment_trial").is_none());
        let contract: crate::contract::Contract =
            serde_json::from_value(data["contract"].clone()).unwrap();
        let judgment = Judgment {
            contract_hash: data["contract_hash"].as_str().unwrap().into(),
            answer_hash: data["answer_hash"].as_str().unwrap().into(),
            judgments: contract
                .criteria
                .iter()
                .map(|c| Row {
                    criterion_id: c.id.clone(),
                    anchor_id: c
                        .anchors
                        .iter()
                        .find(|a| a.points == 100)
                        .unwrap()
                        .id
                        .clone(),
                    evidence: vec![data["answer"].as_str().unwrap().into()],
                    reason: "Synthetic provider fixture only.".into(),
                    conditions: vec![],
                })
                .collect(),
        };
        let text = serde_json::to_string(&judgment).unwrap();
        let raw = match provider {
            Provider::Openai => {
                json!({"status":"completed","output":[{"type":"message","role":"assistant","status":"completed","content":[{"type":"output_text","text":text}]}],"usage":{}})
            }
            Provider::Claude => {
                json!({"type":"message","role":"assistant","stop_reason":"end_turn","content":[{"type":"text","text":text}],"usage":{}})
            }
        };
        Response {
            text,
            raw,
            usage: json!({}),
            attempts: 1,
        }
    }

    #[test]
    fn plan_binds_suite_models_splits_and_fresh_repeats_before_network() {
        let (suite, config, options, temp) = setup();
        let plan = prepare_plan(&suite, &config, &options).unwrap();
        assert_eq!(plan.trial_count, 2);
        assert_eq!(plan.max_initial_api_attempts, 4);
        assert_ne!(plan.trials[0].run_id, plan.trials[1].run_id);
        assert!(!temp.0.exists());
        let mut options2 = options.clone();
        options2.repeats = 1;
        assert_ne!(
            plan.plan_id,
            prepare_plan(&suite, &config, &options2).unwrap().plan_id
        );
        options2.split = "holdout".into();
        assert!(prepare_plan(&suite, &config, &options2).is_err());
        let mut config2 = config.clone();
        config2.claude_model = "other-model".into();
        assert_ne!(
            plan.plan_id,
            prepare_plan(&suite, &config2, &options).unwrap().plan_id
        );
    }

    #[test]
    fn repeats_call_fresh_providers_but_completed_experiment_replays_without_calls() {
        let (suite, config, options, temp) = setup();
        let mut prompts = Vec::new();
        let result = run_with(
            &suite,
            &config,
            &options,
            &temp.0,
            false,
            |p, _, s, u, _, _| {
                prompts.push((s.to_owned(), u.to_owned()));
                Ok(response(p, u))
            },
        )
        .unwrap();
        assert_eq!(prompts.len(), 4);
        assert!(prompts.iter().all(|p| p == &prompts[0]));
        assert_eq!(result["reserved_api_attempts"], 4);
        let again = run_with(
            &suite,
            &config,
            &options,
            &temp.0,
            true,
            |_, _, _, _, _, _| panic!("cache must not call"),
        )
        .unwrap();
        assert_eq!(again["cached"], true);
        assert_eq!(again["report"], result["report"]);
        let saved: Results =
            read_json(Path::new(result["results_file"].as_str().unwrap())).unwrap();
        assert_ne!(saved.trials[0].run_id, saved.trials[1].run_id);
    }

    #[test]
    fn partial_experiment_preserves_first_provider_and_reserves_failed_requests() {
        let (suite, config, options, temp) = setup();
        let mut calls = 0;
        assert!(
            run_with(
                &suite,
                &config,
                &options,
                &temp.0,
                false,
                |p, _, _, u, _, _| {
                    calls += 1;
                    if calls == 2 {
                        Err("simulated transport failure".into())
                    } else {
                        Ok(response(p, u))
                    }
                }
            )
            .is_err()
        );
        let mut resumed = Vec::new();
        let result = run_with(
            &suite,
            &config,
            &options,
            &temp.0,
            false,
            |p, _, _, u, _, _| {
                resumed.push(p);
                Ok(response(p, u))
            },
        )
        .unwrap();
        assert_eq!(
            resumed,
            vec![Provider::Claude, Provider::Openai, Provider::Claude]
        );
        assert_eq!(result["reserved_api_attempts"], 5);
    }

    #[test]
    fn declared_budget_blocks_initial_and_resumed_work_without_transport() {
        let (suite, config, mut options, temp) = setup();
        options.max_api_calls = 3;
        assert!(
            run_with(
                &suite,
                &config,
                &options,
                &temp.0,
                false,
                |_, _, _, _, _, _| panic!("budget preflight")
            )
            .unwrap_err()
            .contains("initial API attempts")
        );
        assert!(!temp.0.exists());
        options.max_api_calls = 4;
        assert!(
            run_with(
                &suite,
                &config,
                &options,
                &temp.0,
                false,
                |_, _, _, _, _, _| Err("transport failed".into())
            )
            .is_err()
        );
        assert!(
            run_with(
                &suite,
                &config,
                &options,
                &temp.0,
                false,
                |_, _, _, _, _, _| panic!("persisted budget exhausted")
            )
            .unwrap_err()
            .contains("persisted budget")
        );
    }

    #[test]
    fn modified_results_and_reservation_ledger_are_rejected() {
        let (suite, config, options, temp) = setup();
        let result = run_with(
            &suite,
            &config,
            &options,
            &temp.0,
            false,
            |p, _, _, u, _, _| Ok(response(p, u)),
        )
        .unwrap();
        let results_path = Path::new(result["results_file"].as_str().unwrap());
        let original = fs::read(results_path).unwrap();
        let mut saved: Results = read_json(results_path).unwrap();
        saved.trials.pop();
        fs::write(results_path, serde_json::to_vec(&saved).unwrap()).unwrap();
        assert!(
            run_with(
                &suite,
                &config,
                &options,
                &temp.0,
                false,
                |_, _, _, _, _, _| panic!("no call on tamper")
            )
            .is_err()
        );
        fs::write(results_path, original).unwrap();
        let reservation =
            Path::new(result["run_directory"].as_str().unwrap()).join("reservations/000001.json");
        let mut record: Reservation = read_json(&reservation).unwrap();
        record.reserved_attempts = 0;
        fs::write(reservation, serde_json::to_vec(&record).unwrap()).unwrap();
        assert!(
            run_with(
                &suite,
                &config,
                &options,
                &temp.0,
                false,
                |_, _, _, _, _, _| panic!("no call on ledger tamper")
            )
            .is_err()
        );
        let ledger = Path::new(result["run_directory"].as_str().unwrap()).join("reservations");
        for sequence in 1..=4 {
            fs::remove_file(ledger.join(format!("{sequence:06}.json"))).unwrap();
        }
        assert!(
            run_with(
                &suite,
                &config,
                &options,
                &temp.0,
                false,
                |_, _, _, _, _, _| panic!("missing ledger cannot reset the budget")
            )
            .unwrap_err()
            .contains("no matching API attempt reservation")
        );
    }
}

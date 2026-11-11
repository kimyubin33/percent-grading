//! These fixtures are manually authored software examples, not live AI results.
use percent_grading::contract::{LockedContract, compile, hash};
use percent_grading::grading::{Judgment, Report, Row, aggregate};
use serde::Deserialize;

const POLICY: &str = include_str!("../policies/apple.aha");

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ManualFixture {
    answer: String,
    openai: Vec<Row>,
    claude: Vec<Row>,
}

fn evaluate_fixture(text: &str) -> Report {
    let lock = compile(POLICY).unwrap();
    let fixture: ManualFixture = serde_json::from_str(text).unwrap();
    let answer_hash = hash(fixture.answer.as_bytes());
    let openai = Judgment {
        contract_hash: lock.contract_hash.clone(),
        answer_hash: answer_hash.clone(),
        judgments: fixture.openai,
    };
    let claude = Judgment {
        contract_hash: lock.contract_hash.clone(),
        answer_hash: answer_hash.clone(),
        judgments: fixture.claude,
    };
    aggregate(
        &lock.contract,
        &lock.contract_hash,
        &fixture.answer,
        &answer_hash,
        &openai,
        &claude,
    )
    .unwrap()
}

#[test]
fn checked_apple_contract_has_independent_current_and_future_weights() {
    let lock = compile(POLICY).unwrap();
    lock.verify().unwrap();
    assert_eq!(lock.contract.id, "apple_v1");
    assert_eq!(lock.contract.criteria.len(), 5);
    assert_eq!(
        lock.contract
            .criteria
            .iter()
            .filter(|c| c.dimension != "future")
            .map(|c| c.weight)
            .sum::<u32>(),
        100
    );
    assert_eq!(
        lock.contract
            .criteria
            .iter()
            .filter(|c| c.dimension == "future")
            .map(|c| c.weight)
            .sum::<u32>(),
        100
    );
    assert_eq!(lock.contract_hash.len(), 64);
    assert_eq!(lock, compile(POLICY).unwrap());
}

#[test]
fn weights_cannot_move_between_current_and_future_profiles() {
    let wrong_current = POLICY.replace(
        "weight 25, description State",
        "weight 24, description State",
    );
    assert!(compile(&wrong_current).is_err());
    let wrong_future = POLICY.replace(
        "dimension future, weight 100",
        "dimension future, weight 99",
    );
    assert!(compile(&wrong_future).is_err());
    assert!(compile(&POLICY.replace("weight 30,", "weight -30,")).is_err());
}

#[test]
fn semantic_profile_rejects_unknown_record_fields() {
    let expanded = POLICY
        .lines()
        .map(|line| {
            if line.starts_with("낱말 Criterion:") || line.trim_start().starts_with("Criterion ")
            {
                format!(
                    "{line}, invented{}",
                    if line.starts_with("낱말") {
                        "*"
                    } else {
                        " yes"
                    }
                )
            } else {
                line.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    let error = compile(&expanded).unwrap_err();
    assert!(error.contains("exactly these fields"), "{error}");
}

#[test]
fn orphan_anchor_and_missing_perspective_fail_closed() {
    let orphan = POLICY.replace(
        "Anchor intent_wrong: criterion intent",
        "Anchor intent_wrong: criterion missing",
    );
    assert!(compile(&orphan).unwrap_err().contains("unknown criterion"));
    let uncovered = POLICY
        .lines()
        .filter(|line| !line.contains("Perspective conditional_design:"))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(compile(&uncovered).unwrap_err().contains("Every criterion"));
}

#[test]
fn locked_json_cannot_override_its_checked_language_source() {
    let lock = compile(POLICY).unwrap();
    let mut changed = lock.clone();
    changed.contract.criteria[0].weight += 1;
    assert!(changed.verify().is_err());
    let mut changed = lock.clone();
    changed.contract_hash = "0".repeat(64);
    assert!(changed.verify().is_err());
    let mut changed = lock;
    changed.source = changed.source.replace("version 1,", "version 2,");
    assert!(changed.verify().is_err());
}

#[test]
fn lock_deserialization_rejects_unknown_fields() {
    let lock = compile(POLICY).unwrap();
    let mut value = serde_json::to_value(lock).unwrap();
    value["override_score"] = serde_json::json!(100);
    assert!(serde_json::from_value::<LockedContract>(value).is_err());
}

#[test]
fn red_fixture_meets_present_rubric_without_a_future_proposal() {
    let report = evaluate_fixture(include_str!("../examples/rust/apple-red.fixture.json"));
    assert_eq!(report.status, "ready");
    assert_eq!(report.current.percent, Some(100.0));
    assert_eq!(report.future.unwrap().percent, Some(0.0));
}

#[test]
fn blue_fixture_preserves_valid_physics_but_blocks_wrong_present_goal() {
    let report = evaluate_fixture(include_str!("../examples/rust/apple-blue.fixture.json"));
    assert_eq!(report.status, "blocked");
    assert_eq!(report.current.percent, Some(46.25));
    assert_eq!(report.critical_failures, vec!["intent"]);
    assert_eq!(
        report
            .criteria
            .iter()
            .find(|c| c.criterion_id == "principle")
            .unwrap()
            .awarded_points,
        Some(100)
    );
    assert_eq!(
        report
            .criteria
            .iter()
            .find(|c| c.criterion_id == "intent")
            .unwrap()
            .awarded_points,
        Some(0)
    );
}

#[test]
fn future_hypothesis_does_not_inflate_present_correctness() {
    let red = evaluate_fixture(include_str!("../examples/rust/apple-red.fixture.json"));
    let future = evaluate_fixture(include_str!("../examples/rust/apple-future.fixture.json"));
    assert_eq!(future.current, red.current);
    assert_eq!(future.future.unwrap().percent, Some(100.0));
    assert_eq!(future.status, "ready");
}

#[test]
fn disagreement_is_a_visible_range_not_a_compromise_score() {
    let report = evaluate_fixture(include_str!(
        "../examples/rust/apple-disagreement.fixture.json"
    ));
    assert_eq!(report.status, "review");
    assert_eq!(report.current.percent, None);
    assert_eq!(report.current.coverage_percent, 70.0);
    assert_eq!(report.current.lower_percent, 85.0);
    assert_eq!(report.current.upper_percent, 100.0);
    assert_eq!(report.disagreements, vec!["principle"]);
}

#[test]
fn standalone_answer_files_match_fixture_answers() {
    for (fixture, answer) in [
        (
            include_str!("../examples/rust/apple-red.fixture.json"),
            include_str!("../examples/rust/apple-red.txt"),
        ),
        (
            include_str!("../examples/rust/apple-blue.fixture.json"),
            include_str!("../examples/rust/apple-blue.txt"),
        ),
        (
            include_str!("../examples/rust/apple-future.fixture.json"),
            include_str!("../examples/rust/apple-future.txt"),
        ),
        (
            include_str!("../examples/rust/apple-disagreement.fixture.json"),
            include_str!("../examples/rust/apple-disagreement.txt"),
        ),
    ] {
        let fixture: ManualFixture = serde_json::from_str(fixture).unwrap();
        assert_eq!(fixture.answer, answer);
    }
}

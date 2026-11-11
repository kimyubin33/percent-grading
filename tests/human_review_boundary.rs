//! Independent checks across the public examiner-review API and report renderer.
//! All judgments and examiner statements are synthetic; no provider is called.
use percent_grading::{
    contract::{self, LockedContract},
    grading::{self, Judgment, Report},
    human::{self, ExaminerReview, ReviewMode},
    service::{self, Response, ServiceContext},
};
use serde_json::{Value, json};

fn fixture(name: &str, suffix: &str) -> (LockedContract, String, Report) {
    let source = match name {
        "red" => include_str!("../examples/rust/apple-red.fixture.json"),
        "disagreement" => include_str!("../examples/rust/apple-disagreement.fixture.json"),
        "blue" => include_str!("../examples/rust/apple-blue.fixture.json"),
        _ => panic!("Unknown audit fixture"),
    };
    let fixture: Value = serde_json::from_str(source).unwrap();
    let lock = contract::compile(include_str!("../policies/apple.aha")).unwrap();
    let answer = format!("{}{}", fixture["answer"].as_str().unwrap(), suffix);
    let answer_hash = contract::hash(answer.as_bytes());
    let judgment = |name: &str| Judgment {
        contract_hash: lock.contract_hash.clone(),
        answer_hash: answer_hash.clone(),
        judgments: serde_json::from_value(fixture[name].clone()).unwrap(),
    };
    let report = grading::aggregate(
        &lock.contract,
        &lock.contract_hash,
        &answer,
        &answer_hash,
        &judgment("openai"),
        &judgment("claude"),
    )
    .unwrap();
    (lock, answer, report)
}

fn invoke(payload: Value) -> Response {
    service::handle_json(
        &ServiceContext::default(),
        &serde_json::to_vec(&json!({
            "api_version":service::API_VERSION,
            "request_id":"independent-human-review-audit",
            "payload":payload
        }))
        .unwrap(),
    )
}

fn authored_review(
    lock: &LockedContract,
    answer: &str,
    report: &Report,
    score: f64,
    mode: ReviewMode,
) -> ExaminerReview {
    let mut review = human::template(lock, answer, report).unwrap();
    review.examiner_id = "synthetic_examiner".into();
    review.human_percent = score;
    review.mode = mode;
    review.grading_basis =
        "The examiner's documented assessment of the task and its conditions.".into();
    review.rationale =
        "Synthetic independent reviewer fixture; not an observed classroom judgment.".into();
    review.reviewed_ai_report = true;
    review.acknowledged_ai_limits = true;
    review
}

#[test]
fn the_public_api_never_treats_an_incomplete_human_form_as_an_official_grade() {
    let (lock, answer, report) = fixture("red", "");
    assert_eq!(report.status, "ready");
    assert_eq!(report.current.percent, Some(99.9));
    assert!(report.decision.as_ref().unwrap().final_percent.is_none());
    let result =
        invoke(json!({"operation":"review_template","lock":lock,"answer":answer,"report":report}));
    assert!(result.error.is_none());
    let template = result.result.unwrap()["review"].clone();
    assert_eq!(template["human_percent"], -1.0);
    assert_eq!(template["reviewed_ai_report"], false);
    let rejected = invoke(
        json!({"operation":"examiner_review","lock":lock,"answer":answer,"report":report,"review":template}),
    );
    assert_eq!(rejected.http_status(), 422);
    assert!(rejected.result.is_none());

    let mut missing_review =
        json!({"operation":"examiner_review","lock":lock,"answer":answer,"report":report});
    assert_eq!(invoke(missing_review.clone()).http_status(), 400);
    missing_review["human_percent"] = json!(99.9);
    assert_eq!(invoke(missing_review).http_status(), 400);
}

#[test]
fn a_full_human_override_preserves_the_ai_record_and_prior_balanced_review() {
    let (lock, answer, report) = fixture("red", "");
    let ai_hash = human::report_hash(&report).unwrap();
    let first = authored_review(&lock, &answer, &report, 80.0, ReviewMode::Balanced);
    let first_response = invoke(
        json!({"operation":"examiner_review","lock":lock,"answer":answer,"report":report,"review":first}),
    );
    assert!(first_response.error.is_none());
    let balanced: Report =
        serde_json::from_value(first_response.result.unwrap()["report"].clone()).unwrap();
    assert_eq!(
        balanced.decision.as_ref().unwrap().final_percent,
        Some(89.95)
    );
    assert_eq!(balanced.current.percent, Some(99.9));

    let override_review =
        authored_review(&lock, &answer, &balanced, 42.0, ReviewMode::HumanOverride);
    let finalized = human::review(&lock, &answer, &balanced, &override_review).unwrap();
    let decision = finalized.decision.as_ref().unwrap();
    assert_eq!(decision.ai_weight_percent, 0);
    assert_eq!(decision.human_weight_percent, 100);
    assert_eq!(decision.final_percent, Some(42.0));
    assert_eq!(decision.history, vec![first.clone()]);
    assert_eq!(human::report_hash(&finalized).unwrap(), ai_hash);
    assert_eq!(finalized.criteria, report.criteria);
    human::validate_report(&lock, &answer, &finalized).unwrap();
    assert!(human::review(&lock, &answer, &balanced, &first).is_err());
    assert!(human::review(&lock, &answer, &finalized, &override_review).is_err());
}

#[test]
fn unresolved_or_blocked_ai_does_not_veto_an_explicit_examiner_override() {
    for name in ["disagreement", "blue"] {
        let (lock, answer, report) = fixture(name, "");
        assert_ne!(report.status, "ready");
        let mut review = authored_review(&lock, &answer, &report, 71.25, ReviewMode::Balanced);
        assert!(human::review(&lock, &answer, &report, &review).is_err());
        review.mode = ReviewMode::HumanOverride;
        let finalized = human::review(&lock, &answer, &report, &review).unwrap();
        assert_eq!(
            finalized.decision.as_ref().unwrap().final_percent,
            Some(71.25)
        );
        assert_eq!(finalized.status, report.status);
        assert_eq!(finalized.current, report.current);
        human::validate_report(&lock, &answer, &finalized).unwrap();
    }
}

#[test]
fn rehashing_a_forged_decision_cannot_bypass_recomputed_math_or_required_fields() {
    let (lock, answer, report) = fixture("red", "");
    let review = authored_review(&lock, &answer, &report, 80.0, ReviewMode::Balanced);
    let finalized = human::review(&lock, &answer, &report, &review).unwrap();
    let mut forged = finalized.clone();
    let decision = forged.decision.as_mut().unwrap();
    decision.final_percent = Some(99.9);
    decision.decision_hash = human::decision_hash(decision).unwrap();
    assert!(human::validate_report(&lock, &answer, &forged).is_err());

    let mut missing = report.clone();
    missing.decision = None;
    assert!(human::validate_report(&lock, &answer, &missing).is_err());
    let mut modified_ai = report.clone();
    modified_ai.current.percent = Some(80.0);
    modified_ai.decision = Some(human::pending(&modified_ai).unwrap());
    assert!(human::validate_report(&lock, &answer, &modified_ai).is_err());
    assert!(human::review(&lock, &(answer.clone() + "changed"), &report, &review).is_err());

    for score in [-1.0, 100.0, f64::INFINITY, f64::NAN] {
        let mut invalid = review.clone();
        invalid.human_percent = score;
        assert!(human::review(&lock, &answer, &report, &invalid).is_err());
    }
}

#[test]
fn changing_evidence_after_review_cannot_leave_a_stale_official_grade() {
    let (lock, answer, report) = fixture("red", "");
    let review = authored_review(&lock, &answer, &report, 80.0, ReviewMode::Balanced);
    let mut finalized = human::review(&lock, &answer, &report, &review).unwrap();
    let before = finalized.clone();
    grading::apply_evidence_gate(&mut finalized, &[]).unwrap();
    assert_eq!(finalized, before);
    assert!(grading::apply_evidence_gate(&mut finalized, &["principle".into()]).is_err());
    assert_eq!(finalized, before);
    human::validate_report(&lock, &answer, &finalized).unwrap();
}

#[test]
fn chat_style_review_renders_untrusted_answer_text_without_executable_markup() {
    let payload = "</script><img src=x onerror=alert('audit-xss')><script>";
    let (lock, answer, report) = fixture("red", payload);
    let rendered =
        invoke(json!({"operation":"review_report","lock":lock,"answer":answer,"report":report}));
    assert!(rendered.error.is_none(), "{:?}", rendered.error);
    let output = rendered.result.unwrap();
    let html = output["html"].as_str().unwrap();
    assert!(!html.contains(payload));
    assert!(!html.contains("<img src=x"));
    assert!(html.contains("audit-xss"));
    assert!(
        output["markdown"]
            .as_str()
            .is_some_and(|value| !value.is_empty())
    );
}

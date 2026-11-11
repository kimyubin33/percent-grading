//! Mandatory human adjudication of a provisional AI grading record.
//!
//! Hashes bind records and detect accidental changes. They do not authenticate
//! the examiner or certify that a recorded review actually occurred.
use crate::{
    contract::{self, LockedContract},
    grading::{self, Judgment, Report},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const REVIEW_FORMAT: &str = "percent-grading-examiner-review-v1";
pub const MAX_HISTORY: usize = 32;
pub const NOTICE: &str = "The examiner has final decision authority. AI scores and explanations can be wrong, biased, or incomplete; two-model agreement is not proof. The default official score uses 50% examiner and 50% AI. An explicit, documented override uses 100% examiner weight and preserves the AI record. A 100% human weight does not remove the 99.9% score ceiling. Recorded acknowledgements and hashes do not authenticate identity, prove careful review, or establish correctness. The host must authorize reviewers, preserve records, and provide a fair review and appeal process. No score, score difference, writing style, gaze observation, or research marker proves AI use or misconduct.";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewMode {
    Balanced,
    HumanOverride,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExaminerReview {
    pub format: String,
    pub report_hash: String,
    pub contract_hash: String,
    pub answer_hash: String,
    pub expected_decision_hash: String,
    pub examiner_id: String,
    pub human_percent: f64,
    pub mode: ReviewMode,
    pub grading_basis: String,
    pub rationale: String,
    pub reviewed_ai_report: bool,
    pub acknowledged_ai_limits: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Decision {
    pub status: String,
    pub report_hash: String,
    pub mode: Option<ReviewMode>,
    pub ai_weight_percent: u32,
    pub human_weight_percent: u32,
    pub ai_percent: Option<f64>,
    pub human_percent: Option<f64>,
    pub final_percent: Option<f64>,
    pub review: Option<ExaminerReview>,
    pub history: Vec<ExaminerReview>,
    pub decision_hash: String,
    pub notice: String,
}

fn canonical_hash<T: Serialize>(domain: &str, value: &T, excluded: &str) -> Result<String, String> {
    let mut value = serde_json::to_value(value).map_err(|_| "Cannot serialize review record")?;
    value
        .as_object_mut()
        .ok_or("Review record must be an object")?
        .remove(excluded);
    normalize_numbers(&mut value);
    let mut bytes = domain.as_bytes().to_vec();
    bytes.extend(serde_json::to_vec(&value).map_err(|_| "Cannot encode review record")?);
    Ok(contract::hash(&bytes))
}

/// Normalize exactly represented integral floating-point values before hashing.
/// Browsers serialize 80.0 as 80; both must bind the same review record. Objects
/// use serde_json's sorted map representation; arrays retain their order. This
/// is a protocol-specific encoding, not a claim of RFC 8785 conformance.
pub fn normalize_numbers(value: &mut Value) {
    match value {
        Value::Array(items) => items.iter_mut().for_each(normalize_numbers),
        Value::Object(fields) => fields.values_mut().for_each(normalize_numbers),
        Value::Number(number) if number.is_f64() => {
            if let Some(n) = number.as_f64()
                && n.is_finite()
                && n.fract() == 0.0
                && n.abs() <= 9_007_199_254_740_991.0
            {
                *number = serde_json::Number::from(n as i64);
            }
        }
        _ => {}
    }
}

/// Canonical sorted-object JSON of the original AI report, excluding `decision`.
pub fn report_hash(report: &Report) -> Result<String, String> {
    canonical_hash("percent-grading-ai-report-v1\n", report, "decision")
}

/// Canonical sorted-object JSON of the decision, excluding its own hash field.
pub fn decision_hash(decision: &Decision) -> Result<String, String> {
    canonical_hash(
        "percent-grading-human-decision-v1\n",
        decision,
        "decision_hash",
    )
}

fn seal(mut decision: Decision) -> Result<Decision, String> {
    decision.decision_hash = decision_hash(&decision)?;
    Ok(decision)
}

pub fn pending(report: &Report) -> Result<Decision, String> {
    if report.engine_version != "0.7.0" {
        return Err("Examiner decisions require an engine 0.7.0 AI report".into());
    }
    seal(Decision {
        status: "awaiting_examiner".into(),
        report_hash: report_hash(report)?,
        mode: None,
        ai_weight_percent: 50,
        human_weight_percent: 50,
        ai_percent: report.current.percent,
        human_percent: None,
        final_percent: None,
        review: None,
        history: Vec::new(),
        decision_hash: String::new(),
        notice: NOTICE.into(),
    })
}

fn bounded_text(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 16_384 && !value.contains('\0')
}

impl ExaminerReview {
    pub fn validate(&self) -> Result<(), String> {
        if self.format != REVIEW_FORMAT
            || self.examiner_id.is_empty()
            || self.examiner_id.len() > 64
            || !self
                .examiner_id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
            || !self.human_percent.is_finite()
            || !(0.0..=99.9).contains(&self.human_percent)
            || !bounded_text(&self.grading_basis)
            || !bounded_text(&self.rationale)
            || !self.reviewed_ai_report
            || !self.acknowledged_ai_limits
        {
            return Err("An authorized examiner must provide a 0..99.9 score, grading basis, rationale, and explicit review and limitation acknowledgements".into());
        }
        for hash in [
            &self.report_hash,
            &self.contract_hash,
            &self.answer_hash,
            &self.expected_decision_hash,
        ] {
            if hash.len() != 64
                || !hash
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            {
                return Err("Review bindings must be lowercase SHA-256 values".into());
            }
        }
        Ok(())
    }
}

fn apply(
    report: &Report,
    previous: &Decision,
    review: &ExaminerReview,
) -> Result<Decision, String> {
    review.validate()?;
    if review.report_hash != previous.report_hash
        || review.contract_hash != report.contract_hash
        || review.answer_hash != report.answer_hash
        || review.expected_decision_hash != previous.decision_hash
    {
        return Err(
            "Review does not bind the AI report, answer, contract, and current decision revision"
                .into(),
        );
    }
    let (ai_weight, human_weight, final_percent) = match review.mode {
        ReviewMode::Balanced => {
            if report.status != "ready" {
                return Err("An unresolved or blocked AI report cannot be blended; inspect the evidence and explicitly choose a documented human override or obtain a new AI evaluation".into());
            }
            let ai = report
                .current
                .percent
                .ok_or("The AI score is not settled")?;
            (50, 50, (ai + review.human_percent) / 2.0)
        }
        ReviewMode::HumanOverride => (0, 100, review.human_percent),
    };
    let mut history = previous.history.clone();
    if let Some(prior) = &previous.review {
        history.push(prior.clone());
    }
    if history.len() > MAX_HISTORY {
        return Err("Review history is full; preserve this record and begin an explicitly linked new assessment record".into());
    }
    seal(Decision {
        status: "finalized".into(),
        report_hash: previous.report_hash.clone(),
        mode: Some(review.mode),
        ai_weight_percent: ai_weight,
        human_weight_percent: human_weight,
        ai_percent: report.current.percent,
        human_percent: Some(review.human_percent),
        final_percent: Some(final_percent),
        review: Some(review.clone()),
        history,
        decision_hash: String::new(),
        notice: NOTICE.into(),
    })
}

/// Recompute every retained model judgment and decision. This validates the
/// protocol and arithmetic, not the truth or independence of supplied evidence.
pub fn validate_report(lock: &LockedContract, answer: &str, report: &Report) -> Result<(), String> {
    lock.verify()?;
    crate::runner::validate_answer(answer)?;
    if report.engine_version != "0.7.0"
        || report.contract_hash != lock.contract_hash
        || report.answer_hash != contract::hash(answer.as_bytes())
    {
        return Err(
            "Review requires the exact engine 0.7.0 report, locked contract, and answer".into(),
        );
    }
    let provider = |openai: bool| Judgment {
        contract_hash: report.contract_hash.clone(),
        answer_hash: report.answer_hash.clone(),
        judgments: report
            .criteria
            .iter()
            .map(|row| {
                if openai {
                    row.openai.clone()
                } else {
                    row.claude.clone()
                }
            })
            .collect(),
    };
    let mut expected = grading::aggregate_for_version(
        "0.7.0",
        &lock.contract,
        &lock.contract_hash,
        answer,
        &report.answer_hash,
        &provider(true),
        &provider(false),
    )?;
    let blocked: Vec<_> = report
        .criteria
        .iter()
        .filter(|r| r.status == "evidence_unresolved")
        .map(|r| r.criterion_id.clone())
        .collect();
    grading::apply_evidence_gate(&mut expected, &blocked)?;
    let mut supplied = report.clone();
    let decision = supplied
        .decision
        .take()
        .ok_or("AI-only reports cannot omit their mandatory examiner decision state")?;
    let mut expected_decision = expected
        .decision
        .take()
        .ok_or("Missing provisional decision")?;
    if expected != supplied {
        return Err(
            "AI report differs from recomputed judgments and evidence-gate metadata".into(),
        );
    }
    if decision.history.len() > MAX_HISTORY {
        return Err("Examiner review history exceeds the bound".into());
    }
    for historical in &decision.history {
        expected_decision = apply(&supplied, &expected_decision, historical)?;
    }
    if let Some(current) = &decision.review {
        expected_decision = apply(&supplied, &expected_decision, current)?;
    }
    if decision != expected_decision {
        return Err("Examiner decision, revision chain, or final arithmetic is invalid".into());
    }
    Ok(())
}

/// An intentionally incomplete form. The invalid -1 score prevents an omitted
/// human score from silently becoming zero; the examiner must enter a real score.
pub fn template(
    lock: &LockedContract,
    answer: &str,
    report: &Report,
) -> Result<ExaminerReview, String> {
    validate_report(lock, answer, report)?;
    Ok(ExaminerReview {
        format: REVIEW_FORMAT.into(),
        report_hash: report_hash(report)?,
        contract_hash: report.contract_hash.clone(),
        answer_hash: report.answer_hash.clone(),
        expected_decision_hash: report
            .decision
            .as_ref()
            .ok_or("Missing decision")?
            .decision_hash
            .clone(),
        examiner_id: String::new(),
        human_percent: -1.0,
        mode: ReviewMode::Balanced,
        grading_basis: String::new(),
        rationale: String::new(),
        reviewed_ai_report: false,
        acknowledged_ai_limits: false,
    })
}

pub fn review(
    lock: &LockedContract,
    answer: &str,
    report: &Report,
    review: &ExaminerReview,
) -> Result<Report, String> {
    validate_report(lock, answer, report)?;
    let previous = report.decision.as_ref().ok_or("Missing decision")?;
    let mut finalized = report.clone();
    finalized.decision = Some(apply(report, previous, review)?);
    Ok(finalized)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grading::Row;

    fn fixture(version: &str) -> (LockedContract, String, Report) {
        let lock = contract::compile(include_str!("../policies/apple.aha")).unwrap();
        let answer =
            "Synthetic examiner-review fixture; not a real evaluated student answer.".to_string();
        let answer_hash = contract::hash(answer.as_bytes());
        let judgment = Judgment {
            contract_hash: lock.contract_hash.clone(),
            answer_hash: answer_hash.clone(),
            judgments: lock
                .contract
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
                    evidence: vec![answer.clone()],
                    reason: "Manual protocol fixture only.".into(),
                    conditions: vec![],
                })
                .collect(),
        };
        let report = grading::aggregate_for_version(
            version,
            &lock.contract,
            &lock.contract_hash,
            &answer,
            &answer_hash,
            &judgment,
            &judgment,
        )
        .unwrap();
        (lock, answer, report)
    }

    fn completed_form(lock: &LockedContract, answer: &str, report: &Report) -> ExaminerReview {
        let mut form = template(lock, answer, report).unwrap();
        form.examiner_id = "examiner_fixture".into();
        form.human_percent = 90.0;
        form.grading_basis =
            "Manually authored fixture score under the declared review method.".into();
        form.rationale =
            "Synthetic test of score combination, not a subject-matter evaluation.".into();
        form.reviewed_ai_report = true;
        form.acknowledged_ai_limits = true;
        form
    }

    #[test]
    fn old_wire_formats_do_not_gain_an_implicit_human_approval() {
        for version in ["0.2.0", "0.3.0", "0.4.0", "0.5.0", "0.6.0"] {
            let (lock, answer, report) = fixture(version);
            assert!(report.decision.is_none());
            assert!(
                serde_json::to_value(&report)
                    .unwrap()
                    .get("decision")
                    .is_none()
            );
            assert!(template(&lock, &answer, &report).is_err());
            assert_eq!(
                report.current.percent,
                Some(if version == "0.6.0" { 99.9 } else { 100.0 })
            );
        }
    }

    #[test]
    fn fifty_fifty_preserves_fractional_scores_and_requires_explicit_form_completion() {
        let (lock, answer, report) = fixture("0.7.0");
        let incomplete = template(&lock, &answer, &report).unwrap();
        assert_eq!(
            report.decision.as_ref().unwrap().status,
            "awaiting_examiner"
        );
        assert_eq!(report.decision.as_ref().unwrap().final_percent, None);
        assert_eq!(incomplete.human_percent, -1.0);
        assert!(review(&lock, &answer, &report, &incomplete).is_err());
        let finalized = review(
            &lock,
            &answer,
            &report,
            &completed_form(&lock, &answer, &report),
        )
        .unwrap();
        let result = finalized.decision.as_ref().unwrap();
        assert!((result.final_percent.unwrap() - 94.95).abs() < 1e-10);
        assert_eq!(
            (result.ai_weight_percent, result.human_weight_percent),
            (50, 50)
        );
        assert_eq!(finalized.current, report.current);
        validate_report(&lock, &answer, &finalized).unwrap();
    }

    #[test]
    fn documented_override_preserves_the_original_ai_record_and_prior_human_review() {
        let (lock, answer, report) = fixture("0.7.0");
        let balanced = review(
            &lock,
            &answer,
            &report,
            &completed_form(&lock, &answer, &report),
        )
        .unwrap();
        let mut correction = completed_form(&lock, &answer, &balanced);
        correction.mode = ReviewMode::HumanOverride;
        correction.human_percent = 81.25;
        correction.grading_basis =
            "Examiner adopts a corrected grading method for the same answer.".into();
        correction.rationale =
            "The recorded AI interpretation is inappropriate for this synthetic case.".into();
        let corrected = review(&lock, &answer, &balanced, &correction).unwrap();
        let decision = corrected.decision.as_ref().unwrap();
        assert_eq!(decision.final_percent, Some(81.25));
        assert_eq!(
            (decision.ai_weight_percent, decision.human_weight_percent),
            (0, 100)
        );
        assert_eq!(decision.history.len(), 1);
        assert_eq!(decision.history[0].human_percent, 90.0);
        assert_eq!(
            report_hash(&corrected).unwrap(),
            report_hash(&report).unwrap()
        );
        assert_eq!(corrected.criteria, report.criteria);
        validate_report(&lock, &answer, &corrected).unwrap();
    }

    #[test]
    fn browser_integral_number_roundtrips_keep_canonical_bindings() {
        let (lock, answer, report) = fixture("0.7.0");
        let finalized = review(
            &lock,
            &answer,
            &report,
            &completed_form(&lock, &answer, &report),
        )
        .unwrap();
        let mut wire = serde_json::to_value(&finalized).unwrap();
        normalize_numbers(&mut wire);
        assert_eq!(wire["current"]["coverage_percent"], serde_json::json!(100));
        assert_eq!(wire["decision"]["human_percent"], serde_json::json!(90));
        let reparsed: Report = serde_json::from_value(wire).unwrap();
        assert_eq!(
            report_hash(&finalized).unwrap(),
            report_hash(&reparsed).unwrap()
        );
        validate_report(&lock, &answer, &reparsed).unwrap();
        let mut edge = serde_json::json!([-0.0, 0.5, 9007199254740991.0, 9007199254740992.0]);
        normalize_numbers(&mut edge);
        assert_eq!(edge[0], serde_json::json!(0));
        assert!(edge[1].is_f64());
        assert!(edge[2].is_i64());
        assert!(edge[3].is_f64());
    }
}

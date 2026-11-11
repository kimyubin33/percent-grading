//! Common evidence revisions and explicit, immutable reassessment plans.
//!
//! A plan covers every member of the supplied cohort. It does not establish that
//! an external class roster is complete, attest ownership of a previous run, or
//! perform a new assessment. Hashes detect changes relative to the supplied
//! inputs; they are not signatures or proof of scientific validity.

use crate::{
    contract::{LockedContract, hash},
    evidence::EvidencePack,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

pub const COHORT_FORMAT: &str = "percent-grading-cohort-v1";
pub const PLAN_FORMAT: &str = "percent-grading-revision-plan-v1";
pub const MAX_COHORT_MEMBERS: usize = 10_000;
const SCOPE_NOTICE: &str = "This plan covers the declared cohort only; completeness against an external roster is not verified. Previous run IDs are references, not verified associations. All members remain pending until separately assessed using the required evidence pack. No prior result is modified and no API call is made.";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Cohort {
    pub format: String,
    pub task_id: String,
    pub members: Vec<CohortMember>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CohortMember {
    pub submission_id: String,
    pub answer_hash: String,
    pub previous_run_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PendingAssessment {
    pub submission_id: String,
    pub answer_hash: String,
    pub previous_run_id: Option<String>,
    pub required_evidence_hash: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RevisionPlan {
    pub format: String,
    pub contract_hash: String,
    pub old_evidence_hash: String,
    pub new_evidence_hash: String,
    pub old_policy_version: String,
    pub new_policy_version: String,
    pub reason: String,
    pub cohort: Cohort,
    pub cohort_hash: String,
    pub pending: Vec<PendingAssessment>,
    pub scope_notice: String,
    pub plan_hash: String,
}

fn identifier(value: &str, label: &str) -> Result<(), String> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
    {
        return Err(format!(
            "{label} must contain 1..128 ASCII letters, digits, underscores or hyphens"
        ));
    }
    Ok(())
}

fn digest(value: &str, label: &str) -> Result<(), String> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(format!(
            "{label} must be a 64-character lowercase SHA-256 digest"
        ));
    }
    Ok(())
}

impl Cohort {
    pub fn validate(&self) -> Result<(), String> {
        if self.format != COHORT_FORMAT {
            return Err(format!(
                "Unsupported cohort format; expected {COHORT_FORMAT}"
            ));
        }
        identifier(&self.task_id, "task_id")?;
        if self.members.is_empty() || self.members.len() > MAX_COHORT_MEMBERS {
            return Err(format!(
                "A cohort must contain 1..{MAX_COHORT_MEMBERS} members"
            ));
        }
        let mut ids = BTreeSet::new();
        for member in &self.members {
            identifier(&member.submission_id, "submission_id")?;
            if !ids.insert(&member.submission_id) {
                return Err(format!("Duplicate submission_id: {}", member.submission_id));
            }
            digest(&member.answer_hash, "answer_hash")?;
            if let Some(id) = &member.previous_run_id {
                digest(id, "previous_run_id")?;
            }
        }
        Ok(())
    }
}

/// Make a complete pending plan for the declared cohort under a new policy
/// version. Cohort order is retained as supplied and contributes to its hash.
pub fn plan(
    lock: &LockedContract,
    old: &EvidencePack,
    new: &EvidencePack,
    cohort: &Cohort,
    reason: &str,
) -> Result<RevisionPlan, String> {
    old.verify(lock)?;
    new.verify(lock)?;
    cohort.validate()?;
    if reason.trim().is_empty() || reason.len() > 4096 || reason.contains('\0') {
        return Err("Revision reason must contain 1..4096 bytes without NUL".into());
    }
    if old.pack_hash == new.pack_hash {
        return Err("A revision requires a different evidence pack".into());
    }
    let old_version = &old.input.policy.policy.version;
    let new_version = &new.input.policy.policy.version;
    if old_version == new_version {
        return Err("A common revision requires a new evidence policy version".into());
    }
    let pending = cohort
        .members
        .iter()
        .map(|member| PendingAssessment {
            submission_id: member.submission_id.clone(),
            answer_hash: member.answer_hash.clone(),
            previous_run_id: member.previous_run_id.clone(),
            required_evidence_hash: new.pack_hash.clone(),
            status: "pending".into(),
        })
        .collect();
    let mut result = RevisionPlan {
        format: PLAN_FORMAT.into(),
        contract_hash: lock.contract_hash.clone(),
        old_evidence_hash: old.pack_hash.clone(),
        new_evidence_hash: new.pack_hash.clone(),
        old_policy_version: old_version.clone(),
        new_policy_version: new_version.clone(),
        reason: reason.into(),
        cohort: cohort.clone(),
        cohort_hash: hash(&serde_json::to_vec(cohort).map_err(|e| e.to_string())?),
        pending,
        scope_notice: SCOPE_NOTICE.into(),
        plan_hash: String::new(),
    };
    // Hash the fixed serialization with plan_hash empty. verify reconstructs all
    // fields and rejects omissions or changes rather than trusting this digest.
    result.plan_hash = hash(&serde_json::to_vec(&result).map_err(|e| e.to_string())?);
    Ok(result)
}

impl RevisionPlan {
    pub fn verify(
        &self,
        lock: &LockedContract,
        old: &EvidencePack,
        new: &EvidencePack,
    ) -> Result<(), String> {
        let expected = plan(lock, old, new, &self.cohort, &self.reason)?;
        if *self != expected {
            return Err("Revision plan differs from deterministic reconstruction".into());
        }
        Ok(())
    }
}

/// Compare the evidence assessments, not learner scores or calibrated confidence.
/// Unlike a common revision plan, a comparison may keep the policy version fixed.
pub fn compare(
    lock: &LockedContract,
    old: &EvidencePack,
    new: &EvidencePack,
) -> Result<Value, String> {
    old.verify(lock)?;
    new.verify(lock)?;
    let old_blocked: BTreeSet<_> = old.blocked_criteria.iter().cloned().collect();
    let new_blocked: BTreeSet<_> = new.blocked_criteria.iter().cloned().collect();
    let old_claims: BTreeMap<_, _> = old
        .claims
        .iter()
        .map(|claim| (claim.claim_id.clone(), claim))
        .collect();
    let new_claims: BTreeMap<_, _> = new
        .claims
        .iter()
        .map(|claim| (claim.claim_id.clone(), claim))
        .collect();
    let ids: BTreeSet<_> = old_claims.keys().chain(new_claims.keys()).collect();
    let mut changed_claims = Vec::new();
    for id in ids {
        let before = serde_json::to_value(old_claims.get(id)).map_err(|e| e.to_string())?;
        let after = serde_json::to_value(new_claims.get(id)).map_err(|e| e.to_string())?;
        if before != after {
            changed_claims.push(json!({"claim_id": id, "before": before, "after": after}));
        }
    }
    let old_definitions =
        serde_json::to_value(&old.input.policy.claims).map_err(|e| e.to_string())?;
    let new_definitions =
        serde_json::to_value(&new.input.policy.claims).map_err(|e| e.to_string())?;
    let changed_definitions = if old_definitions == new_definitions {
        Value::Null
    } else {
        json!({"before": old_definitions, "after": new_definitions})
    };
    Ok(json!({
        "format": "percent-grading-evidence-comparison-v1",
        "contract_hash": lock.contract_hash,
        "old_evidence_hash": old.pack_hash,
        "new_evidence_hash": new.pack_hash,
        "same_policy": old.input.policy.policy_hash == new.input.policy.policy_hash,
        "old_policy_version": old.input.policy.policy.version,
        "new_policy_version": new.input.policy.policy.version,
        "newly_blocked_criteria": new_blocked.difference(&old_blocked).collect::<Vec<_>>(),
        "newly_unblocked_criteria": old_blocked.difference(&new_blocked).collect::<Vec<_>>(),
        "changed_claims": changed_claims,
        "changed_claim_definitions": changed_definitions,
        "interpretation": "This compares source-claim evidence assessments only. It does not recalculate any learner score or measure a probability. A changed policy may also change the claims being assessed; inspect both policy versions."
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evidence;

    fn packs() -> (LockedContract, EvidencePack, EvidencePack) {
        let (lock, input) = evidence::test_input();
        let review = evidence::test_review(&input);
        let old = evidence::aggregate(&lock, &input, &review, &review).unwrap();
        let policy = evidence::compile_policy(
            &lock,
            &input.policy.source.replace("version 1,", "version 2,"),
        )
        .unwrap();
        let input = evidence::prepare(&lock, &policy, &input.dossier).unwrap();
        let review = evidence::test_review(&input);
        let new = evidence::aggregate(&lock, &input, &review, &review).unwrap();
        (lock, old, new)
    }

    fn cohort() -> Cohort {
        Cohort {
            format: COHORT_FORMAT.into(),
            task_id: "apple-class-a".into(),
            members: vec![
                CohortMember {
                    submission_id: "student-a".into(),
                    answer_hash: hash(b"same answer"),
                    previous_run_id: Some(hash(b"first run")),
                },
                CohortMember {
                    submission_id: "student-b".into(),
                    answer_hash: hash(b"same answer"),
                    previous_run_id: None,
                },
            ],
        }
    }

    #[test]
    fn identical_answers_remain_distinct_cohort_members() {
        let original = cohort();
        original.validate().unwrap();
        let restored: Cohort =
            serde_json::from_slice(&serde_json::to_vec(&original).unwrap()).unwrap();
        assert_eq!(restored, original);
        assert_eq!(restored.members.len(), 2);
    }

    #[test]
    fn duplicate_submission_and_malformed_hashes_are_rejected() {
        let mut value = cohort();
        value.members[1].submission_id = value.members[0].submission_id.clone();
        assert!(value.validate().is_err());
        for malformed in ["0".repeat(63), "G".repeat(64), "A".repeat(64)] {
            let mut value = cohort();
            value.members[0].answer_hash = malformed.clone();
            assert!(value.validate().is_err());
            value = cohort();
            value.members[0].previous_run_id = Some(malformed);
            assert!(value.validate().is_err());
        }
    }

    #[test]
    fn cohort_limits_and_unknown_fields_fail_closed() {
        let mut value = cohort();
        value.members.clear();
        assert!(value.validate().is_err());
        value = cohort();
        value.members = (0..=MAX_COHORT_MEMBERS)
            .map(|i| CohortMember {
                submission_id: format!("student-{i}"),
                answer_hash: hash(b"answer"),
                previous_run_id: None,
            })
            .collect();
        assert!(value.validate().is_err());
        let mut json = serde_json::to_value(cohort()).unwrap();
        json["selected_subset"] = json!(["student-a"]);
        assert!(serde_json::from_value::<Cohort>(json).is_err());
    }

    #[test]
    fn revision_retains_all_members_and_never_claims_assessment_complete() {
        let (lock, old, new) = packs();
        let declared = cohort();
        let result = plan(&lock, &old, &new, &declared, "Broaden the source policy.").unwrap();
        result.verify(&lock, &old, &new).unwrap();
        assert_eq!(result.cohort, declared);
        assert_eq!(result.pending.len(), 2);
        assert!(
            result
                .pending
                .iter()
                .all(|member| member.status == "pending"
                    && member.required_evidence_hash == new.pack_hash)
        );
        assert_ne!(
            result.pending[0].submission_id,
            result.pending[1].submission_id
        );
        assert_eq!(result.pending[0].answer_hash, result.pending[1].answer_hash);
        assert!(
            result
                .scope_notice
                .contains("external roster is not verified")
        );
        let repeated = plan(&lock, &old, &new, &declared, "Broaden the source policy.").unwrap();
        assert_eq!(result, repeated);
    }

    #[test]
    fn omitted_members_fabricated_completion_and_tampered_inputs_fail_verification() {
        let (lock, old, new) = packs();
        let original = plan(&lock, &old, &new, &cohort(), "Review all submissions.").unwrap();
        let mut altered = original.clone();
        altered.pending.pop();
        assert!(altered.verify(&lock, &old, &new).is_err());
        let mut altered = original.clone();
        altered.pending[0].status = "completed".into();
        assert!(altered.verify(&lock, &old, &new).is_err());
        let mut altered = original.clone();
        altered.pending[0].required_evidence_hash = old.pack_hash.clone();
        assert!(altered.verify(&lock, &old, &new).is_err());
        let mut altered = original;
        altered.cohort.members.pop();
        assert!(altered.verify(&lock, &old, &new).is_err());
    }

    #[test]
    fn revision_requires_a_new_policy_and_verified_same_task_inputs() {
        let (lock, old, new) = packs();
        assert!(plan(&lock, &old, &old, &cohort(), "No change.").is_err());
        assert!(plan(&lock, &old, &new, &cohort(), " ").is_err());
        let mut changed_dossier = old.input.dossier.clone();
        changed_dossier.searches[0].outcome = "A changed search log under the same policy.".into();
        let changed_input = evidence::prepare(&lock, &old.input.policy, &changed_dossier).unwrap();
        let review = evidence::test_review(&changed_input);
        let same_version = evidence::aggregate(&lock, &changed_input, &review, &review).unwrap();
        assert_ne!(old.pack_hash, same_version.pack_hash);
        assert!(
            plan(&lock, &old, &same_version, &cohort(), "New log only.")
                .unwrap_err()
                .contains("new evidence policy version")
        );
        let mut invalid = new.clone();
        invalid.pack_hash = "0".repeat(64);
        assert!(plan(&lock, &old, &invalid, &cohort(), "Changed.").is_err());
        let other =
            crate::contract::compile(&lock.source.replace("version 1,", "version 2,")).unwrap();
        assert!(plan(&other, &old, &new, &cohort(), "Other task.").is_err());
    }

    #[test]
    fn comparison_does_not_invent_learner_score_changes() {
        let (lock, old, new) = packs();
        let result = compare(&lock, &old, &new).unwrap();
        assert_eq!(result["same_policy"], false);
        assert_eq!(result["changed_claims"], json!([]));
        assert_eq!(result["changed_claim_definitions"], Value::Null);
        assert_eq!(result["newly_blocked_criteria"], json!([]));
        assert_eq!(result["newly_unblocked_criteria"], json!([]));
        assert!(result.get("score_change").is_none());
        assert!(compare(&lock, &old, &old).is_ok());
    }

    #[test]
    fn comparison_exposes_definition_changes_even_with_equal_assessments() {
        let (lock, old, new) = packs();
        let changed_policy = evidence::compile_policy(
            &lock,
            &new.input.policy.source.replace(
                "statement The specified apple appears red.",
                "statement The specimen is perceived as red.",
            ),
        )
        .unwrap();
        let input = evidence::prepare(&lock, &changed_policy, &new.input.dossier).unwrap();
        let review = evidence::test_review(&input);
        let changed = evidence::aggregate(&lock, &input, &review, &review).unwrap();
        let result = compare(&lock, &old, &changed).unwrap();
        assert_eq!(result["changed_claims"], json!([]));
        assert!(result["changed_claim_definitions"].is_object());
        assert_ne!(
            result["changed_claim_definitions"]["before"],
            result["changed_claim_definitions"]["after"]
        );
    }
}

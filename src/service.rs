//! Versioned, transport-independent application interface.
//!
//! Callers supply data, never filesystem paths, credentials, provider URLs, or
//! arbitrary commands. The host owns storage and explicitly enables paid calls.
use crate::{
    benchmark::{BenchmarkSuite, Results},
    benchmark_gate::{self, GatePolicy},
    contract::{self, LockedContract},
    evidence::{self, Dossier, EvidenceInput, EvidencePack, LockedEvidencePolicy, Review},
    grading::{self, Judgment, Report},
    human,
    proof::{self, Certificate, LockedProofPolicy},
    providers::{self, Provider},
    review_view,
    runner::{self, RunConfig},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::path::PathBuf;

pub const API_VERSION: &str = "percent-grading-api-v1";
pub const MAX_REQUEST_BYTES: usize = 2 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct ServiceContext {
    pub runs_dir: PathBuf,
    pub allow_live: bool,
}

impl Default for ServiceContext {
    fn default() -> Self {
        Self {
            runs_dir: PathBuf::from("local-runs/api"),
            allow_live: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub api_version: String,
    pub request_id: String,
    pub payload: Operation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum Operation {
    Compile {
        source: String,
    },
    ReviewTemplate {
        lock: Box<LockedContract>,
        answer: String,
        report: Box<Report>,
    },
    ExaminerReview {
        lock: Box<LockedContract>,
        answer: String,
        report: Box<Report>,
        review: Box<human::ExaminerReview>,
    },
    ReviewReport {
        lock: Box<LockedContract>,
        answer: String,
        report: Box<Report>,
    },
    ProofPolicy {
        lock: Box<LockedContract>,
        source: String,
    },
    ProofPrompt {
        lock: Box<LockedContract>,
        policy: Box<LockedProofPolicy>,
        answer: String,
    },
    ProofEvaluate {
        lock: Box<LockedContract>,
        policy: Box<LockedProofPolicy>,
        answer: String,
        certificate: Box<Certificate>,
        openai: Box<Judgment>,
        claude: Box<Judgment>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        evidence: Option<Box<EvidencePack>>,
    },
    BenchmarkGate {
        suite: Box<BenchmarkSuite>,
        results: Box<Results>,
        policy: Box<GatePolicy>,
    },
    Prompt {
        lock: Box<LockedContract>,
        answer: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        evidence: Option<Box<EvidencePack>>,
    },
    Evaluate {
        lock: Box<LockedContract>,
        answer: String,
        openai: Box<Judgment>,
        claude: Box<Judgment>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        evidence: Option<Box<EvidencePack>>,
    },
    Grade {
        lock: Box<LockedContract>,
        answer: String,
        config: RunConfig,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        evidence: Option<Box<EvidencePack>>,
    },
    EvidencePolicy {
        lock: Box<LockedContract>,
        source: String,
    },
    EvidencePrepare {
        lock: Box<LockedContract>,
        policy: Box<LockedEvidencePolicy>,
        dossier: Box<Dossier>,
    },
    EvidenceAggregate {
        lock: Box<LockedContract>,
        input: Box<EvidenceInput>,
        openai: Box<Review>,
        claude: Box<Review>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ApiError {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Response {
    pub api_version: String,
    pub request_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ApiError>,
}

impl Response {
    pub fn failure(request_id: Option<String>, code: &str, message: &str) -> Self {
        Self {
            api_version: API_VERSION.into(),
            request_id,
            result: None,
            error: Some(ApiError {
                code: code.into(),
                message: message.chars().take(1024).collect(),
            }),
        }
    }

    pub fn http_status(&self) -> u16 {
        match self.error.as_ref().map(|e| e.code.as_str()) {
            None => 200,
            Some("invalid_request") => 400,
            Some("unauthorized") => 401,
            Some("live_disabled" | "origin_forbidden") => 403,
            Some("not_found") => 404,
            Some("method_not_allowed") => 405,
            Some("request_timeout") => 408,
            Some("payload_too_large") => 413,
            Some("unsupported_media_type") => 415,
            Some("validation_failed") => 422,
            Some("busy") => 429,
            Some("provider_failure") => 502,
            Some(_) => 500,
        }
    }
}

fn valid_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_-.".contains(&b))
}

fn validate<T>(value: Result<T, String>) -> Result<T, ApiError> {
    value.map_err(|message| ApiError {
        code: "validation_failed".into(),
        message,
    })
}

fn execute(context: &ServiceContext, operation: &Operation) -> Result<Value, ApiError> {
    match operation {
        Operation::Compile { source } => Ok(json!({"lock":validate(contract::compile(source))?})),
        Operation::ReviewTemplate {
            lock,
            answer,
            report,
        } => Ok(json!({
            "review":validate(human::template(lock,answer,report))?,
            "notice":"An examiner must independently supply a score, grading basis and rationale after reading the AI report. This unsubmitted template is not an approved grade."
        })),
        Operation::ExaminerReview {
            lock,
            answer,
            report,
            review,
        } => Ok(json!({
            "report":validate(human::review(lock,answer,report,review))?,
            "notice":"The examiner retains final authority. Balanced review uses 50% examiner and 50% AI; an explicit human override uses 100% examiner. The original AI record remains visible. This supplied examiner declaration does not authenticate the examiner's identity or establish objective truth."
        })),
        Operation::ReviewReport {
            lock,
            answer,
            report,
        } => Ok(json!({
            "html":validate(review_view::render(lock,answer,report))?,
            "markdown":validate(review_view::markdown(lock,answer,report))?,
            "notice":"Private examiner report. AI explanations are supplied justifications, not proof or private internal reasoning. A draft exported by the page must be submitted to examiner_review before it becomes a final grade."
        })),
        Operation::ProofPolicy { lock, source } => {
            Ok(json!({"policy":validate(proof::compile_policy(lock, source))?}))
        }
        Operation::ProofPrompt {
            lock,
            policy,
            answer,
        } => Ok(json!(validate(proof::certificate_prompt(
            lock, policy, answer
        ))?)),
        Operation::ProofEvaluate {
            lock,
            policy,
            answer,
            certificate,
            openai,
            claude,
            evidence,
        } => Ok(json!(validate(proof::evaluate(
            lock,
            policy,
            answer,
            certificate,
            openai,
            claude,
            evidence.as_deref()
        ))?)),
        Operation::BenchmarkGate {
            suite,
            results,
            policy,
        } => Ok(json!({"gate":validate(benchmark_gate::evaluate(suite, results, policy))?})),
        Operation::Prompt {
            lock,
            answer,
            evidence,
        } => {
            let (system, user) = validate(runner::build_prompt_with_evidence(
                lock,
                answer,
                None,
                evidence.as_deref(),
            ))?;
            Ok(
                json!({"system":system,"user":user,"schema":grading::response_schema(&lock.contract)}),
            )
        }
        Operation::Evaluate {
            lock,
            answer,
            openai,
            claude,
            evidence,
        } => {
            validate(lock.verify())?;
            validate(runner::validate_answer(answer))?;
            if let Some(pack) = evidence {
                validate(pack.verify(lock))?;
            }
            let mut report = validate(grading::aggregate(
                &lock.contract,
                &lock.contract_hash,
                answer,
                &contract::hash(answer.as_bytes()),
                openai,
                claude,
            ))?;
            if let Some(pack) = evidence {
                validate(grading::apply_evidence_gate(
                    &mut report,
                    &pack.blocked_criteria,
                ))?;
            }
            Ok(json!({
                "origin":"imported_judgments",
                "evidence_hash":evidence.as_ref().map(|p| &p.pack_hash),
                "evidence_assessments":evidence.as_ref().map(|p| &p.claims),
                "report":report
            }))
        }
        Operation::Grade {
            lock,
            answer,
            config,
            evidence,
        } => {
            if !context.allow_live {
                return Err(ApiError {
                    code: "live_disabled".into(),
                    message: "The host has not enabled live grading.".into(),
                });
            }
            let (system, user) = validate(runner::build_prompt_with_evidence(
                lock,
                answer,
                None,
                evidence.as_deref(),
            ))?;
            let schema = grading::response_schema(&lock.contract);
            for (provider, model) in [
                (Provider::Openai, &config.openai_model),
                (Provider::Claude, &config.claude_model),
            ] {
                validate(providers::request_body(
                    provider,
                    model,
                    &system,
                    &user,
                    &schema,
                    &config.limits,
                ))?;
            }
            let mut result = runner::grade_with_evidence(
                lock,
                answer,
                config,
                &context.runs_dir,
                None,
                evidence.as_deref(),
            )
            .map_err(|_| ApiError {
                code: "provider_failure".into(),
                // Host paths and raw adapter/storage diagnostics stay local.
                message: "Grading could not be completed. The host can inspect its configured credentials and saved run records.".into(),
            })?;
            if let Some(object) = result.as_object_mut() {
                object.remove("run_directory");
            }
            Ok(result)
        }
        Operation::EvidencePolicy { lock, source } => {
            Ok(json!({"policy":validate(evidence::compile_policy(lock, source))?}))
        }
        Operation::EvidencePrepare {
            lock,
            policy,
            dossier,
        } => Ok(json!({"input":validate(evidence::prepare(lock, policy, dossier))?})),
        Operation::EvidenceAggregate {
            lock,
            input,
            openai,
            claude,
        } => Ok(json!({
            "origin":"imported_reviews",
            "pack":validate(evidence::aggregate(lock,input,openai,claude))?
        })),
    }
}

/// Dispatch a typed request. HTTP and command-line transports use this same path.
/// A request ID is correlation metadata, not an idempotency or grading key.
pub fn dispatch(context: &ServiceContext, request: &Request) -> Response {
    if request.api_version != API_VERSION || !valid_id(&request.request_id) {
        return Response::failure(
            valid_id(&request.request_id).then(|| request.request_id.clone()),
            "invalid_request",
            "Use the supported API version and a 1..128 character ASCII request ID (letters, numbers, underscore, hyphen or dot).",
        );
    }
    match serde_json::to_vec(request) {
        Ok(bytes) if bytes.len() <= MAX_REQUEST_BYTES => {}
        _ => {
            return Response::failure(
                Some(request.request_id.clone()),
                "payload_too_large",
                "Request exceeds the 2097152-byte API limit.",
            );
        }
    }
    match execute(context, &request.payload) {
        Ok(result) => Response {
            api_version: API_VERSION.into(),
            request_id: Some(request.request_id.clone()),
            result: Some(result),
            error: None,
        },
        Err(error) => Response::failure(
            Some(request.request_id.clone()),
            &error.code,
            &error.message,
        ),
    }
}

/// Parse bounded JSON without exposing parser excerpts containing supplied data.
pub fn handle_json(context: &ServiceContext, bytes: &[u8]) -> Response {
    if bytes.len() > MAX_REQUEST_BYTES {
        return Response::failure(
            None,
            "payload_too_large",
            "Request exceeds the 2097152-byte API limit.",
        );
    }
    match serde_json::from_slice::<Request>(bytes) {
        Ok(request) => dispatch(context, &request),
        Err(_) => Response::failure(
            None,
            "invalid_request",
            "Expected a valid UTF-8 JSON API envelope with a supported operation and no unknown fields.",
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(payload: Operation) -> Request {
        Request {
            api_version: API_VERSION.into(),
            request_id: "test-1".into(),
            payload,
        }
    }

    fn examiner_fixture() -> (LockedContract, String, Report) {
        let lock = contract::compile(include_str!("../policies/apple.aha")).unwrap();
        let answer =
            "Synthetic protocol fixture; these are manually authored judgments.".to_owned();
        let judgment = Judgment {
            contract_hash: lock.contract_hash.clone(),
            answer_hash: contract::hash(answer.as_bytes()),
            judgments: lock
                .contract
                .criteria
                .iter()
                .map(|c| grading::Row {
                    criterion_id: c.id.clone(),
                    anchor_id: c
                        .anchors
                        .iter()
                        .find(|a| a.points == 100)
                        .unwrap()
                        .id
                        .clone(),
                    evidence: vec![answer.clone()],
                    reason: "A manually authored API test fixture, not a factual claim.".into(),
                    conditions: vec![],
                })
                .collect(),
        };
        let result = dispatch(
            &ServiceContext::default(),
            &request(Operation::Evaluate {
                lock: Box::new(lock.clone()),
                answer: answer.clone(),
                openai: Box::new(judgment.clone()),
                claude: Box::new(judgment),
                evidence: None,
            }),
        );
        let report = serde_json::from_value(result.result.unwrap()["report"].clone()).unwrap();
        (lock, answer, report)
    }

    #[test]
    fn official_score_requires_explicit_examiner_input_and_supports_fifty_fifty() {
        let (lock, answer, report) = examiner_fixture();
        assert_eq!(report.status, "ready");
        let decision = report.decision.as_ref().unwrap();
        assert_eq!(decision.status, "awaiting_examiner");
        assert!(decision.final_percent.is_none());
        let template = dispatch(
            &ServiceContext::default(),
            &request(Operation::ReviewTemplate {
                lock: Box::new(lock.clone()),
                answer: answer.clone(),
                report: Box::new(report.clone()),
            }),
        );
        let mut review: human::ExaminerReview =
            serde_json::from_value(template.result.unwrap()["review"].clone()).unwrap();
        assert!(!review.reviewed_ai_report);
        assert!(!review.acknowledged_ai_limits);
        assert!(review.human_percent < 0.0);
        let unsubmitted = dispatch(
            &ServiceContext::default(),
            &request(Operation::ExaminerReview {
                lock: Box::new(lock.clone()),
                answer: answer.clone(),
                report: Box::new(report.clone()),
                review: Box::new(review.clone()),
            }),
        );
        assert_eq!(unsubmitted.http_status(), 422);
        review.examiner_id = "examiner_01".into();
        review.human_percent = 80.0;
        review.grading_basis = "Apply the announced rubric to the demonstrated explanation.".into();
        review.rationale="The examiner awards partial credit for the reasoning while retaining the AI comparison.".into();
        review.reviewed_ai_report = true;
        review.acknowledged_ai_limits = true;
        let accepted = dispatch(
            &ServiceContext::default(),
            &request(Operation::ExaminerReview {
                lock: Box::new(lock),
                answer,
                report: Box::new(report),
                review: Box::new(review),
            }),
        );
        assert!(accepted.error.is_none());
        let result = accepted.result.unwrap();
        assert!(
            (result["report"]["decision"]["final_percent"]
                .as_f64()
                .unwrap()
                - 89.95)
                .abs()
                < 1e-9
        );
        assert_eq!(result["report"]["decision"]["human_weight_percent"], 50);
        assert_eq!(result["report"]["decision"]["ai_weight_percent"], 50);
        assert_eq!(result["report"]["current"]["percent"], 99.9);
    }

    #[test]
    fn examiner_override_keeps_ai_record_and_rejects_stale_or_mismatched_review() {
        let (lock, answer, report) = examiner_fixture();
        let mut review = human::template(&lock, &answer, &report).unwrap();
        review.examiner_id = "examiner_01".into();
        review.human_percent = 42.0;
        review.mode = human::ReviewMode::HumanOverride;
        review.grading_basis =
            "The examiner applies the intended interpretation and checks all claims.".into();
        review.rationale =
            "The AI assessed a different question. Use this examiner assessment in full.".into();
        review.reviewed_ai_report = true;
        review.acknowledged_ai_limits = true;
        let result = dispatch(
            &ServiceContext::default(),
            &request(Operation::ExaminerReview {
                lock: Box::new(lock.clone()),
                answer: answer.clone(),
                report: Box::new(report.clone()),
                review: Box::new(review.clone()),
            }),
        )
        .result
        .unwrap();
        assert_eq!(result["report"]["decision"]["final_percent"], 42.0);
        assert_eq!(result["report"]["decision"]["human_weight_percent"], 100);
        assert_eq!(result["report"]["decision"]["ai_weight_percent"], 0);
        assert_eq!(
            result["report"]["criteria"],
            serde_json::to_value(&report.criteria).unwrap()
        );
        let reviewed = serde_json::from_value(result["report"].clone()).unwrap();
        let stale = dispatch(
            &ServiceContext::default(),
            &request(Operation::ExaminerReview {
                lock: Box::new(lock.clone()),
                answer: answer.clone(),
                report: Box::new(reviewed),
                review: Box::new(review.clone()),
            }),
        );
        assert_eq!(stale.http_status(), 422);
        let different = dispatch(
            &ServiceContext::default(),
            &request(Operation::ExaminerReview {
                lock: Box::new(lock),
                answer: answer + " changed",
                report: Box::new(report),
                review: Box::new(review),
            }),
        );
        assert_eq!(different.http_status(), 422);
    }

    #[test]
    fn readable_review_report_keeps_pending_grade_and_does_not_call_providers() {
        let (lock, answer, report) = examiner_fixture();
        let response = dispatch(
            &ServiceContext::default(),
            &request(Operation::ReviewReport {
                lock: Box::new(lock),
                answer,
                report: Box::new(report),
            }),
        );
        assert!(response.error.is_none());
        let result = response.result.unwrap();
        assert!(
            result["html"]
                .as_str()
                .unwrap()
                .to_ascii_lowercase()
                .starts_with("<!doctype html>")
        );
        assert!(result["markdown"].as_str().unwrap().contains("examiner"));
    }

    #[test]
    fn compile_and_prompt_share_the_library_contract() {
        let context = ServiceContext::default();
        let source = include_str!("../policies/apple.aha");
        let result = dispatch(
            &context,
            &request(Operation::Compile {
                source: source.into(),
            }),
        );
        assert!(result.error.is_none());
        let lock: LockedContract =
            serde_json::from_value(result.result.unwrap()["lock"].clone()).unwrap();
        assert_eq!(lock, contract::compile(source).unwrap());
        let result = dispatch(
            &context,
            &request(Operation::Prompt {
                lock: Box::new(lock),
                answer: "Red.".into(),
                evidence: None,
            }),
        );
        let data = result.result.unwrap();
        assert!(data["schema"].is_object());
        let user: Value = serde_json::from_str(data["user"].as_str().unwrap()).unwrap();
        assert_eq!(user["answer"], "Red.");
    }

    #[test]
    fn unknown_fields_and_credentials_are_rejected_as_data_not_configuration() {
        let context = ServiceContext::default();
        for payload in [
            json!({"operation":"compile","source":"x","api_key":"secret"}),
            json!({"operation":"compile","source":"x","path":"../../private"}),
            json!({"operation":"third_provider","source":"x"}),
        ] {
            let bytes = serde_json::to_vec(
                &json!({"api_version":API_VERSION,"request_id":"t","payload":payload}),
            )
            .unwrap();
            let result = handle_json(&context, &bytes);
            assert_eq!(result.error.unwrap().code, "invalid_request");
        }
        assert_eq!(
            handle_json(&context, &[255]).error.unwrap().code,
            "invalid_request"
        );
        assert_eq!(
            handle_json(&context, &vec![b' '; MAX_REQUEST_BYTES + 1]).http_status(),
            413
        );
    }

    #[test]
    fn live_calls_are_disabled_by_default_before_credentials_or_storage() {
        let lock = contract::compile(include_str!("../policies/apple.aha")).unwrap();
        let result = dispatch(
            &ServiceContext::default(),
            &request(Operation::Grade {
                lock: Box::new(lock),
                answer: "red".into(),
                evidence: None,
                config: RunConfig {
                    openai_model: "model-a".into(),
                    claude_model: "model-b".into(),
                    limits: providers::Limits::default(),
                },
            }),
        );
        assert_eq!(result.http_status(), 403);
        assert_eq!(result.error.unwrap().code, "live_disabled");
    }

    #[test]
    fn evidence_gate_cannot_be_bypassed_through_application_interface() {
        let (lock, input) = evidence::test_input();
        let mut dossier = input.dossier.clone();
        dossier.links.clear();
        let input = evidence::prepare(&lock, &input.policy, &dossier).unwrap();
        let reviews = evidence::test_review(&input);
        let pack = evidence::aggregate(&lock, &input, &reviews, &reviews).unwrap();
        let answer = "A demonstration answer.";
        let judgments = lock
            .contract
            .criteria
            .iter()
            .map(|c| grading::Row {
                criterion_id: c.id.clone(),
                anchor_id: c
                    .anchors
                    .iter()
                    .find(|a| a.points == 100)
                    .unwrap()
                    .id
                    .clone(),
                evidence: vec![answer.into()],
                reason: "Manual interface test, not a real judgment.".into(),
                conditions: vec![],
            })
            .collect();
        let judgment = Judgment {
            contract_hash: lock.contract_hash.clone(),
            answer_hash: contract::hash(answer.as_bytes()),
            judgments,
        };
        let result = dispatch(
            &ServiceContext::default(),
            &request(Operation::Evaluate {
                lock: Box::new(lock),
                answer: answer.into(),
                openai: Box::new(judgment.clone()),
                claude: Box::new(judgment),
                evidence: Some(Box::new(pack)),
            }),
        );
        assert!(result.error.is_none());
        let data = result.result.unwrap();
        assert_eq!(data["report"]["current"]["percent"], Value::Null);
        assert_eq!(
            data["report"]["criteria"][0]["status"],
            "evidence_unresolved"
        );
    }

    #[test]
    fn version_and_request_id_fail_closed() {
        let mut value = request(Operation::Compile { source: "x".into() });
        value.api_version = "future-api".into();
        assert_eq!(
            dispatch(&ServiceContext::default(), &value).http_status(),
            400
        );
        value.api_version = API_VERSION.into();
        value.request_id = "bad\nrequest".into();
        let result = dispatch(&ServiceContext::default(), &value);
        assert!(result.request_id.is_none());
        assert_eq!(result.http_status(), 400);
    }
}

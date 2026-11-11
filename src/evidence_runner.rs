//! Independent source-claim audits by the two configured providers.
//!
//! Each provider sees the same frozen input without the other's review. The first
//! returned response is preserved, including invalid reviews. A hash identifies
//! local inputs and outputs; it is not a provider signature or a truth guarantee.
use crate::{
    VERSION,
    contract::{LockedContract, hash},
    evidence::{self, EvidenceInput, EvidencePack, Review},
    providers::{self, Limits, Provider, Response},
    runner::{RunConfig, read_json, write_new},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

pub const PROMPT_VERSION: &str = evidence::PROMPT_VERSION;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Identity {
    format: String,
    engine_version: String,
    prompt_version: String,
    contract_hash: String,
    input_hash: String,
    config: RunConfig,
}

impl Identity {
    fn run_id(&self) -> Result<String, String> {
        Ok(hash(&serde_json::to_vec(self).map_err(|e| e.to_string())?))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Snapshot {
    identity: Identity,
    lock: LockedContract,
    input: EvidenceInput,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ArchivedResponse {
    provider: Provider,
    requested_model: String,
    response: Response,
}

struct Lease(PathBuf);

impl Drop for Lease {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

fn equal<T: Serialize>(left: &T, right: &T) -> Result<bool, String> {
    Ok(serde_json::to_value(left).map_err(|e| e.to_string())?
        == serde_json::to_value(right).map_err(|e| e.to_string())?)
}

fn identity(
    lock: &LockedContract,
    input: &EvidenceInput,
    config: &RunConfig,
) -> Result<Identity, String> {
    lock.verify()?;
    input.verify(lock)?;
    let (system, user) = evidence::build_prompt(input)?;
    let schema = evidence::response_schema(input);
    // Validate both requests before any transport, including model names/limits.
    for (provider, model) in [
        (Provider::Openai, &config.openai_model),
        (Provider::Claude, &config.claude_model),
    ] {
        providers::request_body(provider, model, &system, &user, &schema, &config.limits)?;
    }
    Ok(Identity {
        format: "percent-grading-evidence-run-v1".into(),
        engine_version: VERSION.into(),
        prompt_version: PROMPT_VERSION.into(),
        contract_hash: lock.contract_hash.clone(),
        input_hash: input.input_hash.clone(),
        config: config.clone(),
    })
}

fn parse_response(snapshot: &Snapshot, archive: &ArchivedResponse) -> Result<Review, String> {
    let limits = &snapshot.identity.config.limits;
    if serde_json::to_vec(&archive.response.raw)
        .map_err(|e| e.to_string())?
        .len()
        > limits.max_response_bytes
    {
        return Err("Archived evidence response exceeds its response size limit".into());
    }
    let extracted = providers::extract_text(archive.provider, &archive.response.raw)?;
    if extracted != archive.response.text
        || archive.response.attempts == 0
        || archive.response.attempts > limits.max_attempts
        || archive.response.usage
            != archive
                .response
                .raw
                .get("usage")
                .cloned()
                .unwrap_or(Value::Null)
    {
        return Err(
            "Archived evidence response text, usage or attempts differ from the provider response"
                .into(),
        );
    }
    let review: Review = serde_json::from_str(&archive.response.text).map_err(|e| {
        format!(
            "{} response is not a valid evidence review: {e}",
            archive.provider
        )
    })?;
    evidence::validate_review(&snapshot.input, &review)?;
    Ok(review)
}

fn archived(dir: &Path, snapshot: &Snapshot, provider: Provider) -> Result<Review, String> {
    let archive: ArchivedResponse = read_json(&dir.join(format!("{provider}.response.json")))?;
    let expected_model = match provider {
        Provider::Openai => &snapshot.identity.config.openai_model,
        Provider::Claude => &snapshot.identity.config.claude_model,
    };
    if archive.provider != provider || &archive.requested_model != expected_model {
        return Err("Archived evidence response provider/model mismatch".into());
    }
    parse_response(snapshot, &archive)
}

fn compute(dir: &Path, snapshot: &Snapshot) -> Result<EvidencePack, String> {
    let openai = archived(dir, snapshot, Provider::Openai)?;
    let claude = archived(dir, snapshot, Provider::Claude)?;
    evidence::aggregate(&snapshot.lock, &snapshot.input, &openai, &claude)
}

fn output(dir: &Path, id: &str, pack: &EvidencePack, cached: bool) -> Value {
    json!({
        "origin": "api_evidence_run",
        "run_id": id,
        "run_directory": dir,
        "cached": cached,
        "evidence_file": dir.join("evidence.json"),
        "pack_hash": pack.pack_hash,
        "interpretation": "Two provider reviews under a frozen evidence policy; agreement does not establish scientific truth."
    })
}

/// Verify archived responses and recompute the complete pack without API calls.
pub fn replay(dir: &Path) -> Result<Value, String> {
    let snapshot: Snapshot = read_json(&dir.join("input.json"))?;
    let mut expected = identity(&snapshot.lock, &snapshot.input, &snapshot.identity.config)?;
    // 0.5 adds experiment and application interfaces without changing the 0.4
    // evidence prompt, schema, or aggregation. Keep old audit identities intact.
    // The full comparison below still rejects any prompt or input mismatch.
    if snapshot.identity.engine_version == "0.4.0" {
        expected.engine_version = "0.4.0".into();
    }
    if !equal(&expected, &snapshot.identity)? {
        return Err("Evidence run identity does not match its inputs or engine version".into());
    }
    let id = expected.run_id()?;
    if dir.file_name().and_then(|s| s.to_str()) != Some(&id) {
        return Err("Evidence run directory must match its content-addressed run ID".into());
    }
    let pack = compute(dir, &snapshot)?;
    let saved: EvidencePack = read_json(&dir.join("evidence.json"))?;
    if !equal(&pack, &saved)? {
        return Err("Saved evidence pack differs from deterministic recomputation".into());
    }
    Ok(output(dir, &id, &pack, true))
}

/// Audit a previously frozen evidence input. Only this function calls AI APIs.
pub fn audit(
    lock: &LockedContract,
    input: &EvidenceInput,
    config: &RunConfig,
    runs: &Path,
) -> Result<Value, String> {
    let expected = identity(lock, input, config)?;
    let dir = runs.join(expected.run_id()?);
    if dir.join("evidence.json").exists() {
        return replay(&dir);
    }
    if input.dossier.links.is_empty() {
        return Err("No evidence links to audit; add linked sources before a paid audit, or use imported empty reviews to record insufficient evidence offline".into());
    }
    // Preflight all credentials needed by this run before the first billable call.
    // A preserved provider response is never requested again, even when invalid.
    for provider in [Provider::Openai, Provider::Claude] {
        if !dir.join(format!("{provider}.response.json")).exists() {
            providers::validate_credentials(provider)?;
        }
    }
    run_with(lock, input, config, runs, providers::judge)
}

fn run_with<F>(
    lock: &LockedContract,
    input: &EvidenceInput,
    config: &RunConfig,
    runs: &Path,
    mut judge: F,
) -> Result<Value, String>
where
    F: FnMut(Provider, &str, &str, &str, &Value, &Limits) -> Result<Response, String>,
{
    let identity = identity(lock, input, config)?;
    let id = identity.run_id()?;
    let dir = runs.join(&id);
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let lease_path = dir.join("running.lock");
    let mut lease = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&lease_path)
        .map_err(|_| {
            format!(
                "Evidence run is locked: {}. Inspect for another process before removing a stale lock after a crash.",
                dir.display()
            )
        })?;
    // Install the guard before writing so a write error also releases the lease.
    let _lease = Lease(lease_path);
    writeln!(lease, "process {}", std::process::id()).map_err(|e| e.to_string())?;
    drop(lease);
    if dir.join("evidence.json").exists() {
        return replay(&dir);
    }
    let snapshot = Snapshot {
        identity,
        lock: lock.clone(),
        input: input.clone(),
    };
    let input_path = dir.join("input.json");
    if input_path.exists() {
        let previous: Snapshot = read_json(&input_path)?;
        if !equal(&previous, &snapshot)? {
            return Err("Stored evidence run inputs differ from this request".into());
        }
    } else {
        write_new(&input_path, &snapshot)?;
    }
    // Validate every saved response before any new call, including an existing
    // second-provider response in an unexpectedly incomplete local directory.
    for provider in [Provider::Openai, Provider::Claude] {
        if dir.join(format!("{provider}.response.json")).exists() {
            archived(&dir, &snapshot, provider)?;
        }
    }
    let (system, user) = evidence::build_prompt(input)?;
    let schema = evidence::response_schema(input);
    for (provider, model) in [
        (Provider::Openai, &config.openai_model),
        (Provider::Claude, &config.claude_model),
    ] {
        let path = dir.join(format!("{provider}.response.json"));
        if path.exists() {
            continue;
        }
        let response = match judge(provider, model, &system, &user, &schema, &config.limits) {
            Ok(response) => response,
            Err(error) => {
                let event = format!(
                    "{}-{}.error.json",
                    provider,
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_nanos()
                );
                write_new(
                    &dir.join(event),
                    &json!({"provider": provider, "error": error}),
                )?;
                return Err(format!(
                    "{error}; incomplete evidence run saved at {}",
                    dir.display()
                ));
            }
        };
        let archive = ArchivedResponse {
            provider,
            requested_model: model.clone(),
            response,
        };
        write_new(&path, &archive)?;
        parse_response(&snapshot, &archive).map_err(|error| {
            format!(
                "{error}; first response preserved at {} and will not be rerolled",
                path.display()
            )
        })?;
    }
    let pack = compute(&dir, &snapshot)?;
    write_new(&dir.join("evidence.json"), &pack)?;
    Ok(output(&dir, &id, &pack, false))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evidence::LinkReview;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static NEXT: AtomicUsize = AtomicUsize::new(0);

    fn setup() -> (LockedContract, EvidenceInput, RunConfig, PathBuf) {
        let (lock, input) = evidence::test_input();
        let config = RunConfig {
            openai_model: "test-openai".into(),
            claude_model: "test-claude".into(),
            limits: Limits::default(),
        };
        let dir = std::env::temp_dir().join(format!(
            "percent-grading-evidence-test-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        (lock, input, config, dir)
    }

    fn clean(dir: &Path) {
        assert_eq!(dir.parent(), Some(std::env::temp_dir().as_path()));
        assert!(
            dir.file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("percent-grading-evidence-test-")
        );
        fs::remove_dir_all(dir).unwrap();
    }

    fn review(input: &EvidenceInput) -> Review {
        Review {
            input_hash: input.input_hash.clone(),
            reviews: input
                .dossier
                .links
                .iter()
                .map(|link| LinkReview {
                    link_id: link.id.clone(),
                    relation: "supports".into(),
                    applicable: true,
                    quality: "eligible".into(),
                    reason: "Manual mock review for offline transport testing only.".into(),
                })
                .collect(),
        }
    }

    fn response(review: &Review, provider: Provider) -> Response {
        let text = serde_json::to_string(review).unwrap();
        let raw = match provider {
            Provider::Openai => json!({
                "status": "completed",
                "output": [{"type": "message", "role": "assistant", "status": "completed",
                    "content": [{"type": "output_text", "text": text}]}],
                "usage": {"output_tokens": 100}
            }),
            Provider::Claude => json!({
                "type": "message", "role": "assistant", "stop_reason": "end_turn",
                "content": [{"type": "text", "text": text}],
                "usage": {"output_tokens": 100}
            }),
        };
        Response {
            text,
            usage: raw["usage"].clone(),
            raw,
            attempts: 1,
        }
    }

    #[test]
    fn independent_identical_inputs_are_frozen_and_replay_without_calls() {
        let (lock, input, config, dir) = setup();
        let expected = review(&input);
        let mut requests = Vec::new();
        let result = run_with(&lock, &input, &config, &dir, |p, _, system, user, _, _| {
            requests.push((system.to_owned(), user.to_owned()));
            Ok(response(&expected, p))
        })
        .unwrap();
        assert_eq!(requests.len(), 2);
        assert_eq!(requests[0], requests[1]);
        assert!(!requests[0].1.contains(&expected.reviews[0].reason));
        let cached = run_with(&lock, &input, &config, &dir, |_, _, _, _, _, _| {
            panic!("a completed evidence run must not call providers")
        })
        .unwrap();
        assert_eq!(result["pack_hash"], cached["pack_hash"]);
        assert_eq!(cached["cached"], true);
        let api_cached = audit(&lock, &input, &config, &dir).unwrap();
        assert_eq!(api_cached["pack_hash"], cached["pack_hash"]);
        clean(&dir);
    }

    #[test]
    fn version_04_audits_replay_without_rewriting_and_unknown_versions_fail() {
        let (lock, input, config, dir) = setup();
        let expected = review(&input);
        let result = run_with(&lock, &input, &config, &dir, |p, _, _, _, _, _| {
            Ok(response(&expected, p))
        })
        .unwrap();
        let current = PathBuf::from(result["run_directory"].as_str().unwrap());
        let mut legacy: Snapshot = read_json(&current.join("input.json")).unwrap();
        legacy.identity.engine_version = "0.4.0".into();
        let legacy_id = legacy.identity.run_id().unwrap();
        let legacy_dir = dir.join(&legacy_id);
        if legacy_dir != current {
            write_new(&legacy_dir.join("input.json"), &legacy).unwrap();
            for name in [
                "openai.response.json",
                "claude.response.json",
                "evidence.json",
            ] {
                fs::copy(current.join(name), legacy_dir.join(name)).unwrap();
            }
        }
        let before = fs::read(legacy_dir.join("input.json")).unwrap();
        let restored = replay(&legacy_dir).unwrap();
        assert_eq!(restored["cached"], true);
        assert_eq!(restored["run_id"], legacy_id);
        assert_eq!(restored["pack_hash"], result["pack_hash"]);
        assert_eq!(fs::read(legacy_dir.join("input.json")).unwrap(), before);

        let mut unknown = legacy.clone();
        unknown.identity.engine_version = "0.3.0".into();
        let unknown_dir = dir.join(unknown.identity.run_id().unwrap());
        write_new(&unknown_dir.join("input.json"), &unknown).unwrap();
        assert!(replay(&unknown_dir).unwrap_err().contains("engine version"));
        legacy.identity.prompt_version.push_str("-changed");
        let changed_prompt_dir = dir.join(legacy.identity.run_id().unwrap());
        write_new(&changed_prompt_dir.join("input.json"), &legacy).unwrap();
        assert!(
            replay(&changed_prompt_dir)
                .unwrap_err()
                .contains("identity")
        );
        clean(&dir);
    }

    #[test]
    fn interrupted_evidence_run_resumes_only_missing_provider() {
        let (lock, input, config, dir) = setup();
        let expected = review(&input);
        let first = run_with(&lock, &input, &config, &dir, |p, _, _, _, _, _| {
            if p == Provider::Claude {
                Err("simulated provider unavailable".into())
            } else {
                Ok(response(&expected, p))
            }
        });
        assert!(first.is_err());
        let mut calls = Vec::new();
        let result = run_with(&lock, &input, &config, &dir, |p, _, _, _, _, _| {
            calls.push(p);
            Ok(response(&expected, p))
        })
        .unwrap();
        assert_eq!(calls, vec![Provider::Claude]);
        assert!(Path::new(result["evidence_file"].as_str().unwrap()).is_file());
        clean(&dir);
    }

    #[test]
    fn incomplete_review_is_preserved_without_reroll_or_second_charge() {
        let (lock, input, config, dir) = setup();
        let mut invalid = review(&input);
        invalid.reviews.clear();
        let mut calls = 0;
        let first = run_with(&lock, &input, &config, &dir, |p, _, _, _, _, _| {
            calls += 1;
            Ok(response(&invalid, p))
        });
        assert_eq!(calls, 1);
        assert!(first.unwrap_err().contains("first response preserved"));
        let run = dir.join(identity(&lock, &input, &config).unwrap().run_id().unwrap());
        let path = run.join("openai.response.json");
        let before = fs::read(&path).unwrap();
        assert!(!run.join("claude.response.json").exists());
        assert!(
            run_with(&lock, &input, &config, &dir, |_, _, _, _, _, _| {
                panic!("an invalid preserved review must not trigger any more calls")
            })
            .is_err()
        );
        assert_eq!(before, fs::read(path).unwrap());
        clean(&dir);
    }

    #[test]
    fn replay_rejects_tampered_pack_and_response_metadata() {
        let (lock, input, config, dir) = setup();
        let expected = review(&input);
        let result = run_with(&lock, &input, &config, &dir, |p, _, _, _, _, _| {
            Ok(response(&expected, p))
        })
        .unwrap();
        let run = PathBuf::from(result["run_directory"].as_str().unwrap());
        let path = run.join("openai.response.json");
        let original = fs::read(&path).unwrap();
        let mut saved: ArchivedResponse = read_json(&path).unwrap();
        saved.response.usage = json!({"output_tokens": 0});
        fs::write(&path, serde_json::to_vec(&saved).unwrap()).unwrap();
        assert!(replay(&run).unwrap_err().contains("usage or attempts"));
        fs::write(&path, &original).unwrap();
        let mut saved: ArchivedResponse = read_json(&path).unwrap();
        saved.response.attempts = 0;
        fs::write(&path, serde_json::to_vec(&saved).unwrap()).unwrap();
        assert!(replay(&run).is_err());
        fs::write(&path, &original).unwrap();
        let pack_path = run.join("evidence.json");
        let mut pack: EvidencePack = read_json(&pack_path).unwrap();
        pack.pack_hash.push('0');
        fs::write(pack_path, serde_json::to_vec(&pack).unwrap()).unwrap();
        assert!(
            replay(&run)
                .unwrap_err()
                .contains("deterministic recomputation")
        );
        clean(&dir);
    }

    #[test]
    fn changed_models_create_new_audit_identity_and_limits_fail_before_calls() {
        let (lock, input, mut config, dir) = setup();
        let first = identity(&lock, &input, &config).unwrap().run_id().unwrap();
        config.claude_model = "another-claude-snapshot".into();
        assert_ne!(
            first,
            identity(&lock, &input, &config).unwrap().run_id().unwrap()
        );
        config.limits.max_attempts = 0;
        assert!(
            run_with(&lock, &input, &config, &dir, |_, _, _, _, _, _| {
                panic!("invalid limits must fail before transport")
            })
            .is_err()
        );
        assert!(!dir.exists());
    }

    #[test]
    fn running_audit_lease_prevents_duplicate_calls() {
        let (lock, input, config, dir) = setup();
        let run = dir.join(identity(&lock, &input, &config).unwrap().run_id().unwrap());
        fs::create_dir_all(&run).unwrap();
        fs::write(run.join("running.lock"), "other process").unwrap();
        let error = run_with(&lock, &input, &config, &dir, |_, _, _, _, _, _| {
            panic!("an existing lease must prevent transport")
        })
        .unwrap_err();
        assert!(error.contains("locked"));
        assert!(run.join("running.lock").exists());
        clean(&dir);
    }

    #[test]
    fn empty_links_reject_paid_audit_but_can_record_insufficiency_offline() {
        let (lock, input, config, dir) = setup();
        let mut dossier = input.dossier.clone();
        dossier.links.clear();
        let empty = evidence::prepare(&lock, &input.policy, &dossier).unwrap();
        let error = audit(&lock, &empty, &config, &dir).unwrap_err();
        assert!(error.contains("No evidence links to audit"));
        assert!(!dir.exists());
        let review = Review {
            input_hash: empty.input_hash.clone(),
            reviews: Vec::new(),
        };
        let pack = evidence::aggregate(&lock, &empty, &review, &review).unwrap();
        assert!(!pack.blocked_criteria.is_empty());
    }
}

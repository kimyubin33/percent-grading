//! Frozen, content-addressed runs. The first accepted response is never rerolled.
use crate::{
    VERSION,
    contract::{LockedContract, hash},
    grading::{self, Judgment, Report, Row},
    providers::{self, Limits, Provider, Response},
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use std::{
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};

const MAX_FILE_BYTES: u64 = 32 * 1024 * 1024;
pub const MAX_ANSWER_BYTES: usize = 64 * 1024;

pub fn read_text(path: &Path, limit: u64) -> Result<String, String> {
    let file = fs::File::open(path).map_err(|e| format!("Cannot open {}: {e}", path.display()))?;
    let mut bytes = Vec::new();
    file.take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > limit {
        return Err(format!("{} exceeds the file size limit", path.display()));
    }
    String::from_utf8(bytes).map_err(|_| "Input must be UTF-8".into())
}
pub fn read_json<T: DeserializeOwned>(path: &Path) -> Result<T, String> {
    let text = read_text(path, MAX_FILE_BYTES)?;
    serde_json::from_str(text.trim_start_matches('\u{feff}'))
        .map_err(|e| format!("Invalid JSON in {}: {e}", path.display()))
}
pub fn write_new<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let bytes = serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| {
            format!(
                "Cannot create {} (existing files are never replaced): {e}",
                path.display()
            )
        })?;
    file.write_all(&bytes)
        .and_then(|_| file.write_all(b"\n"))
        .and_then(|_| file.sync_all())
        .map_err(|e| e.to_string())
}
pub fn load_contract(path: &Path) -> Result<LockedContract, String> {
    let lock: LockedContract = read_json(path)?;
    lock.verify()?;
    Ok(lock)
}
pub fn validate_answer(answer: &str) -> Result<(), String> {
    if answer.trim().is_empty() || answer.len() > MAX_ANSWER_BYTES {
        return Err("Answer must be nonempty UTF-8 text of at most 65536 bytes".into());
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunConfig {
    pub openai_model: String,
    pub claude_model: String,
    pub limits: Limits,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Identity {
    pub format: String,
    pub engine_version: String,
    pub prompt_version: String,
    pub contract_hash: String,
    pub answer_hash: String,
    pub config: RunConfig,
}
impl Identity {
    pub fn run_id(&self) -> Result<String, String> {
        Ok(hash(&serde_json::to_vec(self).map_err(|e| e.to_string())?))
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Snapshot {
    identity: Identity,
    lock: LockedContract,
    answer: String,
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

fn identity(lock: &LockedContract, answer: &str, config: &RunConfig) -> Result<Identity, String> {
    lock.verify()?;
    validate_answer(answer)?;
    let ch = &lock.contract_hash;
    let ah = hash(answer.as_bytes());
    let user = grading::user_prompt(&lock.contract, ch, answer, &ah)?;
    let schema = grading::response_schema(&lock.contract);
    for (provider, model) in [
        (Provider::Openai, &config.openai_model),
        (Provider::Claude, &config.claude_model),
    ] {
        providers::request_body(
            provider,
            model,
            grading::system_prompt(),
            &user,
            &schema,
            &config.limits,
        )?;
    }
    Ok(Identity {
        format: "percent-grading-run-v1".into(),
        engine_version: VERSION.into(),
        prompt_version: grading::PROMPT_VERSION.into(),
        contract_hash: ch.clone(),
        answer_hash: ah,
        config: config.clone(),
    })
}
fn parse_response(snapshot: &Snapshot, archive: &ArchivedResponse) -> Result<Judgment, String> {
    let extracted = providers::extract_text(archive.provider, &archive.response.raw)?;
    if extracted != archive.response.text
        || archive.response.attempts == 0
        || archive.response.attempts > snapshot.identity.config.limits.max_attempts
        || archive.response.usage
            != archive
                .response
                .raw
                .get("usage")
                .cloned()
                .unwrap_or(Value::Null)
    {
        return Err(
            "Archived response text, usage or attempts are inconsistent with the provider response"
                .into(),
        );
    }
    let judgment: Judgment = serde_json::from_str(&archive.response.text)
        .map_err(|e| format!("{} response is not a valid judgment: {e}", archive.provider))?;
    grading::validate_judgment(
        &snapshot.lock.contract,
        &snapshot.identity.contract_hash,
        &snapshot.answer,
        &snapshot.identity.answer_hash,
        &judgment,
    )?;
    Ok(judgment)
}
fn archived(dir: &Path, snapshot: &Snapshot, provider: Provider) -> Result<Judgment, String> {
    let result: ArchivedResponse = read_json(&dir.join(format!("{provider}.response.json")))?;
    let model = match provider {
        Provider::Openai => &snapshot.identity.config.openai_model,
        Provider::Claude => &snapshot.identity.config.claude_model,
    };
    if result.provider != provider || &result.requested_model != model {
        return Err("Archived response provider/model mismatch".into());
    }
    parse_response(snapshot, &result)
}
fn compute(dir: &Path, snapshot: &Snapshot) -> Result<Report, String> {
    let openai = archived(dir, snapshot, Provider::Openai)?;
    let claude = archived(dir, snapshot, Provider::Claude)?;
    grading::aggregate(
        &snapshot.lock.contract,
        &snapshot.identity.contract_hash,
        &snapshot.answer,
        &snapshot.identity.answer_hash,
        &openai,
        &claude,
    )
}
fn output(dir: &Path, id: &str, report: Report, cached: bool) -> Value {
    json!({"origin":"api_run","run_id":id,"run_directory":dir,"cached":cached,"report":report})
}

pub fn replay(dir: &Path) -> Result<Value, String> {
    let snapshot: Snapshot = read_json(&dir.join("input.json"))?;
    let expected = identity(&snapshot.lock, &snapshot.answer, &snapshot.identity.config)?;
    if serde_json::to_value(&expected).unwrap() != serde_json::to_value(&snapshot.identity).unwrap()
    {
        return Err("Run identity does not match its inputs or this engine version".into());
    }
    let id = expected.run_id()?;
    if dir.file_name().and_then(|s| s.to_str()) != Some(&id) {
        return Err("Run directory name must match the content-addressed run ID".into());
    }
    let report = compute(dir, &snapshot)?;
    let saved: Report = read_json(&dir.join("report.json"))?;
    if saved != report {
        return Err("Saved report differs from deterministic recomputation".into());
    }
    Ok(output(dir, &id, report, true))
}

pub fn grade(
    lock: &LockedContract,
    answer: &str,
    config: &RunConfig,
    runs: &Path,
) -> Result<Value, String> {
    let id = identity(lock, answer, config)?.run_id()?;
    let dir = runs.join(id);
    // Check both required credentials before any billable work. Completed runs need none.
    if !dir.join("report.json").exists() {
        for provider in [Provider::Openai, Provider::Claude] {
            if !dir.join(format!("{provider}.response.json")).exists() {
                providers::validate_credentials(provider)?;
            }
        }
    }
    run_with(lock, answer, config, runs, providers::judge)
}

fn run_with<F>(
    lock: &LockedContract,
    answer: &str,
    config: &RunConfig,
    runs: &Path,
    mut judge: F,
) -> Result<Value, String>
where
    F: FnMut(Provider, &str, &str, &str, &Value, &Limits) -> Result<Response, String>,
{
    let identity = identity(lock, answer, config)?;
    let id = identity.run_id()?;
    let dir = runs.join(&id);
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let lease_path = dir.join("running.lock");
    let mut lease = OpenOptions::new().write(true).create_new(true).open(&lease_path).map_err(|_| format!("Run is locked: {}. Another process may be running; inspect before removing a stale lock after a crash.", dir.display()))?;
    writeln!(lease, "process {}", std::process::id()).map_err(|e| e.to_string())?;
    let _lease = Lease(lease_path);
    drop(lease);
    if dir.join("report.json").exists() {
        return replay(&dir);
    }
    let snapshot = Snapshot {
        identity,
        lock: lock.clone(),
        answer: answer.into(),
    };
    if dir.join("input.json").exists() {
        let previous: Snapshot = read_json(&dir.join("input.json"))?;
        if serde_json::to_value(&previous).unwrap() != serde_json::to_value(&snapshot).unwrap() {
            return Err("Stored run inputs differ from this request".into());
        }
    } else {
        write_new(&dir.join("input.json"), &snapshot)?;
    }
    let schema = grading::response_schema(&lock.contract);
    let user = grading::user_prompt(
        &lock.contract,
        &lock.contract_hash,
        answer,
        &snapshot.identity.answer_hash,
    )?;
    // Each judge sees the same data and never sees the other judge's response.
    for (provider, model) in [
        (Provider::Openai, &config.openai_model),
        (Provider::Claude, &config.claude_model),
    ] {
        let path = dir.join(format!("{provider}.response.json"));
        if path.exists() {
            archived(&dir, &snapshot, provider)?;
            continue;
        }
        let response = match judge(
            provider,
            model,
            grading::system_prompt(),
            &user,
            &schema,
            &config.limits,
        ) {
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
                    &json!({"provider":provider,"error":error}),
                )?;
                return Err(format!(
                    "{error}; incomplete run saved at {}",
                    dir.display()
                ));
            }
        };
        let archive = ArchivedResponse {
            provider,
            requested_model: model.clone(),
            response,
        };
        // Preserve even invalid returned judgments; rerunning never silently replaces them.
        write_new(&path, &archive)?;
        parse_response(&snapshot, &archive)?;
    }
    let report = compute(&dir, &snapshot)?;
    write_new(&dir.join("report.json"), &report)?;
    Ok(output(&dir, &id, report, false))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fixture {
    pub answer: String,
    pub openai: Vec<Row>,
    pub claude: Vec<Row>,
}
pub fn demo(lock: &LockedContract, fixture: &Fixture) -> Result<Value, String> {
    lock.verify()?;
    validate_answer(&fixture.answer)?;
    let ah = hash(fixture.answer.as_bytes());
    let openai = Judgment {
        contract_hash: lock.contract_hash.clone(),
        answer_hash: ah.clone(),
        judgments: fixture.openai.clone(),
    };
    let claude = Judgment {
        judgments: fixture.claude.clone(),
        ..openai.clone()
    };
    let report = grading::aggregate(
        &lock.contract,
        &lock.contract_hash,
        &fixture.answer,
        &ah,
        &openai,
        &claude,
    )?;
    Ok(json!({"origin":"manual_fixture","live_api_tested":false,"report":report}))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    fn setup() -> (LockedContract, Fixture, RunConfig, PathBuf) {
        let lock = crate::contract::compile(include_str!("../policies/apple.aha")).unwrap();
        let fixture: Fixture =
            serde_json::from_str(include_str!("../examples/rust/apple-red.fixture.json")).unwrap();
        let config = RunConfig {
            openai_model: "test-openai".into(),
            claude_model: "test-claude".into(),
            limits: Limits::default(),
        };
        let dir = std::env::temp_dir().join(format!(
            "percent-grading-test-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        (lock, fixture, config, dir)
    }
    fn response(lock: &LockedContract, fixture: &Fixture, provider: Provider) -> Response {
        let j = Judgment {
            contract_hash: lock.contract_hash.clone(),
            answer_hash: hash(fixture.answer.as_bytes()),
            judgments: match provider {
                Provider::Openai => fixture.openai.clone(),
                Provider::Claude => fixture.claude.clone(),
            },
        };
        let text = serde_json::to_string(&j).unwrap();
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
    fn frozen_run_replays_without_calls_and_detects_modified_report() {
        let (lock, fixture, config, dir) = setup();
        let mut calls = 0;
        let result = run_with(&lock, &fixture.answer, &config, &dir, |p, _, _, _, _, _| {
            calls += 1;
            Ok(response(&lock, &fixture, p))
        })
        .unwrap();
        assert_eq!(calls, 2);
        let cached = run_with(&lock, &fixture.answer, &config, &dir, |_, _, _, _, _, _| {
            panic!("cache must not call an API")
        })
        .unwrap();
        assert_eq!(result["report"], cached["report"]);
        assert_eq!(cached["cached"], true);
        let run = PathBuf::from(cached["run_directory"].as_str().unwrap());
        fs::write(run.join("report.json"), "{}").unwrap();
        assert!(replay(&run).is_err());
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn interrupted_run_resumes_only_missing_provider() {
        let (lock, fixture, config, dir) = setup();
        let first = run_with(&lock, &fixture.answer, &config, &dir, |p, _, _, _, _, _| {
            if p == Provider::Claude {
                Err("simulated unavailable".into())
            } else {
                Ok(response(&lock, &fixture, p))
            }
        });
        assert!(first.is_err());
        let mut calls = Vec::new();
        run_with(&lock, &fixture.answer, &config, &dir, |p, _, _, _, _, _| {
            calls.push(p);
            Ok(response(&lock, &fixture, p))
        })
        .unwrap();
        assert_eq!(calls, vec![Provider::Claude]);
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn content_changes_create_new_run_id() {
        let (lock, fixture, mut config, _) = setup();
        let a = identity(&lock, &fixture.answer, &config)
            .unwrap()
            .run_id()
            .unwrap();
        config.claude_model = "different-snapshot".into();
        assert_ne!(
            a,
            identity(&lock, &fixture.answer, &config)
                .unwrap()
                .run_id()
                .unwrap()
        );
        assert_ne!(
            a,
            identity(&lock, &format!("{} ", fixture.answer), &config)
                .unwrap()
                .run_id()
                .unwrap()
        );
    }
}

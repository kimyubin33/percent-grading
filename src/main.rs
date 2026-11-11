use percent_grading::{
    VERSION, benchmark, benchmark_gate, benchmark_runner, contract, discovery, evidence,
    evidence_runner, grading, proof,
    providers::Limits,
    research, revision,
    runner::{self, RunConfig},
    server, service,
};
use serde_json::{Value, json};
use std::{collections::BTreeMap, path::Path};

const HELP: &str = r#"Percent Grading: a checked contract and exactly two AI providers

  percent-grading compile POLICY.aha --out CONTRACT.json
  percent-grading inspect CONTRACT.json
  percent-grading discover CONTRACT.json --out SEARCH.json [--query TEXT] [--rows 10] [--timeout-seconds 15]
  percent-grading select CONTRACT.json CATALOG.json --out RESEARCH.json [--max-perspectives 8] [--max-context-bytes 16000]
  percent-grading inspect-research CONTRACT.json RESEARCH.json
  percent-grading evidence-policy CONTRACT.json POLICY.aha --out POLICY.json
  percent-grading evidence-prepare CONTRACT.json POLICY.json DOSSIER.json --out INPUT.json
  percent-grading evidence-prompt CONTRACT.json INPUT.json
  percent-grading evidence-import CONTRACT.json INPUT.json OPENAI_REVIEW.json CLAUDE_REVIEW.json --out EVIDENCE.json
  percent-grading inspect-evidence CONTRACT.json EVIDENCE.json
  percent-grading audit-evidence CONTRACT.json INPUT.json --openai-model MODEL --claude-model MODEL [API_OPTIONS]
  percent-grading replay-evidence RUN_DIRECTORY
  percent-grading evidence-compare CONTRACT.json OLD_EVIDENCE.json NEW_EVIDENCE.json
  percent-grading revise-evidence CONTRACT.json OLD_EVIDENCE.json NEW_EVIDENCE.json COHORT.json --reason TEXT --out PLAN.json
  percent-grading prompt CONTRACT.json ANSWER.txt [--research RESEARCH.json | --evidence EVIDENCE.json]
  percent-grading demo CONTRACT.json FIXTURE.json
  percent-grading evaluate CONTRACT.json ANSWER.txt OPENAI.json CLAUDE.json [--evidence EVIDENCE.json]
  percent-grading grade CONTRACT.json ANSWER.txt --openai-model MODEL --claude-model MODEL [--research RESEARCH.json | --evidence EVIDENCE.json] [API_OPTIONS]
  percent-grading replay RUN_DIRECTORY
  percent-grading proof-policy CONTRACT.json CHALLENGE.aha --out POLICY.json
  percent-grading proof-prompt CONTRACT.json POLICY.json ANSWER.txt
  percent-grading proof-evaluate CONTRACT.json POLICY.json ANSWER.txt CERTIFICATE.json OPENAI.json CLAUDE.json [--evidence EVIDENCE.json]
  percent-grading benchmark-validate SUITE.json
  percent-grading benchmark SUITE.json RESULTS.json --out REPORT.json
  percent-grading benchmark-gate SUITE.json RESULTS.json POLICY.json --out REPORT.json
  percent-grading benchmark-plan SUITE.json --openai-model MODEL --claude-model MODEL --max-api-calls N [--split development] [--repeats 1] [API_OPTIONS] --out PLAN.json
  percent-grading benchmark-run SUITE.json --openai-model MODEL --claude-model MODEL --max-api-calls N [--split development] [--repeats 1] [API_OPTIONS]
  percent-grading api REQUEST.json [--allow-live false] [--runs-dir local-runs/api]
  percent-grading serve [--bind 127.0.0.1:8787] [--allow-live false] [--runs-dir local-runs/api]

API_OPTIONS: --runs-dir DIR --max-output-tokens 4096 --max-attempts 1 --timeout-seconds 60
Grade and audit-evidence call AI APIs and incur provider usage. Set OPENAI_API_KEY and ANTHROPIC_API_KEY in the environment. Models must support structured JSON output. Both providers receive the same inputs independently.
Benchmark-run also calls the two providers within its declared attempt budget. Its expectations never enter judge prompts; repeated trials retain all outcomes. API grade operations can call providers only with --allow-live true. Serve requires a separate PERCENT_GRADING_API_TOKEN (at least 32 ASCII characters) and binds only to loopback. Browser frontends should call their own backend rather than exposing tokens.
Discover sends the explicit query (or contract question) to Crossref for bibliographic candidates, not validated facts. Evidence sources and search logs are supplied records; preparation does not fetch or certify them. Imported reviews and demo fixtures do not attest provider execution. Revision creates a pending plan for every declared cohort member, not a completed regrade. Existing files and saved judgments are never overwritten.
New official score profiles are capped at 99.9%; raw anchor points and coverage still use 0..100. Historical runs replay with their original score rules. Proof operations are offline and verify only a bounded propositional challenge, not arbitrary mathematical proofs. Their 10,000% celebration is separate from the official grade. Benchmark-gate writes its report and exits 3 when valid results fail the supplied gate; malformed inputs exit 2."#;

fn options(args: &[String], allowed: &[&str]) -> Result<BTreeMap<String, String>, String> {
    if !args.len().is_multiple_of(2) {
        return Err("Each option requires a value".into());
    }
    let mut out = BTreeMap::new();
    for pair in args.chunks(2) {
        if !allowed.contains(&pair[0].as_str())
            || out.insert(pair[0].clone(), pair[1].clone()).is_some()
        {
            return Err(format!("Unknown or duplicate option: {}", pair[0]));
        }
    }
    Ok(out)
}
fn required<'a>(options: &'a BTreeMap<String, String>, key: &str) -> Result<&'a str, String> {
    options
        .get(key)
        .map(String::as_str)
        .ok_or_else(|| format!("Missing option {key}"))
}
fn number<T: std::str::FromStr>(
    options: &BTreeMap<String, String>,
    key: &str,
    default: T,
) -> Result<T, String> {
    match options.get(key) {
        None => Ok(default),
        Some(value) => value.parse().map_err(|_| format!("{key}: invalid integer")),
    }
}
fn run_config(opts: &BTreeMap<String, String>) -> Result<RunConfig, String> {
    Ok(RunConfig {
        openai_model: required(opts, "--openai-model")?.into(),
        claude_model: required(opts, "--claude-model")?.into(),
        limits: Limits {
            timeout_seconds: number(opts, "--timeout-seconds", 60)?,
            max_output_tokens: number(opts, "--max-output-tokens", 4096)?,
            max_response_bytes: 2 * 1024 * 1024,
            max_attempts: number(opts, "--max-attempts", 1)?,
        },
    })
}
fn allow_live(opts: &BTreeMap<String, String>) -> Result<bool, String> {
    opts.get("--allow-live")
        .map(|v| {
            v.parse()
                .map_err(|_| "--allow-live must be true or false".into())
        })
        .unwrap_or(Ok(false))
}

fn run(args: &[String]) -> Result<Value, String> {
    match args.first().map(String::as_str) {
        Some("proof-policy") if args.len() >= 3 => {
            let opts = options(&args[3..], &["--out"])?;
            let lock = runner::load_contract(Path::new(&args[1]))?;
            let source = runner::read_text(Path::new(&args[2]), 32 * 1024)?;
            let policy = proof::compile_policy(&lock, &source)?;
            let target = required(&opts, "--out")?;
            runner::write_new(Path::new(target), &policy)?;
            Ok(json!({"status":"proof_policy_compiled","output":target,"policy":policy}))
        }
        Some("proof-prompt") if args.len() == 4 => {
            let lock = runner::load_contract(Path::new(&args[1]))?;
            let policy = runner::read_json(Path::new(&args[2]))?;
            let answer = runner::read_text(Path::new(&args[3]), runner::MAX_ANSWER_BYTES as u64)?;
            let prompt = proof::certificate_prompt(&lock, &policy, &answer)?;
            serde_json::to_value(prompt).map_err(|e| e.to_string())
        }
        Some("proof-evaluate") if args.len() >= 7 => {
            let opts = options(&args[7..], &["--evidence"])?;
            let lock = runner::load_contract(Path::new(&args[1]))?;
            let policy = runner::read_json(Path::new(&args[2]))?;
            let answer = runner::read_text(Path::new(&args[3]), runner::MAX_ANSWER_BYTES as u64)?;
            let certificate = runner::read_json(Path::new(&args[4]))?;
            let openai = runner::read_json(Path::new(&args[5]))?;
            let claude = runner::read_json(Path::new(&args[6]))?;
            let evidence: Option<evidence::EvidencePack> = opts
                .get("--evidence")
                .map(|p| runner::read_json(Path::new(p)))
                .transpose()?;
            let result = proof::evaluate(
                &lock,
                &policy,
                &answer,
                &certificate,
                &openai,
                &claude,
                evidence.as_ref(),
            )?;
            serde_json::to_value(result).map_err(|e| e.to_string())
        }
        Some("benchmark-gate") if args.len() >= 4 => {
            let opts = options(&args[4..], &["--out"])?;
            let suite = runner::read_json(Path::new(&args[1]))?;
            let results = runner::read_json(Path::new(&args[2]))?;
            let policy = runner::read_json(Path::new(&args[3]))?;
            let report = benchmark_gate::evaluate(&suite, &results, &policy)?;
            let target = required(&opts, "--out")?;
            runner::write_new(Path::new(target), &report)?;
            Ok(json!({"status":"benchmark_gate_reported","output":target,"gate":report}))
        }
        Some("benchmark-validate") if args.len() == 2 => {
            let suite: benchmark::BenchmarkSuite = runner::read_json(Path::new(&args[1]))?;
            benchmark::validate(&suite)?;
            Ok(
                json!({"status":"valid","suite_hash":benchmark::suite_hash(&suite)?,"tasks":suite.tasks.len(),"cases":suite.tasks.iter().map(|t| t.cases.len()).sum::<usize>(),"interpretation":"Authored expectations and task splits are structurally valid; this does not measure live-model accuracy."}),
            )
        }
        Some("benchmark") if args.len() >= 3 => {
            let opts = options(&args[3..], &["--out"])?;
            let suite = runner::read_json(Path::new(&args[1]))?;
            let results = runner::read_json(Path::new(&args[2]))?;
            let report = benchmark::evaluate(&suite, &results)?;
            let target = required(&opts, "--out")?;
            runner::write_new(Path::new(target), &report)?;
            Ok(
                json!({"status":"benchmark_reported","output":target,"report_hash":report.report_hash,"origin":report.origin,"split":report.split,"repeats":report.repeats,"metrics":report.metrics,"relation_metrics":report.relation_metrics,"interpretation":report.interpretation}),
            )
        }
        Some(command @ ("benchmark-plan" | "benchmark-run")) if args.len() >= 2 => {
            let mut allowed = vec![
                "--openai-model",
                "--claude-model",
                "--runs-dir",
                "--max-output-tokens",
                "--max-attempts",
                "--timeout-seconds",
                "--split",
                "--repeats",
                "--max-api-calls",
            ];
            if command == "benchmark-plan" {
                allowed.push("--out");
            }
            let opts = options(&args[2..], &allowed)?;
            let suite = runner::read_json(Path::new(&args[1]))?;
            required(&opts, "--max-api-calls")?;
            let config = run_config(&opts)?;
            let config_options = benchmark_runner::Options {
                split: opts
                    .get("--split")
                    .cloned()
                    .unwrap_or_else(|| "development".into()),
                repeats: number(&opts, "--repeats", 1)?,
                max_api_calls: number(&opts, "--max-api-calls", 0)?,
            };
            if command == "benchmark-plan" {
                let plan = benchmark_runner::prepare_plan(&suite, &config, &config_options)?;
                let target = required(&opts, "--out")?;
                runner::write_new(Path::new(target), &plan)?;
                Ok(json!({"status":"benchmark_planned","output":target,"plan":plan}))
            } else {
                benchmark_runner::run(
                    &suite,
                    &config,
                    &config_options,
                    Path::new(
                        opts.get("--runs-dir")
                            .map(String::as_str)
                            .unwrap_or("local-runs/benchmarks"),
                    ),
                )
            }
        }
        Some("api") if args.len() >= 2 => {
            let opts = options(&args[2..], &["--allow-live", "--runs-dir"])?;
            let body = runner::read_text(Path::new(&args[1]), 2 * 1024 * 1024)?;
            let context = service::ServiceContext {
                runs_dir: opts
                    .get("--runs-dir")
                    .map(Into::into)
                    .unwrap_or_else(|| "local-runs/api".into()),
                allow_live: allow_live(&opts)?,
            };
            serde_json::to_value(service::handle_json(&context, body.as_bytes()))
                .map_err(|e| e.to_string())
        }
        Some("serve") => {
            let opts = options(&args[1..], &["--bind", "--allow-live", "--runs-dir"])?;
            let bind = opts
                .get("--bind")
                .map(String::as_str)
                .unwrap_or("127.0.0.1:8787")
                .parse()
                .map_err(|_| "--bind must be an IP socket address")?;
            let token = std::env::var("PERCENT_GRADING_API_TOKEN")
                .map_err(|_| "Set PERCENT_GRADING_API_TOKEN for local API authentication")?;
            let context = service::ServiceContext {
                runs_dir: opts
                    .get("--runs-dir")
                    .map(Into::into)
                    .unwrap_or_else(|| "local-runs/api".into()),
                allow_live: allow_live(&opts)?,
            };
            server::serve(bind, context, token)?;
            Ok(json!({"status":"server_stopped"}))
        }
        Some("compile") if args.len() >= 2 => {
            let opts = options(&args[2..], &["--out"])?;
            let source = runner::read_text(Path::new(&args[1]), 256 * 1024)?;
            let locked = contract::compile(&source)?;
            let target = required(&opts, "--out")?;
            runner::write_new(Path::new(target), &locked)?;
            Ok(
                json!({"status":"compiled","contract_hash":locked.contract_hash,"output":target,"compiler_revision":locked.compiler_revision}),
            )
        }
        Some("inspect") if args.len() == 2 => Ok(serde_json::to_value(runner::load_contract(
            Path::new(&args[1]),
        )?)
        .map_err(|e| e.to_string())?),
        Some("discover") if args.len() >= 2 => {
            let opts = options(
                &args[2..],
                &["--out", "--query", "--rows", "--timeout-seconds"],
            )?;
            let lock = runner::load_contract(Path::new(&args[1]))?;
            let target = Path::new(required(&opts, "--out")?);
            if target.exists() {
                return Err("Discovery output already exists; choose a new path".into());
            }
            let config = discovery::SearchConfig {
                query: opts
                    .get("--query")
                    .cloned()
                    .unwrap_or(lock.contract.question),
                rows: number(&opts, "--rows", 10)?,
                timeout_seconds: number(&opts, "--timeout-seconds", 15)?,
            };
            let result = discovery::search(&config)?;
            runner::write_new(target, &result)?;
            Ok(
                json!({"status":"discovered","output":target,"candidates":result.items.len(),"bundle_hash":result.bundle_hash,"interpretation":"Bibliographic candidates only; read and assess the sources before curating a catalog."}),
            )
        }
        Some("select") if args.len() >= 3 => {
            let opts = options(
                &args[3..],
                &["--out", "--max-perspectives", "--max-context-bytes"],
            )?;
            let lock = runner::load_contract(Path::new(&args[1]))?;
            let catalog = runner::read_json(Path::new(&args[2]))?;
            let config = research::SelectionConfig {
                max_perspectives: number(&opts, "--max-perspectives", 8)?,
                max_context_bytes: number(&opts, "--max-context-bytes", 16000)?,
            };
            let pack = research::select(&lock, &catalog, &config)?;
            let target = required(&opts, "--out")?;
            runner::write_new(Path::new(target), &pack)?;
            Ok(
                json!({"status":"selected","output":target,"pack_hash":pack.pack_hash,"diagnostics":pack.diagnostics}),
            )
        }
        Some("inspect-research") if args.len() == 3 => {
            let lock = runner::load_contract(Path::new(&args[1]))?;
            let pack: research::ResearchPack = runner::read_json(Path::new(&args[2]))?;
            pack.verify(&lock)?;
            Ok(
                json!({"pack_hash":pack.pack_hash,"contract_hash":pack.contract_hash,"config":pack.config,"diagnostics":pack.diagnostics,"context":pack.context()}),
            )
        }
        Some("evidence-policy") if args.len() >= 3 => {
            let opts = options(&args[3..], &["--out"])?;
            let lock = runner::load_contract(Path::new(&args[1]))?;
            let source = runner::read_text(Path::new(&args[2]), 256 * 1024)?;
            let policy = evidence::compile_policy(&lock, &source)?;
            let target = required(&opts, "--out")?;
            runner::write_new(Path::new(target), &policy)?;
            Ok(
                json!({"status":"evidence_policy_compiled","output":target,"policy_hash":policy.policy_hash}),
            )
        }
        Some("evidence-prepare") if args.len() >= 4 => {
            let opts = options(&args[4..], &["--out"])?;
            let lock = runner::load_contract(Path::new(&args[1]))?;
            let policy = runner::read_json(Path::new(&args[2]))?;
            let dossier = runner::read_json(Path::new(&args[3]))?;
            let input = evidence::prepare(&lock, &policy, &dossier)?;
            let target = required(&opts, "--out")?;
            runner::write_new(Path::new(target), &input)?;
            Ok(
                json!({"status":"evidence_prepared","output":target,"input_hash":input.input_hash,"interpretation":"Structure and exact quotes checked; semantic evidence review remains required."}),
            )
        }
        Some("evidence-prompt") if args.len() == 3 => {
            let lock = runner::load_contract(Path::new(&args[1]))?;
            let input: evidence::EvidenceInput = runner::read_json(Path::new(&args[2]))?;
            input.verify(&lock)?;
            let (system, user) = evidence::build_prompt(&input)?;
            Ok(json!({"system":system,"user":user,"schema":evidence::response_schema(&input)}))
        }
        Some("evidence-import") if args.len() >= 5 => {
            let opts = options(&args[5..], &["--out"])?;
            let lock = runner::load_contract(Path::new(&args[1]))?;
            let input = runner::read_json(Path::new(&args[2]))?;
            let openai = runner::read_json(Path::new(&args[3]))?;
            let claude = runner::read_json(Path::new(&args[4]))?;
            let pack = evidence::aggregate(&lock, &input, &openai, &claude)?;
            let target = required(&opts, "--out")?;
            runner::write_new(Path::new(target), &pack)?;
            Ok(
                json!({"origin":"imported_reviews","output":target,"pack_hash":pack.pack_hash,"claims":pack.claims,"blocked_criteria":pack.blocked_criteria,"interpretation":"Imported reviews do not attest that either provider was called."}),
            )
        }
        Some("inspect-evidence") if args.len() == 3 => {
            let lock = runner::load_contract(Path::new(&args[1]))?;
            let pack: evidence::EvidencePack = runner::read_json(Path::new(&args[2]))?;
            pack.verify(&lock)?;
            Ok(
                json!({"pack_hash":pack.pack_hash,"claims":pack.claims,"blocked_criteria":pack.blocked_criteria,"context":pack.context()}),
            )
        }
        Some("audit-evidence") if args.len() >= 3 => {
            let opts = options(
                &args[3..],
                &[
                    "--openai-model",
                    "--claude-model",
                    "--runs-dir",
                    "--max-output-tokens",
                    "--max-attempts",
                    "--timeout-seconds",
                ],
            )?;
            let lock = runner::load_contract(Path::new(&args[1]))?;
            let input = runner::read_json(Path::new(&args[2]))?;
            evidence_runner::audit(
                &lock,
                &input,
                &run_config(&opts)?,
                Path::new(
                    opts.get("--runs-dir")
                        .map(String::as_str)
                        .unwrap_or("local-runs/evidence"),
                ),
            )
        }
        Some("replay-evidence") if args.len() == 2 => evidence_runner::replay(Path::new(&args[1])),
        Some("evidence-compare") if args.len() == 4 => {
            let lock = runner::load_contract(Path::new(&args[1]))?;
            let old = runner::read_json(Path::new(&args[2]))?;
            let new = runner::read_json(Path::new(&args[3]))?;
            revision::compare(&lock, &old, &new)
        }
        Some("revise-evidence") if args.len() >= 5 => {
            let opts = options(&args[5..], &["--reason", "--out"])?;
            let lock = runner::load_contract(Path::new(&args[1]))?;
            let old = runner::read_json(Path::new(&args[2]))?;
            let new = runner::read_json(Path::new(&args[3]))?;
            let cohort = runner::read_json(Path::new(&args[4]))?;
            let plan = revision::plan(&lock, &old, &new, &cohort, required(&opts, "--reason")?)?;
            let target = required(&opts, "--out")?;
            runner::write_new(Path::new(target), &plan)?;
            Ok(
                json!({"status":"reassessment_planned","output":target,"plan":plan,"interpretation":"Every declared cohort member is pending reassessment. No answer has been regraded and no API has been called."}),
            )
        }
        Some("prompt") if args.len() >= 3 => {
            let opts = options(&args[3..], &["--research", "--evidence"])?;
            let lock = runner::load_contract(Path::new(&args[1]))?;
            let answer = runner::read_text(Path::new(&args[2]), runner::MAX_ANSWER_BYTES as u64)?;
            let pack: Option<research::ResearchPack> = opts
                .get("--research")
                .map(|p| runner::read_json(Path::new(p)))
                .transpose()?;
            let evidence: Option<evidence::EvidencePack> = opts
                .get("--evidence")
                .map(|p| runner::read_json(Path::new(p)))
                .transpose()?;
            let (system, user) = runner::build_prompt_with_evidence(
                &lock,
                &answer,
                pack.as_ref(),
                evidence.as_ref(),
            )?;
            Ok(
                json!({"system":system,"user":user,"schema":grading::response_schema(&lock.contract)}),
            )
        }
        Some("demo") if args.len() == 3 => {
            let lock = runner::load_contract(Path::new(&args[1]))?;
            runner::demo(&lock, &runner::read_json(Path::new(&args[2]))?)
        }
        Some("evaluate") if args.len() >= 5 => {
            let opts = options(&args[5..], &["--evidence"])?;
            let lock = runner::load_contract(Path::new(&args[1]))?;
            let answer = runner::read_text(Path::new(&args[2]), runner::MAX_ANSWER_BYTES as u64)?;
            runner::validate_answer(&answer)?;
            let openai = runner::read_json(Path::new(&args[3]))?;
            let claude = runner::read_json(Path::new(&args[4]))?;
            let mut report = grading::aggregate(
                &lock.contract,
                &lock.contract_hash,
                &answer,
                &contract::hash(answer.as_bytes()),
                &openai,
                &claude,
            )?;
            let pack: Option<evidence::EvidencePack> = opts
                .get("--evidence")
                .map(|p| runner::read_json(Path::new(p)))
                .transpose()?;
            if let Some(pack) = &pack {
                pack.verify(&lock)?;
                grading::apply_evidence_gate(&mut report, &pack.blocked_criteria)?;
            }
            Ok(
                json!({"origin":"imported_judgments","evidence_hash":pack.as_ref().map(|p| &p.pack_hash),"evidence_assessments":pack.as_ref().map(|p| &p.claims),"report":report}),
            )
        }
        Some("grade") if args.len() >= 3 => {
            let opts = options(
                &args[3..],
                &[
                    "--openai-model",
                    "--claude-model",
                    "--runs-dir",
                    "--max-output-tokens",
                    "--max-attempts",
                    "--timeout-seconds",
                    "--research",
                    "--evidence",
                ],
            )?;
            let lock = runner::load_contract(Path::new(&args[1]))?;
            let answer = runner::read_text(Path::new(&args[2]), runner::MAX_ANSWER_BYTES as u64)?;
            let config = run_config(&opts)?;
            let pack: Option<research::ResearchPack> = opts
                .get("--research")
                .map(|p| runner::read_json(Path::new(p)))
                .transpose()?;
            let evidence: Option<evidence::EvidencePack> = opts
                .get("--evidence")
                .map(|p| runner::read_json(Path::new(p)))
                .transpose()?;
            runner::grade_with_evidence(
                &lock,
                &answer,
                &config,
                Path::new(
                    opts.get("--runs-dir")
                        .map(String::as_str)
                        .unwrap_or("local-runs"),
                ),
                pack.as_ref(),
                evidence.as_ref(),
            )
        }
        Some("replay") if args.len() == 2 => runner::replay(Path::new(&args[1])),
        _ => Err(format!("Invalid command or arguments.\n\n{HELP}")),
    }
}
fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.is_empty() || args == ["--help"] || args == ["help"] {
        println!("{HELP}");
        return;
    }
    if args == ["--version"] {
        println!("percent-grading {VERSION}");
        return;
    }
    match run(&args) {
        Ok(value) => {
            println!(
                "{}",
                serde_json::to_string_pretty(&value).expect("serializable result")
            );
            if args.first().map(String::as_str) == Some("api")
                && value.get("error").is_some_and(|e| !e.is_null())
            {
                std::process::exit(2);
            }
            if args.first().map(String::as_str) == Some("benchmark-gate")
                && value["gate"]["passed"] == false
            {
                std::process::exit(3);
            }
        }
        Err(error) => {
            eprintln!("error: {error}");
            std::process::exit(2);
        }
    }
}

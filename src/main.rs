use percent_grading::{
    VERSION, contract, discovery, grading,
    providers::Limits,
    research,
    runner::{self, RunConfig},
};
use serde_json::{Value, json};
use std::{collections::BTreeMap, path::Path};

const HELP: &str = "Percent Grading: a checked contract and exactly two AI providers\n\n  percent-grading compile POLICY.aha --out CONTRACT.json\n  percent-grading inspect CONTRACT.json\n  percent-grading discover CONTRACT.json --out SEARCH.json [--query TEXT] [--rows 10] [--timeout-seconds 15]\n  percent-grading select CONTRACT.json CATALOG.json --out RESEARCH.json [--max-perspectives 8] [--max-context-bytes 16000]\n  percent-grading inspect-research CONTRACT.json RESEARCH.json\n  percent-grading prompt CONTRACT.json ANSWER.txt [--research RESEARCH.json]\n  percent-grading demo CONTRACT.json FIXTURE.json\n  percent-grading evaluate CONTRACT.json ANSWER.txt OPENAI.json CLAUDE.json\n  percent-grading grade CONTRACT.json ANSWER.txt --openai-model MODEL --claude-model MODEL [--research RESEARCH.json] [--runs-dir local-runs] [--max-output-tokens 4096] [--max-attempts 1] [--timeout-seconds 60]\n  percent-grading replay RUN_DIRECTORY\n\nOnly grade calls AI APIs and incurs provider usage. Discover sends the explicit query (or contract question) to Crossref for bibliographic candidates, not validated facts. Select and prompt work offline. Set OPENAI_API_KEY and ANTHROPIC_API_KEY in the environment. Models must support structured JSON output. Demo uses manual judgments; it is not an AI benchmark. Existing files and saved judgments are never overwritten.";

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
fn run(args: &[String]) -> Result<Value, String> {
    match args.first().map(String::as_str) {
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
        Some("prompt") if args.len() >= 3 => {
            let opts = options(&args[3..], &["--research"])?;
            let lock = runner::load_contract(Path::new(&args[1]))?;
            let answer = runner::read_text(Path::new(&args[2]), runner::MAX_ANSWER_BYTES as u64)?;
            let pack: Option<research::ResearchPack> = opts
                .get("--research")
                .map(|p| runner::read_json(Path::new(p)))
                .transpose()?;
            let (system, user) = runner::build_prompt(&lock, &answer, pack.as_ref())?;
            Ok(
                json!({"system":system,"user":user,"schema":grading::response_schema(&lock.contract)}),
            )
        }
        Some("demo") if args.len() == 3 => {
            let lock = runner::load_contract(Path::new(&args[1]))?;
            runner::demo(&lock, &runner::read_json(Path::new(&args[2]))?)
        }
        Some("evaluate") if args.len() == 5 => {
            let lock = runner::load_contract(Path::new(&args[1]))?;
            let answer = runner::read_text(Path::new(&args[2]), runner::MAX_ANSWER_BYTES as u64)?;
            runner::validate_answer(&answer)?;
            let openai = runner::read_json(Path::new(&args[3]))?;
            let claude = runner::read_json(Path::new(&args[4]))?;
            let report = grading::aggregate(
                &lock.contract,
                &lock.contract_hash,
                &answer,
                &contract::hash(answer.as_bytes()),
                &openai,
                &claude,
            )?;
            Ok(json!({"origin":"imported_judgments","report":report}))
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
                ],
            )?;
            let lock = runner::load_contract(Path::new(&args[1]))?;
            let answer = runner::read_text(Path::new(&args[2]), runner::MAX_ANSWER_BYTES as u64)?;
            let config = RunConfig {
                openai_model: required(&opts, "--openai-model")?.into(),
                claude_model: required(&opts, "--claude-model")?.into(),
                limits: Limits {
                    timeout_seconds: number(&opts, "--timeout-seconds", 60)?,
                    max_output_tokens: number(&opts, "--max-output-tokens", 4096)?,
                    max_response_bytes: 2 * 1024 * 1024,
                    max_attempts: number(&opts, "--max-attempts", 1)?,
                },
            };
            let pack: Option<research::ResearchPack> = opts
                .get("--research")
                .map(|p| runner::read_json(Path::new(p)))
                .transpose()?;
            runner::grade_with_research(
                &lock,
                &answer,
                &config,
                Path::new(
                    opts.get("--runs-dir")
                        .map(String::as_str)
                        .unwrap_or("local-runs"),
                ),
                pack.as_ref(),
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
        Ok(value) => println!(
            "{}",
            serde_json::to_string_pretty(&value).expect("serializable result")
        ),
        Err(error) => {
            eprintln!("error: {error}");
            std::process::exit(2);
        }
    }
}

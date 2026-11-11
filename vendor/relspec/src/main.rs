use std::{env, fs, io, path::Path, process::ExitCode};

mod lsp;
mod report;

const USAGE: &str = "AHAA 0.14\n사용법: aha 검사 <파일.aha> | aha 컴파일 <파일.aha> | aha 그래프 <파일.aha> [-] | aha lsp";

fn main() -> ExitCode {
    let args: Vec<_> = env::args_os().skip(1).collect();
    // 언어 서버: 편집기가 실행합니다. 표준 입출력으로 LSP 메시지를 주고받습니다.
    // VS Code는 `aha lsp --stdio`처럼 통신 방식을 알리는 인자를 덧붙이므로, 첫 인자만 봅니다.
    if matches!(
        args.first().and_then(|arg| arg.to_str()),
        Some("lsp" | "서버")
    ) {
        return lsp::run();
    }
    // `aha 그래프 파일 -`은 HTML을 파일로 쓰지 않고 표준 출력으로 냅니다(편집기가 씁니다).
    let to_stdout = args.len() == 3 && args[2] == "-";
    let [command, filename] = &args[..args.len().min(if to_stdout { 2 } else { 3 })] else {
        if args.len() == 1 && (args[0] == "--help" || args[0] == "-h") {
            println!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    };

    let graph = matches!(command.to_str(), Some("그래프" | "graph"));
    let json = match command.to_str() {
        Some("검사" | "check") => false,
        Some("컴파일" | "compile") => true,
        Some("그래프" | "graph") => false,
        _ => {
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
    };
    let path = Path::new(filename);
    let source = match fs::read_to_string(path) {
        Ok(source) => source,
        Err(error) => {
            if error.kind() == io::ErrorKind::InvalidData {
                eprintln!("{}: UTF-8로 저장된 .aha 파일을 입력하세요.", path.display());
            } else {
                eprintln!("{}: {error}", path.display());
            }
            return ExitCode::from(2);
        }
    };
    let program = match aha::compile(&source) {
        Ok(program) => program,
        Err(diagnostic) => {
            eprintln!("{}{diagnostic}", path.display());
            return ExitCode::from(1);
        }
    };

    if graph {
        let title = path
            .file_name()
            .map_or_else(String::new, |name| name.to_string_lossy().into_owned());
        let html = aha::graph::html(&aha::graph::build(&program), &title);
        if to_stdout {
            println!("{html}");
            return ExitCode::SUCCESS;
        }
        let output = path.with_extension("그래프.html");
        return match fs::write(&output, html) {
            Ok(()) => {
                let built = aha::graph::build(&program);
                let groups = built.components();
                println!(
                    "연결망 | 점 {}개, 선 {}개, 덩어리 {}개",
                    built.nodes.len(),
                    built.edges.len(),
                    groups.len()
                );
                // 덩어리가 여럿이면 가장 큰 것 말고는 무엇이 떨어져 있는지 알려 줍니다.
                for group in groups.iter().skip(1) {
                    let names: Vec<_> = group
                        .iter()
                        .take(4)
                        .map(|&i| format!("{} '{}'", built.nodes[i].kind, built.nodes[i].label))
                        .collect();
                    let more = if group.len() > 4 {
                        format!(" 외 {}개", group.len() - 4)
                    } else {
                        String::new()
                    };
                    println!("떨어진 덩어리: {}{more}", names.join(", "));
                }
                println!("{}", output.display());
                println!("위 파일을 브라우저로 열면 연결망이 보입니다.");
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("{}: {error}", output.display());
                ExitCode::from(2)
            }
        };
    }
    if json {
        match serde_json::to_string_pretty(&program) {
            Ok(output) => println!("{output}"),
            Err(error) => {
                eprintln!("JSON 출력 오류: {error}");
                return ExitCode::from(2);
            }
        }
    } else {
        println!("{}", report::summary(&program));
        for finding in report::findings(&program) {
            println!("{}", finding.text);
        }
        // 본문 점검의 알림. 컴파일은 통과한 것이므로 종료 코드는 0입니다.
        for warning in &program.warnings {
            println!(
                "{}({},{}): warning {}: {}",
                path.display(),
                warning.line,
                warning.column,
                warning.code,
                warning.message
            );
        }
    }

    ExitCode::SUCCESS
}

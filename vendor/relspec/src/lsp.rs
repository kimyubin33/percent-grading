//! 언어 서버: 편집기와 Language Server Protocol(LSP)로 대화합니다.
//!
//! 표준 입출력으로 JSON-RPC 메시지를 주고받습니다. 문서를 열거나 고칠 때마다 컴파일해서
//! 오류, 그리고 통과했다면 목표의 판정·미지값의 범위·규칙의 정도를 해당 줄의 진단으로 보냅니다.
//! 위치는 LSP의 기본값인 UTF-16 단위로 세며, 컴파일러의 열 번호도 같은 단위입니다.

use std::collections::HashMap;
use std::io::{self, BufRead, Write};
use std::process::ExitCode;

use serde_json::{Value, json};

use crate::report::{self, Level};

const SEVERITY_ERROR: u8 = 1;
const SEVERITY_WARNING: u8 = 2;
const SEVERITY_INFORMATION: u8 = 3;

/// 문장을 시작하는 낱말과 그 설명.
const STATEMENTS: &[(&str, &str)] = &[
    ("정의", "정의 이름 = 식 | 정의 이름: 타입 = 식"),
    ("검사", "검사 논리식 | 검사 이름 = 논리식"),
    ("기준", "기준 이름: 시간축 | 위치축"),
    ("대상", "대상 이름, 이름, ..."),
    ("단위", "단위 이름, 이름, ...  (예: 단위 원, 명)"),
    ("사건", "사건 이름: 역할 = 값, ..."),
    ("미지", "미지 이름, ...: 타입 | 미지 이름: 시각 = 기준 @ ?"),
    ("조건", "조건 논리식 | 조건 이름 = 논리식"),
    ("목표", "목표 논리식 | 목표 이름 = 논리식"),
    ("관측", "관측 미지이름 = 값"),
    ("행위", "행위 기본형, ...: 필수역할, ..."),
    ("규칙", "규칙 이름: 전제 이면 강도 [최소로|곱으로]"),
    ("참조", "참조 번호: 본문"),
    ("주제", "주제 <이 파일이 무엇을 적는지>"),
    ("낱말", "낱말 이름: 차원, 차원*, 차원(타입) 가중치, ..."),
    ("장면", "장면 이름  (다음 줄부터 들여 써서 낱말을 적습니다)"),
    ("본문", "본문: 글  (선언한 이름을 알아보고 점검합니다)"),
];
const TYPES: &[&str] = &["정수", "논리", "시간", "거리", "시각", "위치", "정도"];
const WORDS: &[(&str, &str)] = &[
    ("시간축", "기준의 종류"),
    ("위치축", "기준의 종류"),
    ("참", "논리"),
    ("거짓", "논리"),
    ("그리고", "논리 연산"),
    ("또는", "논리 연산"),
    ("아님", "논리 연산"),
    ("이면", "규칙의 전제와 강도 사이"),
    ("최소로", "규칙의 계산 방식: min(전제, 강도)"),
    ("곱으로", "규칙의 계산 방식: 전제 × 강도"),
    ("밀리초", "시간 단위"),
    ("초", "시간 단위 (1000 밀리초)"),
    ("밀리미터", "거리 단위"),
    ("센티미터", "거리 단위 (10 밀리미터)"),
    ("미터", "거리 단위 (1000 밀리미터)"),
];

/// 이름 뒤에 붙는 조사. 긴 것부터 떼어 봅니다.
const PARTICLES: &[&str] = &[
    "에게", "한테", "에서", "부터", "까지", "으로", "이", "가", "은", "는", "을", "를", "에", "로",
];

// LSP SymbolKind
const SYMBOL_NAMESPACE: u8 = 3;
const SYMBOL_FUNCTION: u8 = 12;
const SYMBOL_VARIABLE: u8 = 13;
const SYMBOL_CONSTANT: u8 = 14;
const SYMBOL_STRING: u8 = 15;
const SYMBOL_BOOLEAN: u8 = 17;
const SYMBOL_OBJECT: u8 = 19;
const SYMBOL_EVENT: u8 = 24;

// LSP CompletionItemKind
const KIND_UNIT: u8 = 11;
const KIND_VARIABLE: u8 = 6;
const KIND_KEYWORD: u8 = 14;
const KIND_TYPE: u8 = 22;

pub(crate) fn run() -> ExitCode {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut input = stdin.lock();
    let mut output = stdout.lock();
    let mut server = Server::default();

    loop {
        let message = match read_message(&mut input) {
            Ok(Some(message)) => message,
            // 입력이 닫히면 끝냅니다. 종료 요청 없이 닫혔다면 비정상 종료입니다.
            Ok(None) => return exit_code(server.shutdown),
            Err(error) => {
                eprintln!("aha lsp: {error}");
                return ExitCode::from(1);
            }
        };
        let Ok(message) = serde_json::from_slice::<Value>(&message) else {
            let reply = json!({
                "jsonrpc": "2.0",
                "id": null,
                "error": { "code": -32700, "message": "JSON을 읽을 수 없습니다." },
            });
            if write_message(&mut output, &reply).is_err() {
                return ExitCode::from(1);
            }
            continue;
        };
        if message["method"] == "exit" {
            return exit_code(server.shutdown);
        }
        for reply in server.handle(&message) {
            if write_message(&mut output, &reply).is_err() {
                return ExitCode::from(1);
            }
        }
    }
}

fn exit_code(shutdown: bool) -> ExitCode {
    if shutdown {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}

/// `Content-Length` 머리말과 본문으로 된 메시지 하나를 읽습니다. 입력이 끝났으면 `None`.
fn read_message(input: &mut impl BufRead) -> io::Result<Option<Vec<u8>>> {
    let mut length = None;
    loop {
        let mut line = String::new();
        if input.read_line(&mut line)? == 0 {
            return Ok(None);
        }
        let line = line.trim_end();
        if line.is_empty() {
            break;
        }
        if let Some((name, value)) = line.split_once(':')
            && name.eq_ignore_ascii_case("content-length")
        {
            length = value.trim().parse::<usize>().ok();
        }
    }
    let Some(length) = length else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Content-Length 머리말이 없습니다.",
        ));
    };
    let mut body = vec![0; length];
    input.read_exact(&mut body)?;
    Ok(Some(body))
}

fn write_message(output: &mut impl Write, message: &Value) -> io::Result<()> {
    let body = message.to_string();
    write!(output, "Content-Length: {}\r\n\r\n{body}", body.len())?;
    output.flush()
}

/// 열려 있는 문서 하나.
#[derive(Default)]
struct Document {
    text: String,
    /// 마지막으로 컴파일에 성공했을 때의 설명. 오류가 있는 동안에도 마우스를 올리면 보여 줍니다.
    info: Info,
}

/// 컴파일 결과에서 뽑은, 이름과 줄에 대한 설명.
#[derive(Default)]
struct Info {
    /// 이름 → 설명(마크다운)
    names: HashMap<String, String>,
    /// 문장에 쓰인 행위의 꼴 → 기본형 (`떨어졌다` → `떨어지다`)
    forms: HashMap<String, String>,
    /// 줄(1부터) → 그 줄의 문장에서 (낱말, 역할)
    roles: HashMap<usize, Vec<(String, String)>>,
    /// 본문에서 선언한 이름이 나온 자리: (행, 시작, 길이). 0부터, UTF-16.
    mentions: Vec<(usize, usize, usize)>,
}

#[derive(Default)]
struct Server {
    shutdown: bool,
    documents: HashMap<String, Document>,
}

impl Server {
    /// 메시지 하나를 처리하고, 보낼 메시지들을 돌려줍니다.
    fn handle(&mut self, message: &Value) -> Vec<Value> {
        let id = message.get("id").cloned();
        let method = message["method"].as_str().unwrap_or_default();
        let params = &message["params"];
        let respond = |result: Value| match &id {
            Some(id) => vec![json!({ "jsonrpc": "2.0", "id": id, "result": result })],
            None => Vec::new(),
        };
        let uri = params["textDocument"]["uri"].as_str().unwrap_or_default();
        let row = params["position"]["line"].as_u64().unwrap_or(0) as usize;
        let character = params["position"]["character"].as_u64().unwrap_or(0) as usize;

        match method {
            "initialize" => respond(json!({
                "capabilities": {
                    // 1 = 문서 전체를 받습니다. .aha 파일은 작아서 충분합니다.
                    "textDocumentSync": 1,
                    "completionProvider": {},
                    "hoverProvider": true,
                    "definitionProvider": true,
                    "documentSymbolProvider": true,
                    // 본문 속의 선언한 이름을 색칠합니다.
                    "semanticTokensProvider": {
                        "legend": { "tokenTypes": ["variable"], "tokenModifiers": [] },
                        "full": true,
                    },
                },
                "serverInfo": { "name": "aha", "version": aha::LANGUAGE_VERSION },
            })),
            "shutdown" => {
                self.shutdown = true;
                respond(Value::Null)
            }
            "textDocument/didOpen" => {
                let document = &params["textDocument"];
                self.publish(document["uri"].as_str(), document["text"].as_str())
            }
            "textDocument/didChange" => {
                // 전체 동기화이므로 마지막 변경의 text가 문서 전체입니다.
                let text = params["contentChanges"]
                    .as_array()
                    .and_then(|changes| changes.last())
                    .and_then(|change| change["text"].as_str());
                self.publish(params["textDocument"]["uri"].as_str(), text)
            }
            "textDocument/didClose" => {
                if uri.is_empty() {
                    return Vec::new();
                }
                self.documents.remove(uri);
                vec![notification(uri, Vec::new())]
            }
            "textDocument/completion" => respond(Value::Array(self.completions(uri))),
            "textDocument/hover" => respond(self.hover(uri, row, character)),
            "textDocument/definition" => respond(self.definition(uri, row, character)),
            "textDocument/documentSymbol" => respond(self.symbols(uri)),
            "textDocument/semanticTokens/full" => respond(self.semantic_tokens(uri)),
            // 알림(id 없음)은 모르는 것이어도 조용히 넘깁니다.
            _ => match id {
                Some(id) => vec![json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "error": { "code": -32601, "message": format!("지원하지 않는 요청입니다: {method}") },
                })],
                None => Vec::new(),
            },
        }
    }

    fn publish(&mut self, uri: Option<&str>, text: Option<&str>) -> Vec<Value> {
        let (Some(uri), Some(text)) = (uri, text) else {
            return Vec::new();
        };
        let (diagnostics, info) = analyze(text);
        let document = self.documents.entry(uri.to_string()).or_default();
        document.text = text.to_string();
        if let Some(info) = info {
            document.info = info;
        }
        vec![notification(uri, diagnostics)]
    }

    fn completions(&self, uri: &str) -> Vec<Value> {
        let item = |label: &str, kind: u8, detail: &str| json!({ "label": label, "kind": kind, "detail": detail });
        let mut items: Vec<Value> = STATEMENTS
            .iter()
            .map(|(word, form)| item(word, KIND_KEYWORD, form))
            .collect();
        items.extend(TYPES.iter().map(|word| item(word, KIND_TYPE, "타입")));
        items.extend(WORDS.iter().map(|(word, what)| {
            let kind = if what.contains("단위") {
                KIND_UNIT
            } else {
                KIND_KEYWORD
            };
            item(word, kind, what)
        }));
        // 선언된 이름은 지금 문서에서 바로 읽으므로, 오류가 있는 동안에도 최신입니다.
        if let Some(document) = self.documents.get(uri) {
            let mut seen = Vec::new();
            for symbol in outline(&document.text) {
                if symbol.keyword != "참조" && !seen.contains(&symbol.name) {
                    items.push(item(
                        &symbol.name,
                        KIND_VARIABLE,
                        &format!("{} ({}행)", symbol.keyword, symbol.row + 1),
                    ));
                    seen.push(symbol.name);
                }
            }
        }
        items
    }

    fn hover(&self, uri: &str, row: usize, character: usize) -> Value {
        let Some(document) = self.documents.get(uri) else {
            return Value::Null;
        };
        let Some(line) = document.text.lines().nth(row) else {
            return Value::Null;
        };
        let Some((start, word)) = word_at(line, character) else {
            return Value::Null;
        };
        let info = &document.info;
        let mut parts: Vec<String> = Vec::new();

        // 문장 안의 낱말이면 맡은 역할을 먼저 알려 줍니다.
        if let Some(roles) = info.roles.get(&(row + 1))
            && let Some((_, role)) = roles.iter().find(|(w, _)| w == word)
        {
            parts.push(format!("역할: **{role}**"));
        }
        if let Some(base) = info.forms.get(word) {
            parts.push(format!("`{word}` → 행위 `{base}`"));
            if let Some(text) = info.names.get(base) {
                parts.push(text.clone());
            }
        }
        let name = resolve(word, |candidate| info.names.contains_key(candidate));
        if let Some(text) = name.and_then(|name| info.names.get(name)) {
            parts.push(text.clone());
        }
        if parts.is_empty() {
            if let Some((_, form)) = STATEMENTS.iter().find(|(w, _)| *w == word) {
                parts.push(format!("`{form}`"));
            } else if let Some((_, what)) = WORDS.iter().find(|(w, _)| *w == word) {
                parts.push(format!("`{word}`: {what}"));
            } else if TYPES.contains(&word) {
                parts.push(format!("`{word}`: 타입"));
            }
        }
        if parts.is_empty() {
            return Value::Null;
        }
        json!({
            "contents": { "kind": "markdown", "value": parts.join("\n\n") },
            "range": {
                "start": position(row, start),
                "end": position(row, start + units(word)),
            },
        })
    }

    fn definition(&self, uri: &str, row: usize, character: usize) -> Value {
        let Some(document) = self.documents.get(uri) else {
            return Value::Null;
        };
        let Some(line) = document.text.lines().nth(row) else {
            return Value::Null;
        };
        let Some((start, word)) = word_at(line, character) else {
            return Value::Null;
        };
        let symbols = outline(&document.text);
        let has = |name: &str| symbols.iter().any(|s| s.name == name);

        // `(참조 1)`의 '참조'나 번호 위에서는 `참조 1:` 줄로 갑니다.
        let mut target = None;
        let words = split_words(line);
        if let Some(index) = words
            .iter()
            .position(|(s, w)| *s <= start && start < *s + units(w))
        {
            let clean = |w: &str| w.trim_matches(|c: char| !c.is_alphanumeric()).to_string();
            let here = clean(words[index].1);
            let number = if here == "참조" {
                words.get(index + 1).map(|(_, w)| clean(w))
            } else if index > 0 && clean(words[index - 1].1) == "참조" {
                Some(here)
            } else {
                None
            };
            if let Some(number) =
                number.filter(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()))
            {
                target = Some(format!("참조 {number}"));
            }
        }
        let target = target
            .or_else(|| document.info.forms.get(word).cloned())
            .or_else(|| resolve(word, has).map(str::to_string));
        let Some(symbol) = target.and_then(|name| symbols.into_iter().find(|s| s.name == name))
        else {
            return Value::Null;
        };
        json!({ "uri": uri, "range": symbol.selection() })
    }

    /// 본문 속 이름의 자리를 LSP의 상대 위치 형식(줄 차이, 열 차이, 길이, 종류, 수식)으로 냅니다.
    fn semantic_tokens(&self, uri: &str) -> Value {
        let mut data = Vec::new();
        let (mut last_row, mut last_start) = (0, 0);
        if let Some(document) = self.documents.get(uri) {
            for &(row, start, length) in &document.info.mentions {
                let delta_row = row - last_row;
                let delta_start = if delta_row == 0 {
                    start - last_start
                } else {
                    start
                };
                data.extend([delta_row, delta_start, length, 0, 0]);
                (last_row, last_start) = (row, start);
            }
        }
        json!({ "data": data })
    }

    fn symbols(&self, uri: &str) -> Value {
        let Some(document) = self.documents.get(uri) else {
            return Value::Array(Vec::new());
        };
        let lines: Vec<&str> = document.text.lines().collect();
        Value::Array(
            outline(&document.text)
                .into_iter()
                .map(|symbol| {
                    json!({
                        "name": symbol.name,
                        "detail": symbol.keyword,
                        "kind": symbol.kind(),
                        "range": line_range(&lines, symbol.row + 1),
                        "selectionRange": symbol.selection(),
                    })
                })
                .collect(),
        )
    }
}

/// 문서에서 선언된 이름 하나. 문법 오류가 있는 문서에서도 읽을 수 있도록 줄의 앞머리만 봅니다.
struct Symbol {
    name: String,
    /// 선언한 낱말(`정의`, `대상` ...). 이름 붙은 문장은 `문장`입니다.
    keyword: &'static str,
    /// 0부터 세는 줄
    row: usize,
    /// 이름의 UTF-16 범위
    start: usize,
    end: usize,
}

impl Symbol {
    fn selection(&self) -> Value {
        json!({ "start": position(self.row, self.start), "end": position(self.row, self.end) })
    }

    fn kind(&self) -> u8 {
        match self.keyword {
            "기준" => SYMBOL_NAMESPACE,
            "대상" => SYMBOL_OBJECT,
            "정의" => SYMBOL_CONSTANT,
            "미지" => SYMBOL_VARIABLE,
            "행위" | "규칙" => SYMBOL_FUNCTION,
            "검사" | "조건" | "목표" => SYMBOL_BOOLEAN,
            "참조" => SYMBOL_STRING,
            _ => SYMBOL_EVENT,
        }
    }
}

fn units(text: &str) -> usize {
    text.chars().map(char::len_utf16).sum()
}

/// 공백으로 나눈 낱말과 그 시작 위치(UTF-16).
fn split_words(line: &str) -> Vec<(usize, &str)> {
    let mut words = Vec::new();
    let mut offset = 0;
    let mut start: Option<(usize, usize)> = None;
    for (index, ch) in line.char_indices() {
        if ch.is_whitespace() {
            if let Some((byte, unit)) = start.take() {
                words.push((unit, &line[byte..index]));
            }
        } else if start.is_none() {
            start = Some((index, offset));
        }
        offset += ch.len_utf16();
    }
    if let Some((byte, unit)) = start {
        words.push((unit, &line[byte..]));
    }
    words
}

/// 선언 줄이 만드는 이름들과 그 시작 위치(UTF-16).
/// `대상 가, 나`처럼 여러 이름을 선언하는 줄이 있고,
/// `조건 가 >= 0`처럼 이름 없이 식만 적은 줄은 이름을 만들지 않습니다.
fn declared_names(line: &str, keyword: &str) -> Vec<(usize, String)> {
    let is_word = |c: char| c.is_alphanumeric() || c == '_';
    let Some(at) = line.find(keyword) else {
        return Vec::new();
    };
    let offset = at + keyword.len();
    let body = line[offset..].split('#').next().unwrap_or_default();
    let list = matches!(keyword, "대상" | "단위" | "미지" | "행위");
    let optional = matches!(keyword, "검사" | "조건" | "목표");

    let mut names = Vec::new();
    let mut depth = 0usize;
    let mut current: Option<usize> = None;
    for (index, ch) in body.char_indices().chain([(body.len(), ' ')]) {
        if depth == 0 && is_word(ch) {
            current.get_or_insert(index);
            continue;
        }
        if let Some(start) = current.take() {
            names.push((start, index));
            if !list {
                break;
            }
        }
        match ch {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            ':' | '=' if depth == 0 => break,
            _ => {}
        }
    }
    if optional {
        // 이름은 바로 뒤에 '='(비교 '=='가 아닌)가 올 때만 이름입니다.
        names.retain(|&(_, end)| {
            let rest = body[end..].trim_start();
            rest.starts_with('=') && !rest.starts_with("==")
        });
    }
    names
        .into_iter()
        .filter(|&(start, _)| !body[start..].starts_with(|c: char| c.is_ascii_digit()))
        .map(|(start, end)| (units(&line[..offset + start]), body[start..end].to_string()))
        .collect()
}

fn outline(text: &str) -> Vec<Symbol> {
    const DECLARING: &[&str] = &[
        "정의", "검사", "기준", "대상", "단위", "사건", "미지", "조건", "목표", "행위", "규칙",
        "낱말", "장면",
    ];
    let is_name =
        |text: &str| !text.is_empty() && text.chars().all(|c| c.is_alphanumeric() || c == '_');
    let mut symbols = Vec::new();
    for (row, line) in text.lines().enumerate() {
        let words = split_words(line);
        let Some(&(first_start, first)) = words.first() else {
            continue;
        };
        if first.starts_with('#') {
            continue;
        }
        if let Some(&keyword) = DECLARING.iter().find(|k| **k == first) {
            for (start, name) in declared_names(line, keyword) {
                symbols.push(Symbol {
                    end: start + units(&name),
                    name,
                    keyword,
                    row,
                    start,
                });
            }
        } else if first == "참조" {
            let Some(&(start, word)) = words.get(1) else {
                continue;
            };
            let number = word.split(':').next().unwrap_or_default();
            if !number.is_empty() && number.chars().all(|c| c.is_ascii_digit()) {
                symbols.push(Symbol {
                    name: format!("참조 {number}"),
                    keyword: "참조",
                    row,
                    start: first_start,
                    end: start + units(number),
                });
            }
        } else if let Some((label, _)) = first.split_once(':')
            && is_name(label)
            && !STATEMENTS.iter().any(|(k, _)| *k == label)
        {
            // 이름 붙은 문장: `깨짐: 컵이 떨어지면 ...`
            symbols.push(Symbol {
                name: label.to_string(),
                keyword: "문장",
                row,
                start: first_start,
                end: first_start + units(label),
            });
        }
    }
    symbols
}

/// 커서 아래의 낱말(글자·숫자·밑줄이 이어진 것)과 그 시작 위치(UTF-16).
fn word_at(line: &str, character: usize) -> Option<(usize, &str)> {
    let is_word = |c: char| c.is_alphanumeric() || c == '_';
    let mut offset = 0;
    let mut current: Option<(usize, usize)> = None; // (바이트 시작, UTF-16 시작)
    let mut found = None;
    for (index, ch) in line.char_indices() {
        if is_word(ch) {
            if current.is_none() {
                current = Some((index, offset));
            }
        } else if let Some((byte, unit)) = current.take()
            && unit <= character
            && character <= offset
        {
            found = Some((unit, &line[byte..index]));
            break;
        }
        offset += ch.len_utf16();
    }
    if found.is_none()
        && let Some((byte, unit)) = current
        && unit <= character
        && character <= offset
    {
        found = Some((unit, &line[byte..]));
    }
    found
}

/// 낱말이 아는 이름이면 그대로, 아니면 조사를 뗀 앞부분이 아는 이름인지 봅니다.
fn resolve(word: &str, known: impl Fn(&str) -> bool) -> Option<&str> {
    if known(word) {
        return Some(word);
    }
    PARTICLES
        .iter()
        .filter_map(|particle| word.strip_suffix(particle))
        .find(|stem| !stem.is_empty() && known(stem))
}

fn notification(uri: &str, diagnostics: Vec<Value>) -> Value {
    json!({
        "jsonrpc": "2.0",
        "method": "textDocument/publishDiagnostics",
        "params": { "uri": uri, "diagnostics": diagnostics },
    })
}

/// 문서를 컴파일해 진단을 만듭니다. 성공했으면 이름과 줄에 대한 설명도 돌려줍니다.
fn analyze(text: &str) -> (Vec<Value>, Option<Info>) {
    let lines: Vec<&str> = text.lines().collect();
    // 편집 중인 문서는 어떤 모양이든 될 수 있습니다. 컴파일러가 멈추더라도 서버는 계속 돕니다.
    let compiled = std::panic::catch_unwind(|| aha::compile(text));
    match compiled {
        Err(_) => {
            let entry = json!({
                "range": line_range(&lines, 1),
                "severity": SEVERITY_ERROR,
                "source": "aha",
                "message": "컴파일러 내부 오류입니다. 이 문서를 저장소에 알려 주시면 고치겠습니다.",
            });
            (vec![entry], None)
        }
        Ok(Err(diagnostic)) => {
            let entry = json!({
                "range": token_range(&lines, diagnostic.line, diagnostic.column),
                "severity": SEVERITY_ERROR,
                "code": diagnostic.code,
                "source": "aha",
                "message": diagnostic.message,
            });
            (vec![entry], None)
        }
        Ok(Ok(program)) => {
            let findings = report::findings(&program);
            let warnings = program.warnings.iter().map(|warning| {
                let row = warning.line.saturating_sub(1);
                let start = warning.column.saturating_sub(1);
                json!({
                    "range": {
                        "start": position(row, start),
                        "end": position(row, start + warning.length),
                    },
                    "severity": SEVERITY_WARNING,
                    "code": warning.code,
                    "source": "aha",
                    "message": warning.message,
                })
            });
            let diagnostics = findings
                .iter()
                .map(|finding| {
                    json!({
                        "range": line_range(&lines, finding.line),
                        "severity": match finding.level {
                            Level::Info => SEVERITY_INFORMATION,
                            Level::Warning => SEVERITY_WARNING,
                        },
                        "source": "aha",
                        "message": finding.text,
                    })
                })
                .chain(warnings)
                .collect();
            (diagnostics, Some(describe(&program, findings)))
        }
    }
}

/// 컴파일 결과를 이름별 설명으로 바꿉니다.
fn describe(program: &aha::CompiledProgram, findings: Vec<report::Finding>) -> Info {
    let mut mentions: Vec<_> = program
        .mentions
        .iter()
        .map(|m| {
            (
                m.line.saturating_sub(1),
                m.column.saturating_sub(1),
                m.length,
            )
        })
        .collect();
    mentions.sort_unstable();
    let mut info = Info {
        mentions,
        ..Info::default()
    };
    let quantity = |value: &str, unit: Option<&str>, frame: &Option<String>| {
        let mut text = value.to_string();
        if let Some(unit) = unit {
            text.push_str(&format!(" {unit}"));
        }
        if let Some(frame) = frame {
            text.push_str(&format!(" ({frame} 기준)"));
        }
        text
    };

    for frame in &program.frames {
        info.names.insert(
            frame.name.clone(),
            format!("**기준** `{}`: {}", frame.name, frame.axis),
        );
    }
    for entity in &program.entities {
        info.names
            .insert(entity.name.clone(), format!("**대상** `{}`", entity.name));
    }
    for unit in &program.units {
        info.names.insert(
            unit.clone(),
            format!("**단위** `{unit}`: 이 단위로 센 양끼리만 더하고 비교할 수 있습니다."),
        );
    }
    for statement in &program.statements {
        let text = if statement.kind == "정의" {
            format!(
                "**정의** `{}`: {} = {}",
                statement.name,
                statement.value_type,
                quantity(&statement.value, statement.unit, &statement.frame)
            )
        } else {
            format!("**검사** `{}`: 통과", statement.name)
        };
        info.names.insert(statement.name.clone(), text);
    }
    for event in &program.events {
        let roles: Vec<String> = event
            .roles
            .iter()
            .map(|role| {
                format!(
                    "{} = {}",
                    role.role,
                    quantity(&role.value, role.unit, &role.frame)
                )
            })
            .collect();
        info.names.insert(
            event.name.clone(),
            format!("**사건** `{}`: {}", event.name, roles.join(", ")),
        );
    }
    for constraint in &program.constraints {
        if constraint.kind == "조건" {
            info.names.insert(
                constraint.name.clone(),
                format!("**조건** `{}`: 모든 해가 따르는 가정", constraint.name),
            );
        }
    }
    for verb in &program.verbs {
        info.names.insert(
            verb.name.clone(),
            format!(
                "**행위** `{}`: 필수 역할 {}",
                verb.name,
                verb.required.join(", ")
            ),
        );
    }
    let mut clause = |line: usize, verb: &str, form: &str, roles: &[aha::IrSentenceRole]| {
        info.forms.insert(form.to_string(), verb.to_string());
        let entry = info.roles.entry(line).or_default();
        for role in roles {
            if role.reference.is_none() {
                entry.push((
                    format!("{}{}", role.value, role.particle),
                    role.role.clone(),
                ));
            }
        }
    };
    for sentence in &program.sentences {
        clause(
            sentence.line,
            &sentence.verb,
            &sentence.form,
            &sentence.roles,
        );
    }
    for rule in &program.conditionals {
        clause(
            rule.line,
            &rule.condition.verb,
            &rule.condition.form,
            &rule.condition.roles,
        );
        clause(
            rule.line,
            &rule.conclusion.verb,
            &rule.conclusion.form,
            &rule.conclusion.roles,
        );
    }
    for sentence in &program.sentences {
        if let Some(label) = &sentence.label {
            let tense = sentence
                .tense
                .map_or(String::new(), |t| format!(", {t} 시제"));
            info.names.insert(
                label.clone(),
                format!(
                    "**문장** `{label}`: {} 사건, 정도 {}%{tense}",
                    sentence.verb, sentence.degree
                ),
            );
        }
    }
    // 미지값의 범위, 규칙과 조건문의 정도, 목표의 판정은 명령줄 출력과 같은 문장을 씁니다.
    for finding in findings {
        info.names.insert(finding.name, finding.text);
    }
    info
}

fn position(line: usize, character: usize) -> Value {
    json!({ "line": line, "character": character })
}

/// 진단 위치(1부터 세는 줄과 UTF-16 열)에서 시작하는 낱말의 범위.
fn token_range(lines: &[&str], line: usize, column: usize) -> Value {
    let row = line.saturating_sub(1);
    let start = column.saturating_sub(1);
    let text = lines.get(row).copied().unwrap_or_default();
    let mut offset = 0;
    let mut end = start;
    for ch in text.chars() {
        if offset >= start {
            if ch.is_whitespace() {
                break;
            }
            end = offset + ch.len_utf16();
        }
        offset += ch.len_utf16();
    }
    // 줄 끝을 가리키는 진단(빠진 낱말 등)은 한 칸 너비로 표시합니다.
    let end = end.max(start + 1);
    json!({ "start": position(row, start), "end": position(row, end) })
}

/// 줄 전체(앞의 공백 제외)의 범위.
fn line_range(lines: &[&str], line: usize) -> Value {
    let row = line.saturating_sub(1);
    let text = lines.get(row).copied().unwrap_or_default();
    let start = units(text) - units(text.trim_start());
    json!({ "start": position(row, start), "end": position(row, units(text.trim_end()).max(start + 1)) })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_ranges_cover_the_reported_word() {
        let lines = ["정의 가: 정수 = 1", "검사 틀림 = 가 > 2"];
        let range = token_range(&lines, 2, 4);
        assert_eq!(range["start"], position(1, 3));
        assert_eq!(range["end"], position(1, 5));
        // UTF-16에서 두 단위를 차지하는 글자 뒤의 낱말.
        let lines = ["# 😀", "가나 다라"];
        assert_eq!(token_range(&lines, 2, 4)["end"], position(1, 5));
        // 줄 끝을 가리키면 한 칸 너비입니다.
        assert_eq!(token_range(&["가"], 1, 2)["end"], position(0, 2));
    }

    #[test]
    fn framing_round_trips() {
        let mut buffer = Vec::new();
        write_message(&mut buffer, &json!({ "가": 1 })).unwrap();
        let mut input = io::BufReader::new(buffer.as_slice());
        let body = read_message(&mut input).unwrap().unwrap();
        assert_eq!(serde_json::from_slice::<Value>(&body).unwrap()["가"], 1);
        assert!(read_message(&mut input).unwrap().is_none());
    }
}

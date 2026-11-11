mod degree;
pub mod graph;
mod integer;
mod parser;
mod prose;
mod sentence;
mod solver;
mod syntax;

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt;

use degree::Degree;
use integer::Integer;
use serde::Serialize;
use solver::{Closure, ORIGIN};
use syntax::{Axis, Expression, Kind, Role, Statement, Token, Type};

pub const LANGUAGE_VERSION: &str = "0.14";

#[derive(Debug)]
pub struct Diagnostic {
    pub code: &'static str,
    pub line: usize,
    pub column: usize,
    pub message: String,
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "({},{}): error {}: {}",
            self.line, self.column, self.code, self.message
        )
    }
}

impl std::error::Error for Diagnostic {}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum IrExpression {
    Literal {
        #[serde(rename = "type")]
        kind: &'static str,
        value: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        unit: Option<&'static str>,
    },
    Reference {
        #[serde(rename = "type")]
        kind: &'static str,
        name: String,
    },
    Unary {
        #[serde(rename = "type")]
        kind: &'static str,
        operator: String,
        operands: [Box<IrExpression>; 1],
    },
    Binary {
        #[serde(rename = "type")]
        kind: &'static str,
        operator: String,
        operands: [Box<IrExpression>; 2],
    },
    /// `기준 @ 값`
    Anchor {
        #[serde(rename = "type")]
        kind: &'static str,
        frame: String,
        operands: [Box<IrExpression>; 1],
    },
    /// `사건.역할`
    Member {
        #[serde(rename = "type")]
        kind: &'static str,
        event: String,
        role: String,
    },
}

/// 방식을 정하지 않은 규칙들의 방식 조합 하나와 그때의 값.
#[derive(Serialize)]
pub struct IrOutcome {
    /// 규칙 이름 → `최소` 또는 `곱`
    pub choices: BTreeMap<String, &'static str>,
    pub value: String,
}

#[derive(Serialize)]
pub struct IrRule {
    /// 소스의 줄 번호
    pub line: usize,
    pub name: String,
    /// `최소` 또는 `곱`. 정하지 않았으면 없으며, 두 방식을 모두 계산합니다.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub method: Option<&'static str>,
    pub premise: IrExpression,
    pub strength: IrExpression,
    /// 결론의 정도. 방식에 따라 다르면 `56%~70%`처럼 범위입니다.
    pub value: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outcomes: Option<Vec<IrOutcome>>,
}

#[derive(Serialize)]
pub struct IrStatement {
    pub kind: &'static str,
    pub name: String,
    #[serde(rename = "type")]
    pub value_type: &'static str,
    pub value: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frame: Option<String>,
    /// 규칙의 방식에 따라 값이 갈라지면 조합마다의 값.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outcomes: Option<Vec<IrOutcome>>,
    pub expression: IrExpression,
}

#[derive(Serialize)]
pub struct IrFrame {
    pub name: String,
    pub axis: &'static str,
}

#[derive(Serialize)]
pub struct IrEntity {
    pub name: String,
}

#[derive(Serialize)]
pub struct IrRole {
    pub role: String,
    #[serde(rename = "type")]
    pub value_type: &'static str,
    pub value: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frame: Option<String>,
    /// 역할을 생략해 기본값이 들어간 경우(`종료` = `시작`)에는 식이 없습니다.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expression: Option<IrExpression>,
}

#[derive(Serialize)]
pub struct IrEvent {
    pub name: String,
    pub roles: Vec<IrRole>,
}

#[derive(Serialize)]
pub struct IrUnknown {
    /// 소스의 줄 번호
    pub line: usize,
    pub name: String,
    #[serde(rename = "type")]
    pub value_type: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frame: Option<String>,
    /// 모든 조건을 만족하는 해에서의 최솟값. `null`이면 하한이 없습니다.
    pub min: Option<String>,
    /// 모든 조건을 만족하는 해에서의 최댓값. `null`이면 상한이 없습니다.
    pub max: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub observed: Option<String>,
}

#[derive(Serialize)]
pub struct IrConstraint {
    /// `조건` 또는 `관측`
    pub kind: &'static str,
    pub name: String,
    pub expression: IrExpression,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IrGoal {
    /// 소스의 줄 번호
    pub line: usize,
    pub name: String,
    /// `보장`(모든 해에서 성립), `가능`(일부 해에서 성립), `불가능`(어떤 해에서도 성립하지 않음)
    pub status: &'static str,
    /// `가능`일 때 판정을 확정하려면 관측해야 하는 미지값.
    pub missing_observations: Vec<String>,
    /// `가능`일 때 계산 방식에 따라 판정이 갈리는 규칙.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub undecided_rules: Vec<String>,
    pub expression: IrExpression,
}

#[derive(Serialize)]
pub struct IrVerb {
    pub name: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub forms: Vec<String>,
    pub required: Vec<&'static str>,
}

/// `본문: 글`
#[derive(Serialize)]
pub struct IrText {
    pub line: usize,
    pub text: String,
    /// 아래에 `- 글`로 적은 줄들.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub items: Vec<IrItem>,
}

/// `낱말`로 정의한 낱말과 그 차원.
#[derive(Serialize)]
pub struct IrWord {
    pub name: String,
    pub dimensions: Vec<IrDimension>,
}

#[derive(Serialize)]
pub struct IrDimension {
    pub name: String,
    /// 별표를 붙인, 꼭 적어야 하는 차원.
    pub required: bool,
    /// 채움 점수에서의 무게.
    pub weight: u32,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub kind: Option<&'static str>,
}

/// `장면` 하나.
#[derive(Serialize)]
pub struct IrScene {
    pub name: String,
    pub line: usize,
    /// 채움 점수(0~100): 차원의 가중치 가운데 값을 적은 것의 비율.
    pub fill: u32,
    pub items: Vec<IrSceneItem>,
}

/// 장면 안의 낱말 하나.
#[derive(Serialize)]
pub struct IrSceneItem {
    pub name: String,
    pub word: String,
    pub line: usize,
    /// 들여쓰기의 깊이. 장면 바로 아래가 0.
    pub depth: usize,
    /// 이 낱말이 들어 있는 낱말.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    pub values: Vec<IrFilled>,
    /// 값을 적지 않은 차원.
    pub blanks: Vec<String>,
}

#[derive(Serialize)]
pub struct IrFilled {
    pub dimension: String,
    pub value: String,
}

/// `- 글`로 적은 딸린 줄.
#[derive(Serialize)]
pub struct IrItem {
    pub line: usize,
    pub text: String,
}

/// `- 글` 줄이 딸리는 곳.
#[derive(Clone, Copy)]
enum Host {
    Sentence(usize),
    Conditional(usize),
    Reference(usize),
    Text(usize),
}

/// 본문(설명, 참조, `본문:` 줄)에서 선언한 이름이 나온 자리.
#[derive(Serialize)]
pub struct IrMention {
    pub line: usize,
    /// UTF-16 열(1부터)
    pub column: usize,
    /// 이름의 길이(UTF-16). 뒤에 붙은 조사는 세지 않습니다.
    pub length: usize,
    pub name: String,
}

/// 컴파일을 막지 않는 알림. 본문 점검의 결과입니다.
#[derive(Serialize)]
pub struct IrWarning {
    pub code: &'static str,
    pub line: usize,
    pub column: usize,
    pub length: usize,
    pub message: String,
}

#[derive(Serialize)]
pub struct IrReference {
    pub number: u32,
    pub line: usize,
    pub text: String,
    /// 아래에 `- 글`로 적은 줄들.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub items: Vec<IrItem>,
}

#[derive(Serialize)]
pub struct IrSentenceRole {
    pub role: String,
    pub particle: String,
    #[serde(rename = "type")]
    pub value_type: &'static str,
    pub value: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frame: Option<String>,
    /// 지시어(이렇게, 이것)가 가리키는 참조 번호.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference: Option<u32>,
}

/// 조건문의 전제나 결론.
#[derive(Serialize)]
pub struct IrClause {
    /// 행위의 기본형
    pub verb: String,
    /// 문장에 쓰인 꼴 (`떨어지면`, `깨진다`)
    pub form: String,
    pub roles: Vec<IrSentenceRole>,
}

/// 문장으로 쓴 규칙: `컵이 떨어지면 컵이 80% 깨진다`
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IrConditional {
    pub line: usize,
    /// 문장에 붙인 이름. 붙이지 않았으면 `7행`처럼 줄 번호입니다.
    pub name: String,
    pub labeled: bool,
    /// `최소` 또는 `곱`. 정하지 않았으면 없으며, 두 방식을 모두 계산합니다.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub method: Option<&'static str>,
    pub condition: IrClause,
    /// 전제에 해당한 사실이 적힌 줄.
    pub matched_lines: Vec<usize>,
    /// 전제의 정도: 해당한 사실들의 정도 중 가장 큰 값.
    pub premise: String,
    /// 규칙의 강도: 결론에 적은 정도.
    pub strength: String,
    pub conclusion: IrClause,
    /// 결론의 정도.
    pub value: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outcomes: Option<Vec<IrOutcome>>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub references: Vec<u32>,
    /// 문장 뒤에 `: 설명`으로 적은 글.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// 아래에 `- 글`로 적은 줄들.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub items: Vec<IrItem>,
}

#[derive(Serialize)]
pub struct IrSentence {
    pub line: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// 행위의 기본형
    pub verb: String,
    /// 문장에 쓰인 활용형
    pub form: String,
    /// 문장이 참인 정도(퍼센트). 적지 않으면 100입니다.
    pub degree: u32,
    /// 활용형에서 읽은 시제: 과거, 현재, 미래. 기본형이면 없습니다.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tense: Option<&'static str>,
    pub roles: Vec<IrSentenceRole>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub references: Vec<u32>,
    /// 문장 뒤에 `: 설명`으로 적은 글.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// 아래에 `- 글`로 적은 줄들.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub items: Vec<IrItem>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompiledProgram {
    pub language_version: &'static str,
    pub statements: Vec<IrStatement>,
    /// `단위`로 선언한 단위 이름.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub units: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub frames: Vec<IrFrame>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub entities: Vec<IrEntity>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub events: Vec<IrEvent>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub unknowns: Vec<IrUnknown>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub constraints: Vec<IrConstraint>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub goals: Vec<IrGoal>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub verbs: Vec<IrVerb>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub sentences: Vec<IrSentence>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub references: Vec<IrReference>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub rules: Vec<IrRule>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub conditionals: Vec<IrConditional>,
    /// `주제 <글>`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub topic: Option<String>,
    /// `낱말`로 정의한 낱말.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub words: Vec<IrWord>,
    /// `장면`.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub scenes: Vec<IrScene>,
    /// `본문:` 줄.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub texts: Vec<IrText>,
    /// 본문에서 선언한 이름이 나온 자리.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub mentions: Vec<IrMention>,
    /// 본문 점검의 알림. 컴파일을 막지 않습니다.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<IrWarning>,
}

/// 사건이 받을 수 있는 역할과 각 역할의 값 종류.
const ROLES: &[(&str, Type)] = &[
    ("주체", Type::Entity),
    ("대상", Type::Entity),
    ("관측자", Type::Entity),
    ("시작", Type::Instant),
    ("종료", Type::Instant),
    ("출발", Type::Position),
    ("도착", Type::Position),
];

pub fn compile(source: &str) -> Result<CompiledProgram, Diagnostic> {
    let tokens = syntax::scan(source)?;
    let statements = parser::parse(&tokens)?;
    let mut scope = Scope::default();
    // 미리 정의해 둔 낱말. 같은 이름으로 다시 정의하면 그쪽을 씁니다.
    for (name, dimensions) in BUILTIN_WORDS {
        let dimensions = dimensions
            .iter()
            .map(|&(name, kind)| WordDimension {
                name: name.to_string(),
                required: false,
                weight: 1,
                kind,
            })
            .collect();
        scope.words.insert(name.to_string(), (dimensions, true));
    }
    let mut names = HashSet::new();
    let mut program = CompiledProgram {
        language_version: LANGUAGE_VERSION,
        statements: Vec::new(),
        topic: None,
        words: Vec::new(),
        scenes: Vec::new(),
        texts: Vec::new(),
        mentions: Vec::new(),
        warnings: Vec::new(),
        units: Vec::new(),
        frames: Vec::new(),
        entities: Vec::new(),
        events: Vec::new(),
        unknowns: Vec::new(),
        constraints: Vec::new(),
        goals: Vec::new(),
        verbs: Vec::new(),
        sentences: Vec::new(),
        references: Vec::new(),
        rules: Vec::new(),
        conditionals: Vec::new(),
    };
    let mut pending = Vec::new();
    let mut verbs: Vec<sentence::Verb> = Vec::new();
    let mut citations: Vec<(u32, Token)> = Vec::new();
    // 낱말로 나누지 않는 글: 문장 뒤의 설명, 참조의 본문, `본문:` 줄.
    let mut passages: Vec<prose::Passage> = Vec::new();
    let passage = |token: &Token| prose::Passage {
        line: token.line(),
        column: token.column(),
        text: token.text.clone(),
    };

    // `- 글` 줄이 딸릴 곳: 가장 가까운 위의 문장·조건문·참조·본문.
    let mut host: Option<Host> = None;

    for statement in statements {
        if !matches!(statement, Statement::Item { .. }) {
            host = None;
        }
        let name = match &statement {
            // 이름을 적지 않은 검사·조건·목표는 이름 공간을 쓰지 않습니다.
            Statement::Value {
                anonymous: true, ..
            }
            | Statement::Constraint {
                anonymous: true, ..
            } => None,
            Statement::Value { name, .. }
            | Statement::Frame { name, .. }
            | Statement::Entity { name }
            | Statement::Unit { name }
            | Statement::Event { name, .. }
            | Statement::Unknown { name, .. }
            | Statement::Constraint { name, .. }
            | Statement::Rule { name, .. } => Some(name),
            Statement::Sentence { label, .. } => label.as_ref(),
            // 관측은 새 이름을 만들지 않고 미지값에 값을 기록합니다.
            // 행위와 참조는 값과 다른 이름 공간을 씁니다.
            // 낱말과 장면은 저마다의 이름 공간을 씁니다. 장면 안의 낱말은 그 안에서 등록합니다.
            Statement::Topic { .. } | Statement::Word { .. } | Statement::Scene { .. } => None,
            Statement::Observe { .. }
            | Statement::Verb { .. }
            | Statement::Body { .. }
            | Statement::Item { .. }
            | Statement::ReferenceDefinition { .. } => None,
        };
        if let Some(name) = name
            && !names.insert(name.text.clone())
        {
            return Err(name.error(
                "K301",
                format!("'{}' 이름이 이미 선언되어 있습니다.", name.text),
            ));
        }

        match statement {
            Statement::Value {
                name,
                check: true,
                expression,
                ..
            } if mentions_unknown(&expression, &scope) => {
                // 미지값이 들어간 검사는 모든 해에서 보장되어야 하며, 판정은 모든 조건을 읽은 뒤에 합니다.
                let judged = constrain(expression, &scope)?;
                check_declared_type(&name, Type::Boolean, &judged.value, &scope)?;
                reject_mixed(&name, &judged)?;
                pending.push(Pending {
                    name,
                    is_check: true,
                    judged,
                });
            }
            Statement::Topic { text } => {
                if program.topic.is_some() {
                    return Err(text.error("K200", "주제는 파일에 한 번만 적습니다."));
                }
                passages.push(passage(&text));
                program.topic = Some(text.text);
            }
            Statement::Word { name, dimensions } => {
                if scope
                    .words
                    .get(&name.text)
                    .is_some_and(|(_, builtin)| !builtin)
                {
                    return Err(name.error(
                        "K301",
                        format!("낱말 '{}'은 이미 정의되어 있습니다.", name.text),
                    ));
                }
                let mut seen = HashSet::new();
                let mut declared = Vec::new();
                for dimension in dimensions {
                    if !seen.insert(dimension.name.text.clone()) {
                        return Err(dimension.name.error(
                            "K701",
                            format!(
                                "낱말 '{}'에 차원 '{}'을 두 번 적었습니다.",
                                name.text, dimension.name.text
                            ),
                        ));
                    }
                    if let Some(kind) = dimension.kind {
                        known_type(kind, &dimension.name, &scope)?;
                    }
                    declared.push(WordDimension {
                        name: dimension.name.text,
                        required: dimension.required,
                        weight: dimension.weight,
                        kind: dimension.kind,
                    });
                }
                program.words.push(IrWord {
                    name: name.text.clone(),
                    dimensions: declared
                        .iter()
                        .map(|d| IrDimension {
                            name: d.name.clone(),
                            required: d.required,
                            weight: d.weight,
                            kind: d.kind.map(Type::name),
                        })
                        .collect(),
                });
                scope.words.insert(name.text, (declared, false));
            }
            Statement::Scene { name, members } => {
                if program.scenes.iter().any(|scene| scene.name == name.text) {
                    return Err(
                        name.error("K301", format!("장면 '{}'은 이미 있습니다.", name.text))
                    );
                }
                let mut items: Vec<IrSceneItem> = Vec::new();
                // (들여쓰기 열, items에서의 순서). 더 들여 쓴 줄은 맨 위에 있는 것의 안에 있습니다.
                let mut open: Vec<(usize, usize)> = Vec::new();
                let (mut filled_weight, mut total_weight) = (0u32, 0u32);
                for member in members {
                    let Some((dimensions, _)) = scope.words.get(&member.word.text) else {
                        return Err(member.word.error(
                            "K702",
                            format!(
                                "'{0}'은 낱말이 아닙니다. 장면에 쓰려면 앞에서 '낱말 {0}: 차원, …'으로 정의하세요.",
                                member.word.text
                            ),
                        ));
                    };
                    let dimensions = dimensions.clone();
                    let label = member.name.as_ref().unwrap_or(&member.word);
                    if !names.insert(label.text.clone()) {
                        return Err(label.error(
                            "K301",
                            format!(
                                "'{0}' 이름이 이미 있습니다. 같은 낱말을 또 쓰려면 이름을 붙이세요. 예: '{1} 둘째{1}: …'",
                                label.text, member.word.text
                            ),
                        ));
                    }
                    let column = member.word.column();
                    while open.last().is_some_and(|&(start, _)| start >= column) {
                        open.pop();
                    }
                    let parent = open.last().map(|&(_, index)| items[index].name.clone());
                    let depth = open.len();
                    open.push((column, items.len()));

                    let mut values: Vec<IrFilled> = Vec::new();
                    for (dimension, tokens) in member.values {
                        let Some(declared) = dimensions.iter().find(|d| d.name == dimension.text)
                        else {
                            let known: Vec<_> =
                                dimensions.iter().map(|d| d.name.as_str()).collect();
                            return Err(dimension.error(
                                "K703",
                                format!(
                                    "낱말 '{}'에는 '{}' 차원이 없습니다. 있는 차원: {}",
                                    member.word.text,
                                    dimension.text,
                                    known.join(", ")
                                ),
                            ));
                        };
                        if values.iter().any(|v| v.dimension == dimension.text) {
                            return Err(dimension.error(
                                "K703",
                                format!("'{}' 차원을 두 번 적었습니다.", dimension.text),
                            ));
                        }
                        let value = match declared.kind {
                            // 타입이 있는 차원은 식으로 읽고 타입을 확인합니다.
                            Some(kind) => {
                                let expression = parser::parse_expression(&tokens)?;
                                let Bound { value, .. } = bind(expression, &scope)?;
                                if value.is_symbolic() || value.worlds.is_some() {
                                    return Err(dimension.error(
                                        "K406",
                                        format!("'{}'에는 확정된 값을 적으세요.", dimension.text),
                                    ));
                                }
                                check_declared_type(&dimension, kind, &value, &scope)?;
                                match kind.base_unit() {
                                    Some(unit) => format!("{} {unit}", value.display()),
                                    None => value.display(),
                                }
                            }
                            None => Token::join(&tokens),
                        };
                        values.push(IrFilled {
                            dimension: dimension.text,
                            value,
                        });
                    }
                    let mut blanks = Vec::new();
                    for declared in &dimensions {
                        total_weight += declared.weight;
                        if values.iter().any(|v| v.dimension == declared.name) {
                            filled_weight += declared.weight;
                        } else if declared.required {
                            return Err(member.word.error(
                                "K704",
                                format!(
                                    "'{}'의 '{}'은 꼭 적어야 하는 차원입니다. 예: '{}: {} 값'",
                                    label.text, declared.name, label.text, declared.name
                                ),
                            ));
                        } else {
                            blanks.push(declared.name.clone());
                        }
                    }
                    // 장면 안의 낱말은 문장에서 대상으로 쓸 수 있습니다.
                    let entity = Value {
                        kind: Type::Entity,
                        frame: None,
                        number: Integer::from_small(program.entities.len() as u32),
                        label: Some(label.text.clone()),
                        terms: BTreeMap::new(),
                        scale: 0,
                        worlds: None,
                    };
                    scope.values.insert(label.text.clone(), entity);
                    program.entities.push(IrEntity {
                        name: label.text.clone(),
                    });
                    items.push(IrSceneItem {
                        name: label.text.clone(),
                        word: member.word.text.clone(),
                        line: member.word.line(),
                        depth,
                        parent,
                        values,
                        blanks,
                    });
                }
                if items.is_empty() {
                    return Err(name.error(
                        "K200",
                        "장면이 비어 있습니다. 다음 줄부터 들여 써서 낱말을 적으세요.",
                    ));
                }
                program.scenes.push(IrScene {
                    name: name.text.clone(),
                    line: name.line(),
                    // 차원이 하나도 없는 장면은 빈칸도 없으므로 100입니다.
                    fill: (filled_weight * 100)
                        .checked_div(total_weight)
                        .unwrap_or(100),
                    items,
                });
            }
            Statement::Unit { name } => {
                scope.units.insert(name.text.clone());
                program.units.push(name.text);
            }
            Statement::Value {
                name,
                check,
                declared_type,
                expression,
                ..
            } => {
                let Bound { value, ir } = bind(expression, &scope)?;
                if value.is_symbolic() {
                    return Err(name.error(
                        "K406",
                        format!(
                            "정의 '{}'에 미지값이 들어 있습니다. 정의에는 값이 확정된 식만 쓸 수 있습니다.",
                            name.text
                        ),
                    ));
                }
                // 타입을 적지 않은 정의는 값의 타입을 그대로 따릅니다.
                if check {
                    check_declared_type(&name, Type::Boolean, &value, &scope)?;
                } else if let Some((expected, token)) = declared_type {
                    known_type(expected, &token, &scope)?;
                    check_declared_type(&name, expected, &value, &scope)?;
                }

                let is_definition = !check;
                if !is_definition && value.worlds.is_some() {
                    return Err(split_check(&name, &value, &scope));
                }
                if !is_definition && value.number.is_zero() {
                    return Err(
                        name.error("K401", format!("검사 '{}'의 결과가 거짓입니다.", name.text))
                    );
                }

                program.statements.push(IrStatement {
                    kind: if is_definition { "정의" } else { "검사" },
                    name: name.text.clone(),
                    value_type: value.kind.name(),
                    value: value.display_all(),
                    unit: value.kind.base_unit(),
                    frame: scope.frame_name(&value),
                    outcomes: scope.outcomes(&value),
                    expression: ir,
                });
                if is_definition {
                    scope.values.insert(name.text, value);
                }
            }
            Statement::Frame { name, axis } => {
                scope
                    .frame_ids
                    .insert(name.text.clone(), scope.frames.len());
                scope.frames.push((name.text.clone(), axis));
                program.frames.push(IrFrame {
                    name: name.text,
                    axis: axis.name(),
                });
            }
            Statement::Entity { name } => {
                let value = Value {
                    kind: Type::Entity,
                    frame: None,
                    number: Integer::from_small(program.entities.len() as u32),
                    label: Some(name.text.clone()),
                    terms: BTreeMap::new(),
                    scale: 0,
                    worlds: None,
                };
                scope.values.insert(name.text.clone(), value);
                program.entities.push(IrEntity { name: name.text });
            }
            Statement::Event { name, roles } => {
                let (record, ir, orderings) = bind_event(&name, roles, &scope)?;
                for ordering in &orderings {
                    require_order(&mut scope, ordering)?;
                }
                scope.events.insert(name.text, record);
                program.events.push(ir);
            }
            Statement::Unknown {
                name,
                kind,
                kind_token,
                frame,
            } => {
                known_type(kind, &kind_token, &scope)?;
                let frame = match frame {
                    None => None,
                    Some(token) => {
                        let Some(&id) = scope.frame_ids.get(&token.text) else {
                            return Err(token.error(
                                "K308",
                                format!(
                                    "'@' 앞에는 '기준'으로 선언한 이름이 필요합니다. '{}'은 기준이 아닙니다.",
                                    token.text
                                ),
                            ));
                        };
                        if scope.frames[id].1.point() != kind {
                            return Err(token.error(
                                "K308",
                                format!(
                                    "{} 기준 '{}'에는 '{}'을 놓을 수 없습니다.",
                                    scope.frames[id].1.name(),
                                    token.text,
                                    kind.name(),
                                ),
                            ));
                        }
                        Some(id)
                    }
                };
                let node = scope.closure.add_node();
                let mut value = Value::point(kind, frame, Integer::from_small(0));
                value.terms.insert(node, Integer::from_small(1));
                scope.values.insert(name.text.clone(), value);
                scope.unknown_ids.insert(name.text.clone(), node);
                scope.unknowns.push(UnknownInfo {
                    line: name.line(),
                    name: name.text,
                    kind,
                    frame,
                    observed: None,
                });
            }
            Statement::Constraint {
                name,
                goal,
                expression,
                ..
            } => {
                let judged = constrain(expression, &scope)?;
                check_declared_type(&name, Type::Boolean, &judged.value, &scope)?;
                reject_mixed(&name, &judged)?;
                if !goal && judged.value.worlds.is_some() {
                    return Err(name.error(
                        "K541",
                        format!(
                            "조건 '{}'의 값이 규칙의 계산 방식에 따라 달라집니다. 조건은 미지값의 범위를 정하는 데 쓰고, 규칙의 결과는 목표나 검사로 확인하세요.",
                            name.text
                        ),
                    ));
                }
                if goal {
                    pending.push(Pending {
                        name,
                        is_check: false,
                        judged,
                    });
                    continue;
                }
                if !judged.fixed {
                    return Err(
                        name.error("K404", format!("조건 '{}'은 항상 거짓입니다.", name.text))
                    );
                }
                for atom in &judged.atoms {
                    assume(&mut scope, atom, &name, "조건")?;
                }
                program.constraints.push(IrConstraint {
                    kind: "조건",
                    name: name.text,
                    expression: judged.ir,
                });
            }
            Statement::Rule {
                name,
                premise,
                strength,
                method,
            } => {
                let premise = bind(premise, &scope)?;
                let strength = bind(strength, &scope)?;
                if premise.value.is_symbolic() || strength.value.is_symbolic() {
                    return Err(name.error(
                        "K406",
                        format!("규칙 '{}'에는 미지값을 쓸 수 없습니다.", name.text),
                    ));
                }
                if !matches!(premise.value.kind, Type::Boolean | Type::Degree) {
                    return Err(name.error(
                        "K303",
                        format!(
                            "규칙 '{}'의 전제는 정도나 논리여야 하지만 {}입니다.",
                            name.text,
                            scope.describe(&premise.value)
                        ),
                    ));
                }
                if strength.value.kind != Type::Degree {
                    return Err(name.error(
                        "K303",
                        format!(
                            "규칙 '{}'의 강도는 '80%'처럼 정도여야 하지만 {}입니다.",
                            name.text,
                            scope.describe(&strength.value)
                        ),
                    ));
                }
                let method = method.map(|token| {
                    if token.text == "곱으로" {
                        Method::Product
                    } else {
                        Method::Minimum
                    }
                });
                let value = apply_rule(
                    &mut scope,
                    &name,
                    &name.text,
                    &premise.value,
                    &strength.value,
                    method,
                )?;
                program.rules.push(IrRule {
                    line: name.line(),
                    name: name.text.clone(),
                    method: method.map(Method::name),
                    premise: premise.ir,
                    strength: strength.ir,
                    value: value.display_all(),
                    outcomes: scope.outcomes(&value),
                });
                scope.values.insert(name.text, value);
            }
            Statement::Verb {
                name,
                forms,
                required,
            } => {
                if verbs.iter().any(|v| v.name == name.text) {
                    return Err(name.error(
                        "K301",
                        format!("행위 '{}'이 이미 선언되어 있습니다.", name.text),
                    ));
                }
                let verb = sentence::Verb::new(&name, &forms, &required)?;
                program.verbs.push(IrVerb {
                    name: name.text,
                    forms: forms.into_iter().map(|f| f.text).collect(),
                    required: verb.required.clone(),
                });
                verbs.push(verb);
            }
            Statement::ReferenceDefinition { number, text } => {
                let Ok(n) = number.text.parse::<u32>() else {
                    return Err(number.error("K511", "참조 번호가 너무 큽니다."));
                };
                if program.references.iter().any(|r| r.number == n) {
                    return Err(number.error("K512", format!("참조 {n}이 이미 정의되어 있습니다.")));
                }
                passages.push(passage(&text));
                host = Some(Host::Reference(program.references.len()));
                program.references.push(IrReference {
                    number: n,
                    line: text.line(),
                    text: text.text,
                    items: Vec::new(),
                });
            }
            Statement::Body { text } => {
                passages.push(passage(&text));
                host = Some(Host::Text(program.texts.len()));
                program.texts.push(IrText {
                    line: text.line(),
                    text: text.text,
                    items: Vec::new(),
                });
            }
            Statement::Item { text } => {
                let items = match host {
                    Some(Host::Sentence(index)) => &mut program.sentences[index].items,
                    Some(Host::Conditional(index)) => &mut program.conditionals[index].items,
                    Some(Host::Reference(index)) => &mut program.references[index].items,
                    Some(Host::Text(index)) => &mut program.texts[index].items,
                    None => {
                        return Err(text.error(
                            "K200",
                            "'-'로 시작하는 줄은 바로 위의 문장, 참조, 본문에 딸린 글입니다. 위에 그런 줄이 없습니다.",
                        ));
                    }
                };
                passages.push(passage(&text));
                items.push(IrItem {
                    line: text.line(),
                    text: text.text,
                });
            }
            Statement::Sentence {
                label,
                tokens,
                note,
            } => {
                let line = tokens[0].line();
                let note = note.map(|token| {
                    passages.push(passage(&token));
                    token.text
                });
                match sentence::bind_line(label.as_ref(), &tokens, &scope, &verbs)? {
                    sentence::BoundLine::Plain(bound) => {
                        let mut bound = *bound;
                        bound.ir.note = note;
                        for ordering in &bound.orderings {
                            require_order(&mut scope, ordering)?;
                        }
                        apply_tense(&mut scope, &bound)?;
                        check_not_late(&scope, &bound.pattern, &bound.verb_token)?;
                        let degree = Value::plain(Type::Degree, Integer::from_small(bound.degree));
                        scope.facts.push(Fact {
                            pattern: bound.pattern,
                            degree: degree.clone(),
                            line,
                        });
                        citations.extend(bound.citations);
                        if let Some(label) = label {
                            // 이름 붙은 문장은 정도로도, 사건으로도(`이름.시작`) 읽을 수 있습니다.
                            scope
                                .events
                                .insert(label.text.clone(), EventRecord { roles: bound.roles });
                            scope.values.insert(label.text, degree);
                        }
                        host = Some(Host::Sentence(program.sentences.len()));
                        program.sentences.push(bound.ir);
                    }
                    sentence::BoundLine::Conditional(bound) => {
                        let sentence::BoundConditional {
                            condition,
                            conclusion,
                            strength,
                            method,
                            citations: cited,
                        } = *bound;
                        // 전제: 앞에 적힌 사실 가운데 해당하는 것들. 여럿이면 가장 큰 정도를 씁니다
                        // ("그런 사건이 있다"의 정도는 해당하는 사건들의 정도 중 최댓값입니다).
                        let matched: Vec<&Fact> = scope
                            .facts
                            .iter()
                            .filter(|fact| satisfies(&fact.pattern, &condition.pattern))
                            .collect();
                        let Some((head, rest)) = matched.split_first() else {
                            return Err(condition.verb_token.error(
                                "K551",
                                format!(
                                    "전제 '{}'에 해당하는 사실이 앞에 없습니다. 조건문보다 앞에 그 사실을 문장으로 적으세요. 일어나지 않았다면 '0%'로 적습니다.",
                                    condition.ir.form
                                ),
                            ));
                        };
                        let matched_lines: Vec<usize> = matched.iter().map(|f| f.line).collect();
                        let mut premise = head.degree.clone();
                        for fact in rest {
                            premise = lift2(
                                &condition.verb_token,
                                &premise,
                                &fact.degree,
                                &scope,
                                |a, b| Ok(Value::degree(a.as_degree().max(b.as_degree()))),
                            )?;
                        }
                        let rule_name = label
                            .as_ref()
                            .map_or_else(|| format!("{line}행"), |l| l.text.clone());
                        let strength_value = Value::degree(Degree::percent(strength));
                        let value = apply_rule(
                            &mut scope,
                            &conclusion.verb_token,
                            &rule_name,
                            &premise,
                            &strength_value,
                            method,
                        )?;
                        for ordering in &conclusion.orderings {
                            require_order(&mut scope, ordering)?;
                        }
                        scope.conditions.push((condition.pattern, line));
                        check_not_late(&scope, &conclusion.pattern, &conclusion.verb_token)?;
                        scope.facts.push(Fact {
                            pattern: conclusion.pattern,
                            degree: value.clone(),
                            line,
                        });
                        program.conditionals.push(IrConditional {
                            line,
                            name: rule_name,
                            labeled: label.is_some(),
                            method: method.map(Method::name),
                            condition: condition.ir,
                            matched_lines,
                            premise: premise.display_all(),
                            strength: strength_value.display(),
                            conclusion: conclusion.ir,
                            value: value.display_all(),
                            outcomes: scope.outcomes(&value),
                            references: cited.iter().map(|(n, _)| *n).collect(),
                            note,
                            items: Vec::new(),
                        });
                        host = Some(Host::Conditional(program.conditionals.len() - 1));
                        citations.extend(cited);
                        if let Some(label) = label {
                            scope.events.insert(
                                label.text.clone(),
                                EventRecord {
                                    roles: conclusion.roles,
                                },
                            );
                            scope.values.insert(label.text, value);
                        }
                    }
                }
            }
            Statement::Observe { name, expression } => {
                let Some(&node) = scope.unknown_ids.get(&name.text) else {
                    if scope.values.contains_key(&name.text) {
                        return Err(name.error(
                            "K407",
                            format!("'{}'은 미지값이 아니므로 관측할 수 없습니다.", name.text),
                        ));
                    }
                    return Err(unbound(&name, &scope));
                };
                let info = &scope.unknowns[node - 1];
                if info.observed.is_some() {
                    return Err(name.error(
                        "K407",
                        format!("미지값 '{}'은 이미 관측되었습니다.", name.text),
                    ));
                }
                let Bound { value, ir } = bind(expression, &scope)?;
                if value.is_symbolic() {
                    return Err(name.error(
                        "K406",
                        format!("'{}'의 관측값에는 미지값을 쓸 수 없습니다.", name.text),
                    ));
                }
                check_declared_type(&name, info.kind, &value, &scope)?;
                if value.frame != info.frame {
                    let expected = Value::point(info.kind, info.frame, Integer::from_small(0));
                    return Err(name.error(
                        "K306",
                        format!(
                            "미지값 '{}'은 {}인데 관측값은 {}입니다.",
                            name.text,
                            scope.describe(&expected),
                            scope.describe(&value),
                        ),
                    ));
                }
                let upper = Atom {
                    from: ORIGIN,
                    to: node,
                    bound: value.number.clone(),
                };
                let lower = Atom {
                    from: node,
                    to: ORIGIN,
                    bound: value.number.clone().negate(),
                };
                for atom in [upper, lower] {
                    assume(&mut scope, &atom, &name, "관측")?;
                }
                scope.unknowns[node - 1].observed = Some(value.number);
                program.constraints.push(IrConstraint {
                    kind: "관측",
                    name: name.text,
                    expression: ir,
                });
            }
        }
    }

    for (number, token) in &citations {
        if !program.references.iter().any(|r| r.number == *number) {
            return Err(token.error(
                "K511",
                format!("참조 {number}이 정의되지 않았습니다. '참조 {number}: 내용'을 적으세요."),
            ));
        }
    }

    for Pending {
        name,
        is_check,
        judged,
    } in pending
    {
        let (status, missing, undecided) = if judged.value.worlds.is_some() {
            // 세계가 갈라졌다면 어떤 방식에서는 참, 어떤 방식에서는 거짓입니다.
            if is_check {
                return Err(split_check(&name, &judged.value, &scope));
            }
            ("가능", Vec::new(), scope.deciding_rules(&judged.value))
        } else {
            let (status, missing) = judge(&scope, &judged);
            (status, missing, Vec::new())
        };
        if is_check {
            if status != "보장" {
                let reason = if status == "불가능" {
                    "어떤 해에서도 성립하지 않습니다".to_string()
                } else if missing.is_empty() {
                    "일부 해에서만 성립합니다".to_string()
                } else {
                    format!(
                        "일부 해에서만 성립합니다. 보장 여부를 확정하려면 관측이 필요합니다: {}",
                        missing.join(", ")
                    )
                };
                return Err(name.error(
                    "K403",
                    format!(
                        "검사 '{}'은 모든 해에서 보장되지 않습니다: {reason}.",
                        name.text
                    ),
                ));
            }
            program.statements.push(IrStatement {
                kind: "검사",
                name: name.text,
                value_type: Type::Boolean.name(),
                value: "참".into(),
                unit: None,
                frame: None,
                outcomes: None,
                expression: judged.ir,
            });
        } else {
            program.goals.push(IrGoal {
                line: name.line(),
                name: name.text,
                status,
                missing_observations: missing,
                undecided_rules: undecided,
                expression: judged.ir,
            });
        }
    }

    for (index, info) in scope.unknowns.iter().enumerate() {
        let node = index + 1;
        let probe = Value::point(info.kind, info.frame, Integer::from_small(0));
        program.unknowns.push(IrUnknown {
            line: info.line,
            name: info.name.clone(),
            value_type: info.kind.name(),
            unit: info.kind.base_unit(),
            frame: scope.frame_name(&probe),
            min: scope
                .closure
                .upper(node, ORIGIN)
                .map(|bound| bound.clone().negate().to_string()),
            max: scope.closure.upper(ORIGIN, node).map(ToString::to_string),
            observed: info.observed.as_ref().map(ToString::to_string),
        });
    }

    // 본문 점검: 선언이 모두 끝난 뒤에 하므로, 본문보다 뒤에 선언한 이름도 알아봅니다.
    let mut declared = prose::Declared {
        names: scope.values.keys().cloned().collect(),
        amounts: Vec::new(),
        units: Vec::new(),
    };
    declared
        .names
        .extend(scope.frames.iter().map(|(name, _)| name.clone()));
    declared.names.extend(scope.events.keys().cloned());
    for statement in &program.statements {
        if statement.kind == "정의"
            && let Some(value) = scope.values.get(&statement.name)
            && value.worlds.is_none()
            && matches!(value.kind, Type::Time | Type::Distance | Type::Custom(_))
        {
            declared
                .amounts
                .push((statement.name.clone(), value.kind, value.number.clone()));
        }
    }
    for unit in &program.units {
        declared
            .units
            .push((unit.clone(), Type::Custom(syntax::intern(unit)), 1));
    }
    for (unit, kind, factor) in BUILTIN_UNITS {
        if !scope.units.contains(*unit) {
            declared.units.push((unit.to_string(), *kind, *factor));
        }
    }
    (program.mentions, program.warnings) = prose::analyze(&passages, &declared);

    Ok(program)
}

/// 이미 있는 단위와, 기본 단위(밀리초·밀리미터)로 바꾸는 배수.
const BUILTIN_UNITS: &[(&str, Type, u32)] = &[
    ("밀리초", Type::Time, 1),
    ("초", Type::Time, 1000),
    ("분", Type::Time, 60_000),
    ("시간", Type::Time, 3_600_000),
    ("일", Type::Time, 86_400_000),
    ("밀리미터", Type::Distance, 1),
    ("센티미터", Type::Distance, 10),
    ("미터", Type::Distance, 1000),
    ("킬로미터", Type::Distance, 1_000_000),
];

/// 규칙을 적용해 결론의 정도를 구합니다. 방식을 정하지 않았으면 세계를 둘로 나눕니다:
/// 최소로 계산한 세계와 곱으로 계산한 세계.
fn apply_rule(
    scope: &mut Scope,
    token: &Token,
    rule_name: &str,
    premise: &Value,
    strength: &Value,
    method: Option<Method>,
) -> Result<Value, Diagnostic> {
    if let Some(method) = method {
        return lift2(token, premise, strength, scope, |p, s| {
            Ok(Value::degree(method.apply(&p.as_degree(), &s.as_degree())))
        });
    }
    let id = scope.rules.len();
    scope.rules.push(rule_name.to_string());
    let mut results = Vec::new();
    for a in premise.branches() {
        for b in strength.branches() {
            let consistent = a
                .choice
                .iter()
                .all(|(rule, m)| b.choice.get(rule).is_none_or(|other| other == m));
            if !consistent {
                continue;
            }
            for method in [Method::Minimum, Method::Product] {
                let mut choice = a.choice.clone();
                choice.extend(b.choice.clone());
                choice.insert(id, method);
                results.push(World {
                    choice,
                    value: Value::degree(method.apply(&a.value.as_degree(), &b.value.as_degree())),
                });
            }
            if results.len() > MAX_WORLDS {
                return Err(too_many_worlds(token, scope));
            }
        }
    }
    Ok(collapse(results))
}

/// 문장이 말한 사실, 또는 조건문이 이끌어 낸 사실.
struct Fact {
    pattern: sentence::Pattern,
    degree: Value,
    line: usize,
}

fn same_filler(a: &sentence::FactFiller, b: &sentence::FactFiller) -> bool {
    use sentence::FactFiller::{Reference, Value};
    match (a, b) {
        (Value(x), Value(y)) => x.same_scalar(y) && x.terms == y.terms,
        (Reference(x), Reference(y)) => x == y,
        _ => false,
    }
}

/// 사실이 전제의 모양에 해당하는지 봅니다. 행위가 같고, 전제가 말한 역할을 사실이 모두 같은 값으로
/// 가지면 해당합니다. 사실에 역할이 더 있어도 됩니다: "유빈이 복도에서 공을 굴렸다"는
/// "유빈이 공을 굴렸다"를 함의하기 때문입니다(사건 의미론).
fn satisfies(fact: &sentence::Pattern, wanted: &sentence::Pattern) -> bool {
    fact.verb == wanted.verb
        && wanted.roles.iter().all(|(role, filler)| {
            fact.roles
                .iter()
                .any(|(r, f)| r == role && same_filler(f, filler))
        })
}

/// 새 사실이 앞선 조건문의 전제에 해당하면 안 됩니다. 조건문은 자기보다 앞에 적힌 사실만 봅니다.
fn check_not_late(
    scope: &Scope,
    fact: &sentence::Pattern,
    token: &Token,
) -> Result<(), Diagnostic> {
    if let Some((_, line)) = scope
        .conditions
        .iter()
        .find(|(wanted, _)| satisfies(fact, wanted))
    {
        if *line == token.line() {
            return Err(token.error(
                "K552",
                "이 조건문의 결론이 자기 전제에 해당합니다. 결론이 다시 전제가 되는 순환은 쓸 수 없습니다.",
            ));
        }
        return Err(token.error(
            "K552",
            format!(
                "이 사실은 {line}행 조건문의 전제에 해당하지만 그 조건문보다 뒤에 있습니다. 조건문은 앞에 적힌 사실만 보므로, 이 줄을 {line}행 앞으로 옮기세요."
            ),
        ));
    }
    Ok(())
}

/// 미지값이 든 제약과 규칙 방식에 따라 갈라지는 값은 한 판정에 함께 쓸 수 없습니다.
fn reject_mixed(name: &Token, judged: &Judged) -> Result<(), Diagnostic> {
    if judged.value.worlds.is_some() && !judged.atoms.is_empty() {
        return Err(name.error(
            "K541",
            format!(
                "'{}'에 미지값과 규칙의 결과가 함께 들어 있습니다. 둘을 따로 판정하세요.",
                name.text
            ),
        ));
    }
    Ok(())
}

/// 규칙의 방식에 따라 결과가 갈리는 검사. 두 방식 모두에서 참이어야 통과합니다.
fn split_check(name: &Token, value: &Value, scope: &Scope) -> Diagnostic {
    let worlds = value.worlds.as_deref().unwrap_or_default();
    let examples = |truth: bool| {
        worlds
            .iter()
            .filter(|w| w.value.number.is_zero() != truth)
            .take(2)
            .map(|w| format!("({})", scope.describe_choice(&w.choice)))
            .collect::<Vec<_>>()
            .join(" ")
    };
    name.error(
        "K540",
        format!(
            "검사 '{}'의 결과가 규칙의 계산 방식에 따라 달라집니다. 참: {} / 거짓: {}. 방식에 상관없이 성립해야 하며, 방식을 정하려면 규칙 {}에 '최소로' 또는 '곱으로'를 붙이세요.",
            name.text,
            examples(true),
            examples(false),
            scope
                .deciding_rules(value)
                .iter()
                .map(|r| format!("'{r}'"))
                .collect::<Vec<_>>()
                .join(", "),
        ),
    )
}

/// 미지값 하나 또는 두 미지값의 차이에 대한 상한: `to - from <= bound`.
/// 꼭짓점 `ORIGIN`은 값 0(기준 원점)입니다.
struct Atom {
    from: usize,
    to: usize,
    bound: Integer,
}

/// 조건·목표·검사의 식을 비교식들의 '그리고'로 풀어 둔 결과.
struct Judged {
    /// 타입 확인용 값. 미지값이 없으면 실제 결과입니다.
    value: Value,
    /// 미지값이 없는 부분들의 결과를 모두 '그리고'로 묶은 값.
    fixed: bool,
    atoms: Vec<Atom>,
    ir: IrExpression,
}

struct Pending {
    name: Token,
    is_check: bool,
    judged: Judged,
}

struct UnknownInfo {
    line: usize,
    name: String,
    kind: Type,
    frame: Option<usize>,
    observed: Option<Integer>,
}

fn mentions_unknown(expression: &Expression, scope: &Scope) -> bool {
    match expression {
        Expression::Reference(name) => scope.unknown_ids.contains_key(&name.text),
        Expression::Unary(_, operand) | Expression::Anchor(_, _, operand) => {
            mentions_unknown(operand, scope)
        }
        Expression::Binary(_, left, right) => {
            mentions_unknown(left, scope) || mentions_unknown(right, scope)
        }
        Expression::Literal(..) | Expression::Quantity { .. } | Expression::Member(..) => false,
    }
}

fn constrain(expression: Expression, scope: &Scope) -> Result<Judged, Diagnostic> {
    match expression {
        Expression::Binary(op, left, right) if op.kind == Kind::And => {
            let left = constrain(*left, scope)?;
            let right = constrain(*right, scope)?;
            let value = evaluate_binary(&op, &left.value, &right.value, scope)?;
            let mut atoms = left.atoms;
            atoms.extend(right.atoms);
            let fixed = if atoms.is_empty() && value.worlds.is_none() {
                value.kind == Type::Boolean && !value.number.is_zero()
            } else {
                left.fixed && right.fixed
            };
            Ok(Judged {
                value,
                fixed,
                atoms,
                ir: IrExpression::Binary {
                    kind: Type::Boolean.name(),
                    operator: op.text,
                    operands: [Box::new(left.ir), Box::new(right.ir)],
                },
            })
        }
        Expression::Binary(op, left, right)
            if matches!(
                op.kind,
                Kind::Less
                    | Kind::LessEqual
                    | Kind::Greater
                    | Kind::GreaterEqual
                    | Kind::Equal
                    | Kind::NotEqual
            ) =>
        {
            let left = bind(*left, scope)?;
            let right = bind(*right, scope)?;
            // 타입과 기준은 미지값을 뗀 값으로 확인합니다.
            let value = evaluate_binary(&op, &left.value.fixed(), &right.value.fixed(), scope)?;
            let symbolic = left.value.is_symbolic() || right.value.is_symbolic();
            let ir = IrExpression::Binary {
                kind: Type::Boolean.name(),
                operator: op.text.clone(),
                operands: [Box::new(left.ir), Box::new(right.ir)],
            };
            if !symbolic {
                let fixed = !value.number.is_zero();
                return Ok(Judged {
                    value,
                    fixed,
                    atoms: Vec::new(),
                    ir,
                });
            }

            let difference = left.value.minus(&right.value);
            let one = Integer::from_small(1);
            let mut atoms = Vec::new();
            match op.kind {
                Kind::LessEqual => atoms.push(atom(&op, &difference, Integer::from_small(0))?),
                Kind::Less => atoms.push(atom(&op, &difference, one.negate())?),
                Kind::GreaterEqual => {
                    atoms.push(atom(&op, &difference.negated(), Integer::from_small(0))?)
                }
                Kind::Greater => atoms.push(atom(&op, &difference.negated(), one.negate())?),
                Kind::Equal => {
                    atoms.push(atom(&op, &difference, Integer::from_small(0))?);
                    atoms.push(atom(&op, &difference.negated(), Integer::from_small(0))?);
                }
                _ => return Err(unsupported(&op)),
            }
            Ok(Judged {
                value,
                fixed: true,
                atoms,
                ir,
            })
        }
        expression => {
            let Bound { value, ir } = bind(expression, scope)?;
            let fixed = value.kind == Type::Boolean && !value.number.is_zero();
            Ok(Judged {
                value,
                fixed,
                atoms: Vec::new(),
                ir,
            })
        }
    }
}

fn unsupported(op: &Token) -> Diagnostic {
    op.error(
        "K405",
        format!(
            "미지값이 들어간 '{}' 비교는 지원하지 않습니다. 미지값은 '그리고'로 묶은 <, <=, >, >=, == 비교에서 쓸 수 있습니다.",
            op.text
        ),
    )
}

/// `expression <= limit`을 `to - from <= bound` 꼴로 바꿉니다.
fn atom(op: &Token, expression: &Value, limit: Integer) -> Result<Atom, Diagnostic> {
    let bound = limit.sub(&expression.number);
    let one = Integer::from_small(1);
    let minus_one = one.clone().negate();
    let terms: Vec<(usize, Integer)> = expression
        .terms
        .iter()
        .map(|(&node, c)| (node, c.clone()))
        .collect();
    let (from, to) = match terms.as_slice() {
        [(x, c)] if *c == one => (ORIGIN, *x),
        [(x, c)] if *c == minus_one => (*x, ORIGIN),
        [(x, a), (y, b)] if *a == one && *b == minus_one => (*y, *x),
        [(x, a), (y, b)] if *a == minus_one && *b == one => (*x, *y),
        [] => unreachable!("상수 비교는 미지값 없는 경로에서 처리합니다"),
        _ => {
            return Err(op.error(
                "K405",
                "지원하는 제약은 미지값 하나의 상·하한 또는 두 미지값의 차이(가 - 나)에 대한 상·하한입니다.",
            ));
        }
    };
    Ok(Atom { from, to, bound })
}

/// 조건이나 관측을 해 집합에 더합니다. 모순이면 앞선 조건이 정한 범위와 함께 보고합니다.
fn assume(scope: &mut Scope, atom: &Atom, name: &Token, label: &str) -> Result<(), Diagnostic> {
    let Err(back) = scope.closure.add(atom.from, atom.to, &atom.bound) else {
        return Ok(());
    };
    // 앞선 조건: from - to <= back, 즉 to - from >= -back. 새 조건: to - from <= bound.
    let (subject, earlier, requested) = if atom.to == ORIGIN {
        // to - from = -x 이므로 부호를 뒤집어 x에 대해 말합니다.
        let x = atom.from;
        (
            scope.unknowns[x - 1].name.clone(),
            format!("최대 {}", scope.quantity(x, true, &back)),
            format!(
                "최소 {}",
                scope.quantity(x, true, &atom.bound.clone().negate())
            ),
        )
    } else if atom.from == ORIGIN {
        let x = atom.to;
        (
            scope.unknowns[x - 1].name.clone(),
            format!("최소 {}", scope.quantity(x, true, &back.clone().negate())),
            format!("최대 {}", scope.quantity(x, true, &atom.bound)),
        )
    } else {
        (
            format!(
                "{} - {}",
                scope.unknowns[atom.to - 1].name,
                scope.unknowns[atom.from - 1].name
            ),
            format!(
                "최소 {}",
                scope.quantity(atom.to, false, &back.clone().negate())
            ),
            format!("최대 {}", scope.quantity(atom.to, false, &atom.bound)),
        )
    };
    Err(name.error(
        "K404",
        format!(
            "{label} '{}'이 앞선 조건·관측과 모순됩니다. 앞선 조건: '{subject}' {earlier}. 이 {label}: '{subject}' {requested}.",
            name.text
        ),
    ))
}

/// 조건만으로 값이 하나로 정해진 미지값인지(최솟값 = 최댓값). 관측할 필요가 없습니다.
fn pinned(scope: &Scope, node: usize) -> bool {
    match (
        scope.closure.upper(ORIGIN, node),
        scope.closure.upper(node, ORIGIN),
    ) {
        (Some(max), Some(back)) => max.add(back).is_zero(),
        _ => false,
    }
}

/// 모든 조건 아래에서 목표가 보장·가능·불가능 중 무엇인지 판정합니다.
fn judge(scope: &Scope, judged: &Judged) -> (&'static str, Vec<String>) {
    if !judged.fixed {
        return ("불가능", Vec::new());
    }
    let entailed = judged.atoms.iter().all(|atom| {
        scope
            .closure
            .upper(atom.from, atom.to)
            .is_some_and(|upper| *upper <= atom.bound)
    });
    if entailed {
        return ("보장", Vec::new());
    }
    let mut trial = scope.closure.clone();
    if judged
        .atoms
        .iter()
        .any(|atom| trial.add(atom.from, atom.to, &atom.bound).is_err())
    {
        return ("불가능", Vec::new());
    }
    let mut nodes: Vec<usize> = judged
        .atoms
        .iter()
        .flat_map(|atom| [atom.from, atom.to])
        .filter(|&node| {
            node != ORIGIN && scope.unknowns[node - 1].observed.is_none() && !pinned(scope, node)
        })
        .collect();
    nodes.sort_unstable();
    nodes.dedup();
    let missing = nodes
        .into_iter()
        .map(|node| scope.unknowns[node - 1].name.clone())
        .collect();
    ("가능", missing)
}

#[derive(Default)]
struct Scope {
    values: HashMap<String, Value>,
    frame_ids: HashMap<String, usize>,
    frames: Vec<(String, Axis)>,
    events: HashMap<String, EventRecord>,
    unknown_ids: HashMap<String, usize>,
    /// 꼭짓점 `n`의 미지값은 `unknowns[n - 1]`입니다.
    unknowns: Vec<UnknownInfo>,
    closure: Closure,
    /// 방식을 정하지 않은 규칙의 이름. 세계의 선택은 이 목록의 번호로 적습니다.
    rules: Vec<String>,
    /// 지금까지 말했거나 이끌어 낸 사실.
    facts: Vec<Fact>,
    /// 앞선 조건문들의 전제와 그 줄 번호.
    conditions: Vec<(sentence::Pattern, usize)>,
    /// `단위`로 선언한 단위 이름.
    units: HashSet<String>,
    /// 낱말 이름 → (차원들, 미리 정의된 낱말인지)
    words: HashMap<String, (Vec<WordDimension>, bool)>,
}

#[derive(Clone)]
struct WordDimension {
    name: String,
    required: bool,
    weight: u32,
    kind: Option<Type>,
}

/// 미리 정의해 둔 낱말의 차원들: (이름, 타입).
type BuiltinDimensions = &'static [(&'static str, Option<Type>)];

/// 미리 정의해 둔 낱말과 그 차원.
const BUILTIN_WORDS: &[(&str, BuiltinDimensions)] = &[
    (
        "보는자리",
        &[
            ("거리", Some(Type::Distance)),
            ("높이", None),
            ("방향", None),
            ("화각", None),
        ],
    ),
    (
        "빛",
        &[
            ("방향", None),
            ("세기", Some(Type::Degree)),
            ("색", None),
            ("때", None),
        ],
    ),
    ("배경", &[("내용", None)]),
    ("날씨", &[("상태", None), ("세기", Some(Type::Degree))]),
];

impl Default for Closure {
    fn default() -> Self {
        Self::new()
    }
}

impl Scope {
    fn frame_name(&self, value: &Value) -> Option<String> {
        value.frame.map(|id| self.frames[id].0.clone())
    }

    /// 미지값 `node`에 관한 양을 사람이 읽을 수 있게 씁니다.
    /// `absolute`이면 미지값 자체의 값, 아니면 두 미지값의 차이입니다.
    fn quantity(&self, node: usize, absolute: bool, number: &Integer) -> String {
        let info = &self.unknowns[node - 1];
        let kind = if absolute {
            info.kind
        } else {
            info.kind.displacement().unwrap_or(info.kind)
        };
        let amount = match kind.base_unit() {
            Some(unit) => format!("{number} {unit}"),
            None => number.to_string(),
        };
        match (absolute, info.frame) {
            (true, Some(id)) => format!("{} @ {amount}", self.frames[id].0),
            _ => amount,
        }
    }

    fn outcomes(&self, value: &Value) -> Option<Vec<IrOutcome>> {
        let worlds = value.worlds.as_ref()?;
        Some(
            worlds
                .iter()
                .map(|world| IrOutcome {
                    choices: world
                        .choice
                        .iter()
                        .map(|(&rule, method)| (self.rules[rule].clone(), method.name()))
                        .collect(),
                    value: world.value.display(),
                })
                .collect(),
        )
    }

    /// 세계의 선택을 `깨짐 곱, 번짐 최소`처럼 씁니다.
    fn describe_choice(&self, choice: &BTreeMap<usize, Method>) -> String {
        choice
            .iter()
            .map(|(&rule, method)| format!("{} {}", self.rules[rule], method.name()))
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// 판정을 바꾸는 규칙: 그 규칙의 방식만 바꿨을 때 값이 달라지는 세계 쌍이 있는 규칙.
    fn deciding_rules(&self, value: &Value) -> Vec<String> {
        let Some(worlds) = &value.worlds else {
            return Vec::new();
        };
        let mut rules: Vec<usize> = Vec::new();
        for a in worlds {
            for b in worlds {
                let differing: Vec<usize> = a
                    .choice
                    .iter()
                    .filter(|(rule, m)| b.choice.get(rule) != Some(m))
                    .map(|(&rule, _)| rule)
                    .collect();
                if let [rule] = differing.as_slice()
                    && !a.value.same_scalar(&b.value)
                    && !rules.contains(rule)
                {
                    rules.push(*rule);
                }
            }
        }
        rules.sort_unstable();
        rules
            .into_iter()
            .map(|rule| self.rules[rule].clone())
            .collect()
    }

    /// 값을 사람이 읽을 수 있게 씁니다. 미지값이 있으면 `반응 + 200`처럼 씁니다.
    fn show(&self, value: &Value) -> String {
        if !value.is_symbolic() {
            return value.display();
        }
        let one = Integer::from_small(1);
        let zero = Integer::from_small(0);
        let mut text = String::new();
        for (&node, coefficient) in &value.terms {
            let name = &self.unknowns[node - 1].name;
            let negative = *coefficient < zero;
            let magnitude = if negative {
                coefficient.clone().negate()
            } else {
                coefficient.clone()
            };
            let term = if magnitude == one {
                name.clone()
            } else {
                format!("{magnitude} × {name}")
            };
            text = match (text.is_empty(), negative) {
                (true, false) => term,
                (true, true) => format!("-{term}"),
                (false, false) => format!("{text} + {term}"),
                (false, true) => format!("{text} - {term}"),
            };
        }
        if value.number > zero {
            text = format!("{text} + {}", value.number);
        } else if value.number < zero {
            text = format!("{text} - {}", value.number.clone().negate());
        }
        text
    }

    /// 진단 메시지용 타입 이름. 기준 위의 점이면 기준 이름을 덧붙입니다.
    fn describe(&self, value: &Value) -> String {
        match value.frame {
            Some(id) => format!("{}({})", value.kind.name(), self.frames[id].0),
            None => value.kind.name().into(),
        }
    }
}

struct EventRecord {
    roles: HashMap<&'static str, Value>,
}

#[derive(Clone)]
struct Value {
    kind: Type,
    /// 시각·위치가 놓인 기준. 다른 타입에서는 항상 `None`입니다.
    frame: Option<usize>,
    number: Integer,
    /// 대상의 이름.
    label: Option<String>,
    /// 미지값 꼭짓점별 계수. 값은 `number + Σ 계수 × 미지값`입니다.
    terms: BTreeMap<usize, Integer>,
    /// 정도의 소수 자리. 정도는 `number / 100^scale` 퍼센트입니다. 다른 타입에서는 0입니다.
    scale: u32,
    /// 방식을 정하지 않은 규칙 때문에 값이 갈라졌을 때, 방식의 조합(세계)마다의 값.
    /// 각 세계의 값에는 다시 `worlds`가 없습니다.
    worlds: Option<Vec<World>>,
}

/// 퍼지 규칙의 계산 방식.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Method {
    /// Mamdani: min(전제, 강도)
    Minimum,
    /// Larsen: 전제 × 강도
    Product,
}

impl Method {
    fn name(self) -> &'static str {
        match self {
            Self::Minimum => "최소",
            Self::Product => "곱",
        }
    }

    fn apply(self, premise: &Degree, strength: &Degree) -> Degree {
        match self {
            Self::Minimum => premise.clone().min(strength.clone()),
            Self::Product => premise.product(strength),
        }
    }
}

/// 방식을 정하지 않은 규칙마다 고른 방식과, 그때의 값.
#[derive(Clone)]
struct World {
    choice: BTreeMap<usize, Method>,
    value: Value,
}

/// 한 번에 따질 수 있는 세계의 최대 수. 방식을 정하지 않은 규칙 12개까지의 모든 조합입니다.
const MAX_WORLDS: usize = 4096;

impl Value {
    fn plain(kind: Type, number: Integer) -> Self {
        Self {
            kind,
            frame: None,
            number,
            label: None,
            terms: BTreeMap::new(),
            scale: 0,
            worlds: None,
        }
    }

    fn degree(degree: Degree) -> Self {
        Self {
            scale: degree.scale,
            ..Self::plain(Type::Degree, degree.n)
        }
    }

    /// 정도로 읽습니다. 논리값은 참 100%, 거짓 0%입니다.
    fn as_degree(&self) -> Degree {
        if self.kind == Type::Boolean {
            Degree::percent(if self.number.is_zero() { 0 } else { 100 })
        } else {
            Degree::new(self.number.clone(), self.scale)
        }
    }

    /// 세계마다의 값. 갈라지지 않은 값은 빈 선택 하나로 봅니다.
    fn branches(&self) -> Vec<World> {
        match &self.worlds {
            Some(worlds) => worlds.clone(),
            None => vec![World {
                choice: BTreeMap::new(),
                value: self.clone(),
            }],
        }
    }

    fn same_scalar(&self, other: &Self) -> bool {
        self.kind == other.kind
            && self.number == other.number
            && self.scale == other.scale
            && self.label == other.label
            && self.frame == other.frame
    }

    fn point(kind: Type, frame: Option<usize>, number: Integer) -> Self {
        Self {
            kind,
            frame,
            number,
            label: None,
            terms: BTreeMap::new(),
            scale: 0,
            worlds: None,
        }
    }

    fn is_symbolic(&self) -> bool {
        !self.terms.is_empty()
    }

    /// 미지값 부분을 뗀 값. 타입·기준 확인에 씁니다.
    fn fixed(&self) -> Self {
        Self {
            terms: BTreeMap::new(),
            ..self.clone()
        }
    }

    fn negated(&self) -> Self {
        Self {
            number: self.number.clone().negate(),
            terms: self
                .terms
                .iter()
                .map(|(&node, c)| (node, c.clone().negate()))
                .collect(),
            ..self.clone()
        }
    }

    /// 계수를 포함한 뺄셈. 결과의 타입은 호출한 쪽에서 정합니다.
    fn minus(&self, other: &Self) -> Self {
        let mut result = self.clone();
        result.number = self.number.sub(&other.number);
        result.terms = combine(&self.terms, &other.terms, false);
        result
    }

    fn logical(result: bool) -> Self {
        Self::plain(Type::Boolean, Integer::from_small(u32::from(result)))
    }

    fn display(&self) -> String {
        match self.kind {
            Type::Boolean => if self.number.is_zero() {
                "거짓"
            } else {
                "참"
            }
            .into(),
            Type::Entity => self.label.clone().unwrap_or_default(),
            Type::Degree => Degree::new(self.number.clone(), self.scale).display(),
            _ => self.number.to_string(),
        }
    }

    /// 세계마다 값이 다르면 범위로 씁니다. 정도는 `56%~70%`, 논리는 `참|거짓`입니다.
    fn display_all(&self) -> String {
        let Some(worlds) = &self.worlds else {
            return self.display();
        };
        if self.kind == Type::Degree {
            let degrees: Vec<Degree> = worlds.iter().map(|w| w.value.as_degree()).collect();
            let low = degrees.iter().cloned().reduce(Degree::min).unwrap();
            let high = degrees.into_iter().reduce(Degree::max).unwrap();
            return format!("{}~{}", low.display(), high.display());
        }
        let mut shown: Vec<String> = worlds.iter().map(|w| w.value.display()).collect();
        shown.sort();
        shown.dedup();
        shown.join("|")
    }
}

/// 세계별 결과를 모읍니다. 값에 영향을 주지 않는 규칙은 세계에서 빼고,
/// 모든 세계의 값이 같으면 방식과 상관없는 값이므로 하나로 합칩니다.
fn collapse(mut results: Vec<World>) -> Value {
    let rules: Vec<usize> = results
        .iter()
        .flat_map(|w| w.choice.keys().copied())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    for rule in rules {
        // 이 규칙의 방식을 빼고도 값이 하나로 정해지면, 이 규칙은 값에 영향을 주지 않습니다.
        let mut reduced: Vec<World> = Vec::new();
        let mut seen: BTreeMap<BTreeMap<usize, Method>, usize> = BTreeMap::new();
        let mut irrelevant = true;
        for world in &results {
            let mut choice = world.choice.clone();
            choice.remove(&rule);
            match seen.get(&choice) {
                Some(&index) if reduced[index].value.same_scalar(&world.value) => {}
                Some(_) => {
                    irrelevant = false;
                    break;
                }
                None => {
                    seen.insert(choice.clone(), reduced.len());
                    reduced.push(World {
                        choice,
                        value: world.value.clone(),
                    });
                }
            }
        }
        if irrelevant {
            results = reduced;
        }
    }
    let first = results[0].value.clone();
    if results.len() == 1 || results.iter().all(|w| w.value.same_scalar(&first)) {
        return first;
    }
    Value {
        worlds: Some(results),
        ..first
    }
}

/// 두 값의 세계를 짝지어 연산합니다. 같은 규칙에 서로 다른 방식을 고른 짝은 버립니다.
fn lift2(
    op: &Token,
    left: &Value,
    right: &Value,
    scope: &Scope,
    f: impl Fn(&Value, &Value) -> Result<Value, Diagnostic>,
) -> Result<Value, Diagnostic> {
    let mut results = Vec::new();
    for a in left.branches() {
        for b in right.branches() {
            let consistent = a
                .choice
                .iter()
                .all(|(rule, method)| b.choice.get(rule).is_none_or(|m| m == method));
            if !consistent {
                continue;
            }
            let mut choice = a.choice.clone();
            choice.extend(b.choice.clone());
            results.push(World {
                choice,
                value: f(&a.value, &b.value)?,
            });
            if results.len() > MAX_WORLDS {
                return Err(too_many_worlds(op, scope));
            }
        }
    }
    Ok(collapse(results))
}

fn too_many_worlds(token: &Token, _scope: &Scope) -> Diagnostic {
    token.error(
        "K542",
        format!(
            "방식을 정하지 않은 규칙의 조합이 {MAX_WORLDS}가지를 넘습니다. 일부 규칙에 '최소로' 또는 '곱으로'를 붙이세요."
        ),
    )
}

struct Bound {
    value: Value,
    ir: IrExpression,
}

/// 지속 시간과 시각, 길이와 위치처럼 차원은 같지만 역할이 다른 조합인지 확인합니다.
/// 타입 자리에 적은 이름이 선언한 단위인지 확인합니다.
fn known_type(kind: Type, token: &Token, scope: &Scope) -> Result<(), Diagnostic> {
    match kind {
        Type::Custom(name) if !scope.units.contains(name) => Err(token.error(
            "K315",
            format!(
                "'{name}'은 타입도 단위도 아닙니다. 단위로 쓰려면 앞에서 '단위 {name}'으로 선언하세요.",
            ),
        )),
        _ => Ok(()),
    }
}

/// 수 뒤에 적은 낱말을 단위로 읽습니다. 선언한 단위가 먼저이고, 그다음이 `분`·`시간`·`일`·`킬로미터`입니다.
fn unit_of(word: &str, scope: &Scope) -> Option<(Type, u32)> {
    if scope.units.contains(word) {
        return Some((Type::Custom(syntax::intern(word)), 1));
    }
    BUILTIN_UNITS
        .iter()
        .find(|(name, _, _)| *name == word)
        .map(|&(_, kind, factor)| (kind, factor))
}

fn point_vector_pair(a: Type, b: Type) -> bool {
    a.displacement() == Some(b) || b.displacement() == Some(a)
}

fn check_declared_type(
    name: &Token,
    expected: Type,
    value: &Value,
    scope: &Scope,
) -> Result<(), Diagnostic> {
    if value.kind == expected {
        return Ok(());
    }
    if point_vector_pair(expected, value.kind) {
        return Err(name.error(
            "K307",
            format!(
                "'{}'에는 {} 값이 필요하지만 식은 {}입니다. {}",
                name.text,
                expected.name(),
                scope.describe(value),
                point_hint(expected),
            ),
        ));
    }
    Err(name.error(
        "K303",
        format!(
            "'{}'에는 {} 타입이 필요하지만 식은 {} 타입입니다.",
            name.text,
            expected.name(),
            scope.describe(value),
        ),
    ))
}

fn point_hint(expected: Type) -> &'static str {
    match expected {
        Type::Instant => "시각은 '기준 @ 1 초'처럼 시간축 기준 위에 놓아야 합니다.",
        Type::Position => "위치는 '기준 @ 1 미터'처럼 위치축 기준 위에 놓아야 합니다.",
        Type::Time => "두 시각의 차이(종료 - 시작)가 지속 시간입니다.",
        Type::Distance => "두 위치의 차이(도착 - 출발)가 거리입니다.",
        _ => "",
    }
}

fn bind_event(
    name: &Token,
    roles: Vec<Role>,
    scope: &Scope,
) -> Result<(EventRecord, IrEvent, Vec<Ordering>), Diagnostic> {
    let mut orderings = Vec::new();
    let mut values: HashMap<&'static str, Value> = HashMap::new();
    let mut tokens: HashMap<&'static str, Token> = HashMap::new();
    let mut ir_roles = Vec::new();

    for Role {
        name: role,
        expression,
    } in roles
    {
        let Some(&(role_name, expected)) = ROLES.iter().find(|(r, _)| *r == role.text) else {
            let known: Vec<_> = ROLES.iter().map(|(r, _)| *r).collect();
            return Err(role.error(
                "K311",
                format!(
                    "'{}'은 사건 역할이 아닙니다. 사용할 수 있는 역할: {}",
                    role.text,
                    known.join(", "),
                ),
            ));
        };
        if values.contains_key(role_name) {
            return Err(role.error(
                "K310",
                format!(
                    "사건 '{}'에 '{}' 역할이 두 번 선언되었습니다.",
                    name.text, role_name
                ),
            ));
        }

        let Bound { value, ir } = bind(expression, scope)?;
        if value.kind != expected {
            let (code, hint) = if point_vector_pair(expected, value.kind) {
                ("K307", point_hint(expected))
            } else {
                ("K313", "")
            };
            return Err(role.error(
                code,
                format!(
                    "'{}' 역할에는 {} 값이 필요하지만 식은 {}입니다. {}",
                    role_name,
                    expected.name(),
                    scope.describe(&value),
                    hint,
                )
                .trim_end()
                .to_string(),
            ));
        }

        ir_roles.push(IrRole {
            role: role_name.into(),
            value_type: value.kind.name(),
            value: scope.show(&value),
            unit: value.kind.base_unit(),
            frame: scope.frame_name(&value),
            expression: Some(ir),
        });
        values.insert(role_name, value);
        tokens.insert(role_name, role);
    }

    let Some(start) = values.get("시작").cloned() else {
        return Err(name.error(
            "K312",
            format!("사건 '{}'에는 '시작' 역할이 필요합니다.", name.text),
        ));
    };

    match values.get("종료") {
        Some(end) => {
            if end.frame != start.frame {
                return Err(tokens["종료"].error(
                    "K306",
                    format!(
                        "사건 '{}'의 시작은 {}, 종료는 {}입니다. 같은 기준을 사용하세요.",
                        name.text,
                        scope.describe(&start),
                        scope.describe(end),
                    ),
                ));
            }
            orderings.push(Ordering {
                earlier: start.clone(),
                later: end.clone(),
                token: name.clone(),
                what: format!("사건 '{}'의 종료", name.text),
            });
        }
        None => {
            // 종료를 생략한 사건은 한 시각에 일어난 사건으로 봅니다.
            ir_roles.push(IrRole {
                role: "종료".into(),
                value_type: start.kind.name(),
                value: scope.show(&start),
                unit: start.kind.base_unit(),
                frame: scope.frame_name(&start),
                expression: None,
            });
            values.insert("종료", start.clone());
        }
    }

    match (values.get("출발"), values.get("도착")) {
        (Some(from), Some(to)) if from.frame != to.frame => {
            return Err(tokens["도착"].error(
                "K306",
                format!(
                    "사건 '{}'의 출발은 {}, 도착은 {}입니다. 같은 기준을 사용하세요.",
                    name.text,
                    scope.describe(from),
                    scope.describe(to),
                ),
            ));
        }
        (Some(_), None) | (None, Some(_)) => {
            return Err(name.error(
                "K312",
                format!(
                    "사건 '{}'에는 '출발'과 '도착'을 함께 적어야 합니다.",
                    name.text
                ),
            ));
        }
        _ => {}
    }

    Ok((
        EventRecord { roles: values },
        IrEvent {
            name: name.text.clone(),
            roles: ir_roles,
        },
        orderings,
    ))
}

/// `earlier`가 `later`보다 늦지 않아야 한다는 관계. 값이 확정되어 있으면 바로 확인하고,
/// 미지값이 있으면 해 집합에 제약으로 더합니다.
pub(crate) struct Ordering {
    pub(crate) earlier: Value,
    pub(crate) later: Value,
    pub(crate) token: Token,
    /// 진단에 쓰는 설명. 예: "사건 '굴림'의 종료"
    pub(crate) what: String,
}

fn require_order(scope: &mut Scope, ordering: &Ordering) -> Result<(), Diagnostic> {
    let Ordering {
        earlier,
        later,
        token,
        what,
    } = ordering;
    require_before(
        scope,
        earlier,
        later,
        false,
        token,
        "K402",
        format!(
            "{what}({})가 시작({})보다 앞섭니다.",
            scope.show(later),
            scope.show(earlier)
        ),
    )
}

/// `earlier <= later`(엄격하면 `earlier < later`)를 확인하거나 제약으로 더합니다.
/// 두 값은 같은 기준에 있어야 합니다.
#[allow(clippy::too_many_arguments)]
fn require_before(
    scope: &mut Scope,
    earlier: &Value,
    later: &Value,
    strict: bool,
    token: &Token,
    code: &'static str,
    message: String,
) -> Result<(), Diagnostic> {
    let difference = earlier.minus(later);
    let bound = if strict {
        Integer::from_small(1).negate()
    } else {
        Integer::from_small(0)
    };
    if !difference.is_symbolic() {
        return if difference.number > bound {
            Err(token.error(code, message))
        } else {
            Ok(())
        };
    }
    let atom = atom(token, &difference, bound)?;
    scope
        .closure
        .add(atom.from, atom.to, &atom.bound)
        .map_err(|_| {
            token.error(
                code,
                format!("{message} 앞선 조건에 따르면 항상 그렇습니다."),
            )
        })
}

/// 시제를 말하는 시각 `지금`과 사건 시각의 관계로 바꿉니다(Reichenbach의 시제 모형).
/// 과거: 사건이 지금까지 끝남, 현재: 지금이 사건 안에 있음, 미래: 사건이 지금보다 뒤에 시작함.
fn apply_tense(scope: &mut Scope, bound: &sentence::BoundSentence) -> Result<(), Diagnostic> {
    use sentence::Tense;
    let Some(tense) = bound.tense else {
        return Ok(());
    };
    let Some(now) = scope.values.get("지금").cloned() else {
        return Ok(());
    };
    if now.kind != Type::Instant {
        return Ok(());
    }
    let start = bound.roles.get("시작");
    let end = bound.roles.get("종료");
    let token = &bound.verb_token;
    for value in [start, end].into_iter().flatten() {
        if value.frame != now.frame {
            return Err(token.error(
                "K306",
                format!(
                    "시제를 확인하려면 사건 시각과 '지금'이 같은 기준에 있어야 합니다. 사건은 {}, 지금은 {}입니다.",
                    scope.describe(value),
                    scope.describe(&now)
                ),
            ));
        }
    }
    let message = |relation: &str| {
        format!(
            "{} {} 시제이므로 {relation}이어야 하지만 그렇지 않습니다.",
            sentence::josa(&format!("'{}'", token.text), "은", "는"),
            tense.name()
        )
    };
    match tense {
        Tense::Past => {
            if let Some(last) = end.or(start) {
                let text = message("사건이 지금까지 끝난 것");
                require_before(scope, last, &now, false, token, "K530", text)?;
            }
        }
        Tense::Present => {
            if let Some(start) = start {
                let text = message("사건이 지금 이전에 시작한 것");
                require_before(scope, start, &now, false, token, "K530", text)?;
            }
            if let Some(end) = end {
                let text = message("사건이 지금 이후에 끝나는 것");
                require_before(scope, &now, end, false, token, "K530", text)?;
            }
        }
        Tense::Future => {
            if let Some(start) = start {
                let text = message("사건이 지금보다 뒤에 시작하는 것");
                require_before(scope, &now, start, true, token, "K530", text)?;
            }
        }
    }
    Ok(())
}

fn member(event_token: &Token, field: &Token, scope: &Scope) -> Result<Value, Diagnostic> {
    let Some(event) = scope.events.get(&event_token.text) else {
        return Err(event_token.error(
            "K302",
            format!(
                "'{}'을 사용하기 전에 '사건'으로 선언하세요.",
                event_token.text
            ),
        ));
    };
    let roles = &event.roles;
    let derived = |a: &str, b: &str, kind: Type| {
        let (Some(first), Some(last)) = (roles.get(a), roles.get(b)) else {
            return None;
        };
        let mut difference = last.minus(first);
        difference.kind = kind;
        difference.frame = None;
        Some(difference)
    };
    let value = match field.text.as_str() {
        "소요" => derived("시작", "종료", Type::Time),
        "이동" => derived("출발", "도착", Type::Distance),
        role => roles.get(role).cloned(),
    };
    value.ok_or_else(|| {
        field.error(
            "K314",
            format!(
                "사건 '{}'에서 '{}'을 읽을 수 없습니다. 선언한 역할 또는 '소요'·'이동'을 사용하세요.",
                event_token.text, field.text,
            ),
        )
    })
}

fn bind(expression: Expression, scope: &Scope) -> Result<Bound, Diagnostic> {
    let (value, ir) = match expression {
        Expression::Literal(kind, number) => {
            let value = Value::plain(kind, number);
            let ir = IrExpression::Literal {
                kind: kind.name(),
                value: value.display(),
                unit: kind.base_unit(),
            };
            (value, ir)
        }
        Expression::Quantity {
            settled,
            last,
            word,
        } => {
            // `3만 원`은 낱말 전체가 단위이고, `3만원`은 큰 수 낱말과 단위가 붙어 있습니다.
            let whole = unit_of(&word.text, scope).map(|unit| (settled.add(&last), unit));
            let split = || {
                let (factor, rest) = syntax::split_multiplier(&word.text)?;
                let amount = settled.add(&last.mul(&Integer::from_decimal(factor)));
                Some((amount, unit_of(rest, scope)?))
            };
            let Some((amount, (kind, factor))) = whole.or_else(split) else {
                return Err(word.error(
                    "K315",
                    format!(
                        "'{0}'은 단위가 아닙니다. 단위로 쓰려면 앞에서 '단위 {0}'으로 선언하세요. 이미 있는 단위는 밀리초, 초, 분, 시간, 일, 밀리미터, 센티미터, 미터, 킬로미터입니다.",
                        word.text
                    ),
                ));
            };
            let value = Value::plain(kind, amount.mul(&Integer::from_small(factor)));
            let ir = IrExpression::Literal {
                kind: kind.name(),
                value: value.display(),
                unit: kind.base_unit(),
            };
            (value, ir)
        }
        Expression::Reference(name) => {
            let Some(value) = scope.values.get(&name.text).cloned() else {
                return Err(unbound(&name, scope));
            };
            let ir = IrExpression::Reference {
                kind: value.kind.name(),
                name: name.text,
            };
            (value, ir)
        }
        Expression::Member(event, field) => {
            let value = member(&event, &field, scope)?;
            let ir = IrExpression::Member {
                kind: value.kind.name(),
                event: event.text,
                role: field.text,
            };
            (value, ir)
        }
        Expression::Anchor(frame, at, operand) => {
            let Some(&id) = scope.frame_ids.get(&frame.text) else {
                return Err(frame.error(
                    "K308",
                    format!(
                        "'@' 앞에는 '기준'으로 선언한 이름이 필요합니다. '{}'은 기준이 아닙니다.",
                        frame.text
                    ),
                ));
            };
            let axis = scope.frames[id].1;
            let operand = bind(*operand, scope)?;
            if operand.value.kind != axis.offset() {
                return Err(at.error(
                    "K308",
                    format!(
                        "{} 기준 '{}'에는 {} 값을 놓아야 하지만 '{}'이 주어졌습니다.",
                        axis.name(),
                        frame.text,
                        axis.offset().name(),
                        scope.describe(&operand.value),
                    ),
                ));
            }
            let mut value = Value::point(axis.point(), Some(id), operand.value.number);
            value.terms = operand.value.terms;
            let ir = IrExpression::Anchor {
                kind: value.kind.name(),
                frame: frame.text,
                operands: [Box::new(operand.ir)],
            };
            (value, ir)
        }
        Expression::Unary(op, operand) => {
            let operand = bind(*operand, scope)?;
            let value = evaluate_unary(&op, operand.value, scope)?;
            let ir = IrExpression::Unary {
                kind: value.kind.name(),
                operator: op.text,
                operands: [Box::new(operand.ir)],
            };
            (value, ir)
        }
        Expression::Binary(op, left, right) => {
            let left = bind(*left, scope)?;
            let right = bind(*right, scope)?;
            let value = evaluate_binary(&op, &left.value, &right.value, scope)?;
            let ir = IrExpression::Binary {
                kind: value.kind.name(),
                operator: op.text,
                operands: [Box::new(left.ir), Box::new(right.ir)],
            };
            (value, ir)
        }
    };
    Ok(Bound { value, ir })
}

fn unbound(name: &Token, scope: &Scope) -> Diagnostic {
    if let Some(&id) = scope.frame_ids.get(&name.text) {
        let unit = match scope.frames[id].1 {
            Axis::Time => "0 초",
            Axis::Space => "0 미터",
        };
        return name.error(
            "K309",
            format!(
                "'{}'은 기준이므로 값으로 쓸 수 없습니다. '{} @ {unit}'처럼 기준 위의 점을 만드세요.",
                name.text, name.text,
            ),
        );
    }
    if scope.events.contains_key(&name.text) {
        return name.error(
            "K309",
            format!(
                "'{}'은 사건이므로 값으로 쓸 수 없습니다. '{}.시작'처럼 역할을 고르세요.",
                name.text, name.text,
            ),
        );
    }
    name.error(
        "K302",
        format!("'{}'을 사용하기 전에 '정의'로 선언하세요.", name.text),
    )
}

fn evaluate_unary(op: &Token, value: Value, scope: &Scope) -> Result<Value, Diagnostic> {
    if let Some(worlds) = &value.worlds {
        let mut results = Vec::new();
        for world in worlds {
            results.push(World {
                choice: world.choice.clone(),
                value: evaluate_unary(op, world.value.clone(), scope)?,
            });
        }
        return Ok(collapse(results));
    }
    match op.kind {
        Kind::Minus if value.kind.is_amount() => Ok(value.negated()),
        Kind::Not if value.kind == Type::Boolean => Ok(Value::logical(value.number.is_zero())),
        // 퍼지 부정: 100% - 정도
        Kind::Not if value.kind == Type::Degree => {
            Ok(Value::degree(value.as_degree().complement()))
        }
        _ => Err(op.error(
            "K304",
            format!(
                "'{}' 연산은 {} 타입에 사용할 수 없습니다.",
                op.text,
                scope.describe(&value),
            ),
        )),
    }
}

/// 두 점이 같은 기준에 있는지 확인합니다.
fn same_frame(op: &Token, left: &Value, right: &Value, scope: &Scope) -> Result<(), Diagnostic> {
    if left.frame == right.frame {
        return Ok(());
    }
    Err(op.error(
        "K306",
        format!(
            "'{}' 연산의 두 값이 서로 다른 기준에 있습니다. 왼쪽은 {}, 오른쪽은 {}입니다.",
            op.text,
            scope.describe(left),
            scope.describe(right),
        ),
    ))
}

fn combine(
    left: &BTreeMap<usize, Integer>,
    right: &BTreeMap<usize, Integer>,
    add: bool,
) -> BTreeMap<usize, Integer> {
    let mut terms = left.clone();
    for (&node, coefficient) in right {
        let current = terms
            .remove(&node)
            .unwrap_or_else(|| Integer::from_small(0));
        let next = if add {
            current.add(coefficient)
        } else {
            current.sub(coefficient)
        };
        if !next.is_zero() {
            terms.insert(node, next);
        }
    }
    terms
}

fn scale(terms: &BTreeMap<usize, Integer>, factor: &Integer) -> BTreeMap<usize, Integer> {
    if factor.is_zero() {
        return BTreeMap::new();
    }
    terms
        .iter()
        .map(|(&node, c)| (node, c.mul(factor)))
        .collect()
}

/// 미지값 계수까지 함께 계산합니다. 비교·논리 연산의 결과는 미지값을 담을 수 없습니다.
fn evaluate_binary(
    op: &Token,
    left: &Value,
    right: &Value,
    scope: &Scope,
) -> Result<Value, Diagnostic> {
    if left.worlds.is_some() || right.worlds.is_some() {
        return lift2(op, left, right, scope, |a, b| {
            evaluate_binary(op, a, b, scope)
        });
    }
    let mut result = evaluate_fixed(op, &left.fixed(), &right.fixed(), scope)?;
    if !left.is_symbolic() && !right.is_symbolic() {
        return Ok(result);
    }
    result.terms = match op.kind {
        Kind::Plus => combine(&left.terms, &right.terms, true),
        Kind::Minus => combine(&left.terms, &right.terms, false),
        Kind::Star if left.is_symbolic() && right.is_symbolic() => {
            return Err(op.error(
                "K405",
                "미지값끼리 곱할 수 없습니다. 제약은 미지값에 대해 일차식이어야 합니다.",
            ));
        }
        Kind::Star if left.is_symbolic() => scale(&left.terms, &right.number),
        Kind::Star => scale(&right.terms, &left.number),
        _ => return Err(unsupported(op)),
    };
    Ok(result)
}

fn evaluate_fixed(
    op: &Token,
    left: &Value,
    right: &Value,
    scope: &Scope,
) -> Result<Value, Diagnostic> {
    use Type::*;

    let (l, r) = (left.kind, right.kind);
    let (a, b) = (&left.number, &right.number);
    let vector = l == r && l.is_amount();
    let point = l == r && matches!(l, Instant | Position);
    let mismatch = || {
        let (code, hint) = if point_vector_pair(l, r) {
            (
                "K307",
                " 시각과 지속 시간(위치와 거리)은 비교할 수 없습니다. 점끼리 빼서 차이를 구하세요.",
            )
        } else {
            ("K304", "")
        };
        op.error(
            code,
            format!(
                "'{}' 연산에 사용할 수 없는 조합입니다. 왼쪽은 {}, 오른쪽은 {}입니다.{hint}",
                op.text,
                scope.describe(left),
                scope.describe(right),
            ),
        )
    };
    let adds_points = || {
        op.error(
            "K305",
            format!(
                "'{}' 연산으로 '{}'과 '{}'을 계산할 수 없습니다. 기준 위의 점에는 차이(시간·거리)만 더하거나 뺄 수 있습니다.",
                op.text,
                scope.describe(left),
                scope.describe(right),
            ),
        )
    };

    let result = match op.kind {
        Kind::Plus => match (l, r) {
            _ if vector => Value::plain(l, a.add(b)),
            (Instant, Time) | (Position, Distance) => Value::point(l, left.frame, a.add(b)),
            (Time, Instant) | (Distance, Position) => Value::point(r, right.frame, a.add(b)),
            _ if point => return Err(adds_points()),
            _ => return Err(mismatch()),
        },
        Kind::Minus => match (l, r) {
            _ if vector => Value::plain(l, a.sub(b)),
            (Instant, Time) | (Position, Distance) => Value::point(l, left.frame, a.sub(b)),
            _ if point => {
                same_frame(op, left, right, scope)?;
                Value::plain(l.displacement().unwrap_or(l), a.sub(b))
            }
            (Time, Instant) | (Distance, Position) => return Err(adds_points()),
            _ => return Err(mismatch()),
        },
        Kind::Star => match (l, r) {
            (Integer, _) if r.is_amount() => Value::plain(r, a.mul(b)),
            (_, Integer) if l.is_amount() => Value::plain(l, a.mul(b)),
            _ => return Err(mismatch()),
        },
        Kind::Less | Kind::LessEqual | Kind::Greater | Kind::GreaterEqual => {
            if point {
                same_frame(op, left, right, scope)?;
            } else if !(vector || (l == r && l == Degree)) {
                return Err(mismatch());
            }
            let order = if l == Degree {
                left.as_degree().compare(&right.as_degree())
            } else {
                a.cmp(b)
            };
            Value::logical(match op.kind {
                Kind::Less => order.is_lt(),
                Kind::LessEqual => order.is_le(),
                Kind::Greater => order.is_gt(),
                _ => order.is_ge(),
            })
        }
        Kind::Equal | Kind::NotEqual => {
            if l != r {
                return Err(mismatch());
            }
            same_frame(op, left, right, scope)?;
            let equal = if l == Degree {
                left.as_degree() == right.as_degree()
            } else {
                a == b
            };
            Value::logical(equal == (op.kind == Kind::Equal))
        }
        // 퍼지 논리(Zadeh): 그리고 = 최솟값, 또는 = 최댓값. 논리값은 참 100%, 거짓 0%로 봅니다.
        Kind::And | Kind::Or
            if matches!(l, Boolean | Degree)
                && matches!(r, Boolean | Degree)
                && (l == Degree || r == Degree) =>
        {
            let (x, y) = (left.as_degree(), right.as_degree());
            Value::degree(if op.kind == Kind::And {
                x.min(y)
            } else {
                x.max(y)
            })
        }
        Kind::And if l == Boolean && r == Boolean => Value::logical(!a.is_zero() && !b.is_zero()),
        Kind::Or if l == Boolean && r == Boolean => Value::logical(!a.is_zero() || !b.is_zero()),
        _ => return Err(mismatch()),
    };
    Ok(result)
}

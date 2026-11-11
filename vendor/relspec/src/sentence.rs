//! 한국어 문장 문법.
//!
//! 문장 하나는 사건 하나입니다(신-데이비드슨 사건 의미론). 낱말 끝의 조사가 그 낱말이
//! 사건에서 맡는 역할(주체, 대상, 시작 ...)을 정하고, 마지막 낱말이 행위(동사)입니다.
//! 행위마다 반드시 채워야 하는 역할(논항 구조)을 선언하며, 빠지면 컴파일되지 않습니다.

use std::collections::HashMap;

use crate::syntax::{Kind, Token, Type};
use crate::{Diagnostic, IrClause, IrSentence, IrSentenceRole, Method, Ordering, Scope, Value};

/// 문장에서 쓸 수 있는 역할. 행위 선언의 필수 역할도 이 이름으로 적습니다.
pub(crate) const SENTENCE_ROLES: &[&str] = &[
    "주체", "대상", "상대", "도구", "시작", "종료", "장소", "출발", "도착", "방식",
];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Particle {
    /// 이/가/은/는
    Subject,
    /// 을/를
    Object,
    /// 에게/한테
    Recipient,
    /// 으로/로
    Instrument,
    /// 에: 시각이면 시작, 위치면 장소
    At,
    /// 에서: 장소
    In,
    /// 부터: 시각이면 시작, 위치면 출발
    From,
    /// 까지: 시각이면 종료, 위치면 도착
    Until,
}

/// 긴 조사를 먼저 확인합니다(`에게`를 `에`보다 먼저).
const PARTICLES: &[(&str, Particle)] = &[
    ("에게", Particle::Recipient),
    ("한테", Particle::Recipient),
    ("에서", Particle::In),
    ("부터", Particle::From),
    ("까지", Particle::Until),
    ("으로", Particle::Instrument),
    ("이", Particle::Subject),
    ("가", Particle::Subject),
    ("은", Particle::Subject),
    ("는", Particle::Subject),
    ("을", Particle::Object),
    ("를", Particle::Object),
    ("에", Particle::At),
    ("로", Particle::Instrument),
];

/// 지시 부사: 문장 전체의 방식을 가리키며 참조가 필요합니다.
const DEMONSTRATIVE_ADVERBS: &[&str] = &["이렇게", "그렇게", "저렇게"];
/// 지시 대명사: 조사와 함께 역할을 채우며 참조가 필요합니다.
const DEMONSTRATIVE_NOUNS: &[&str] = &["이것", "그것", "저것"];

/// 행위 선언.
pub(crate) struct Verb {
    pub(crate) name: String,
    /// 기본형에서 '다'를 뗀 어간.
    pub(crate) stem: Vec<char>,
    forms: Vec<String>,
    pub(crate) required: Vec<&'static str>,
}

impl Verb {
    pub(crate) fn new(
        name: &Token,
        forms: &[Token],
        required: &[Token],
    ) -> Result<Self, Diagnostic> {
        let mut roles = vec!["주체"];
        for role in required {
            let Some(&known) = SENTENCE_ROLES.iter().find(|r| **r == role.text) else {
                return Err(role.error(
                    "K311",
                    format!(
                        "'{}'은 문장 역할이 아닙니다. 사용할 수 있는 역할: {}",
                        role.text,
                        SENTENCE_ROLES.join(", ")
                    ),
                ));
            };
            if !roles.contains(&known) {
                roles.push(known);
            }
        }
        let mut stem: Vec<char> = name.text.chars().collect();
        stem.pop();
        Ok(Self {
            name: name.text.clone(),
            stem,
            forms: forms.iter().map(|form| form.text.clone()).collect(),
            required: roles,
        })
    }

    /// 르 불규칙: 어간 끝 '르'의 'ㅡ'가 빠지고 앞 음절에 'ㄹ' 받침이 붙습니다
    /// (구르다 → 굴렀다, 부르다 → 불러, 흐르다 → 흘렀다).
    fn ru_irregular(&self, chars: &[char]) -> bool {
        const RIEUL: u32 = 8;
        const RIEUL_INITIAL: u32 = 5;
        let n = self.stem.len();
        if n < 2 || chars.len() < n || chars.last() != Some(&'다') {
            return false;
        }
        let before = &self.stem[..n - 2];
        if chars[..n - 2] != *before {
            return false;
        }
        let (Some(stem), Some(form), Some(next)) = (
            jamo(self.stem[n - 2]),
            jamo(chars[n - 2]),
            jamo(chars[n - 1]),
        ) else {
            return false;
        };
        stem.2 == 0
            && form.0 == stem.0
            && form.1 == stem.1
            && form.2 == RIEUL
            && next.0 == RIEUL_INITIAL
    }

    /// '하다'의 활용형인지(기능동사 구문의 뒷부분인지) 확인합니다.
    fn is_light(word: &str) -> bool {
        let chars: Vec<char> = word.chars().collect();
        chars.last() == Some(&'다')
            && chars
                .first()
                .and_then(|&c| jamo(c))
                .is_some_and(|(initial, medial, _)| {
                    const HIEUH: u32 = 18;
                    initial == HIEUH && (medial == 0 || medial == 1)
                })
    }

    /// 조건문의 전제를 닫는 꼴인지 확인합니다: 어간 + '-면'(받침이 없거나 ㄹ 받침) 또는 '-으면'(그 밖의 받침).
    /// 불규칙 활용(듣다 → 들으면)은 선언의 괄호에 적은 꼴과 견줍니다.
    pub(crate) fn matches_conditional(&self, word: &str) -> bool {
        const RIEUL: u32 = 8;
        if self.forms.iter().any(|form| form == word) {
            return word.ends_with('면');
        }
        let stem: String = self.stem.iter().collect();
        let batchim = self.stem.last().and_then(|&c| jamo(c)).map(|j| j.2);
        match batchim {
            Some(0) | Some(RIEUL) => word.strip_suffix('면') == Some(stem.as_str()),
            Some(_) => word.strip_suffix("으면") == Some(stem.as_str()),
            None => false,
        }
    }

    /// 문장의 마지막 낱말이 이 행위의 활용형인지 확인합니다.
    ///
    /// 어간의 마지막 음절을 뺀 앞부분은 그대로 있어야 하고, 마지막 음절은 같은 초성에
    /// 같은 중성이거나 한국어 모음 축약(하→했, 지→졌, 오→왔 ...)으로 바뀐 중성이어야 합니다.
    /// 받침이 있는 어간(먹다)은 그 음절이 그대로 있어야 합니다. 불규칙 활용(듣다→들었다)은
    /// 선언에 활용형을 직접 적습니다.
    fn matches(&self, word: &str) -> bool {
        if word == self.name || self.forms.iter().any(|form| form == word) {
            return true;
        }
        let chars: Vec<char> = word.chars().collect();
        let Some((last_stem, prefix)) = self.stem.split_last() else {
            return false;
        };
        if *last_stem == '르' && self.ru_irregular(&chars) {
            return true;
        }
        if chars.len() < self.stem.len() || chars.last() != Some(&'다') {
            return false;
        }
        if chars[..prefix.len()] != *prefix {
            return false;
        }
        let candidate = chars[prefix.len()];
        let (Some(stem), Some(form)) = (jamo(*last_stem), jamo(candidate)) else {
            return candidate == *last_stem;
        };
        if stem.2 != 0 {
            return candidate == *last_stem;
        }
        stem.0 == form.0 && (stem.1 == form.1 || contracts(stem.1, form.1))
    }
}

/// 모음 축약으로 바뀌는 중성 쌍(어간 → 활용형). 중성 번호는 유니코드 한글 음절 순서입니다.
fn contracts(stem: u32, form: u32) -> bool {
    const A: u32 = 0;
    const AE: u32 = 1;
    const EO: u32 = 4;
    const YEO: u32 = 6;
    const O: u32 = 8;
    const WA: u32 = 9;
    const WAE: u32 = 10;
    const OE: u32 = 11;
    const U: u32 = 13;
    const WEO: u32 = 14;
    const EU: u32 = 18;
    const I: u32 = 20;
    matches!(
        (stem, form),
        (A, AE) | (I, YEO) | (O, WA) | (U, WEO) | (OE, WAE) | (EU, EO) | (EU, A)
    )
}

/// 한글 음절을 (초성, 중성, 종성) 번호로 나눕니다. 종성 0은 받침 없음입니다.
fn jamo(ch: char) -> Option<(u32, u32, u32)> {
    let code = u32::from(ch).checked_sub(0xAC00)?;
    if code >= 11172 {
        return None;
    }
    Some((code / 588, (code % 588) / 28, code % 28))
}

/// 진단 메시지용: 낱말 뒤에 받침에 맞는 조사를 붙입니다. 한글이 아니면 둘을 함께 적습니다.
pub(crate) fn josa(word: &str, with_batchim: &str, without: &str) -> String {
    // 따옴표·괄호는 건너뛰고 마지막 글자로 판단합니다.
    match word
        .chars()
        .rev()
        .find(|c| c.is_alphanumeric())
        .and_then(jamo)
    {
        Some((_, _, 0)) => format!("{word}{without}"),
        Some(_) => format!("{word}{with_batchim}"),
        None => format!("{word}{with_batchim}({without})"),
    }
}

/// 진단 메시지에서 따옴표로 묶은 낱말 바로 뒤의 조사를 받침에 맞게 고칩니다.
/// 예: `'실험시계'은` → `'실험시계'는`, `'유빈'가` → `'유빈'이`.
/// 따옴표 안의 마지막 글자가 한글 음절이 아니면 그대로 둡니다.
pub(crate) fn fix_particles(message: &str) -> String {
    const PAIRS: &[(&str, &str)] = &[
        ("으로", "로"),
        ("은", "는"),
        ("이", "가"),
        ("을", "를"),
        ("과", "와"),
    ];
    let mut out = String::with_capacity(message.len());
    let mut rest = message;
    let mut last_quoted: Option<char> = None;
    let mut last_char: Option<char> = None;
    let mut inside = false;
    while let Some(ch) = rest.chars().next() {
        rest = &rest[ch.len_utf8()..];
        out.push(ch);
        if ch != '\'' {
            // 괄호·기호는 건너뛰고 마지막 글자로 판단합니다: '시각(실험시계)' → 계
            if inside {
                last_char = Some(ch);
                if ch.is_alphanumeric() {
                    last_quoted = Some(ch);
                }
            }
            continue;
        }
        inside = !inside;
        if inside {
            last_quoted = None;
            last_char = None;
            continue;
        }
        // 닫는 따옴표 뒤: 조사인지 확인합니다. '= 기준 @ ?'처럼 기호로 끝나면 그대로 둡니다.
        let reading = match last_char {
            Some(c) if c.is_alphanumeric() => Some(c),
            Some(')') => last_quoted,
            _ => None,
        };
        let Some((_, _, final_consonant)) = reading.and_then(jamo) else {
            continue;
        };
        for &(with, without) in PAIRS {
            let found = [with, without].into_iter().find(|p| {
                rest.strip_prefix(p).is_some_and(|after| {
                    after
                        .chars()
                        .next()
                        .is_none_or(|c| c.is_whitespace() || ".,:;)!".contains(c))
                })
            });
            if let Some(found) = found {
                const RIEUL: u32 = 8;
                let correct = if with == "으로" {
                    if final_consonant != 0 && final_consonant != RIEUL {
                        with
                    } else {
                        without
                    }
                } else if final_consonant != 0 {
                    with
                } else {
                    without
                };
                out.push_str(correct);
                rest = &rest[found.len()..];
                break;
            }
        }
    }
    out
}

/// 앞말의 받침에 따라 맞는 조사 형태를 돌려줍니다. 맞으면 `None`.
fn particle_form_error(stem: &str, particle: &str) -> Option<&'static str> {
    const RIEUL: u32 = 8;
    let (_, _, last) = jamo(stem.chars().last()?)?;
    let batchim = last != 0;
    let expected = match particle {
        "이" | "가" => {
            if batchim {
                "이"
            } else {
                "가"
            }
        }
        "은" | "는" => {
            if batchim {
                "은"
            } else {
                "는"
            }
        }
        "을" | "를" => {
            if batchim {
                "을"
            } else {
                "를"
            }
        }
        "으로" | "로" => {
            if batchim && last != RIEUL {
                "으로"
            } else {
                "로"
            }
        }
        _ => return None,
    };
    (expected != particle).then_some(expected)
}

/// 역할을 채운 값. 지시어는 참조 번호를 가리킵니다.
enum Filler {
    Value(Value),
    /// 문장 끝 참조 목록에서 몇 번째 참조를 가리키는지.
    Reference(usize),
}

/// 동사의 시제. 말하는 시각 `지금`과 사건 시각의 관계로 바뀝니다.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Tense {
    /// -았/었/였- (했다, 떨어졌다): 사건이 지금까지 끝남
    Past,
    /// -ㄴ다/-는다 (한다, 먹는다): 지금이 사건 안에 있음
    Present,
    /// -겠- (하겠다): 사건이 지금보다 뒤에 시작함
    Future,
}

impl Tense {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Past => "과거",
            Self::Present => "현재",
            Self::Future => "미래",
        }
    }

    /// 활용형의 선어말어미로 시제를 읽습니다. 기본형(설계하다)은 시제가 없습니다.
    fn of(verb: &Verb, word: &str) -> Option<Self> {
        const SSANG_SIOT: u32 = 20;
        const NIEUN: u32 = 4;
        if word == verb.name {
            return None;
        }
        let chars: Vec<char> = word.chars().collect();
        let (last, body) = chars.split_last()?;
        if *last != '다' {
            return None;
        }
        if body.contains(&'겠') {
            return Some(Self::Future);
        }
        // 어간 앞부분(바뀌지 않는 음절)은 건너뛰어 '있다'의 '있' 같은 받침을 시제로 읽지 않습니다.
        let skip = verb.stem.len().saturating_sub(1).min(body.len());
        let ending = &body[skip..];
        if ending
            .iter()
            .any(|&c| jamo(c).is_some_and(|j| j.2 == SSANG_SIOT))
        {
            return Some(Self::Past);
        }
        let penultimate = body.last().and_then(|&c| jamo(c));
        if word.ends_with("는다") || penultimate.is_some_and(|j| j.2 == NIEUN) {
            return Some(Self::Present);
        }
        None
    }
}

pub(crate) struct BoundSentence {
    pub(crate) ir: IrSentence,
    pub(crate) degree: u32,
    /// 값으로 채운 역할. 이름 붙은 문장은 `이름.역할`로 읽습니다.
    pub(crate) roles: HashMap<&'static str, Value>,
    /// 시간 순서처럼 해 집합에 더해야 하는 관계.
    pub(crate) orderings: Vec<Ordering>,
    pub(crate) tense: Option<Tense>,
    pub(crate) verb_token: Token,
    /// 문장이 인용한 참조 번호와 그 위치. 정의되었는지는 파일 끝에서 확인합니다.
    pub(crate) citations: Citations,
    /// 이 문장이 말하는 사실. 조건문의 전제와 견줍니다.
    pub(crate) pattern: Pattern,
}

/// 사실과 전제를 견줄 때 쓰는 역할 값. 지시어는 가리키는 참조 번호로 견줍니다.
#[derive(Clone)]
pub(crate) enum FactFiller {
    Value(Value),
    Reference(u32),
}

/// 행위와 역할들. 문장이 말하는 사실이자, 조건문의 전제가 찾는 모양입니다.
#[derive(Clone)]
pub(crate) struct Pattern {
    pub(crate) verb: String,
    pub(crate) roles: Vec<(&'static str, FactFiller)>,
}

/// 조건문의 전제나 결론 하나.
pub(crate) struct BoundClause {
    pub(crate) ir: IrClause,
    pub(crate) pattern: Pattern,
    pub(crate) roles: HashMap<&'static str, Value>,
    pub(crate) orderings: Vec<Ordering>,
    pub(crate) verb_token: Token,
}

/// `컵이 떨어지면 컵이 80% 깨진다`
pub(crate) struct BoundConditional {
    pub(crate) condition: BoundClause,
    pub(crate) conclusion: BoundClause,
    /// 결론에 적은 정도. 규칙의 강도이며, 적지 않으면 100입니다.
    pub(crate) strength: u32,
    pub(crate) method: Option<Method>,
    pub(crate) citations: Citations,
}

pub(crate) enum BoundLine {
    Plain(Box<BoundSentence>),
    Conditional(Box<BoundConditional>),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    /// 사실을 말하는 문장
    Assert,
    /// 조건문의 전제: 행위가 '-면'으로 끝납니다.
    Condition,
    /// 조건문의 결론
    Conclusion,
}

struct Clause<'a> {
    verb: &'a Verb,
    verb_token: Token,
    degree: Option<u32>,
    method: Option<Method>,
    /// 이 절까지 나온 지시어의 수.
    demonstratives: usize,
    ir_roles: Vec<IrSentenceRole>,
    values: HashMap<&'static str, Value>,
    orderings: Vec<Ordering>,
    pattern: Pattern,
}

impl Clause<'_> {
    fn into_bound(self) -> BoundClause {
        BoundClause {
            ir: IrClause {
                verb: self.verb.name.clone(),
                form: self.verb_token.text.clone(),
                roles: self.ir_roles,
            },
            pattern: self.pattern,
            roles: self.values,
            orderings: self.orderings,
            verb_token: self.verb_token,
        }
    }
}

/// '-면'으로 끝나 조건문의 전제를 닫는 낱말인지 봅니다. 선언된 이름(예: 대상 '화면')은 제외합니다.
fn closes_condition(token: &Token, scope: &Scope) -> bool {
    token.kind == Kind::Identifier
        && token.text.ends_with('면')
        && token.text.chars().count() > 1
        && !scope.values.contains_key(&token.text)
}

pub(crate) fn bind_line(
    label: Option<&Token>,
    tokens: &[Token],
    scope: &Scope,
    verbs: &[Verb],
) -> Result<BoundLine, Diagnostic> {
    let first = &tokens[0];
    let (body, citations) = split_citations(tokens)?;
    let last = body.len().saturating_sub(1);

    let split = body[..last]
        .iter()
        .position(|token| closes_condition(token, scope));
    let Some(split) = split else {
        if let Some(end) = body.last()
            && closes_condition(end, scope)
            && verbs.iter().any(|v| v.matches_conditional(&end.text))
        {
            return Err(end.error(
                "K550",
                format!(
                    "'{}' 뒤에 결론이 없습니다. '컵이 떨어지면 컵이 깨진다'처럼 '-면' 뒤에 결론 문장을 적으세요.",
                    end.text
                ),
            ));
        }
        let clause = bind_clause(body, &citations, 0, Mode::Assert, scope, verbs, first)?;
        let degree = clause.degree.unwrap_or(100);
        let tense = Tense::of(clause.verb, &clause.verb_token.text);
        return Ok(BoundLine::Plain(Box::new(BoundSentence {
            ir: IrSentence {
                line: first.line(),
                label: label.map(|l| l.text.clone()),
                verb: clause.verb.name.clone(),
                form: clause.verb_token.text.clone(),
                degree,
                tense: tense.map(Tense::name),
                roles: clause.ir_roles,
                references: citations.iter().map(|(n, _)| *n).collect(),
                note: None,
                items: Vec::new(),
            },
            degree,
            roles: clause.values,
            orderings: clause.orderings,
            tense,
            verb_token: clause.verb_token,
            citations,
            pattern: clause.pattern,
        })));
    };

    let (condition_body, mut conclusion_body) = body.split_at(split + 1);
    // 계산 방식은 결론 뒤에 붙여 쓸 수 있습니다: '... 컵이 80% 깨진다 곱으로'
    let mut trailing = None;
    if let Some((end, rest)) = conclusion_body.split_last()
        && end.kind == Kind::Identifier
        && !rest.is_empty()
    {
        let method = match end.text.as_str() {
            "최소로" => Some(Method::Minimum),
            "곱으로" => Some(Method::Product),
            _ => None,
        };
        if method.is_some() {
            trailing = method.map(|m| (m, end.clone()));
            conclusion_body = rest;
        }
    }
    if let Some(extra) = conclusion_body[..conclusion_body.len() - 1]
        .iter()
        .find(|token| closes_condition(token, scope))
    {
        return Err(extra.error(
            "K550",
            "조건문 하나에는 '-면'을 한 번만 씁니다. 이어지는 규칙은 줄을 나누어 적으세요.",
        ));
    }
    let condition = bind_clause(
        condition_body,
        &citations,
        0,
        Mode::Condition,
        scope,
        verbs,
        first,
    )?;
    let conclusion = bind_clause(
        conclusion_body,
        &citations,
        condition.demonstratives,
        Mode::Conclusion,
        scope,
        verbs,
        first,
    )?;
    let method = match (conclusion.method, trailing) {
        (Some(_), Some((_, token))) => {
            return Err(token.error("K505", "계산 방식은 한 번만 적습니다."));
        }
        (inner, trailing) => inner.or(trailing.map(|(m, _)| m)),
    };
    Ok(BoundLine::Conditional(Box::new(BoundConditional {
        strength: conclusion.degree.unwrap_or(100),
        method,
        condition: condition.into_bound(),
        conclusion: conclusion.into_bound(),
        citations,
    })))
}

/// 절 하나(낱말들과 마지막의 행위)를 읽습니다.
fn bind_clause<'a>(
    body: &[Token],
    citations: &Citations,
    demonstrative_offset: usize,
    mode: Mode,
    scope: &Scope,
    verbs: &'a [Verb],
    first: &Token,
) -> Result<Clause<'a>, Diagnostic> {
    let Some((verb_token, words)) = body.split_last() else {
        return Err(first.error("K500", "문장 끝에 행위(동사)를 적으세요."));
    };
    if verb_token.kind != Kind::Identifier {
        return Err(verb_token.error(
            "K500",
            "문장은 행위(동사)로 끝나야 합니다. 값을 선언하려면 '정의'로 시작하세요.",
        ));
    }
    let fits = |verb: &Verb, word: &str| {
        if mode == Mode::Condition {
            verb.matches_conditional(word)
        } else {
            verb.matches(word)
        }
    };
    let mut words = words;
    let mut matched: Vec<&Verb> = verbs.iter().filter(|v| fits(v, &verb_token.text)).collect();
    // 기능동사 구문: '설계를 하다'는 '설계하다'와 같은 행위입니다.
    if matched.is_empty()
        && let Some((noun, rest)) = words.split_last()
        && noun.kind == Kind::Identifier
        && let Some(stem) = noun
            .text
            .strip_suffix('을')
            .or_else(|| noun.text.strip_suffix('를'))
    {
        let joined = format!("{stem}{}", verb_token.text);
        let light: Vec<&Verb> = verbs.iter().filter(|v| fits(v, &joined)).collect();
        let is_light = if mode == Mode::Condition {
            verb_token.text == "하면"
        } else {
            Verb::is_light(&verb_token.text)
        };
        if !light.is_empty() && is_light {
            matched = light;
            words = rest;
        }
    }
    let verb = match matched.as_slice() {
        [verb] => *verb,
        [] if mode == Mode::Condition => {
            return Err(verb_token.error(
                "K503",
                format!(
                    "'{}'에 해당하는 행위가 선언되지 않았습니다. 전제는 '떨어지면', '먹으면'처럼 선언한 행위의 어간에 '-면'이나 '-으면'을 붙여 씁니다.",
                    verb_token.text
                ),
            ));
        }
        [] => {
            return Err(verb_token.error(
                "K503",
                format!(
                    "'{}'에 해당하는 행위가 선언되지 않았습니다. '행위 ...다: 주체, ...'로 먼저 선언하세요.",
                    verb_token.text
                ),
            ));
        }
        many => {
            let names: Vec<_> = many.iter().map(|v| v.name.as_str()).collect();
            return Err(verb_token.error(
                "K507",
                format!(
                    "'{}'이 여러 행위({})와 맞습니다. 행위 선언에 활용형을 괄호로 적어 구분하세요.",
                    verb_token.text,
                    names.join(", ")
                ),
            ));
        }
    };

    let mut degree = None;
    let mut method = None;
    let mut roles: HashMap<&'static str, (Filler, Token, &'static str)> = HashMap::new();
    let mut order = Vec::new();
    let mut demonstratives = demonstrative_offset;
    let mut index = 0;

    while index < words.len() {
        let word = &words[index];
        index += 1;
        match word.kind {
            Kind::Number if words.get(index).map(|t| t.kind) == Some(Kind::Percent) => {
                index += 1;
                if mode == Mode::Condition {
                    return Err(word.error(
                        "K550",
                        "전제에는 정도를 적을 수 없습니다. 규칙의 강도는 '컵이 80% 깨진다'처럼 결론에 적으세요.",
                    ));
                }
                if degree.is_some() {
                    return Err(word.error("K505", "문장 하나에는 정도를 한 번만 적습니다."));
                }
                let value: u32 = word.text.parse().unwrap_or(u32::MAX);
                if value > 100 {
                    return Err(word.error("K520", "정도는 0%에서 100% 사이입니다."));
                }
                degree = Some(value);
                continue;
            }
            Kind::Identifier => {}
            _ => {
                return Err(word.error(
                    "K500",
                    format!(
                        "문장에는 낱말과 정도(70%)만 쓸 수 있습니다. '{}'은 쓸 수 없습니다. 값을 선언하려면 '정의'로 시작하세요.",
                        word.text
                    ),
                ));
            }
        }

        // 계산 방식: 조건문의 결론에서만 씁니다.
        let chosen = match word.text.as_str() {
            "최소로" => Some(Method::Minimum),
            "곱으로" => Some(Method::Product),
            _ => None,
        };
        if let Some(chosen) = chosen {
            let noun = word.text.trim_end_matches("으로").trim_end_matches('로');
            if mode == Mode::Conclusion {
                if method.is_some() {
                    return Err(word.error("K505", "계산 방식은 한 번만 적습니다."));
                }
                method = Some(chosen);
                continue;
            }
            if !scope.values.contains_key(noun) {
                return Err(word.error(
                    "K550",
                    format!(
                        "'{}'는 조건문의 결론에서만 씁니다. 예: '컵이 떨어지면 컵이 80% 깨진다 곱으로'",
                        word.text
                    ),
                ));
            }
        }

        if DEMONSTRATIVE_ADVERBS.contains(&word.text.as_str()) {
            let filler = Filler::Reference(demonstratives);
            demonstratives += 1;
            insert_role(&mut roles, &mut order, "방식", filler, word, "")?;
            continue;
        }

        let (stem, particle, kind) = split_particle(word, scope)?;
        if let Some(expected) = particle_form_error(stem, particle) {
            return Err(word.error(
                "K502",
                format!(
                    "'{stem}' 뒤에는 '{particle}' 대신 '{expected}'를 씁니다: '{stem}{expected}'.",
                ),
            ));
        }

        let (role, filler) = if DEMONSTRATIVE_NOUNS.contains(&stem) {
            let role = match kind {
                Particle::Subject => "주체",
                Particle::Object => "대상",
                Particle::Recipient => "상대",
                Particle::Instrument => "도구",
                _ => {
                    return Err(word.error(
                        "K506",
                        format!(
                            "'{stem}'은 시각이나 위치가 아니므로 '{particle}'와 쓸 수 없습니다."
                        ),
                    ));
                }
            };
            let filler = Filler::Reference(demonstratives);
            demonstratives += 1;
            (role, filler)
        } else {
            let value = scope
                .values
                .get(stem)
                .cloned()
                .expect("split_particle checked");
            let role = role_for(kind, &value, word, stem, particle, scope)?;
            (role, Filler::Value(value))
        };
        insert_role(&mut roles, &mut order, role, filler, word, particle)?;
    }

    for &required in &verb.required {
        if !roles.contains_key(required) {
            let hint = match required {
                "주체" => "누가 했는지(주체)가 없습니다. '유빈이'처럼 이/가 조사로 밝히세요.",
                "대상" => "무엇을 했는지(대상)가 없습니다. '설계안을'처럼 을/를 조사로 밝히세요.",
                "시작" => "언제인지(시작)가 없습니다. '던짐에'처럼 시각에 '에'를 붙여 밝히세요.",
                _ => "",
            };
            return Err(verb_token.error(
                "K504",
                format!(
                    "행위 '{}'에는 '{required}' 역할이 필요합니다. {hint}",
                    verb.name
                )
                .trim_end()
                .to_string(),
            ));
        }
    }

    if demonstratives > citations.len() {
        let word = order
            .iter()
            .find_map(|role| match &roles[role] {
                (Filler::Reference(k), token, _) if *k == citations.len() => Some(token),
                _ => None,
            })
            .unwrap_or(first);
        return Err(word.error(
            "K510",
            format!(
                "{} 가리키는 내용이 없습니다. 문장 끝에 '(참조 1)'처럼 참조를 달고 '참조 1: 내용'을 적으세요.",
                josa(&format!("'{}'", word.text), "이", "가")
            ),
        ));
    }

    let mut orderings = Vec::new();
    check_frames(&roles, "시작", "종료", scope)?;
    check_frames(&roles, "출발", "도착", scope)?;
    // 시간은 앞으로만 흐르므로 종료는 시작보다 앞설 수 없습니다. 위치는 뒤로도 움직일 수 있습니다.
    if let (Some((Filler::Value(start), _, _)), Some((Filler::Value(end), token, _))) =
        (roles.get("시작"), roles.get("종료"))
    {
        orderings.push(Ordering {
            earlier: start.clone(),
            later: end.clone(),
            token: token.clone(),
            what: format!("행위 '{}'의 종료", verb.name),
        });
    }

    let ir_roles = order
        .iter()
        .map(|role| {
            let (filler, _, particle) = &roles[role];
            match filler {
                Filler::Value(value) => IrSentenceRole {
                    role: (*role).into(),
                    particle: (*particle).into(),
                    value_type: value.kind.name(),
                    value: scope.show(value),
                    unit: value.kind.base_unit(),
                    frame: scope.frame_name(value),
                    reference: None,
                },
                Filler::Reference(k) => IrSentenceRole {
                    role: (*role).into(),
                    particle: (*particle).into(),
                    value_type: "참조",
                    value: format!("참조 {}", citations[*k].0),
                    unit: None,
                    frame: None,
                    reference: Some(citations[*k].0),
                },
            }
        })
        .collect();

    let pattern = Pattern {
        verb: verb.name.clone(),
        roles: order
            .iter()
            .map(|role| {
                let filler = match &roles[role].0 {
                    Filler::Value(value) => FactFiller::Value(value.clone()),
                    Filler::Reference(k) => FactFiller::Reference(citations[*k].0),
                };
                (*role, filler)
            })
            .collect(),
    };
    let values = roles
        .iter()
        .filter_map(|(role, (filler, _, _))| match filler {
            Filler::Value(value) => Some((*role, value.clone())),
            Filler::Reference(_) => None,
        })
        .collect();
    Ok(Clause {
        verb,
        verb_token: verb_token.clone(),
        degree,
        method,
        demonstratives,
        ir_roles,
        values,
        orderings,
        pattern,
    })
}

fn insert_role(
    roles: &mut HashMap<&'static str, (Filler, Token, &'static str)>,
    order: &mut Vec<&'static str>,
    role: &'static str,
    filler: Filler,
    word: &Token,
    particle: &'static str,
) -> Result<(), Diagnostic> {
    if roles.contains_key(role) {
        return Err(word.error("K505", format!("문장에 '{role}' 역할이 두 번 나왔습니다.")));
    }
    roles.insert(role, (filler, word.clone(), particle));
    order.push(role);
    Ok(())
}

/// 낱말을 '선언된 이름 또는 지시 대명사 + 조사'로 나눕니다.
fn split_particle<'a>(
    word: &'a Token,
    scope: &Scope,
) -> Result<(&'a str, &'static str, Particle), Diagnostic> {
    let text = word.text.as_str();
    for &(particle, kind) in PARTICLES {
        let Some(stem) = text.strip_suffix(particle) else {
            continue;
        };
        if stem.is_empty() {
            continue;
        }
        if DEMONSTRATIVE_NOUNS.contains(&stem) {
            return Ok((stem, particle, kind));
        }
        if scope.values.contains_key(stem) {
            return Ok((stem, particle, kind));
        }
    }
    if scope.values.contains_key(text) {
        return Err(word.error(
            "K501",
            format!(
                "'{text}'에 역할을 나타내는 조사가 없습니다. '{text}이/가', '{text}을/를'처럼 조사를 붙이세요."
            ),
        ));
    }
    Err(word.error(
        "K501",
        format!(
            "'{text}'을 알 수 없습니다. 문장의 낱말은 '대상'·'정의'로 선언한 이름에 조사를 붙인 것이어야 합니다."
        ),
    ))
}

/// 조사와 값의 타입으로 역할을 정합니다. '에'처럼 뜻이 여럿인 조사는 값의 타입이 정합니다.
fn role_for(
    kind: Particle,
    value: &Value,
    word: &Token,
    stem: &str,
    particle: &str,
    scope: &Scope,
) -> Result<&'static str, Diagnostic> {
    let role = match (kind, value.kind) {
        (Particle::Subject, Type::Entity) => Some("주체"),
        (Particle::Object, Type::Entity) => Some("대상"),
        (Particle::Recipient, Type::Entity) => Some("상대"),
        (Particle::Instrument, Type::Entity) => Some("도구"),
        (Particle::At | Particle::From, Type::Instant) => Some("시작"),
        (Particle::Until, Type::Instant) => Some("종료"),
        (Particle::At | Particle::In, Type::Position) => Some("장소"),
        (Particle::From, Type::Position) => Some("출발"),
        (Particle::Until, Type::Position) => Some("도착"),
        _ => None,
    };
    role.ok_or_else(|| {
        let need = match kind {
            Particle::Subject | Particle::Object | Particle::Recipient | Particle::Instrument => {
                "대상"
            }
            Particle::In => "위치",
            Particle::At | Particle::From | Particle::Until => "시각이나 위치",
        };
        word.error(
            "K506",
            format!(
                "'{stem}{particle}': '{particle}' 앞에는 '{need}'이 와야 하지만 '{stem}'은 {}입니다.",
                scope.describe(value)
            ),
        )
    })
}

fn check_frames(
    roles: &HashMap<&'static str, (Filler, Token, &'static str)>,
    first: &str,
    last: &str,
    scope: &Scope,
) -> Result<(), Diagnostic> {
    let (Some((Filler::Value(a), _, _)), Some((Filler::Value(b), token, _))) =
        (roles.get(first), roles.get(last))
    else {
        return Ok(());
    };
    if a.frame != b.frame {
        return Err(token.error(
            "K306",
            format!(
                "문장의 '{first}'은 {}, '{last}'은 {}입니다. 같은 기준을 사용하세요.",
                scope.describe(a),
                scope.describe(b)
            ),
        ));
    }
    Ok(())
}

/// 문장이 인용한 참조 번호와 그 위치.
type Citations = Vec<(u32, Token)>;

fn split_citations(tokens: &[Token]) -> Result<(&[Token], Citations), Diagnostic> {
    let Some(open) = tokens.iter().position(|t| t.kind == Kind::LeftParen) else {
        return Ok((tokens, Vec::new()));
    };
    let mut citations = Vec::new();
    let mut index = open + 1;
    loop {
        let (Some(keyword), Some(number)) = (tokens.get(index), tokens.get(index + 1)) else {
            return Err(tokens[open].error("K500", "'(참조 1)'처럼 참조를 적고 괄호를 닫으세요."));
        };
        if keyword.kind != Kind::Reference || number.kind != Kind::Number {
            return Err(keyword.error("K500", "괄호 안에는 '참조 1'처럼 참조만 적을 수 있습니다."));
        }
        let Ok(n) = number.text.parse::<u32>() else {
            return Err(number.error("K511", "참조 번호가 너무 큽니다."));
        };
        citations.push((n, number.clone()));
        index += 2;
        match tokens.get(index).map(|t| t.kind) {
            Some(Kind::Comma) => index += 1,
            Some(Kind::RightParen) => break,
            _ => {
                return Err(tokens[index.min(tokens.len() - 1)]
                    .error("K500", "참조 목록은 ','로 나누고 ')'로 닫으세요."));
            }
        }
    }
    if index + 1 != tokens.len() {
        return Err(tokens[index + 1].error("K500", "참조 괄호는 문장 맨 끝에 적으세요."));
    }
    Ok((&tokens[..open], citations))
}

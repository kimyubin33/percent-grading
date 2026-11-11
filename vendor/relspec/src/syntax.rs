use icu_normalizer::ComposingNormalizerBorrowed;
use icu_properties::{CodePointMapData, props::GeneralCategory};

use std::collections::BTreeSet;
use std::sync::Mutex;

use crate::Diagnostic;
use crate::integer::Integer;

const MAX_TOKENS_PER_LINE: usize = 128;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Type {
    Integer,
    Boolean,
    /// 지속 시간(두 시각의 차이). 0.1의 `시간` 타입과 같은 값입니다.
    /// (더하고 뺄 수 있는 양인지는 `is_amount`로 확인합니다.)
    Time,
    /// 길이(두 위치의 차이).
    Distance,
    /// 시간축 기준 위의 한 점.
    Instant,
    /// 위치축 기준 위의 한 점.
    Position,
    /// 사건에 참여하는 이름 있는 대상.
    Entity,
    /// 퍼지 논리의 참 정도(0%~100%).
    Degree,
    /// `단위 원`으로 선언한 단위로 세는 양. 단위가 다르면 서로 다른 타입입니다.
    Custom(&'static str),
}

/// 단위 이름을 프로그램이 끝날 때까지 유지되는 문자열로 바꿉니다.
/// 같은 이름은 한 번만 저장하므로 저장량은 서로 다른 단위 이름의 수를 넘지 않습니다.
pub(crate) fn intern(name: &str) -> &'static str {
    static NAMES: Mutex<BTreeSet<&'static str>> = Mutex::new(BTreeSet::new());
    let mut names = NAMES
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(&known) = names.get(name) {
        return known;
    }
    let leaked: &'static str = Box::leak(name.to_owned().into_boxed_str());
    names.insert(leaked);
    leaked
}

impl Type {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Integer => "정수",
            Self::Boolean => "논리",
            Self::Time => "시간",
            Self::Distance => "거리",
            Self::Instant => "시각",
            Self::Position => "위치",
            Self::Entity => "대상",
            Self::Degree => "정도",
            Self::Custom(name) => name,
        }
    }

    pub(crate) fn base_unit(self) -> Option<&'static str> {
        match self {
            Self::Time | Self::Instant => Some("밀리초"),
            Self::Distance | Self::Position => Some("밀리미터"),
            Self::Custom(name) => Some(name),
            _ => None,
        }
    }

    /// 더하고 빼고 정수배할 수 있는 양인지.
    pub(crate) fn is_amount(self) -> bool {
        matches!(
            self,
            Self::Integer | Self::Time | Self::Distance | Self::Custom(_)
        )
    }

    /// 기준 위의 점이면 그 점들의 차이에 해당하는 타입을 돌려줍니다.
    pub(crate) fn displacement(self) -> Option<Type> {
        match self {
            Self::Instant => Some(Self::Time),
            Self::Position => Some(Self::Distance),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Axis {
    Time,
    Space,
}

impl Axis {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Time => "시간축",
            Self::Space => "위치축",
        }
    }

    pub(crate) fn point(self) -> Type {
        match self {
            Self::Time => Type::Instant,
            Self::Space => Type::Position,
        }
    }

    pub(crate) fn offset(self) -> Type {
        match self {
            Self::Time => Type::Time,
            Self::Space => Type::Distance,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    End,
    NewLine,
    Identifier,
    Number,
    Unit(Type, u32),
    Type(Type),
    Define,
    Check,
    Frame,
    Event,
    Unknown,
    Assume,
    Goal,
    Observe,
    Verb,
    Reference,
    Rule,
    /// `단위 원`
    UnitDeclaration,
    /// `본문: 글`
    Body,
    /// `주제 <글>`
    Topic,
    /// `낱말 이름: 차원, …`
    Word,
    /// `장면 이름`
    Scene,
    /// `참조 N:` 뒤의 본문. 낱말로 나누지 않고 그대로 보관합니다.
    Text,
    Axis(Axis),
    True,
    False,
    And,
    Or,
    Not,
    Colon,
    Comma,
    Dot,
    At,
    Question,
    Percent,
    Assign,
    LeftParen,
    RightParen,
    Plus,
    Minus,
    Star,
    Equal,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
}

#[derive(Clone, Debug)]
pub(crate) struct Token {
    pub(crate) kind: Kind,
    pub(crate) text: String,
    line: usize,
    column: usize,
}

impl Token {
    pub(crate) fn line(&self) -> usize {
        self.line
    }

    pub(crate) fn column(&self) -> usize {
        self.column
    }

    /// 같은 자리를 가리키되 글만 바꾼 토큰. 이름 없는 검사·조건·목표의 이름으로 씁니다.
    pub(crate) fn renamed(&self, text: String) -> Token {
        Token {
            kind: Kind::Identifier,
            text,
            line: self.line,
            column: self.column,
        }
    }

    /// 한 줄에 이어 적힌 토큰들을 원문의 띄어쓰기대로 다시 잇습니다.
    pub(crate) fn join(tokens: &[Token]) -> String {
        let mut text = String::new();
        let mut end = 0;
        for token in tokens {
            if !text.is_empty() && token.column > end {
                text.push(' ');
            }
            text.push_str(&token.text);
            end = token.column + token.text.encode_utf16().count();
        }
        text
    }

    pub(crate) fn error(&self, code: &'static str, message: impl Into<String>) -> Diagnostic {
        Diagnostic {
            code,
            line: self.line,
            column: self.column,
            message: crate::sentence::fix_particles(&message.into()),
        }
    }
}

fn classify_name(name: &str) -> Kind {
    match name {
        "정의" => Kind::Define,
        "검사" => Kind::Check,
        "기준" => Kind::Frame,
        "사건" => Kind::Event,
        "미지" => Kind::Unknown,
        "조건" => Kind::Assume,
        "목표" => Kind::Goal,
        "관측" => Kind::Observe,
        "행위" => Kind::Verb,
        "규칙" => Kind::Rule,
        "참조" => Kind::Reference,
        "단위" => Kind::UnitDeclaration,
        "본문" => Kind::Body,
        "주제" => Kind::Topic,
        "낱말" => Kind::Word,
        "장면" => Kind::Scene,
        "정도" => Kind::Type(Type::Degree),
        "시간축" => Kind::Axis(Axis::Time),
        "위치축" => Kind::Axis(Axis::Space),
        "시각" => Kind::Type(Type::Instant),
        "위치" => Kind::Type(Type::Position),
        "대상" => Kind::Type(Type::Entity),
        "참" => Kind::True,
        "거짓" => Kind::False,
        "그리고" => Kind::And,
        "또는" => Kind::Or,
        "아님" => Kind::Not,
        "정수" => Kind::Type(Type::Integer),
        "논리" => Kind::Type(Type::Boolean),
        "시간" => Kind::Type(Type::Time),
        "거리" => Kind::Type(Type::Distance),
        "밀리초" => Kind::Unit(Type::Time, 1),
        "초" => Kind::Unit(Type::Time, 1000),
        "밀리미터" => Kind::Unit(Type::Distance, 1),
        "센티미터" => Kind::Unit(Type::Distance, 10),
        "미터" => Kind::Unit(Type::Distance, 1000),
        _ => Kind::Identifier,
    }
}

/// `만`, `천만`, `억`처럼 큰 수를 나타내는 낱말로 시작하면 그 배수(십진수 글)와 나머지를 돌려줍니다.
pub(crate) fn split_multiplier(word: &str) -> Option<(&'static str, &str)> {
    const WORDS: &[(&str, &str)] = &[
        ("천조", "1000000000000000"),
        ("백조", "100000000000000"),
        ("십조", "10000000000000"),
        ("조", "1000000000000"),
        ("천억", "100000000000"),
        ("백억", "10000000000"),
        ("십억", "1000000000"),
        ("억", "100000000"),
        ("천만", "10000000"),
        ("백만", "1000000"),
        ("십만", "100000"),
        ("만", "10000"),
        ("천", "1000"),
        ("백", "100"),
        ("십", "10"),
    ];
    WORDS
        .iter()
        .find_map(|&(name, factor)| word.strip_prefix(name).map(|rest| (factor, rest)))
}

fn is_name_start(ch: char) -> bool {
    // The first implementation used UTF-16 code units for identifier classification.
    ch == '_'
        || (ch.len_utf16() == 1
            && matches!(
                CodePointMapData::<GeneralCategory>::new().get(ch),
                GeneralCategory::UppercaseLetter
                    | GeneralCategory::LowercaseLetter
                    | GeneralCategory::TitlecaseLetter
                    | GeneralCategory::ModifierLetter
                    | GeneralCategory::OtherLetter
            ))
}

fn is_name_part(ch: char) -> bool {
    is_name_start(ch)
        || (ch.len_utf16() == 1
            && matches!(
                CodePointMapData::<GeneralCategory>::new().get(ch),
                GeneralCategory::DecimalNumber
                    | GeneralCategory::NonspacingMark
                    | GeneralCategory::SpacingMark
            ))
}

pub(crate) fn scan(source: &str) -> Result<Vec<Token>, Diagnostic> {
    let source = source.strip_prefix('\u{feff}').unwrap_or(source);
    let normalized = source.replace("\r\n", "\n").replace('\r', "\n");
    let normalizer = ComposingNormalizerBorrowed::new_nfc();
    let mut tokens = Vec::new();
    let mut lines = normalized.split('\n').enumerate().peekable();
    // `장면` 줄의 열. 그보다 들여 쓴 줄은 장면 안의 줄입니다.
    let mut scene: Option<usize> = None;

    while let Some((i, text)) = lines.next() {
        let line = i + 1;
        let mut characters = text.char_indices().peekable();
        let mut column = 1;
        let mut count = 0;
        let mut in_scene = false;

        while let Some((start, ch)) = characters.next() {
            let token_column = column;
            column += ch.len_utf16();

            if ch.is_whitespace() {
                continue;
            }
            if ch == '#' {
                break;
            }
            count += 1;
            if count > MAX_TOKENS_PER_LINE {
                return Err(Diagnostic {
                    code: "K101",
                    line,
                    column: token_column,
                    message: format!("한 줄에 기호를 {MAX_TOKENS_PER_LINE}개까지 쓸 수 있습니다."),
                });
            }

            let kind;
            let token_text;
            if ch.is_ascii_digit() || is_name_start(ch) {
                let is_part = if ch.is_ascii_digit() {
                    |c: char| c.is_ascii_digit()
                } else {
                    is_name_part
                };
                while let Some((_, next)) = characters.next_if(|&(_, next)| is_part(next)) {
                    column += next.len_utf16();
                }
                let end = characters
                    .peek()
                    .map_or(text.len(), |&(position, _)| position);
                if ch.is_ascii_digit() {
                    kind = Kind::Number;
                    token_text = text[start..end].into();
                } else {
                    token_text = normalizer.normalize(&text[start..end]).into_owned();
                    kind = classify_name(&token_text);
                }
            } else {
                let followed_by_equals = matches!(ch, '=' | '!' | '<' | '>')
                    && characters.next_if(|&(_, next)| next == '=').is_some();
                if followed_by_equals {
                    column += 1;
                }
                kind = match (ch, followed_by_equals) {
                    (':', _) => Kind::Colon,
                    (',', _) => Kind::Comma,
                    ('.', _) => Kind::Dot,
                    ('@', _) => Kind::At,
                    ('?', _) => Kind::Question,
                    ('%', _) => Kind::Percent,
                    ('(', _) => Kind::LeftParen,
                    (')', _) => Kind::RightParen,
                    ('+', _) => Kind::Plus,
                    ('-', _) => Kind::Minus,
                    ('*', _) => Kind::Star,
                    ('=', false) => Kind::Assign,
                    ('=', true) => Kind::Equal,
                    ('!', true) => Kind::NotEqual,
                    ('<', false) => Kind::Less,
                    ('<', true) => Kind::LessEqual,
                    ('>', false) => Kind::Greater,
                    ('>', true) => Kind::GreaterEqual,
                    _ => {
                        return Err(Diagnostic {
                            code: "K100",
                            line,
                            column: token_column,
                            message: format!("알 수 없는 기호 '{ch}'입니다."),
                        });
                    }
                };
                let end = characters
                    .peek()
                    .map_or(text.len(), |&(position, _)| position);
                token_text = text[start..end].into();
            }

            tokens.push(Token {
                kind,
                text: token_text,
                line,
                column: token_column,
            });

            // `참조 N:` 뒤는 자유 본문입니다. 주석 기호 `#`도 본문에 포함합니다.
            // 문장 뒤의 `: 설명`도 같습니다. 문장은 선언 낱말로 시작하지 않는 줄이며,
            // 두 번째 자리의 ':'는 문장에 붙인 이름(`이름: 문장`)이므로 설명이 아닙니다.
            let line_tokens = &tokens[tokens.len() - count..];
            let reference_body = count == 3
                && matches!(
                    line_tokens
                        .iter()
                        .map(|t| t.kind)
                        .collect::<Vec<_>>()
                        .as_slice(),
                    [Kind::Reference, Kind::Number, Kind::Colon]
                );
            if count == 1 {
                in_scene = scene.is_some_and(|start| token_column > start);
                if !in_scene {
                    scene = (kind == Kind::Scene).then_some(token_column);
                }
            }
            // 장면 안의 줄(`컵 큰컵: 색 흰색`)에서는 ':' 뒤도 낱말로 나눕니다.
            let sentence_note = !in_scene
                && count >= 3
                && kind == Kind::Colon
                && line_tokens[0].kind == Kind::Identifier;
            // `본문: 글`
            let body_text = count == 2 && kind == Kind::Colon && line_tokens[0].kind == Kind::Body;
            // `- 글`: 위의 문장·참조·본문에 딸린 줄
            let item_text = count == 1 && kind == Kind::Minus;
            // `주제 <글>`
            let topic_text = count == 1 && kind == Kind::Topic;
            if reference_body || sentence_note || body_text || item_text || topic_text {
                let rest = characters
                    .peek()
                    .map_or(text.len(), |&(position, _)| position);
                let body = &text[rest..];
                let leading = body.len() - body.trim_start().len();
                let body_column = column + body[..leading].encode_utf16().count();
                tokens.push(Token {
                    kind: Kind::Text,
                    text: normalizer.normalize(body.trim()).into_owned(),
                    line,
                    column: body_column,
                });
                break;
            }
        }

        let kind = if lines.peek().is_some() {
            Kind::NewLine
        } else {
            Kind::End
        };
        tokens.push(Token {
            kind,
            text: String::new(),
            line,
            column: text.encode_utf16().count() + 1,
        });
    }
    Ok(tokens)
}

/// 낱말의 차원 하나. `색*`, `채움(정도) 2`
pub(crate) struct Dimension {
    pub(crate) name: Token,
    /// 별표를 붙인, 꼭 적어야 하는 차원.
    pub(crate) required: bool,
    /// 채움 점수에서 이 차원이 차지하는 무게. 적지 않으면 1.
    pub(crate) weight: u32,
    pub(crate) kind: Option<Type>,
}

/// 장면 안의 줄 하나. `컵 큰컵: 색 흰색, 채움 80%`
pub(crate) struct Member {
    /// 줄의 첫 토큰. 열이 들여쓰기의 깊이입니다.
    pub(crate) word: Token,
    pub(crate) name: Option<Token>,
    /// (차원 이름, 값의 토큰들)
    pub(crate) values: Vec<(Token, Vec<Token>)>,
}

pub(crate) enum Expression {
    Literal(Type, Integer),
    /// `120만 원`, `30 분`처럼 수 뒤에 낱말이 붙은 양.
    /// 낱말이 단위이면 값은 `settled + last`, `만원`처럼 큰 수 낱말이 붙어 있으면
    /// `settled + last × 만`입니다. 어느 쪽인지는 선언된 단위를 보고 정합니다.
    Quantity {
        settled: Integer,
        last: Integer,
        word: Token,
    },
    Reference(Token),
    Unary(Token, Box<Expression>),
    Binary(Token, Box<Expression>, Box<Expression>),
    /// `기준 @ 값`: 기준 위의 점을 만듭니다.
    Anchor(Token, Token, Box<Expression>),
    /// `사건.역할`: 사건에 연결된 역할 값을 읽습니다.
    Member(Token, Token),
}

pub(crate) struct Role {
    pub(crate) name: Token,
    pub(crate) expression: Expression,
}

pub(crate) enum Statement {
    /// `정의`와 `검사`. 정의의 타입과 검사의 이름은 생략할 수 있습니다.
    Value {
        name: Token,
        check: bool,
        /// 이름을 적지 않은 검사. `name`은 식을 그대로 옮긴 글입니다.
        anonymous: bool,
        declared_type: Option<(Type, Token)>,
        expression: Expression,
    },
    /// `단위 원`
    Unit {
        name: Token,
    },
    Frame {
        name: Token,
        axis: Axis,
    },
    Entity {
        name: Token,
    },
    Event {
        name: Token,
        roles: Vec<Role>,
    },
    /// `미지 이름: 시간 = ?`, `미지 이름: 시각 = 기준 @ ?`
    Unknown {
        name: Token,
        kind: Type,
        kind_token: Token,
        frame: Option<Token>,
    },
    /// `조건`(모든 해가 따라야 하는 가정)과 `목표`(가능·보장 여부를 판정할 식).
    Constraint {
        name: Token,
        /// 이름을 적지 않았으면 `name`은 식을 그대로 옮긴 글입니다.
        anonymous: bool,
        goal: bool,
        expression: Expression,
    },
    /// `관측 미지이름 = 값`
    Observe {
        name: Token,
        expression: Expression,
    },
    /// `행위 설계하다 (설계했다): 주체, 대상`
    Verb {
        name: Token,
        forms: Vec<Token>,
        required: Vec<Token>,
    },
    /// `규칙 깨짐: 떨어짐 이면 80% 곱으로`
    Rule {
        name: Token,
        premise: Expression,
        strength: Expression,
        /// `최소로` 또는 `곱으로`. 없으면 두 방식을 모두 계산합니다.
        method: Option<Token>,
    },
    /// `본문: 글`. 낱말로 나누지 않는 자유로운 글 한 줄.
    Body {
        text: Token,
    },
    /// `주제 <글>`. 이 파일이 무엇을 적는지.
    Topic {
        text: Token,
    },
    /// `낱말 컵: 색*, 재질, 채움(정도) 2`
    Word {
        name: Token,
        dimensions: Vec<Dimension>,
    },
    /// `장면 이름`과 그 아래에 들여 쓴 줄들.
    Scene {
        name: Token,
        members: Vec<Member>,
    },
    /// `- 글`. 바로 위의 문장·참조·본문에 딸린 글 한 줄.
    Item {
        text: Token,
    },
    /// `참조 1: 본문`
    ReferenceDefinition {
        number: Token,
        text: Token,
    },
    /// 키워드로 시작하지 않는 줄. `이름: 문장`처럼 이름을 붙일 수 있습니다.
    Sentence {
        label: Option<Token>,
        tokens: Vec<Token>,
        /// 문장 뒤에 `: 설명`으로 적은 글.
        note: Option<Token>,
    },
}

//! 본문 점검.
//!
//! 문장 뒤의 설명, `참조`의 본문, `본문:` 줄은 낱말로 나누지 않는 자유로운 글입니다.
//! 자유로운 글이 한 가지로만 읽히는지는 판정할 수 없으므로, 여기서는 확실하게 말할 수 있는
//! 것만 다룹니다.
//!
//! - 선언한 이름이 글에 나오면 그 자리를 기록합니다(편집기의 색칠과 설명에 씁니다).
//! - 정의한 값과 같은 양이 글에 따로 적혀 있으면 알립니다(`K601`).
//! - 기준이 정해지지 않은 낱말을 알립니다(`K602`).
//! - 가리키는 대상을 확인할 수 없는 지시어를 알립니다(`K603`).
//! - 선언한 이름을 띄어 쓴 곳을 알립니다(`K604`).
//! - 한 줄에 너무 길게 적은 글을 알립니다(`K605`).
//!
//! 모두 경고이며 컴파일을 막지 않습니다. 경고가 없다는 것이 글이 모호하지 않다는 뜻은 아닙니다.

use std::collections::BTreeSet;

use crate::integer::Integer;
use crate::syntax::{Type, split_multiplier};
use crate::{IrMention, IrWarning};

/// 글 한 덩어리와 그 시작 위치(줄, UTF-16 열. 둘 다 1부터).
pub(crate) struct Passage {
    pub(crate) line: usize,
    pub(crate) column: usize,
    pub(crate) text: String,
}

/// 글을 읽을 때 견줄, 명세에 선언된 것들.
pub(crate) struct Declared {
    /// 값의 이름: 정의, 대상, 미지값, 기준, 사건, 이름 붙은 문장, 규칙.
    pub(crate) names: BTreeSet<String>,
    /// 단위 있는 값으로 정의한 것: (이름, 타입, 기본 단위로 센 수)
    pub(crate) amounts: Vec<(String, Type, Integer)>,
    /// 단위: (낱말, 타입, 기본 단위로 바꾸는 배수)
    pub(crate) units: Vec<(String, Type, u32)>,
}

/// 이름이나 단위 뒤에 붙을 수 있는 조사와 서술격 어미. 긴 것을 먼저 확인합니다.
const ENDINGS: &[&str] = &[
    "으로부터",
    "에게서",
    "으로써",
    "으로서",
    "이라는",
    "이라고",
    "입니다",
    "에게",
    "한테",
    "에서",
    "부터",
    "까지",
    "으로",
    "보다",
    "처럼",
    "만큼",
    "마다",
    "라는",
    "라고",
    "이며",
    "이고",
    "이나",
    "이란",
    "이다",
    "와",
    "과",
    "은",
    "는",
    "이",
    "가",
    "을",
    "를",
    "의",
    "에",
    "로",
    "도",
    "만",
    "나",
    "며",
    "고",
    "인",
    "랑",
];

/// 기준이 정해지지 않은 낱말. 요구사항을 적을 때 피하라고 하는 대표적인 것들입니다.
const VAGUE: &[&str] = &[
    "적절히",
    "적절한",
    "적절하게",
    "적당히",
    "적당한",
    "적당하게",
    "충분히",
    "충분한",
    "충분하게",
    "필요시",
    "필요하면",
    "가급적",
    "가능하면",
    "대략",
    "대충",
    "약간",
    "최대한",
    "신속히",
    "신속하게",
    "조속히",
    "빨리",
    "나중에",
    "추후",
];

/// 글 한 줄이 이보다 길면(UTF-16 단위) 나누어 적으라고 알립니다.
const LONG: usize = 80;

/// 앞의 무엇인가를 가리키는 낱말. 글 안에서는 무엇을 가리키는지 확인할 수 없습니다.
const POINTERS: &[&str] = &["이것", "그것", "저것", "이것들", "그것들", "해당", "상기"];

/// 낱말 끝이 비었거나 조사(둘까지 이어진 것 포함)인지.
fn is_ending(rest: &str) -> bool {
    rest.is_empty()
        || ENDINGS.contains(&rest)
        || ENDINGS.iter().any(|first| {
            rest.strip_prefix(first)
                .is_some_and(|second| ENDINGS.contains(&second))
        })
}

/// `word`가 `candidates` 가운데 하나에 조사가 붙은 꼴이면 가장 긴 것을 돌려줍니다.
fn stem_of<'a>(word: &str, candidates: impl Iterator<Item = &'a str>) -> Option<&'a str> {
    candidates
        .filter(|name| word.strip_prefix(name).is_some_and(is_ending))
        .max_by_key(|name| name.len())
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Piece {
    Number,
    Word,
}

struct Span<'a> {
    piece: Piece,
    text: &'a str,
    /// 글 안에서의 시작 위치(UTF-16).
    start: usize,
    /// 앞 조각과 붙어 있는지, 공백만 사이에 두었는지.
    glued: bool,
    spaced: bool,
}

fn units(text: &str) -> usize {
    text.encode_utf16().count()
}

/// 글을 수와 낱말로 나눕니다. 수는 `1,200,000`처럼 쉼표를 품을 수 있습니다.
fn split(text: &str) -> Vec<Span<'_>> {
    let mut spans: Vec<Span> = Vec::new();
    let mut chars = text.char_indices().peekable();
    let mut offset = 0;
    let mut last_end = 0;
    while let Some(&(start, ch)) = chars.peek() {
        let piece = if ch.is_ascii_digit() {
            Piece::Number
        } else if ch.is_alphabetic() || ch == '_' {
            Piece::Word
        } else {
            chars.next();
            offset += ch.len_utf16();
            continue;
        };
        let mut end = start;
        while let Some(&(index, next)) = chars.peek() {
            let inside = match piece {
                Piece::Number => {
                    next.is_ascii_digit()
                        || (next == ','
                            && text[index + 1..].starts_with(|c: char| c.is_ascii_digit()))
                }
                Piece::Word => next.is_alphanumeric() || next == '_',
            };
            if !inside {
                break;
            }
            end = index + next.len_utf8();
            chars.next();
        }
        let gap = &text[last_end..start];
        spans.push(Span {
            piece,
            text: &text[start..end],
            start: offset,
            glued: !spans.is_empty() && gap.is_empty(),
            spaced: !spans.is_empty() && !gap.is_empty() && gap.chars().all(|c| c == ' '),
        });
        offset += units(&text[start..end]);
        last_end = end;
    }
    spans
}

pub(crate) fn analyze(
    passages: &[Passage],
    declared: &Declared,
) -> (Vec<IrMention>, Vec<IrWarning>) {
    let mut mentions = Vec::new();
    let mut warnings = Vec::new();

    for passage in passages {
        let spans = split(&passage.text);
        let length = units(&passage.text);
        if length > LONG {
            let first = spans.first().map_or(1, |span| units(span.text));
            warnings.push(IrWarning {
                code: "K605",
                line: passage.line,
                column: passage.column,
                length: first,
                message: format!(
                    "글 한 줄이 {length}자입니다. {LONG}자를 넘으면 읽는 사람이 놓치기 쉬우므로, 지시 하나를 한 줄로 하여 아래에 '- '로 시작하는 줄로 나누어 적으세요."
                ),
            });
        }
        let mut warn = |code: &'static str, start: usize, length: usize, message: String| {
            warnings.push(IrWarning {
                code,
                line: passage.line,
                column: passage.column + start,
                length,
                message: crate::sentence::fix_particles(&message),
            });
        };
        let unit_of = |word: &str| {
            let unit = stem_of(
                word,
                declared.units.iter().map(|(name, _, _)| name.as_str()),
            )?;
            declared.units.iter().find(|(name, _, _)| name == unit)
        };

        for (index, span) in spans.iter().enumerate() {
            let next = spans.get(index + 1);
            if span.piece == Piece::Number {
                // 수 + (큰 수 낱말) + 단위: `120만 원`, `30분`, `3만원`
                let Some(word) = next.filter(|n| n.piece == Piece::Word && (n.glued || n.spaced))
                else {
                    continue;
                };
                let digits: String = span.text.chars().filter(char::is_ascii_digit).collect();
                let mut amount = Integer::from_decimal(&digits);
                let mut found = None;
                let scaled = word
                    .glued
                    .then(|| split_multiplier(word.text))
                    .flatten()
                    .filter(|(_, rest)| rest.is_empty() || unit_of(rest).is_some());
                if let Some((factor, rest)) = scaled {
                    amount = amount.mul(&Integer::from_decimal(factor));
                    if rest.is_empty() {
                        if let Some(after) = spans
                            .get(index + 2)
                            .filter(|n| n.piece == Piece::Word && n.spaced)
                        {
                            found = unit_of(after.text).map(|unit| (unit, after.start));
                        }
                    } else {
                        let at = word.start + units(word.text) - units(rest);
                        found = unit_of(rest).map(|unit| (unit, at));
                    }
                } else {
                    found = unit_of(word.text).map(|unit| (unit, word.start));
                }
                let Some(((unit, kind, factor), unit_start)) = found else {
                    continue;
                };
                let amount = amount.mul(&Integer::from_small(*factor));
                if let Some((name, _, _)) = declared
                    .amounts
                    .iter()
                    .find(|(_, k, number)| k == kind && *number == amount)
                {
                    let length = unit_start + units(unit) - span.start;
                    let written: String = passage
                        .text
                        .encode_utf16()
                        .skip(span.start)
                        .take(length)
                        .map(|u| char::from_u32(u32::from(u)).unwrap_or('?'))
                        .collect();
                    warn(
                        "K601",
                        span.start,
                        length,
                        format!(
                            "본문의 '{written}'은 정의 '{name}'과 같은 값입니다. 정의를 고치면 본문이 틀려지므로 수 대신 이름 '{name}'을 적으세요."
                        ),
                    );
                }
                continue;
            }

            let names = || declared.names.iter().map(String::as_str);
            if let Some(name) = stem_of(span.text, names()) {
                mentions.push(IrMention {
                    line: passage.line,
                    column: passage.column + span.start,
                    length: units(name),
                    name: name.to_string(),
                });
                continue;
            }
            // 선언한 이름을 띄어 쓴 곳: `최저 가격` ↔ `최저가격`
            if let Some(after) = next.filter(|n| n.piece == Piece::Word && n.spaced) {
                let joined = format!("{}{}", span.text, after.text);
                if let Some(name) = stem_of(&joined, names()).filter(|n| n.len() > span.text.len())
                {
                    let length = after.start - span.start + units(name) - units(span.text);
                    warn(
                        "K604",
                        span.start,
                        length,
                        format!(
                            "'{} {}'은 선언한 이름 '{name}'과 띄어쓰기가 다릅니다. 같은 것을 뜻한다면 '{name}'으로 붙여 적으세요.",
                            span.text,
                            &after.text[..name.len() - span.text.len()],
                        ),
                    );
                }
            }
            if VAGUE.contains(&span.text) {
                warn(
                    "K602",
                    span.start,
                    units(span.text),
                    format!(
                        "'{}'은 기준이 정해지지 않은 낱말입니다. 읽는 사람마다 다르게 받아들일 수 있으므로 수치나 조건으로 적으세요.",
                        span.text
                    ),
                );
            }
            if let Some(pointer) = stem_of(span.text, POINTERS.iter().copied()) {
                warn(
                    "K603",
                    span.start,
                    units(pointer),
                    format!(
                        "'{pointer}'이 무엇을 가리키는지 확인할 수 없습니다. 선언한 이름으로 적으세요."
                    ),
                );
            }
        }
    }
    (mentions, warnings)
}

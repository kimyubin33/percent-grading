//! 검사 결과를 사람이 읽는 줄로 만듭니다. 명령줄 출력과 언어 서버가 함께 씁니다.

use aha::{CompiledProgram, IrOutcome};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Level {
    /// 계산된 결과를 알려 줍니다.
    Info,
    /// 컴파일은 되지만 눈여겨봐야 하는 결과입니다(불가능한 목표).
    Warning,
}

/// 소스의 한 줄에 대한 판정 결과.
pub(crate) struct Finding {
    /// 결과가 속한 이름. 이름 없는 조건문은 `7행`입니다.
    pub(crate) name: String,
    pub(crate) line: usize,
    pub(crate) level: Level,
    pub(crate) text: String,
}

/// `검사 통과 | 정의 5개, 검사 2개, ...`
pub(crate) fn summary(program: &CompiledProgram) -> String {
    let checks = program
        .statements
        .iter()
        .filter(|statement| statement.kind == "검사")
        .count();
    let mut summary = format!(
        "검사 통과 | 정의 {}개, 검사 {checks}개",
        program.statements.len() - checks
    );
    for (label, count) in [
        ("기준", program.frames.len()),
        ("대상", program.entities.len()),
        ("사건", program.events.len()),
        ("미지", program.unknowns.len()),
        ("조건·관측", program.constraints.len()),
        ("목표", program.goals.len()),
        ("행위", program.verbs.len()),
        ("문장", program.sentences.len()),
        ("참조", program.references.len()),
        ("본문", program.texts.len()),
        ("낱말", program.words.len()),
        ("장면", program.scenes.len()),
        ("규칙", program.rules.len()),
        ("조건문", program.conditionals.len()),
    ] {
        if count > 0 {
            summary.push_str(&format!(", {label} {count}개"));
        }
    }
    summary
}

/// 미지값의 범위, 규칙과 조건문의 정도, 목표의 판정을 줄마다 만듭니다.
pub(crate) fn findings(program: &CompiledProgram) -> Vec<Finding> {
    let mut findings = Vec::new();

    for unknown in &program.unknowns {
        let show = |bound: &Option<String>, open: &str| match (bound, unknown.unit) {
            (None, _) => open.to_string(),
            (Some(number), Some(unit)) => format!("{number} {unit}"),
            (Some(number), None) => number.clone(),
        };
        let frame = unknown
            .frame
            .as_ref()
            .map_or(String::new(), |frame| format!(" ({frame} 기준)"));
        let observed = if unknown.observed.is_some() {
            " · 관측됨"
        } else {
            ""
        };
        findings.push(Finding {
            name: unknown.name.clone(),
            line: unknown.line,
            level: Level::Info,
            text: format!(
                "미지 '{}': {} ~ {}{frame}{observed}",
                unknown.name,
                show(&unknown.min, "하한 없음"),
                show(&unknown.max, "상한 없음"),
            ),
        });
    }

    // 장면: 채움 점수와, 값을 적지 않은 차원(빈칸).
    for scene in &program.scenes {
        let blanks: Vec<String> = scene
            .items
            .iter()
            .flat_map(|item| {
                item.blanks
                    .iter()
                    .map(move |blank| format!("{}.{blank}", item.name))
            })
            .collect();
        let blank_text = if blanks.is_empty() {
            "빈칸 없음".to_string()
        } else {
            format!("빈칸 {}개: {}", blanks.len(), blanks.join(", "))
        };
        findings.push(Finding {
            name: scene.name.clone(),
            line: scene.line,
            level: Level::Info,
            text: format!(
                "장면 '{}': 낱말 {}개, 채움 {}% ({blank_text})",
                scene.name,
                scene.items.len(),
                scene.fill
            ),
        });
    }

    for rule in &program.rules {
        findings.push(Finding {
            name: rule.name.clone(),
            line: rule.line,
            level: Level::Info,
            text: format!(
                "규칙 '{}': {}{}",
                rule.name,
                rule.value,
                detail(&rule.name, &rule.method, &rule.outcomes)
            ),
        });
    }

    for rule in &program.conditionals {
        let name = if rule.labeled {
            format!("'{}'", rule.name)
        } else {
            rule.name.clone()
        };
        findings.push(Finding {
            name: rule.name.clone(),
            line: rule.line,
            level: Level::Info,
            text: format!(
                "조건문 {name}: {}{}",
                rule.value,
                detail(&rule.name, &rule.method, &rule.outcomes)
            ),
        });
    }

    for goal in &program.goals {
        let mut text = format!("목표 '{}': {}", goal.name, goal.status);
        if !goal.undecided_rules.is_empty() {
            text.push_str(&format!(
                " (규칙의 계산 방식에 따라 다름: {})",
                goal.undecided_rules.join(", ")
            ));
        }
        if !goal.missing_observations.is_empty() {
            text.push_str(&format!(
                " (보장 여부를 확정하려면 관측 필요: {})",
                goal.missing_observations.join(", ")
            ));
        }
        findings.push(Finding {
            name: goal.name.clone(),
            line: goal.line,
            level: if goal.status == "불가능" {
                Level::Warning
            } else {
                Level::Info
            },
            text,
        });
    }

    findings
}

/// 규칙의 값 뒤에 붙이는 설명: 계산 방식, 또는 방식마다의 값.
fn detail(name: &str, method: &Option<&'static str>, outcomes: &Option<Vec<IrOutcome>>) -> String {
    match (method, outcomes) {
        (Some(method), _) => {
            let particle = if *method == "곱" { "으로" } else { "로" };
            format!(" ({method}{particle} 계산)")
        }
        (None, Some(outcomes))
            if outcomes
                .iter()
                .all(|o| o.choices.len() == 1 && o.choices.contains_key(name)) =>
        {
            let parts: Vec<_> = outcomes
                .iter()
                .map(|o| format!("{} {}", o.choices.values().next().unwrap_or(&""), o.value))
                .collect();
            format!(" ({})", parts.join(", "))
        }
        (None, Some(outcomes)) if outcomes.len() <= 4 => {
            let parts: Vec<_> = outcomes
                .iter()
                .map(|o| {
                    let choice: Vec<_> = o
                        .choices
                        .iter()
                        .map(|(rule, method)| format!("{rule} {method}"))
                        .collect();
                    format!("{} → {}", choice.join("·"), o.value)
                })
                .collect();
            format!(" ({})", parts.join(", "))
        }
        (None, Some(outcomes)) => format!(" (방식 조합 {}가지)", outcomes.len()),
        (None, None) => " (두 방식의 결과가 같음)".to_string(),
    }
}

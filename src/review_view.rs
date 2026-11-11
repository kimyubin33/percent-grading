//! Readable, self-contained examiner review. Browser exports are drafts only.
//!
//! All submitted text is embedded as escaped JSON and rendered with textContent.
//! The Rust human-review operation must validate any exported decision again.
use crate::{contract::LockedContract, grading::Report, human};
use serde_json::json;
use std::fmt::Write;

const PAGE: &str = include_str!("review_template.html");
const HELPERS: &str = include_str!("review_helpers.js");

fn script_json(value: &serde_json::Value) -> Result<String, String> {
    serde_json::to_string(value)
        .map(|text| {
            text.replace('&', "\\u0026")
                .replace('<', "\\u003c")
                .replace('>', "\\u003e")
                .replace('\u{2028}', "\\u2028")
                .replace('\u{2029}', "\\u2029")
        })
        .map_err(|_| "Could not encode the examiner review page".into())
}

/// Build an offline HTML document containing private assessment material.
/// This function does not approve a grade or call an AI provider.
pub fn render(lock: &LockedContract, answer: &str, report: &Report) -> Result<String, String> {
    human::validate_report(lock, answer, report)?;
    let template = human::template(lock, answer, report).ok();
    let data = script_json(&json!({
        "contract": lock.contract,
        "answer": answer,
        "report": report,
        "review_template": template,
    }))?;
    // Insert trusted script first: a literal placeholder in submitted text must
    // never be interpreted as another template insertion.
    Ok(PAGE
        .replace("__REVIEW_HELPERS__", HELPERS)
        .replace("__REVIEW_DATA__", &data))
}

/// Alias for application integrations that name their output format explicitly.
pub fn html(lock: &LockedContract, answer: &str, report: &Report) -> Result<String, String> {
    render(lock, answer, report)
}

fn prose(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '&' => output.push_str("&amp;"),
            '<' => output.push_str("&lt;"),
            '>' => output.push_str("&gt;"),
            '\\' | '`' | '*' | '_' | '[' | ']' | '!' | '#' | '|' => {
                output.push('\\');
                output.push(character);
            }
            _ => output.push(character),
        }
    }
    output
}

fn percent(value: Option<f64>) -> String {
    value.map_or_else(|| "Unavailable".into(), |number| format!("{number}%"))
}

/// Plain-language companion for a text viewer or accessible review workflow.
pub fn markdown(lock: &LockedContract, answer: &str, report: &Report) -> Result<String, String> {
    human::validate_report(lock, answer, report)?;
    let mut output = String::from(
        "# Examiner review\n\n\
         **Human review is mandatory. The examiner retains final authority.**\n\n\
         The normal decision uses 50% examiner score and 50% combined AI score. \
         An examiner may explicitly apply their own score with 100% weight, \
         recording their grading method and rationale. Original AI judgments \
         remain visible. A 100% override is a weight; the score ceiling remains 99.9%.\n\n\
         AI explanations can be wrong. Model agreement and quoted evidence do not \
         establish truth. Percentages are rubric fulfillment, not AI-use probabilities. \
         No style, gaze, token, or score difference authorizes an automatic misconduct finding.\n\n",
    );
    writeln!(output, "## Decision\n").unwrap();
    writeln!(
        output,
        "AI component: {}. AI status: {}.\n",
        percent(report.current.percent),
        prose(&report.status)
    )
    .unwrap();
    if report.status != "ready" {
        output.push_str("The AI assessment is unresolved or blocked. A 50:50 final grade is unavailable; a documented examiner override remains possible.\n\n");
    }
    if let Some(decision) = &report.decision {
        writeln!(
            output,
            "Decision status: {}. Final grade: {}.\n",
            prose(&decision.status),
            percent(decision.final_percent)
        )
        .unwrap();
        writeln!(
            output,
            "Recorded weights: examiner {}%; combined AI {}%.\n",
            decision.human_weight_percent, decision.ai_weight_percent
        )
        .unwrap();
    } else {
        output.push_str(
            "Legacy record: no examiner-approved final grade is established by this file.\n\n",
        );
    }
    writeln!(
        output,
        "## Question\n\n{}\n\nDeclared context: {}\n",
        prose(&lock.contract.question),
        prose(&lock.contract.context)
    )
    .unwrap();
    writeln!(output, "## Submitted answer\n\n{}\n", prose(answer)).unwrap();
    writeln!(
        output,
        "## Examiner reference\n\n{}\n",
        prose(&lock.contract.reference)
    )
    .unwrap();
    output.push_str("## AI explanations\n\nThese are supplied, checkable justifications, not private internal reasoning. Future criteria remain separate from the current final grade.\n\n");
    for criterion in &report.criteria {
        writeln!(
            output,
            "### {}\n\nDimension: {}; weight: {}; status: {}.\n",
            prose(&criterion.criterion_id),
            prose(&criterion.dimension),
            criterion.weight,
            prose(&criterion.status)
        )
        .unwrap();
        if let Some(definition) = lock
            .contract
            .criteria
            .iter()
            .find(|c| c.id == criterion.criterion_id)
        {
            writeln!(output, "{}\n", prose(&definition.description)).unwrap();
        }
        for (provider, row) in [("OpenAI", &criterion.openai), ("Claude", &criterion.claude)] {
            writeln!(
                output,
                "**{provider}** — selected anchor: {}\n\n{}\n",
                prose(&row.anchor_id),
                prose(&row.reason)
            )
            .unwrap();
            if row.evidence.is_empty() {
                output.push_str("No supporting excerpt supplied.\n\n");
            }
            for quote in &row.evidence {
                writeln!(output, "Evidence excerpt: {}\n", prose(quote)).unwrap();
            }
            for condition in &row.conditions {
                writeln!(output, "Condition: {}\n", prose(condition)).unwrap();
            }
        }
    }
    if let Some(decision) = &report.decision {
        output.push_str("## Examiner decision history\n\n");
        for review in decision.history.iter().chain(decision.review.iter()) {
            let mode = match review.mode {
                human::ReviewMode::Balanced => "50% examiner and 50% combined AI",
                human::ReviewMode::HumanOverride => "100% examiner override",
            };
            writeln!(
                output,
                "Examiner: {}. Human score: {}%. Decision method: {mode}.\n\nGrading method: {}\n\nRationale: {}\n",
                prose(&review.examiner_id),
                review.human_percent,
                prose(&review.grading_basis),
                prose(&review.rationale)
            )
            .unwrap();
        }
    }
    output.push_str("## Handling and review\n\nKeep this file private: it contains student work and examiner references. The host must authorize reviewers, retain the decision history, apply consistent criteria, and offer a review or appeal route. A hash is a consistency check, not proof of a reviewer's identity or fairness.\n\nThis readable report does not itself approve a new decision. Use the host's examiner-review operation to validate and record one.\n");
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        contract,
        grading::{self, Judgment, Row},
    };

    fn fixture(answer: &str) -> (LockedContract, Report) {
        let lock = contract::compile(include_str!("../policies/apple.aha")).unwrap();
        let judgment = Judgment {
            contract_hash: lock.contract_hash.clone(),
            answer_hash: contract::hash(answer.as_bytes()),
            judgments: lock
                .contract
                .criteria
                .iter()
                .map(|criterion| Row {
                    criterion_id: criterion.id.clone(),
                    anchor_id: criterion
                        .anchors
                        .iter()
                        .find(|anchor| anchor.points == 100)
                        .unwrap()
                        .id
                        .clone(),
                    evidence: vec![answer.into()],
                    reason: "Synthetic rendering fixture; no semantic validity is asserted.".into(),
                    conditions: vec!["Review the supplied context.".into()],
                })
                .collect(),
        };
        let report = grading::aggregate(
            &lock.contract,
            &lock.contract_hash,
            answer,
            &judgment.answer_hash,
            &judgment,
            &judgment,
        )
        .unwrap();
        (lock, report)
    }

    #[test]
    fn untrusted_text_cannot_close_the_data_script_or_create_markup() {
        let answer = "</script><script>globalThis.stolen=true</script><img src=x onerror=alert(1)> & \u{2028}";
        let (lock, report) = fixture(answer);
        let page = render(&lock, answer, &report).unwrap();
        assert!(!page.contains(answer));
        assert!(page.contains("\\u003c/script\\u003e"));
        assert!(page.contains("\\u2028"));
        assert!(!page.contains(".innerHTML"));
        let start = page
            .find("<script id=\"review-data\" type=\"application/json\">")
            .unwrap()
            + "<script id=\"review-data\" type=\"application/json\">".len();
        let end = page[start..].find("</script>").unwrap() + start;
        let recovered: serde_json::Value = serde_json::from_str(&page[start..end]).unwrap();
        assert_eq!(recovered["answer"], answer);
        assert_eq!(recovered["review_template"]["reviewed_ai_report"], false);
        assert_eq!(
            recovered["review_template"]["acknowledged_ai_limits"],
            false
        );
    }

    #[test]
    fn readable_exports_require_exact_bindings_and_preserve_human_authority() {
        let (lock, report) = fixture("Red under the declared white illumination.");
        assert!(render(&lock, "a different answer", &report).is_err());
        let text = markdown(&lock, "Red under the declared white illumination.", &report).unwrap();
        assert!(text.contains("Human review is mandatory"));
        assert!(text.contains("**OpenAI**"));
        assert!(text.contains("**Claude**"));
        assert!(text.contains("awaiting\\_examiner"));
        assert!(text.contains("Final grade: Unavailable"));
    }

    #[test]
    fn markdown_escapes_html_and_template_markers_remain_literal_data() {
        let answer = "<img src=x> [click](javascript:bad) __REVIEW_HELPERS__";
        let (lock, report) = fixture(answer);
        let text = markdown(&lock, answer, &report).unwrap();
        assert!(!text.contains("<img src=x>"));
        assert!(text.contains("&lt;img src=x&gt;"));
        let page = render(&lock, answer, &report).unwrap();
        assert!(page.contains("__REVIEW_HELPERS__"));
        assert_eq!(page.matches("globalThis.ExaminerReviewForm =").count(), 1);
    }

    #[test]
    fn finalized_override_keeps_ai_explanations_and_prior_examiner_method_visible() {
        let answer = "Red under the declared white illumination.";
        let (lock, report) = fixture(answer);
        let mut first = human::template(&lock, answer, &report).unwrap();
        first.examiner_id = "examiner_01".into();
        first.human_percent = 60.0;
        first.grading_basis = "Initial synthetic grading method.".into();
        first.rationale = "The explanation is incomplete under my stated method.".into();
        first.reviewed_ai_report = true;
        first.acknowledged_ai_limits = true;
        let reviewed = human::review(&lock, answer, &report, &first).unwrap();
        let mut revised = human::template(&lock, answer, &reviewed).unwrap();
        revised.examiner_id = "examiner_01".into();
        revised.human_percent = 80.0;
        revised.mode = human::ReviewMode::HumanOverride;
        revised.grading_basis = "Revised synthetic grading method.".into();
        revised.rationale = "Correct the earlier interpretation while retaining its record.".into();
        revised.reviewed_ai_report = true;
        revised.acknowledged_ai_limits = true;
        let final_report = human::review(&lock, answer, &reviewed, &revised).unwrap();
        let text = markdown(&lock, answer, &final_report).unwrap();
        assert!(text.contains("Final grade: 80%"));
        assert!(text.contains("50% examiner and 50% combined AI"));
        assert!(text.contains("100% examiner override"));
        assert!(text.contains("Initial synthetic grading method"));
        assert!(text.contains("Revised synthetic grading method"));
        assert!(text.contains("**OpenAI**"));
        assert!(render(&lock, answer, &final_report).is_ok());
    }
}

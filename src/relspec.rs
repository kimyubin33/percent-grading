//! Real AHAA compilation followed by a deliberately narrow scene-only boundary.
//!
//! The source compiler validates grammar, names, required dimensions, duplicates,
//! and typed values. The grading compiler must still validate every record kind,
//! field, identifier, score anchor, and cross-reference in the returned records.

use std::collections::BTreeMap;

pub const UPSTREAM_COMMIT: &str = "de1e956555dcfafc5b62324ea6ed3f62655c6ba3";
pub const MAX_CONTRACT_BYTES: usize = 256 * 1024;
pub const MAX_CONTRACT_LINES: usize = 2048;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SceneRecord {
    pub word: String,
    pub name: String,
    pub values: BTreeMap<String, String>,
}

/// Compile one flat AHAA scene, rejecting any construct the grading layer would
/// otherwise silently ignore. Scene fill is a syntax-completeness metric, never
/// an answer grade, and is intentionally absent from the returned records.
pub fn compile_scene(source: &str) -> Result<Vec<SceneRecord>, String> {
    if source.len() > MAX_CONTRACT_BYTES || source.lines().count() > MAX_CONTRACT_LINES {
        return Err("RelSpec contract exceeds the size or line limit".into());
    }
    let program = aha::compile(source).map_err(|d| {
        format!(
            "RelSpec compilation failed: {} at line {}, column {}",
            d.code, d.line, d.column
        )
    })?;
    if program.scenes.len() != 1 {
        return Err("A grading contract must contain exactly one RelSpec scene".into());
    }
    let scene = &program.scenes[0];
    // Scene members are also registered as upstream entities. Any additional
    // entity would have been declared outside the supported scene subset.
    if !program.statements.is_empty()
        || !program.units.is_empty()
        || !program.frames.is_empty()
        || program.entities.len() != scene.items.len()
        || !program.events.is_empty()
        || !program.unknowns.is_empty()
        || !program.constraints.is_empty()
        || !program.goals.is_empty()
        || !program.verbs.is_empty()
        || !program.sentences.is_empty()
        || !program.references.is_empty()
        || !program.rules.is_empty()
        || !program.conditionals.is_empty()
        || program.topic.is_some()
        || !program.texts.is_empty()
    {
        return Err("Grading contracts permit only word declarations and one flat scene".into());
    }
    let mut records = Vec::with_capacity(scene.items.len());
    for item in &scene.items {
        if item.depth != 0 || item.parent.is_some() {
            return Err(format!("RelSpec record {} must not be nested", item.name));
        }
        if !item.blanks.is_empty() {
            return Err(format!(
                "RelSpec record {} has unspecified dimensions: {}",
                item.name,
                item.blanks.join(", ")
            ));
        }
        let mut values = BTreeMap::new();
        for value in &item.values {
            if value.value.trim().is_empty()
                || values
                    .insert(value.dimension.clone(), value.value.clone())
                    .is_some()
            {
                return Err(format!(
                    "RelSpec record {} has an empty or duplicate field",
                    item.name
                ));
            }
        }
        records.push(SceneRecord {
            word: item.word.clone(),
            name: item.name.clone(),
            values,
        });
    }
    Ok(records)
}

#[cfg(test)]
mod tests {
    use super::*;

    const WORDS: &str = "낱말 Criterion: weight*(정수), dimension*, critical*\n";

    #[test]
    fn actual_compiler_accepts_english_record_names_and_typed_fields() {
        let source = format!(
            "{WORDS}장면 Rubric\n  Criterion color: weight 30, dimension intent, critical yes\n  Criterion reasoning: weight 40, dimension reasoning, critical no\n"
        );
        let records = compile_scene(&source).unwrap();
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].word, "Criterion");
        assert_eq!(records[0].name, "color");
        assert_eq!(records[0].values["weight"], "30");
    }

    #[test]
    fn actual_compiler_rejects_missing_required_field_and_invalid_type() {
        let missing =
            format!("{WORDS}장면 Rubric\n  Criterion color: weight 30, dimension intent\n");
        assert!(compile_scene(&missing).unwrap_err().contains("K704"));
        let wrong_type = format!(
            "{WORDS}장면 Rubric\n  Criterion color: weight 30%, dimension intent, critical yes\n"
        );
        assert!(compile_scene(&wrong_type).unwrap_err().contains("K303"));
    }

    #[test]
    fn duplicate_unknown_and_undeclared_fields_fail_in_upstream() {
        for source in [
            format!(
                "{WORDS}장면 Rubric\n  Criterion color: weight 30, weight 40, dimension intent, critical yes\n"
            ),
            format!(
                "{WORDS}장면 Rubric\n  Criterion color: weight 30, dimension intent, critical yes, invented true\n"
            ),
            "장면 Rubric\n  Invented color: value yes\n".into(),
        ] {
            assert!(compile_scene(&source).is_err());
        }
    }

    #[test]
    fn no_ignored_constructs_nesting_or_optional_blanks() {
        let valid = format!(
            "{WORDS}장면 Rubric\n  Criterion color: weight 30, dimension intent, critical yes\n"
        );
        assert!(compile_scene(&format!("{valid}정의 hidden = 1\n")).is_err());
        assert!(compile_scene(&format!("주제 hidden\n{valid}")).is_err());
        assert!(compile_scene(&format!("대상 hidden\n{valid}")).is_err());
        assert!(
            compile_scene(&format!(
                "{valid}    Criterion child: weight 40, dimension reasoning, critical no\n"
            ))
            .is_err()
        );
        assert!(
            compile_scene(
                "낱말 Criterion: weight*, optional\n장면 Rubric\n  Criterion color: weight 30\n"
            )
            .is_err()
        );
        assert!(
            compile_scene(&format!(
                "{valid}장면 Other\n  Criterion other: weight 10, dimension intent, critical yes\n"
            ))
            .is_err()
        );
    }

    #[test]
    fn contract_size_is_bounded_before_upstream_compilation() {
        assert!(compile_scene(&" ".repeat(MAX_CONTRACT_BYTES + 1)).is_err());
        assert!(compile_scene(&"\n".repeat(MAX_CONTRACT_LINES + 1)).is_err());
    }
}

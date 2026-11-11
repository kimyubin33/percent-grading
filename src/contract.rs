//! A closed grading profile over the real AHAA checked scene IR.
use crate::relspec::{SceneRecord, compile_scene};
use ring::digest::{SHA256, digest};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const COMPILER_REVISION: &str = "de1e956555dcfafc5b62324ea6ed3f62655c6ba3";
pub const FORMAT: &str = "percent-grading-contract-v1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Contract {
    pub id: String,
    pub version: String,
    pub question: String,
    pub context: String,
    pub reference: String,
    pub criteria: Vec<Criterion>,
    pub perspectives: Vec<Perspective>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Criterion {
    pub id: String,
    pub dimension: String,
    pub weight: u32,
    pub description: String,
    pub critical_min: Option<u32>,
    pub anchors: Vec<Anchor>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Anchor {
    pub id: String,
    pub points: u32,
    pub description: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Perspective {
    pub id: String,
    pub criteria: Vec<String>,
    pub kind: String,
    pub source: String,
    pub assumptions: String,
    pub description: String,
    pub overlap: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct LockedContract {
    pub format: String,
    pub compiler_revision: String,
    pub source: String,
    pub contract: Contract,
    pub contract_hash: String,
}

pub fn hash(bytes: &[u8]) -> String {
    digest(&SHA256, bytes)
        .as_ref()
        .iter()
        .map(|x| format!("{x:02x}"))
        .collect()
}
fn identifier(s: &str) -> Result<(), String> {
    if s.is_empty()
        || s.len() > 64
        || !s
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
    {
        return Err(format!(
            "Invalid identifier: {s:?}; use 1..64 ASCII letters, digits, underscores or hyphens"
        ));
    }
    Ok(())
}
fn fields(record: &SceneRecord, expected: &[&str]) -> Result<(), String> {
    let keys: BTreeSet<_> = record.values.keys().map(String::as_str).collect();
    if keys != expected.iter().copied().collect() {
        return Err(format!(
            "{} {} must have exactly these fields: {}",
            record.word,
            record.name,
            expected.join(", ")
        ));
    }
    for value in record.values.values() {
        if value.trim().is_empty() || value.len() > 16_384 {
            return Err(format!(
                "{}: field values must contain 1..16384 bytes",
                record.name
            ));
        }
    }
    Ok(())
}
fn integer(value: &str, label: &str) -> Result<u32, String> {
    if value.is_empty() || !value.bytes().all(|c| c.is_ascii_digit()) {
        return Err(format!("{label}: expected an unsigned decimal integer"));
    }
    value
        .parse()
        .map_err(|_| format!("{label}: integer overflow"))
}

pub fn compile(source: &str) -> Result<LockedContract, String> {
    let records = compile_scene(source)?;
    let mut policy = None;
    let mut criteria: BTreeMap<String, Criterion> = BTreeMap::new();
    let mut anchors = Vec::new();
    let mut perspectives = Vec::new();
    let mut all_ids = BTreeSet::new();
    for record in records {
        identifier(&record.name)?;
        if !all_ids.insert(record.name.clone()) || record.name == "unresolved" {
            return Err(format!(
                "Duplicate or reserved record identifier: {}",
                record.name
            ));
        }
        let v = &record.values;
        match record.word.as_str() {
            "Policy" => {
                fields(&record, &["version", "question", "context", "reference"])?;
                if policy.is_some() {
                    return Err("Exactly one Policy record is required".into());
                }
                policy = Some((
                    record.name.clone(),
                    v["version"].clone(),
                    v["question"].clone(),
                    v["context"].clone(),
                    v["reference"].clone(),
                ));
            }
            "Criterion" => {
                fields(
                    &record,
                    &["dimension", "weight", "description", "critical_min"],
                )?;
                let dimension = &v["dimension"];
                if !["intent", "principle", "reasoning", "conditions", "future"]
                    .contains(&dimension.as_str())
                {
                    return Err(format!("{}: unknown dimension", record.name));
                }
                let weight = integer(&v["weight"], "weight")?;
                if !(1..=100).contains(&weight) {
                    return Err("Criterion weights must be 1..100".into());
                }
                let critical_min = if v["critical_min"] == "none" {
                    None
                } else {
                    Some(integer(&v["critical_min"], "critical_min")?)
                };
                if critical_min.is_some_and(|n| n > 100)
                    || (dimension == "future" && critical_min.is_some())
                {
                    return Err(
                        "Critical minimum must be 0..100 and may not gate a future criterion"
                            .into(),
                    );
                }
                criteria.insert(
                    record.name.clone(),
                    Criterion {
                        id: record.name,
                        dimension: dimension.clone(),
                        weight,
                        description: v["description"].clone(),
                        critical_min,
                        anchors: Vec::new(),
                    },
                );
            }
            "Anchor" => {
                fields(&record, &["criterion", "points", "description"])?;
                let points = integer(&v["points"], "anchor points")?;
                if points > 100 {
                    return Err("Anchor points must be 0..100".into());
                }
                anchors.push((
                    v["criterion"].clone(),
                    Anchor {
                        id: record.name,
                        points,
                        description: v["description"].clone(),
                    },
                ));
            }
            "Perspective" => {
                fields(
                    &record,
                    &[
                        "criteria",
                        "kind",
                        "source",
                        "assumptions",
                        "description",
                        "overlap",
                    ],
                )?;
                if !["established", "method", "hypothesis"].contains(&v["kind"].as_str()) {
                    return Err("Perspective kind must be established, method or hypothesis".into());
                }
                let ids: Vec<String> = v["criteria"]
                    .split_whitespace()
                    .map(str::to_owned)
                    .collect();
                if ids.is_empty() || ids.iter().collect::<BTreeSet<_>>().len() != ids.len() {
                    return Err("Perspective criteria must be a nonempty unique list".into());
                }
                perspectives.push(Perspective {
                    id: record.name,
                    criteria: ids,
                    kind: v["kind"].clone(),
                    source: v["source"].clone(),
                    assumptions: v["assumptions"].clone(),
                    description: v["description"].clone(),
                    overlap: v["overlap"].clone(),
                });
            }
            other => return Err(format!("Unsupported grading record type: {other}")),
        }
    }
    let (id, version, question, context, reference) =
        policy.ok_or("Exactly one Policy record is required")?;
    if criteria.is_empty() || criteria.len() > 100 {
        return Err("A contract requires 1..100 criteria".into());
    }
    for (cid, anchor) in anchors {
        criteria
            .get_mut(&cid)
            .ok_or_else(|| format!("Anchor references unknown criterion: {cid}"))?
            .anchors
            .push(anchor);
    }
    let mut current_weight = 0;
    let mut future_weight = 0;
    for criterion in criteria.values_mut() {
        criterion.anchors.sort_by_key(|a| a.points);
        let points: BTreeSet<_> = criterion.anchors.iter().map(|a| a.points).collect();
        if criterion.anchors.len() < 2
            || criterion.anchors.len() > 8
            || points.len() != criterion.anchors.len()
            || !points.contains(&0)
            || !points.contains(&100)
        {
            return Err(format!(
                "{} needs 2..8 anchors with unique points including 0 and 100",
                criterion.id
            ));
        }
        if criterion.dimension == "future" {
            future_weight += criterion.weight;
        } else {
            current_weight += criterion.weight;
        }
    }
    if current_weight != 100 || (future_weight != 0 && future_weight != 100) {
        return Err("Current criterion weights must sum to 100; future weights must separately sum to 100 when present".into());
    }
    if !criteria.values().any(|c| c.dimension == "intent") {
        return Err("At least one intent criterion is required".into());
    }
    if perspectives.is_empty() || perspectives.len() > 256 {
        return Err("A contract requires 1..256 perspectives".into());
    }
    let mut covered = BTreeSet::new();
    for p in &mut perspectives {
        identifier(&p.overlap)?;
        for cid in &p.criteria {
            if !criteria.contains_key(cid) {
                return Err(format!(
                    "Perspective {} references unknown criterion {cid}",
                    p.id
                ));
            }
            covered.insert(cid.clone());
        }
        p.criteria.sort();
    }
    if covered.len() != criteria.len() {
        return Err("Every criterion must have at least one declared perspective".into());
    }
    perspectives.sort_by(|a, b| a.id.cmp(&b.id));
    let contract = Contract {
        id,
        version,
        question,
        context,
        reference,
        criteria: criteria.into_values().collect(),
        perspectives,
    };
    let contract_hash = hash(
        &serde_json::to_vec(&(FORMAT, COMPILER_REVISION, source, &contract))
            .map_err(|e| e.to_string())?,
    );
    Ok(LockedContract {
        format: FORMAT.into(),
        compiler_revision: COMPILER_REVISION.into(),
        source: source.into(),
        contract,
        contract_hash,
    })
}

impl LockedContract {
    /// Recompile the embedded source; a changed JSON weight cannot bypass the language.
    pub fn verify(&self) -> Result<(), String> {
        let expected = compile(&self.source)?;
        if *self != expected {
            return Err(
                "Locked contract differs from its checked source or compiler version".into(),
            );
        }
        Ok(())
    }
}

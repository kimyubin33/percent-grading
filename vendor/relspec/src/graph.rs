//! 연결망.
//!
//! 명세에 적힌 것들을 점으로, 그 사이의 관계를 선으로 옮깁니다.
//! 대상·행위·값·기준이 점이 되고, 문장 하나는 그 점들을 역할로 잇는 점이 됩니다.
//! 정도(80% 등)는 점과 선의 세기가 됩니다.
//!
//! 선에는 두 가지가 있습니다.
//!
//! - **확인된 선**: 컴파일러가 문법과 타입으로 확인한 관계입니다(역할, 식에서의 쓰임, 전제와 결론).
//! - **언급**: 자유로운 글에 선언한 이름이 나온 자리입니다. 글의 뜻을 확인한 것은 아닙니다.

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;

use crate::{CompiledProgram, IrClause, IrExpression, IrItem, IrSentenceRole};

#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct Node {
    pub id: String,
    /// 대상, 행위, 기준, 단위, 정의, 검사, 미지, 조건, 관측, 목표, 문장, 조건문, 규칙, 사건, 장면, 글
    pub kind: &'static str,
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<usize>,
    /// 값, 범위, 판정처럼 이 점에 대해 컴파일러가 아는 것.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub detail: String,
    /// 문장·조건문·규칙의 정도(예: `80%`, `56%~70%`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub degree: Option<String>,
    /// 목표의 판정(보장·가능·불가능).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<&'static str>,
    /// 이 줄에서 나온 본문 점검 알림의 수.
    #[serde(skip_serializing_if = "is_zero")]
    pub warnings: usize,
}

fn is_zero(count: &usize) -> bool {
    *count == 0
}

#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct Edge {
    pub from: String,
    pub to: String,
    /// 역할, 행위, 쓰임, 전제, 기준, 단위, 포함, 참조, 딸림, 언급
    pub kind: &'static str,
    /// 역할 이름(주체, 대상, 시작 …)이나 선에 붙는 글.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub label: String,
    /// 컴파일러가 확인한 관계이면 참, 글 속의 언급이면 거짓.
    pub checked: bool,
}

#[derive(Serialize, Debug, Default)]
pub struct Graph {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
}

struct Builder {
    graph: Graph,
    ids: BTreeSet<String>,
    /// 줄 번호 → 그 줄에 적힌 글의 점.
    texts: BTreeMap<usize, String>,
    seen: BTreeSet<(String, String, &'static str, String)>,
}

impl Builder {
    fn node(&mut self, id: String, kind: &'static str, label: &str) -> &mut Node {
        if self.ids.insert(id.clone()) {
            self.graph.nodes.push(Node {
                id: id.clone(),
                kind,
                label: label.to_string(),
                line: None,
                detail: String::new(),
                degree: None,
                status: None,
                warnings: 0,
            });
        }
        self.graph
            .nodes
            .iter_mut()
            .find(|node| node.id == id)
            .expect("node was just ensured")
    }

    fn edge(&mut self, from: &str, to: &str, kind: &'static str, label: &str, checked: bool) {
        if from == to || !self.ids.contains(from) || !self.ids.contains(to) {
            return;
        }
        let key = (from.to_string(), to.to_string(), kind, label.to_string());
        if self.seen.insert(key) {
            self.graph.edges.push(Edge {
                from: from.to_string(),
                to: to.to_string(),
                kind,
                label: label.to_string(),
                checked,
            });
        }
    }

    /// 식에 나온 이름마다 `from`에서 그 이름의 점으로 선을 긋습니다.
    fn uses(&mut self, from: &str, expression: &IrExpression) {
        match expression {
            IrExpression::Literal { .. } => {}
            IrExpression::Reference { name, .. } => self.edge(from, &named(name), "쓰임", "", true),
            IrExpression::Member { event, role, .. } => {
                self.edge(from, &named(event), "쓰임", role, true);
            }
            IrExpression::Unary { operands, .. } => self.uses(from, &operands[0]),
            IrExpression::Binary { operands, .. } => {
                self.uses(from, &operands[0]);
                self.uses(from, &operands[1]);
            }
            IrExpression::Anchor {
                frame, operands, ..
            } => {
                self.edge(from, &named(frame), "기준", "", true);
                self.uses(from, &operands[0]);
            }
        }
    }

    /// 문장의 역할을 선으로 옮깁니다. 대상은 그 대상의 점으로, 시각과 위치는 기준의 점으로 갑니다.
    fn roles(&mut self, from: &str, roles: &[IrSentenceRole]) {
        for role in roles {
            if role.value_type == "대상" {
                self.edge(from, &named(&role.value), "역할", &role.role, true);
            } else if let Some(frame) = &role.frame {
                let label = format!("{} @ {}", role.role, quantity(&role.value, role.unit));
                self.edge(from, &named(frame), "역할", &label, true);
            }
            if let Some(number) = role.reference {
                self.edge(from, &format!("참조:{number}"), "참조", &role.role, true);
            }
        }
    }

    fn text(&mut self, host: Option<&str>, line: usize, what: &'static str, text: &str) {
        let id = format!("글:{line}");
        let node = self.node(id.clone(), "글", &shorten(text));
        node.line = Some(line);
        node.detail = format!("{what}: {text}");
        self.texts.insert(line, id.clone());
        if let Some(host) = host {
            self.edge(&id, host, "딸림", what, true);
        }
    }

    fn items(&mut self, host: &str, items: &[IrItem]) {
        for item in items {
            self.text(Some(host), item.line, "딸린 줄", &item.text);
        }
    }
}

impl Graph {
    /// 선으로 이어진 덩어리들. 덩어리마다 그 안의 점 번호(`nodes`에서의 순서)를 담습니다.
    /// 덩어리가 여럿이면 명세가 서로 이어지지 않은 부분들로 나뉘어 있다는 뜻입니다.
    pub fn components(&self) -> Vec<Vec<usize>> {
        let index: BTreeMap<&str, usize> = self
            .nodes
            .iter()
            .enumerate()
            .map(|(i, node)| (node.id.as_str(), i))
            .collect();
        let mut parent: Vec<usize> = (0..self.nodes.len()).collect();
        fn find(parent: &mut [usize], mut i: usize) -> usize {
            while parent[i] != i {
                parent[i] = parent[parent[i]];
                i = parent[i];
            }
            i
        }
        for edge in &self.edges {
            if let (Some(&a), Some(&b)) =
                (index.get(edge.from.as_str()), index.get(edge.to.as_str()))
            {
                let (a, b) = (find(&mut parent, a), find(&mut parent, b));
                parent[a.max(b)] = a.min(b);
            }
        }
        let mut groups: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
        for i in 0..self.nodes.len() {
            let root = find(&mut parent, i);
            groups.entry(root).or_default().push(i);
        }
        let mut groups: Vec<_> = groups.into_values().collect();
        groups.sort_by_key(|group| std::cmp::Reverse(group.len()));
        groups
    }
}

/// 이름 있는 것(대상, 정의, 미지값, 기준, 사건, 이름 붙은 문장, 규칙)의 점 번호.
fn named(name: &str) -> String {
    format!("이름:{name}")
}

fn quantity(value: &str, unit: Option<&str>) -> String {
    match unit {
        Some(unit) => format!("{value} {unit}"),
        None => value.to_string(),
    }
}

fn shorten(text: &str) -> String {
    let mut short: String = text.chars().take(16).collect();
    if text.chars().count() > 16 {
        short.push('…');
    }
    short
}

/// "신청자가 강의실을 예약한다"처럼 절을 다시 글로 적습니다.
fn clause(clause: &IrClause) -> String {
    words(&clause.roles, &clause.form)
}

fn words(roles: &[IrSentenceRole], form: &str) -> String {
    let mut words: Vec<String> = roles
        .iter()
        .filter(|role| role.value_type == "대상")
        .map(|role| format!("{}{}", role.value, role.particle))
        .collect();
    words.push(form.to_string());
    words.join(" ")
}

/// 연결망을 브라우저에서 볼 수 있는 HTML 한 장으로 만듭니다. 다른 파일이나 인터넷이 필요 없습니다.
pub fn html(graph: &Graph, title: &str) -> String {
    let data =
        serde_json::to_string(graph).unwrap_or_else(|_| "{\"nodes\":[],\"edges\":[]}".into());
    // 글에 `</script>`가 들어 있어도 문서가 깨지지 않게 합니다.
    let data = data.replace("</", "<\\/");
    let title: String = title
        .chars()
        .map(|c| match c {
            '<' => '＜',
            '>' => '＞',
            '&' => '＆',
            c => c,
        })
        .collect();
    include_str!("graph.html")
        .replace("/*TITLE*/", &title)
        .replace("/*GRAPH*/{\"nodes\":[],\"edges\":[]}", &data)
}

/// 컴파일한 명세를 연결망으로 옮깁니다.
pub fn build(program: &CompiledProgram) -> Graph {
    let mut b = Builder {
        graph: Graph::default(),
        ids: BTreeSet::new(),
        texts: BTreeMap::new(),
        seen: BTreeSet::new(),
    };

    // 1. 이름 있는 점을 먼저 모두 만듭니다. 선은 양 끝의 점이 있어야 그어집니다.
    for frame in &program.frames {
        b.node(named(&frame.name), "기준", &frame.name).detail = frame.axis.to_string();
    }
    for entity in &program.entities {
        b.node(named(&entity.name), "대상", &entity.name);
    }
    for unit in &program.units {
        b.node(format!("단위:{unit}"), "단위", unit);
    }
    for verb in &program.verbs {
        b.node(format!("행위:{}", verb.name), "행위", &verb.name)
            .detail = format!("필수 역할: {}", verb.required.join(", "));
    }
    for statement in &program.statements {
        let id = if statement.kind == "정의" {
            named(&statement.name)
        } else {
            format!("검사:{}", statement.name)
        };
        let node = b.node(id, statement.kind, &statement.name);
        node.detail = format!(
            "{} = {}",
            statement.value_type,
            quantity(&statement.value, statement.unit)
        );
    }
    for unknown in &program.unknowns {
        let bound = |value: &Option<String>, none: &str| match value {
            Some(value) => quantity(value, unknown.unit),
            None => none.to_string(),
        };
        let node = b.node(named(&unknown.name), "미지", &unknown.name);
        node.line = Some(unknown.line);
        node.detail = match &unknown.observed {
            Some(observed) => format!("관측 {}", quantity(observed, unknown.unit)),
            None => format!(
                "{} ~ {}",
                bound(&unknown.min, "하한 없음"),
                bound(&unknown.max, "상한 없음")
            ),
        };
    }
    for event in &program.events {
        b.node(named(&event.name), "사건", &event.name);
    }
    for reference in &program.references {
        let id = format!("참조:{}", reference.number);
        let label = format!("참조 {}", reference.number);
        let node = b.node(id.clone(), "글", &label);
        node.line = Some(reference.line);
        node.detail = format!("참조 {}: {}", reference.number, reference.text);
        b.texts.insert(reference.line, id);
    }
    // 장면: 안에 든 낱말은 이미 대상의 점으로 있습니다. 거기에 채운 값과 빈칸을 적고, 포함을 선으로 잇습니다.
    for scene in &program.scenes {
        let scene_id = format!("장면:{}", scene.name);
        let node = b.node(scene_id.clone(), "장면", &scene.name);
        node.line = Some(scene.line);
        node.detail = format!("채움 {}%", scene.fill);
        for item in &scene.items {
            let id = named(&item.name);
            let mut lines: Vec<String> = item
                .values
                .iter()
                .map(|v| format!("{}: {}", v.dimension, v.value))
                .collect();
            if !item.blanks.is_empty() {
                lines.push(format!("빈칸: {}", item.blanks.join(", ")));
            }
            let node = b.node(id.clone(), "대상", &item.name);
            node.line = Some(item.line);
            node.detail = lines.join("\n");
            let parent = match &item.parent {
                Some(parent) => named(parent),
                None => scene_id.clone(),
            };
            b.edge(&parent, &id, "포함", "", true);
        }
    }
    for sentence in &program.sentences {
        let id = sentence_id(sentence.label.as_deref(), sentence.line);
        let node = b.node(id, "문장", &words(&sentence.roles, &sentence.form));
        node.line = Some(sentence.line);
        node.degree = Some(format!("{}%", sentence.degree));
        if let Some(tense) = sentence.tense {
            node.detail = format!("시제: {tense}");
        }
    }
    for conditional in &program.conditionals {
        let id = sentence_id(
            conditional.labeled.then_some(conditional.name.as_str()),
            conditional.line,
        );
        let label = format!(
            "{} {}",
            clause(&conditional.condition),
            clause(&conditional.conclusion)
        );
        let node = b.node(id, "조건문", &label);
        node.line = Some(conditional.line);
        node.degree = Some(conditional.value.clone());
        node.detail = format!(
            "전제 {} · 강도 {} · {}",
            conditional.premise,
            conditional.strength,
            conditional.method.map_or("두 방식", |method| method)
        );
    }
    for rule in &program.rules {
        let node = b.node(named(&rule.name), "규칙", &rule.name);
        node.line = Some(rule.line);
        node.degree = Some(rule.value.clone());
    }

    // 2. 확인된 선.
    for statement in &program.statements {
        let id = if statement.kind == "정의" {
            named(&statement.name)
        } else {
            format!("검사:{}", statement.name)
        };
        b.uses(&id, &statement.expression);
        if let Some(frame) = &statement.frame {
            b.edge(&id, &named(frame), "기준", "", true);
        }
        if let Some(unit) = statement.unit {
            b.edge(&id, &format!("단위:{unit}"), "단위", "", true);
        }
    }
    for unknown in &program.unknowns {
        if let Some(frame) = &unknown.frame {
            b.edge(&named(&unknown.name), &named(frame), "기준", "", true);
        }
        if let Some(unit) = unknown.unit {
            b.edge(
                &named(&unknown.name),
                &format!("단위:{unit}"),
                "단위",
                "",
                true,
            );
        }
    }
    for event in &program.events {
        let id = named(&event.name);
        for role in &event.roles {
            if role.value_type == "대상" {
                b.edge(&id, &named(&role.value), "역할", &role.role, true);
            }
            if let Some(expression) = &role.expression {
                b.uses(&id, expression);
            }
        }
    }
    for (index, constraint) in program.constraints.iter().enumerate() {
        let id = format!("{}:{index}", constraint.kind);
        b.node(id.clone(), constraint.kind, &constraint.name);
        b.uses(&id, &constraint.expression);
        if constraint.kind == "관측" {
            b.edge(&id, &named(&constraint.name), "쓰임", "", true);
        }
    }
    for (index, goal) in program.goals.iter().enumerate() {
        let id = format!("목표:{index}");
        let node = b.node(id.clone(), "목표", &goal.name);
        node.line = Some(goal.line);
        node.status = Some(goal.status);
        node.detail = goal.status.to_string();
        b.uses(&id, &goal.expression);
    }
    for sentence in &program.sentences {
        let id = sentence_id(sentence.label.as_deref(), sentence.line);
        b.edge(&id, &format!("행위:{}", sentence.verb), "행위", "", true);
        b.roles(&id, &sentence.roles);
        for number in &sentence.references {
            b.edge(&id, &format!("참조:{number}"), "참조", "", true);
        }
        if let Some(note) = &sentence.note {
            b.text(Some(&id), sentence.line, "설명", note);
        }
        b.items(&id, &sentence.items);
    }
    // 조건문의 전제는 앞의 문장이나 조건문에서 옵니다. 줄 번호로 그 점을 찾습니다.
    let by_line: BTreeMap<usize, String> = program
        .sentences
        .iter()
        .map(|s| (s.line, sentence_id(s.label.as_deref(), s.line)))
        .chain(program.conditionals.iter().map(|c| {
            (
                c.line,
                sentence_id(c.labeled.then_some(c.name.as_str()), c.line),
            )
        }))
        .collect();
    for conditional in &program.conditionals {
        let id = by_line[&conditional.line].clone();
        b.edge(
            &id,
            &format!("행위:{}", conditional.conclusion.verb),
            "행위",
            "결론",
            true,
        );
        b.edge(
            &id,
            &format!("행위:{}", conditional.condition.verb),
            "행위",
            "전제",
            true,
        );
        b.roles(&id, &conditional.conclusion.roles);
        for line in &conditional.matched_lines {
            if let Some(source) = by_line.get(line) {
                b.edge(source, &id, "전제", &conditional.strength, true);
            }
        }
        for number in &conditional.references {
            b.edge(&id, &format!("참조:{number}"), "참조", "", true);
        }
        if let Some(note) = &conditional.note {
            b.text(Some(&id), conditional.line, "설명", note);
        }
        b.items(&id, &conditional.items);
    }
    for rule in &program.rules {
        let id = named(&rule.name);
        b.uses(&id, &rule.premise);
        b.uses(&id, &rule.strength);
    }
    for reference in &program.references {
        let id = format!("참조:{}", reference.number);
        b.items(&id, &reference.items);
    }
    for text in &program.texts {
        b.text(None, text.line, "본문", &text.text);
        let id = format!("글:{}", text.line);
        b.items(&id, &text.items);
    }

    // 3. 언급: 글에 선언한 이름이 나온 자리. 확인된 관계가 아닙니다.
    for mention in &program.mentions {
        if let Some(text) = b.texts.get(&mention.line).cloned() {
            b.edge(&text, &named(&mention.name), "언급", "", false);
        }
    }
    for warning in &program.warnings {
        if let Some(text) = b.texts.get(&warning.line).cloned()
            && let Some(node) = b.graph.nodes.iter_mut().find(|node| node.id == text)
        {
            node.warnings += 1;
        }
    }
    b.graph
}

/// 이름 붙은 문장은 이름으로, 아니면 줄 번호로 부릅니다.
fn sentence_id(label: Option<&str>, line: usize) -> String {
    match label {
        Some(label) => named(label),
        None => format!("문장:{line}"),
    }
}

use crate::Diagnostic;
use crate::integer::Integer;
use crate::syntax::{Expression, Kind, Role, Statement, Token, Type, intern, split_multiplier};

pub(crate) fn parse(tokens: &[Token]) -> Result<Vec<Statement>, Diagnostic> {
    Parser { tokens, index: 0 }.parse()
}

/// 토큰 몇 개를 식 하나로 읽습니다. 장면에서 타입이 있는 차원의 값을 읽을 때 씁니다.
pub(crate) fn parse_expression(tokens: &[Token]) -> Result<Expression, Diagnostic> {
    let last = tokens.last().expect("a value has at least one token");
    let mut tokens = tokens.to_vec();
    let mut end = last.renamed(String::new());
    end.kind = Kind::End;
    tokens.push(end);
    let mut parser = Parser {
        tokens: &tokens,
        index: 0,
    };
    let expression = parser.expression(1)?;
    if parser.current().kind != Kind::End {
        return Err(parser.current().error(
            "K200",
            "값 하나만 적으세요. 차원이 여럿이면 쉼표로 나눕니다.",
        ));
    }
    Ok(expression)
}

struct Parser<'a> {
    tokens: &'a [Token],
    index: usize,
}

impl Parser<'_> {
    fn parse(&mut self) -> Result<Vec<Statement>, Diagnostic> {
        let mut statements = Vec::new();

        while self.current().kind != Kind::End {
            if self.current().kind == Kind::NewLine {
                self.take();
                continue;
            }

            let statement = match self.current().kind {
                Kind::Define | Kind::Check => self.value_statement()?,
                Kind::Frame => self.frame_statement()?,
                Kind::Type(Type::Entity) => {
                    // `대상 조사자, 조사범위`: 쉼표로 여러 개를 한 줄에 선언합니다.
                    self.take();
                    let mut names = self.names("선언할 대상의 이름을 적으세요.")?;
                    let last = names.pop().expect("names returns at least one");
                    statements.extend(names.into_iter().map(|name| Statement::Entity { name }));
                    Statement::Entity { name: last }
                }
                Kind::UnitDeclaration => {
                    self.take();
                    let mut names = self.names("선언할 단위의 이름을 적으세요. 예: '단위 원'")?;
                    let last = names.pop().expect("names returns at least one");
                    statements.extend(names.into_iter().map(|name| Statement::Unit { name }));
                    Statement::Unit { name: last }
                }
                Kind::Event => self.event_statement()?,
                Kind::Unknown => {
                    let mut unknowns = self.unknown_statement()?;
                    let last = unknowns.pop().expect("at least one unknown");
                    statements.extend(unknowns);
                    last
                }
                Kind::Assume | Kind::Goal => {
                    let goal = self.take().kind == Kind::Goal;
                    let (name, anonymous, expression) = self.named_expression()?;
                    Statement::Constraint {
                        name,
                        anonymous,
                        goal,
                        expression,
                    }
                }
                Kind::Observe => {
                    self.take();
                    let name = self.expect(Kind::Identifier, "관측한 미지값의 이름을 적으세요.")?;
                    self.expect(Kind::Assign, "이름 뒤에 '='와 관측값을 적으세요.")?;
                    let expression = self.expression(1)?;
                    Statement::Observe { name, expression }
                }
                Kind::Verb => {
                    let mut verbs = self.verb_statement()?;
                    let last = verbs.pop().expect("at least one verb");
                    statements.extend(verbs);
                    last
                }
                Kind::Rule => self.rule_statement()?,
                Kind::Topic => {
                    let keyword = self.take();
                    if self.current().kind != Kind::Text || self.current().text.is_empty() {
                        return Err(keyword.error(
                            "K200",
                            "'주제' 뒤에 이 파일이 무엇을 적는지 쓰세요. 예: '주제 <바람이 불다>'",
                        ));
                    }
                    let mut text = self.take();
                    // 꺾쇠는 있어도 되고 없어도 됩니다.
                    let inner = text
                        .text
                        .strip_prefix('<')
                        .and_then(|t| t.strip_suffix('>'));
                    if let Some(inner) = inner {
                        text.text = inner.trim().to_string();
                    }
                    Statement::Topic { text }
                }
                Kind::Word => self.word_statement()?,
                Kind::Scene => self.scene_statement()?,
                Kind::Minus => {
                    let dash = self.take();
                    if self.current().kind != Kind::Text || self.current().text.is_empty() {
                        return Err(dash.error(
                            "K200",
                            "'-' 뒤에 글을 적으세요. '-'로 시작하는 줄은 위의 문장에 딸린 글입니다.",
                        ));
                    }
                    Statement::Item { text: self.take() }
                }
                Kind::Body => {
                    self.take();
                    self.expect(
                        Kind::Colon,
                        "'본문' 뒤에 ':'와 글을 적으세요. 예: '본문: 글'",
                    )?;
                    if self.current().kind != Kind::Text || self.current().text.is_empty() {
                        return Err(self.current().error("K200", "본문이 비어 있습니다."));
                    }
                    Statement::Body { text: self.take() }
                }
                Kind::Reference => {
                    self.take();
                    let number = self.expect(Kind::Number, "'참조' 뒤에 번호를 적으세요.")?;
                    self.expect(Kind::Colon, "참조 번호 뒤에 ':'와 본문을 적으세요.")?;
                    if self.current().kind != Kind::Text || self.current().text.is_empty() {
                        return Err(self.current().error("K200", "참조 본문이 비어 있습니다."));
                    }
                    let text = self.take();
                    Statement::ReferenceDefinition { number, text }
                }
                _ => {
                    // 키워드로 시작하지 않는 줄은 문장입니다.
                    let label = if self.current().kind == Kind::Identifier
                        && self.tokens[self.index + 1].kind == Kind::Colon
                    {
                        let label = self.take();
                        self.take();
                        Some(label)
                    } else {
                        None
                    };
                    let mut tokens = Vec::new();
                    let mut note = None;
                    while !matches!(self.current().kind, Kind::NewLine | Kind::End) {
                        // 문장 뒤의 `: 설명`. 어휘 분석기가 ':' 뒤를 글 하나로 묶어 둡니다.
                        if self.current().kind == Kind::Colon
                            && self.tokens[self.index + 1].kind == Kind::Text
                        {
                            self.take();
                            if self.current().text.is_empty() {
                                return Err(self.current().error(
                                    "K200",
                                    "':' 뒤에 문장의 설명을 적으세요. 설명이 없으면 ':'를 지웁니다.",
                                ));
                            }
                            note = Some(self.take());
                            break;
                        }
                        tokens.push(self.take());
                    }
                    if tokens.is_empty() {
                        return Err(self.current().error("K500", "이름 뒤에 문장을 적으세요."));
                    }
                    Statement::Sentence {
                        label,
                        tokens,
                        note,
                    }
                }
            };
            statements.push(statement);

            if !matches!(self.current().kind, Kind::NewLine | Kind::End) {
                return Err(self.current().error(
                    "K200",
                    "한 줄에는 문장 하나를 적으세요. 비교에는 '=='를 사용합니다.",
                ));
            }
        }

        if statements.is_empty() {
            return Err(self.current().error("K201", "적어도 한 문장을 작성하세요."));
        }
        Ok(statements)
    }

    /// `낱말 컵: 색*, 재질, 채움(정도) 2`
    fn word_statement(&mut self) -> Result<Statement, Diagnostic> {
        self.take();
        let name = self.expect(
            Kind::Identifier,
            "새 낱말의 이름을 적으세요. 예: '낱말 컵: 색, 크기'",
        )?;
        self.expect(
            Kind::Colon,
            "낱말 이름 뒤에 ':'와 차원을 적으세요. 예: '낱말 컵: 색, 크기'",
        )?;
        let mut dimensions = Vec::new();
        loop {
            let name = self.dimension_name("차원의 이름을 적으세요.")?;
            let required = self.current().kind == Kind::Star;
            if required {
                self.take();
            }
            let kind = if self.current().kind == Kind::LeftParen {
                self.take();
                let (kind, _) =
                    self.type_name("괄호 안에는 타입을 적으세요. 예: '채움(정도)', '가로(거리)'")?;
                self.expect(Kind::RightParen, "닫는 괄호 ')'가 필요합니다.")?;
                Some(kind)
            } else {
                None
            };
            let weight = if self.current().kind == Kind::Number {
                let token = self.take();
                match token.text.parse::<u32>() {
                    Ok(weight) if (1..=100).contains(&weight) => weight,
                    _ => return Err(token.error("K200", "가중치는 1부터 100까지의 수입니다.")),
                }
            } else {
                1
            };
            dimensions.push(crate::syntax::Dimension {
                name,
                required,
                weight,
                kind,
            });
            if self.current().kind != Kind::Comma {
                break;
            }
            self.take();
        }
        Ok(Statement::Word { name, dimensions })
    }

    /// 차원의 이름. `거리`, `시간`, `위치`처럼 타입 이름과 같은 낱말도 차원 이름으로 쓸 수 있습니다.
    fn dimension_name(&mut self, message: &str) -> Result<Token, Diagnostic> {
        match self.current().kind {
            Kind::Identifier | Kind::Type(_) => Ok(self.take()),
            _ => Err(self.current().error("K200", message)),
        }
    }

    /// `장면 이름`과, 그보다 들여 쓴 줄들.
    fn scene_statement(&mut self) -> Result<Statement, Diagnostic> {
        let keyword = self.take();
        let name = self.expect(
            Kind::Identifier,
            "장면의 이름을 적으세요. 예: '장면 책상위'",
        )?;
        if !matches!(self.current().kind, Kind::NewLine | Kind::End) {
            return Err(self
                .current()
                .error("K200", "장면의 내용은 다음 줄부터 들여 써서 적으세요."));
        }
        let mut members = Vec::new();
        loop {
            // 장면이 끝났으면 줄 끝을 넘기지 않고 돌아갑니다. 줄 끝은 바깥에서 확인합니다.
            let before = self.index;
            while self.current().kind == Kind::NewLine {
                self.take();
            }
            if self.current().kind == Kind::End || self.current().column() <= keyword.column() {
                self.index = before;
                break;
            }
            let word = self.expect(
                Kind::Identifier,
                "장면 안의 줄은 낱말로 시작합니다. 예: '컵: 색 흰색'. 장면을 끝내려면 들여쓰기를 없애세요.",
            )?;
            let name = (self.current().kind == Kind::Identifier).then(|| self.take());
            let mut values = Vec::new();
            if self.current().kind == Kind::Colon {
                self.take();
                loop {
                    let dimension = self
                        .dimension_name("차원의 이름과 값을 적으세요. 예: '색 흰색, 채움 80%'")?;
                    let mut value = Vec::new();
                    while !matches!(self.current().kind, Kind::Comma | Kind::NewLine | Kind::End) {
                        value.push(self.take());
                    }
                    if value.is_empty() {
                        return Err(dimension.error(
                            "K200",
                            format!(
                                "'{0}' 뒤에 값을 적으세요. 값을 모르면 '{0}'을 적지 않습니다. 빈칸으로 남습니다.",
                                dimension.text
                            ),
                        ));
                    }
                    values.push((dimension, value));
                    if self.current().kind != Kind::Comma {
                        break;
                    }
                    self.take();
                }
            }
            if !matches!(self.current().kind, Kind::NewLine | Kind::End) {
                return Err(self.current().error(
                    "K200",
                    "낱말 뒤에는 ':'와 차원을 적습니다. 예: '컵: 색 흰색' 또는 '컵 큰컵: 색 흰색'",
                ));
            }
            members.push(crate::syntax::Member { word, name, values });
        }
        Ok(Statement::Scene { name, members })
    }

    /// 쉼표로 나눈 이름 목록.
    fn names(&mut self, message: &str) -> Result<Vec<Token>, Diagnostic> {
        let mut names = vec![self.expect(Kind::Identifier, message)?];
        while self.current().kind == Kind::Comma {
            self.take();
            names.push(self.expect(Kind::Identifier, message)?);
        }
        Ok(names)
    }

    /// `이름 = 식` 또는 이름 없이 `식`. 이름이 없으면 식을 그대로 옮긴 글이 이름이 됩니다.
    fn named_expression(&mut self) -> Result<(Token, bool, Expression), Diagnostic> {
        if self.current().kind == Kind::Identifier
            && self.tokens[self.index + 1].kind == Kind::Assign
        {
            let name = self.take();
            self.take();
            return Ok((name, false, self.expression(1)?));
        }
        let start = self.index;
        let expression = self.expression(1)?;
        let text = Token::join(&self.tokens[start..self.index]);
        Ok((self.tokens[start].renamed(text), true, expression))
    }

    /// 타입 이름 또는 `단위`로 선언한 단위 이름.
    fn type_name(&mut self, message: &str) -> Result<(Type, Token), Diagnostic> {
        let kind = match self.current().kind {
            Kind::Type(kind) => kind,
            Kind::Identifier => Type::Custom(intern(&self.current().text)),
            _ => return Err(self.current().error("K200", message)),
        };
        Ok((kind, self.take()))
    }

    fn value_statement(&mut self) -> Result<Statement, Diagnostic> {
        if self.take().kind == Kind::Check {
            let (name, anonymous, expression) = self.named_expression()?;
            return Ok(Statement::Value {
                name,
                check: true,
                anonymous,
                declared_type: None,
                expression,
            });
        }
        let name = self.expect(Kind::Identifier, "선언할 이름을 적으세요.")?;
        // 타입은 생략할 수 있습니다: `정의 최저가격 = 120만 원`
        let declared_type = if self.current().kind == Kind::Colon {
            self.take();
            Some(self.type_name(
                "타입은 정수, 논리, 시간, 거리, 시각, 위치, 대상, 정도 또는 선언한 단위입니다.",
            )?)
        } else {
            None
        };

        self.expect(
            Kind::Assign,
            "식 앞에 '='를 적으세요. 예: '정의 이름 = 값' 또는 '정의 이름: 타입 = 값'",
        )?;
        let expression = self.expression(1)?;
        Ok(Statement::Value {
            name,
            check: false,
            anonymous: false,
            declared_type,
            expression,
        })
    }

    fn frame_statement(&mut self) -> Result<Statement, Diagnostic> {
        self.take();
        let name = self.expect(Kind::Identifier, "선언할 기준의 이름을 적으세요.")?;
        self.expect(
            Kind::Colon,
            "기준 이름 뒤에 ':'와 '시간축' 또는 '위치축'을 적으세요.",
        )?;
        let Kind::Axis(axis) = self.current().kind else {
            return Err(self
                .current()
                .error("K200", "기준의 종류는 '시간축' 또는 '위치축'입니다."));
        };
        self.take();
        Ok(Statement::Frame { name, axis })
    }

    /// `미지 가: 시간`, `미지 가, 나: 원`, `미지 도착: 시각 = 기준 @ ?`
    fn unknown_statement(&mut self) -> Result<Vec<Statement>, Diagnostic> {
        self.take();
        let names = self.names("선언할 미지값의 이름을 적으세요.")?;
        self.expect(Kind::Colon, "이름 뒤에 ':'와 타입을 적으세요.")?;
        let (kind, type_token) =
            self.type_name("미지값의 타입은 정수, 시간, 거리, 시각, 위치 또는 선언한 단위입니다.")?;
        // 기준이 필요 없는 타입은 '= ?'를 생략할 수 있습니다.
        let frame = if self.current().kind == Kind::Assign {
            self.take();
            let frame = if self.current().kind == Kind::Identifier {
                let frame = self.take();
                self.expect(Kind::At, "기준 이름 뒤에 '@ ?'를 적으세요.")?;
                Some(frame)
            } else {
                None
            };
            self.expect(Kind::Question, "알 수 없는 값은 '?'로 적으세요.")?;
            frame
        } else {
            None
        };

        let needs_frame = matches!(kind, Type::Instant | Type::Position);
        let allowed = kind.is_amount() || needs_frame;
        if !allowed {
            return Err(type_token.error(
                "K200",
                "미지값의 타입은 정수, 시간, 거리, 시각, 위치 또는 선언한 단위입니다.",
            ));
        }
        if needs_frame != frame.is_some() {
            let message = if needs_frame {
                format!(
                    "'{}'은 기준 위의 점이므로 '= 기준 @ ?'로 적으세요.",
                    kind.name()
                )
            } else {
                format!("'{}'은 기준이 필요 없으므로 '= ?'로 적으세요.", kind.name())
            };
            return Err(type_token.error("K200", message));
        }
        Ok(names
            .into_iter()
            .map(|name| Statement::Unknown {
                name,
                kind,
                kind_token: type_token.clone(),
                frame: frame.clone(),
            })
            .collect())
    }

    fn rule_statement(&mut self) -> Result<Statement, Diagnostic> {
        self.take();
        let name = self.expect(Kind::Identifier, "선언할 규칙의 이름을 적으세요.")?;
        self.expect(
            Kind::Colon,
            "규칙 이름 뒤에 ':'와 '전제 이면 강도'를 적으세요. 예: '규칙 깨짐: 떨어짐 이면 80%'",
        )?;
        let premise = self.expression(1)?;
        if !(self.current().kind == Kind::Identifier && self.current().text == "이면") {
            return Err(self.current().error(
                "K200",
                "전제 뒤에 '이면'과 규칙의 강도를 적으세요. 예: '규칙 깨짐: 떨어짐 이면 80%'",
            ));
        }
        self.take();
        let strength = self.expression(1)?;
        let method = match self.current().kind {
            Kind::Identifier if matches!(self.current().text.as_str(), "최소로" | "곱으로") => {
                Some(self.take())
            }
            Kind::Identifier => {
                return Err(self.current().error(
                    "K200",
                    "규칙의 계산 방식은 '최소로' 또는 '곱으로'입니다. 적지 않으면 두 방식을 모두 계산합니다.",
                ));
            }
            _ => None,
        };
        Ok(Statement::Rule {
            name,
            premise,
            strength,
            method,
        })
    }

    /// `행위 확인하다, 조사하다 (조사했다): 주체, 대상`
    /// 쉼표로 나눈 행위들은 ':' 뒤의 필수 역할을 함께 씁니다.
    fn verb_statement(&mut self) -> Result<Vec<Statement>, Diagnostic> {
        self.take();
        let mut heads = Vec::new();
        loop {
            let name = self.expect(
                Kind::Identifier,
                "선언할 행위를 '설계하다'처럼 기본형으로 적으세요.",
            )?;
            if !name.text.ends_with('다') || name.text.chars().count() < 2 {
                return Err(name.error(
                    "K200",
                    "행위는 '설계하다', '떨어지다'처럼 '-다'로 끝나는 기본형으로 선언하세요.",
                ));
            }
            let mut forms = Vec::new();
            if self.current().kind == Kind::LeftParen {
                self.take();
                loop {
                    forms.push(self.expect(Kind::Identifier, "괄호 안에 활용형을 적으세요.")?);
                    if self.current().kind != Kind::Comma {
                        break;
                    }
                    self.take();
                }
                self.expect(Kind::RightParen, "닫는 괄호 ')'가 필요합니다.")?;
            }
            heads.push((name, forms));
            if self.current().kind != Kind::Comma {
                break;
            }
            self.take();
        }
        let mut required = Vec::new();
        if self.current().kind == Kind::Colon {
            self.take();
            loop {
                match self.current().kind {
                    Kind::Identifier | Kind::Type(Type::Entity) => required.push(self.take()),
                    _ => {
                        return Err(self
                            .current()
                            .error("K200", "':' 뒤에 반드시 있어야 하는 역할을 적으세요."));
                    }
                }
                if self.current().kind != Kind::Comma {
                    break;
                }
                self.take();
            }
        }
        Ok(heads
            .into_iter()
            .map(|(name, forms)| Statement::Verb {
                name,
                forms,
                required: required.clone(),
            })
            .collect())
    }

    fn event_statement(&mut self) -> Result<Statement, Diagnostic> {
        self.take();
        let name = self.expect(Kind::Identifier, "선언할 사건의 이름을 적으세요.")?;
        self.expect(Kind::Colon, "사건 이름 뒤에 ':'와 역할을 적으세요.")?;
        let mut roles = Vec::new();
        loop {
            let role = match self.current().kind {
                Kind::Identifier | Kind::Type(Type::Entity) => self.take(),
                _ => {
                    return Err(self
                        .current()
                        .error("K200", "'시작 = 값'처럼 역할 이름과 값을 적으세요."));
                }
            };
            self.expect(Kind::Assign, "역할 이름 뒤에 '='를 적으세요.")?;
            let expression = self.expression(1)?;
            roles.push(Role {
                name: role,
                expression,
            });
            if self.current().kind != Kind::Comma {
                break;
            }
            self.take();
        }
        Ok(Statement::Event { name, roles })
    }

    fn expression(&mut self, minimum: u8) -> Result<Expression, Diagnostic> {
        let mut left = self.prefix()?;
        while precedence(self.current().kind) >= minimum {
            let op = self.take();
            let right = self.expression(precedence(op.kind) + 1)?;
            left = Expression::Binary(op, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn prefix(&mut self) -> Result<Expression, Diagnostic> {
        match self.current().kind {
            Kind::Number => {
                let digits = self.take();
                let number = Integer::from_decimal(&digits.text);
                if self.current().kind == Kind::Percent {
                    self.take();
                    if number > Integer::from_small(100) {
                        return Err(digits.error("K520", "정도는 0%에서 100% 사이입니다."));
                    }
                    return Ok(Expression::Literal(Type::Degree, number));
                }
                if let Kind::Unit(kind, factor) = self.current().kind {
                    self.take();
                    return Ok(Expression::Literal(
                        kind,
                        number.mul(&Integer::from_small(factor)),
                    ));
                }
                // 큰 수 낱말: `120만`, `1억 2000만`
                let zero = || Integer::from_small(0);
                let (mut settled, mut last) = (zero(), number);
                while self.current().kind == Kind::Identifier
                    && let Some((factor, "")) = split_multiplier(&self.current().text)
                {
                    self.take();
                    settled = settled.add(&last.mul(&Integer::from_decimal(factor)));
                    last = zero();
                    if self.current().kind == Kind::Number
                        && self.tokens.get(self.index + 1).is_some_and(unit_word)
                    {
                        last = Integer::from_decimal(&self.take().text);
                    }
                }
                // 수 뒤의 낱말은 단위입니다: `120만 원`, `30 분`, `2 시간`, `3만원`
                if unit_word(self.current()) {
                    let word = self.take();
                    return Ok(Expression::Quantity {
                        settled,
                        last,
                        word,
                    });
                }
                Ok(Expression::Literal(Type::Integer, settled.add(&last)))
            }
            Kind::True | Kind::False => {
                let bit = u32::from(self.take().kind == Kind::True);
                Ok(Expression::Literal(Type::Boolean, Integer::from_small(bit)))
            }
            Kind::Identifier => {
                let name = self.take();
                match self.current().kind {
                    Kind::Dot => {
                        self.take();
                        let field = match self.current().kind {
                            Kind::Identifier | Kind::Type(Type::Entity) => self.take(),
                            _ => {
                                return Err(self
                                    .current()
                                    .error("K200", "'.' 뒤에 역할 이름을 적으세요."));
                            }
                        };
                        Ok(Expression::Member(name, field))
                    }
                    Kind::At => {
                        let at = self.take();
                        Ok(Expression::Anchor(name, at, Box::new(self.prefix()?)))
                    }
                    _ => Ok(Expression::Reference(name)),
                }
            }
            Kind::Minus | Kind::Not => {
                let op = self.take();
                Ok(Expression::Unary(op, Box::new(self.prefix()?)))
            }
            Kind::LeftParen => {
                self.take();
                let expression = self.expression(1)?;
                self.expect(Kind::RightParen, "닫는 괄호 ')'가 필요합니다.")?;
                Ok(expression)
            }
            _ => Err(self
                .current()
                .error("K200", "숫자, 참·거짓, 이름 또는 괄호로 식을 작성하세요.")),
        }
    }

    fn current(&self) -> &Token {
        &self.tokens[self.index]
    }

    fn take(&mut self) -> Token {
        let token = self.current().clone();
        self.index += 1;
        token
    }

    fn expect(&mut self, kind: Kind, message: &str) -> Result<Token, Diagnostic> {
        if self.current().kind == kind {
            Ok(self.take())
        } else {
            Err(self.current().error("K200", message))
        }
    }
}

/// 수 뒤에 와서 단위로 읽을 낱말인지. 규칙 문법의 낱말은 단위가 아닙니다.
fn unit_word(token: &Token) -> bool {
    match token.kind {
        Kind::Identifier => !matches!(token.text.as_str(), "이면" | "최소로" | "곱으로"),
        Kind::Type(Type::Time) => true,
        _ => false,
    }
}

fn precedence(kind: Kind) -> u8 {
    match kind {
        Kind::Or => 1,
        Kind::And => 2,
        Kind::Equal | Kind::NotEqual => 3,
        Kind::Less | Kind::LessEqual | Kind::Greater | Kind::GreaterEqual => 4,
        Kind::Plus | Kind::Minus => 5,
        Kind::Star => 6,
        _ => 0,
    }
}
